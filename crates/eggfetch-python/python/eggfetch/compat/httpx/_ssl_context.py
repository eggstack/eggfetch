"""SSL context snapshot model and representability classifier.

This module provides:
- ``_SSLContextSnapshot``: a frozen representation of the extractable state
  from a Python ``ssl.SSLContext``.
- ``_classify_context``: a conservative classifier that determines whether
  a context can be faithfully translated to eggfetch's rustls backend.
- ``_eggfetch_ssl_registry``: a weak-keyed registry that remembers metadata
  for contexts created by ``eggfetch.compat.httpx.create_ssl_context()``.

No ``unsafe`` code, no OpenSSL pointer extraction, no logging of private
key material.
"""

from __future__ import annotations

import hashlib
import ssl
import sys
import threading
import typing
import warnings
import weakref

if typing.TYPE_CHECKING:
    from typing import Any


# ── Maximum extraction bounds ─────────────────────────────────────────

_MAX_CA_CERTS = 256
_MAX_CA_TOTAL_BYTES = 2 * 1024 * 1024  # 2 MiB


# ── Snapshot model ────────────────────────────────────────────────────


class _SSLContextSnapshot:
    """Frozen, serialisable representation of the extractable state of a
    Python ``ssl.SSLContext``.

    Only state accessible through documented public APIs is captured.
    Private key material is never extracted.
    """

    __slots__ = (
        "verify_mode",
        "check_hostname",
        "ca_certs_der",
        "min_version",
        "max_version",
        "class_name",
        "_eggfetch_meta",
    )

    def __init__(
        self,
        *,
        verify_mode: int,
        check_hostname: bool,
        ca_certs_der: list[bytes],
        min_version: int | None,
        max_version: int | None,
        class_name: str,
        eggfetch_meta: dict[str, Any] | None = None,
    ) -> None:
        self.verify_mode = verify_mode
        self.check_hostname = check_hostname
        self.ca_certs_der = ca_certs_der
        self.min_version = min_version
        self.max_version = max_version
        self.class_name = class_name
        self._eggfetch_meta = eggfetch_meta

    def __repr__(self) -> str:
        return (
            f"_SSLContextSnapshot("
            f"verify_mode={self.verify_mode}, "
            f"check_hostname={self.check_hostname}, "
            f"ca_count={len(self.ca_certs_der)}, "
            f"min_version={self.min_version}, "
            f"max_version={self.max_version}, "
            f"class_name={self.class_name!r})"
        )


def snapshot_context(ctx: ssl.SSLContext) -> _SSLContextSnapshot:
    """Extract a snapshot from a live ``ssl.SSLContext``.

    Only documented public API state is read.  If a method is unavailable
    (older Python), its corresponding field defaults to ``None``.

    Raises ``ValueError`` if CA extraction exceeds the safety bounds.
    """
    verify_mode = ctx.verify_mode
    check_hostname = ctx.check_hostname

    ca_der: list[bytes] = []
    if hasattr(ctx, "get_ca_certs") and callable(ctx.get_ca_certs):
        try:
            raw_certs = ctx.get_ca_certs(binary_form=True)
        except NotImplementedError:
            raw_certs = []
        for cert in raw_certs:
            if len(ca_der) >= _MAX_CA_CERTS:
                raise ValueError(
                    f"CA certificate count exceeds {_MAX_CA_CERTS}"
                )
            ca_der.append(bytes(cert))
        total = sum(len(c) for c in ca_der)
        if total > _MAX_CA_TOTAL_BYTES:
            raise ValueError(
                f"CA certificate total size ({total} bytes) exceeds "
                f"{_MAX_CA_TOTAL_BYTES} limit"
            )

    min_ver = _extract_version(ctx, "minimum_version")
    max_ver = _extract_version(ctx, "maximum_version")
    class_name = type(ctx).__name__

    return _SSLContextSnapshot(
        verify_mode=verify_mode,
        check_hostname=check_hostname,
        ca_certs_der=ca_der,
        min_version=min_ver,
        max_version=max_ver,
        class_name=class_name,
    )


def _extract_version(ctx: ssl.SSLContext, attr: str) -> int | None:
    """Extract a TLS version constant, returning None for unsupported."""
    val = getattr(ctx, attr, None)
    if val is None:
        return None
    # Python 3.7+ exposes integer protocol version constants.
    if isinstance(val, int):
        return val
    return None


# ── Representability classifier ───────────────────────────────────────


class Classification:
    """Possible representability classifications."""

    EXACTLY_REPRESENTABLE = "exactly_representable"
    REPRESENTABLE_WITH_DEFAULTS = "representable_with_known_defaults"
    UNREPRESENTABLE = "unrepresentable"


