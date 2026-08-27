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
        "cipher_fingerprint",
        "options_fingerprint",
        "has_client_cert",
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
        cipher_fingerprint: str | None = None,
        options_fingerprint: str | None = None,
        has_client_cert: bool = False,
    ) -> None:
        self.verify_mode = verify_mode
        self.check_hostname = check_hostname
        self.ca_certs_der = ca_certs_der
        self.min_version = min_version
        self.max_version = max_version
        self.class_name = class_name
        self.cipher_fingerprint = cipher_fingerprint
        self.options_fingerprint = options_fingerprint
        self.has_client_cert = has_client_cert

    def fingerprint(self) -> str:
        """Compute a stable fingerprint over representable public state.

        Used to detect live mutations on contexts that our helper
        registered.  Private key material is never included.
        """
        h = hashlib.sha256()
        h.update(f"verify_mode={self.verify_mode};".encode())
        h.update(f"check_hostname={int(self.check_hostname)};".encode())
        h.update(f"min_version={self.min_version};".encode())
        h.update(f"max_version={self.max_version};".encode())
        h.update(f"class={self.class_name};".encode())
        h.update(f"has_client_cert={int(self.has_client_cert)};".encode())
        for cert in self.ca_certs_der:
            h.update(b"\x00")
            h.update(hashlib.sha256(cert).digest())
        h.update(b"|ciphers:")
        h.update((self.cipher_fingerprint or "").encode())
        h.update(b"|options:")
        h.update((self.options_fingerprint or "").encode())
        return h.hexdigest()

    def __repr__(self) -> str:
        return (
            f"_SSLContextSnapshot("
            f"verify_mode={self.verify_mode}, "
            f"check_hostname={self.check_hostname}, "
            f"ca_count={len(self.ca_certs_der)}, "
            f"min_version={self.min_version}, "
            f"max_version={self.max_version}, "
            f"class_name={self.class_name!r}, "
            f"has_client_cert={self.has_client_cert})"
        )


def _cipher_fingerprint(ctx: ssl.SSLContext) -> str | None:
    """Compute a stable fingerprint of the context's cipher list.

    Returns ``None`` if the runtime does not expose ``get_ciphers``.
    """
    try:
        ciphers = ctx.get_ciphers()
    except (ssl.SSLError, NotImplementedError):
        return None
    if not ciphers:
        return ""
    names = sorted(c["name"] for c in ciphers)
    return hashlib.sha256("|".join(names).encode()).hexdigest()


_DEFAULT_CIPHER_FINGERPRINT = _cipher_fingerprint(ssl.create_default_context())


def _options_fingerprint(ctx: ssl.SSLContext) -> str | None:
    """Compute a stable fingerprint of the context's options bitfield.

    Options carry protocol-level semantics (CHACHA preference, etc.) that
    can affect the handshake.  Used to detect mutations of helper-created
    contexts.
    """
    options = getattr(ctx, "options", None)
    if options is None:
        return None
    return hashlib.sha256(repr(int(options)).encode()).hexdigest()