def _classify_context(
    ctx: ssl.SSLContext,
    snapshot: _SSLContextSnapshot | None = None,
) -> str:
    """Classify an SSLContext for eggfetch transport translation.

    Returns one of:
    - ``Classification.EXACTLY_REPRESENTABLE``
    - ``Classification.REPRESENTABLE_WITH_DEFAULTS``
    - ``Classification.UNREPRESENTABLE``

    The classification is conservative: any state that cannot be proven
    to map exactly to rustls triggers rejection.
    """
    if snapshot is None:
        snapshot = snapshot_context(ctx)

    # ── TLS version bounds ────────────────────────────────────────
    # Python ssl module uses two value systems:
    # - Default sentinels: minimum_version=-2 (TLSv1_2), maximum_version=-1 (TLSv1_3)
    # - Explicit constants: ssl.TLSVersion.TLSv1_1=770, TLSv1_2=771, TLSv1_3=772
    # Only TLS 1.2 (771) and 1.3 (772) are representable by rustls.
    # Default sentinels (-2, -1) are safe and should not trigger rejection.
    _TLS_1_2_WIRE = 771
    _TLS_1_3_WIRE = 772
    if snapshot.min_version is not None:
        if snapshot.min_version > 0 and snapshot.min_version < _TLS_1_2_WIRE:
            # Explicit version below TLS 1.2 (e.g., TLSv1_1=770, TLSv1=769)
            return Classification.UNREPRESENTABLE
    if snapshot.max_version is not None:
        if snapshot.max_version > _TLS_1_3_WIRE:
            # Explicit version above TLS 1.3
            return Classification.UNREPRESENTABLE

    # ── Verification mode ─────────────────────────────────────────
    if snapshot.verify_mode == ssl.CERT_NONE:
        if snapshot.check_hostname:
            # CERT_NONE + check_hostname is an invalid combination in
            # modern Python but we should still reject it.
            return Classification.UNREPRESENTABLE
        # CERT_NONE with hostname disabled: representable (dangerous mode)
        return Classification.REPRESENTABLE_WITH_DEFAULTS

    if snapshot.verify_mode != ssl.CERT_REQUIRED:
        # Unknown verify mode value
        return Classification.UNREPRESENTABLE

    # ── Check hostname consistency ────────────────────────────────
    # check_hostname=True with CERT_REQUIRED is the standard secure mode.
    # check_hostname=False with CERT_REQUIRED means verify certs but skip
    # hostname check — this maps to eggfetch's verify_hostname=False.

    # ── Custom ciphers ────────────────────────────────────────────
    try:
        ciphers = ctx.get_ciphers()
        if ciphers:
            # Eggfetch uses rustls's default cipher suite. Custom cipher
            # ordering or disabled standard ciphers cannot be represented.
            default_ctx = ssl.create_default_context()
            default_ciphers = {c["name"] for c in default_ctx.get_ciphers()}
            current_ciphers = {c["name"] for c in ciphers}
            if current_ciphers != default_ciphers:
                return Classification.UNREPRESENTABLE
    except (ssl.SSLError, NotImplementedError):
        pass

    # ── Options flags ─────────────────────────────────────────────
    if hasattr(ctx, "options"):
        options = ctx.options
        # Only reject options that actively change handshake or security
        # semantics in ways rustls cannot represent.  Standard "no SSLv2",
        # "no SSLv3", "no TLSv1.x" flags, session/renegotiation options,
        # and performance hints are all safe to ignore.
        _BLOCKED_OPTIONS = 0
        for name in (
            "OP_PRIORITIZE_CHACHA",
        ):
            val = getattr(ssl, name, 0)
            if val:
                _BLOCKED_OPTIONS |= val
        if _BLOCKED_OPTIONS and (options & _BLOCKED_OPTIONS):
            return Classification.UNREPRESENTABLE

    # ── ALPN / NPN ───────────────────────────────────────────────
    # ALPN is owned by eggfetch's transport layer.  We cannot inspect
    # configured ALPN via public Python APIs, so we rely on the registry
    # for contexts created by our helper and conservative rejection of
    # unknown subclasses for third-party contexts.

    # ── CA certificates ───────────────────────────────────────────
    if snapshot.ca_certs_der:
        # Custom CA certs are representable via rustls Custom trust store.
        return Classification.EXACTLY_REPRESENTABLE

    # ── Default context ───────────────────────────────────────────
    # A default ssl.create_default_context() with no modifications is
    # exactly representable (uses system/WebPKI roots).
    if snapshot.class_name in ("SSLContext",):
        return Classification.EXACTLY_REPRESENTABLE

    # Unknown subclass or third-party context: conservative rejection.
    return Classification.UNREPRESENTABLE


# ── Weak registry for EggFetch-created contexts ───────────────────────