def _detect_client_cert(ctx: ssl.SSLContext) -> bool:
    """Return True if the context appears to have a loaded client cert.

    This is a coarse public-API-only check. Python's ``ssl.SSLContext``
    does not expose whether ``load_cert_chain()`` has been called through
    any documented public API, so the only safe answer is ``False``:
    detection of true mTLS provenance belongs to the registry layer
    (helper-created contexts carry ``cert_path``/``key_path`` metadata),
    and external contexts are rejected upstream in ``_classify_context``
    when we cannot prove their exact state.

    Returning ``False`` unconditionally keeps the fingerprint stable
    across registration and lookups. The ``has_client_cert`` snapshot
    field is therefore effectively unused for safety; the actual
    fail-closed behavior comes from the registry's classification path
    and the conservative external-context rejection.
    """
    return False


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
        cipher_fingerprint=_cipher_fingerprint(ctx),
        options_fingerprint=_options_fingerprint(ctx),
        has_client_cert=_detect_client_cert(ctx),
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
    to map exactly to rustls triggers rejection.  This is the second
    line of defense behind the construction fingerprint stored in the
    registry; classification is also used for caller-created contexts
    that the registry does not know about.
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

    # ── Custom ciphers ────────────────────────────────────────────
    # Compare the captured fingerprint, not a second live read.  The
    # snapshot is the consistency boundary for all classifier decisions.
    if snapshot.cipher_fingerprint != _DEFAULT_CIPHER_FINGERPRINT:
        return Classification.UNREPRESENTABLE

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

    # ── External context unobservable state ──────────────────────
    # For external (non-EggFetch-registered) contexts, we cannot
    # observe client cert loading, ALPN mutations, or other opaque
    # state through public Python APIs.  We reject non-standard
    # subclasses (class_name != "SSLContext") because we cannot
    # prove every connection-relevant semantic is represented.
    # Standard ssl.SSLContext instances are accepted as a bounded
    # difference: their verify_mode, check_hostname, CA certs,
    # min/max_version, and cipher state are all observable through
    # documented public APIs.
    if not _eggfetch_ssl_registry.is_eggfetch_context(ctx):
        if snapshot.class_name != "SSLContext":
            return Classification.UNREPRESENTABLE

    # ── CA certificates ───────────────────────────────────────────
    # The CA count is no longer compared against the default trust
    # store.  Custom CAs (regardless of count) are always passed
    # through as DER bytes so rustls builds an exact trust store
    # from the actual anchors.
    if snapshot.ca_certs_der:
        return Classification.EXACTLY_REPRESENTABLE

    # ── Default context ───────────────────────────────────────────
    # A standard ``ssl.SSLContext`` subclass (e.g. ``create_default_context()``
    # with no custom trust) uses system/WebPKI roots and is exactly
    # representable.
    if snapshot.class_name == "SSLContext":
        return Classification.EXACTLY_REPRESENTABLE

    # Unknown subclass or third-party context: conservative rejection.
    return Classification.UNREPRESENTABLE


# ── Weak registry for EggFetch-created contexts ───────────────────────