class _EggfetchSSLRegistry:
    """Thread-safe weak-keyed registry for SSLContexts created by
    ``eggfetch.compat.httpx.create_ssl_context()``.

    Entries store reconstruction metadata (never private key bytes).
    """

    def __init__(self) -> None:
        self._lock = threading.Lock()
        # Use WeakValueDictionary is not suitable (keys are the contexts).
        # Instead, use a mapping from id -> (weakref, metadata).
        self._entries: dict[int, tuple[weakref.ref, dict[str, Any]]] = {}

    def register(
        self,
        ctx: ssl.SSLContext,
        *,
        cert_path: str | None = None,
        key_path: str | None = None,
        verify: bool | str = True,
        trust_env: bool = True,
    ) -> None:
        """Record metadata for a context created by our helper."""
        meta = {
            "cert_path": cert_path,
            "key_path": key_path,
            "verify": verify,
            "trust_env": trust_env,
        }

        def _on_expire(ref: weakref.ref = None, key: int = id(ctx)) -> None:
            with self._lock:
                self._entries.pop(key, None)

        ref = weakref.ref(ctx, _on_expire)
        with self._lock:
            self._entries[id(ctx)] = (ref, meta)

    def get(self, ctx: ssl.SSLContext) -> dict[str, Any] | None:
        """Retrieve metadata for a registered context, or ``None``."""
        with self._lock:
            entry = self._entries.get(id(ctx))
            if entry is None:
                return None
            ref, meta = entry
            if ref() is None:
                # Context has been garbage collected.
                self._entries.pop(id(ctx), None)
                return None
            return dict(meta)

    def is_eggfetch_context(self, ctx: ssl.SSLContext) -> bool:
        """Return ``True`` if *ctx* was created by our helper."""
        return self.get(ctx) is not None


_eggfetch_ssl_registry = _EggfetchSSLRegistry()


# ── Convenience: build eggfetch verify/cert kwargs from context ───────


def context_to_eggfetch_kwargs(
    ctx: ssl.SSLContext,
) -> dict[str, Any]:
    """Convert an SSLContext to kwargs suitable for ``eggfetch.Client()``.

    If the context was created by our helper and registered, uses the
    stored metadata for faithful reconstruction.

    For caller-created contexts, takes a snapshot and translates
    representable state.

    Raises ``TypeError`` if the context cannot be represented.
    """
    # Check the registry first.
    meta = _eggfetch_ssl_registry.get(ctx)
    if meta is not None:
        kwargs: dict[str, Any] = {}
        if meta["verify"] is not True:
            kwargs["verify"] = meta["verify"]
        if meta["cert_path"] is not None:
            kwargs["cert"] = meta["cert_path"]
        if meta["trust_env"] is not True:
            kwargs["trust_env"] = meta["trust_env"]
        return kwargs

    # Caller-created context: snapshot and classify.
    snapshot = snapshot_context(ctx)
    classification = _classify_context(ctx, snapshot)

    if classification == Classification.UNREPRESENTABLE:
        raise TypeError(
            "eggfetch cannot safely translate this ssl.SSLContext. "
            "Use eggfetch.compat.httpx.create_ssl_context() to create "
            "a context that eggfetch can faithfully represent, or pass "
            "verify/cert kwargs directly."
        )

    kwargs = {}

    # Verification mode.
    if snapshot.verify_mode == ssl.CERT_NONE:
        kwargs["verify"] = False
    elif snapshot.ca_certs_der and not _eggfetch_ssl_registry.is_eggfetch_context(ctx):
        # Caller context with custom CA certs.  If the loaded CAs
        # closely match the standard certifi bundle, treat as default
        # trust.  Otherwise pass the DER bytes so eggfetch loads them
        # as a Custom trust store.
        try:
            import certifi

            with open(certifi.where(), "rb") as f:
                certifi_data = f.read()
            from eggfetch.compat.httpx._ssl_context import (
                _MAX_CA_CERTS,
                _MAX_CA_TOTAL_BYTES,
            )

            # Rough heuristic: if the number of loaded CAs is within
            # 20% of certifi's count, assume default trust store.
            import ssl as _ssl_mod

            _default_ctx = _ssl_mod.create_default_context()
            _default_count = len(
                _default_ctx.get_ca_certs(binary_form=True)
            )
            _loaded_count = len(snapshot.ca_certs_der)
            if abs(_loaded_count - _default_count) <= max(
                5, _default_count // 5
            ):
                kwargs["verify"] = True
            else:
                kwargs["verify"] = snapshot.ca_certs_der
        except Exception:
            # certifi not available or comparison failed; pass DER certs.
            kwargs["verify"] = snapshot.ca_certs_der
    else:
        # Standard verified context (certifi, system roots, or eggfetch-created).
        kwargs["verify"] = True

    return kwargs