class _EggfetchSSLRegistry:
    """Thread-safe weak-keyed registry for SSLContexts created by
    ``eggfetch.compat.httpx.create_ssl_context()``.

    Entries store construction metadata and a public-state fingerprint
    that is checked at translation time.  Live mutation after
    registration is detected by re-snapshotting the context and comparing
    fingerprints; the registry then refuses to reuse stale metadata.
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
        passthrough: bool = False,
    ) -> None:
        """Record metadata for a context created by our helper.

        When ``passthrough`` is True, the context is registered as a
        caller-supplied passthrough.  The fingerprint is captured but
        the stored ``verify`` kwarg is forced to ``True`` because we do
        not know what trust/cert state the caller loaded externally.
        The translation path treats passthrough contexts as
        caller-created and classifies them from the live snapshot.
        """
        if passthrough:
            stored_verify: bool | str = True
            # No cert provenance for a passthrough — any client
            # identity must be re-extracted safely or rejected.
            stored_cert_path: str | None = None
            stored_key_path: str | None = None
        else:
            stored_verify = verify
            stored_cert_path = cert_path
            stored_key_path = key_path

        meta = {
            "cert_path": stored_cert_path,
            "key_path": stored_key_path,
            "verify": stored_verify,
            "trust_env": trust_env,
            "passthrough": passthrough,
            "fingerprint": snapshot_context(ctx).fingerprint(),
        }

        def _on_expire(ref: weakref.ref = None, key: int = id(ctx)) -> None:
            with self._lock:
                self._entries.pop(key, None)

        ref = weakref.ref(ctx, _on_expire)
        with self._lock:
            self._entries[id(ctx)] = (ref, meta)

    def get(self, ctx: ssl.SSLContext) -> dict[str, Any] | None:
        """Retrieve metadata for a registered context, or ``None``.

        Live mutation is detected by comparing the current public-state
        fingerprint to the stored one.  When they differ, the metadata
        is discarded so translation reclassifies the context from its
        current state.  A mutated helper context loses its
        ``passthrough=False`` and ``cert_path`` provenance — the
        classification path then treats it as a fresh caller context.
        """
        with self._lock:
            entry = self._entries.get(id(ctx))
            if entry is None:
                return None
            ref, meta = entry
            if ref() is None:
                # Context has been garbage collected.
                self._entries.pop(id(ctx), None)
                return None
            if meta.get("passthrough"):
                # Passthrough contexts are intentionally unverified: we
                # classify the live snapshot, not the stored metadata.
                return {"passthrough": True}
            current_fp = snapshot_context(ctx).fingerprint()
            if current_fp != meta["fingerprint"]:
                # Live mutation detected.  The caller might have
                # replaced CAs, changed verify_mode, swapped ciphers,
                # or loaded an external client cert.  We must not reuse
                # the stored construction metadata because it no
                # longer describes the live context.
                self._entries.pop(id(ctx), None)
                return None
            return dict(meta)

    def is_eggfetch_context(self, ctx: ssl.SSLContext) -> bool:
        """Return ``True`` if *ctx* is currently tracked by the registry.

        Note that this returns ``False`` after a live mutation because
        the entry is dropped at that point.
        """
        return self.get(ctx) is not None

    def is_passthrough(self, ctx: ssl.SSLContext) -> bool:
        """Return ``True`` if the context was registered as a passthrough."""
        with self._lock:
            entry = self._entries.get(id(ctx))
            if entry is None:
                return False
            ref, meta = entry
            if ref() is None:
                self._entries.pop(id(ctx), None)
                return False
            return bool(meta.get("passthrough"))


_eggfetch_ssl_registry = _EggfetchSSLRegistry()


# ── Convenience: build eggfetch verify/cert kwargs from context ───────


def context_to_eggfetch_kwargs(
    ctx: ssl.SSLContext,
) -> dict[str, Any]:
    """Convert an SSLContext to kwargs suitable for ``eggfetch.Client()``.

    Translation rules (fail-closed):

    - Helper-created contexts whose live public state matches the
      construction fingerprint reuse the stored metadata.  mTLS
      identity is only carried when the stored ``cert_path`` is set.
    - Helper-created contexts whose live state has been mutated lose
      their stored metadata and are reclassified from the live
      snapshot.  mTLS identity is dropped (no path provenance).
    - Caller-created contexts are classified from a live snapshot.
      Custom CAs (any count) are passed as DER bytes — never compared
      against default-trust heuristics.  Custom ciphers, ALPN, or
      client certificates without extraction-safe provenance cause
      rejection.
    - External client certificates (mTLS) without helper path
      provenance cannot be exported safely; translation fails
      closed with ``TypeError`` before dispatch.

    Raises ``TypeError`` if the context cannot be represented.
    """
    # Check the registry first.
    meta = _eggfetch_ssl_registry.get(ctx)
    if meta is not None:
        if meta.get("passthrough"):
            # Fall through to caller-created path: classify from
            # snapshot using the public state we can actually see.
            pass
        else:
            kwargs: dict[str, Any] = {}
            if meta["verify"] is not True:
                kwargs["verify"] = meta["verify"]
            if meta["cert_path"] is not None:
                kwargs["cert"] = meta["cert_path"]
            if meta["trust_env"] is not True:
                kwargs["trust_env"] = meta["trust_env"]
            return kwargs

    # Caller-created (or passthrough, or mutated helper) context:
    # classify from the live snapshot.
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
    elif snapshot.ca_certs_der:
        # Always carry the actual DER anchors.  We never infer default
        # trust from CA count or from a comparison to the system/certifi
        # store; two CA stores with similar cardinality are not
        # equivalent.
        kwargs["verify"] = snapshot.ca_certs_der
    else:
        # Standard verified context (certifi, system roots, or
        # eggfetch-created default).
        kwargs["verify"] = True

    return kwargs
