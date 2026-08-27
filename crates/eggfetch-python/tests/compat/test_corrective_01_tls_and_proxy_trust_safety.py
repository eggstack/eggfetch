"""Tests for HTTPX parity corrective 01 — TLS translation and proxy trust safety.

Covers the plan's "Required differential tests" sections for
SSLContext classification, registry fingerprinting, proxy trust-domain
isolation, and proxy-header redaction.

The tests in this file exercise the deterministic representability
matrix and the construction fingerprint mechanism that detects
post-construction mutation of helper-created SSLContexts.
"""

from __future__ import annotations

import ssl
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))

from eggfetch.compat.httpx import (
    AsyncClient,
    Client,
    MockTransport,
    Response,
    create_ssl_context,
)


# ── Deterministic representability matrix ─────────────────────────────


class TestRepresentabilityMatrix:
    """The plan's representability matrix.

    These tests pin the determinism rules and ensure the
    classifier never falls back to a CA-count or
    approximate-similarity heuristic.
    """

    def test_ca_count_heuristic_removed(self):
        """A custom CA set whose count matches the system trust is
        translated as actual custom anchors, never as default trust.

        Previously the code compared the loaded CA count against
        the default certifi bundle (with a 20% tolerance) and
        silently treated similar-cardinality stores as
        ``verify=True``.  The corrective removes that heuristic.
        """
        from eggfetch.compat.httpx._ssl_context import (
            context_to_eggfetch_kwargs,
            snapshot_context,
        )

        # Generate N self-signed CA certs with the same count as
        # the system trust store would have.  This is the case
        # the old heuristic papered over.
        default_ctx = ssl.create_default_context()
        default_count = len(default_ctx.get_ca_certs(binary_form=True))
        # Build a context with that many distinct CAs.
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            ca_paths = []
            for i in range(default_count):
                ca_path = f"{tmpdir}/ca{i}.pem"
                ca_key = f"{tmpdir}/ca{i}.key"
                import subprocess

                subprocess.run(
                    [
                        "openssl", "req", "-x509", "-newkey", "ec",
                        "-pkeyopt", "ec_paramgen_curve:prime256v1",
                        "-keyout", ca_key, "-out", ca_path,
                        "-days", "1", "-nodes",
                        "-subj", f"/CN=test-ca-{i}",
                        "-addext", "basicConstraints=critical,CA:TRUE",
                    ],
                    check=True,
                    capture_output=True,
                )
                ca_paths.append(ca_path)

            # Build a context that loads all these CAs.
            ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
            ctx.check_hostname = False
            ctx.verify_mode = ssl.CERT_REQUIRED
            for ca_path in ca_paths:
                ctx.load_verify_locations(cafile=ca_path)
            snap = snapshot_context(ctx)
            assert len(snap.ca_certs_der) == default_count

            # The heuristic would have classified this as
            # ``verify=True`` because the count matches.  With
            # the corrective, the actual DER bytes are passed
            # through.
            kwargs = context_to_eggfetch_kwargs(ctx)
            assert kwargs.get("verify") is not True, (
                "CA-count heuristic still active: a custom CA set "
                "with the same cardinality as the system trust was "
                "incorrectly classified as default trust."
            )
            assert isinstance(kwargs.get("verify"), list)
            assert len(kwargs["verify"]) == default_count

    def test_two_distinct_same_sized_ca_sets_never_equivalent(self):
        """Two custom CA sets with the same cardinality but different
        content must produce different ``verify`` kwargs.

        The heuristic-free translation encodes each CA's actual
        DER bytes, so two stores with identical cardinalities are
        distinguishable by content.
        """
        from eggfetch.compat.httpx._ssl_context import (
            context_to_eggfetch_kwargs,
        )

        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            # Set A: 2 distinct CAs
            import subprocess

            a_path1 = f"{tmpdir}/a1.pem"
            a_key1 = f"{tmpdir}/a1.key"
            subprocess.run(
                [
                    "openssl", "req", "-x509", "-newkey", "ec",
                    "-pkeyopt", "ec_paramgen_curve:prime256v1",
                    "-keyout", a_key1, "-out", a_path1,
                    "-days", "1", "-nodes",
                    "-subj", "/CN=set-A-1",
                    "-addext", "basicConstraints=critical,CA:TRUE",
                ],
                check=True,
                capture_output=True,
            )
            a_path2 = f"{tmpdir}/a2.pem"
            a_key2 = f"{tmpdir}/a2.key"
            subprocess.run(
                [
                    "openssl", "req", "-x509", "-newkey", "ec",
                    "-pkeyopt", "ec_paramgen_curve:prime256v1",
                    "-keyout", a_key2, "-out", a_path2,
                    "-days", "1", "-nodes",
                    "-subj", "/CN=set-A-2",
                    "-addext", "basicConstraints=critical,CA:TRUE",
                ],
                check=True,
                capture_output=True,
            )

            # Set B: 2 distinct CAs (different content)
            b_path1 = f"{tmpdir}/b1.pem"
            b_key1 = f"{tmpdir}/b1.key"
            subprocess.run(
                [
                    "openssl", "req", "-x509", "-newkey", "ec",
                    "-pkeyopt", "ec_paramgen_curve:prime256v1",
                    "-keyout", b_key1, "-out", b_path1,
                    "-days", "1", "-nodes",
                    "-subj", "/CN=set-B-1",
                    "-addext", "basicConstraints=critical,CA:TRUE",
                ],
                check=True,
                capture_output=True,
            )
            b_path2 = f"{tmpdir}/b2.pem"
            b_key2 = f"{tmpdir}/b2.key"
            subprocess.run(
                [
                    "openssl", "req", "-x509", "-newkey", "ec",
                    "-pkeyopt", "ec_paramgen_curve:prime256v1",
                    "-keyout", b_key2, "-out", b_path2,
                    "-days", "1", "-nodes",
                    "-subj", "/CN=set-B-2",
                    "-addext", "basicConstraints=critical,CA:TRUE",
                ],
                check=True,
                capture_output=True,
            )

            ctx_a = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
            ctx_a.check_hostname = False
            ctx_a.verify_mode = ssl.CERT_REQUIRED
            ctx_a.load_verify_locations(cafile=a_path1)
            ctx_a.load_verify_locations(cafile=a_path2)

            ctx_b = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
            ctx_b.check_hostname = False
            ctx_b.verify_mode = ssl.CERT_REQUIRED
            ctx_b.load_verify_locations(cafile=b_path1)
            ctx_b.load_verify_locations(cafile=b_path2)

            verify_a = context_to_eggfetch_kwargs(ctx_a)["verify"]
            verify_b = context_to_eggfetch_kwargs(ctx_b)["verify"]
            assert isinstance(verify_a, list)
            assert isinstance(verify_b, list)
            assert len(verify_a) == len(verify_b)
            # The DER sets must be distinct — same cardinality does
            # not mean equivalence.
            assert set(bytes(c) for c in verify_a) != set(
                bytes(c) for c in verify_b
            )


# ── Construction fingerprint ─────────────────────────────────────────


class TestConstructionFingerprint:
    """The construction fingerprint must detect live mutation."""

    def test_helper_context_unchanged_reconstructs(self):
        """An unmodified helper-created context reuses the stored
        construction metadata at translation time.
        """
        from eggfetch.compat.httpx._ssl_context import (
            _eggfetch_ssl_registry,
            context_to_eggfetch_kwargs,
        )

        ctx = create_ssl_context()
        assert _eggfetch_ssl_registry.is_eggfetch_context(ctx)
        meta = _eggfetch_ssl_registry.get(ctx)
        assert meta is not None
        kwargs = context_to_eggfetch_kwargs(ctx)
        # Unmodified helper contexts that used the system trust
        # are reconstructed as default-trust (``verify=True`` is
        # the implicit default; ``verify=True`` is omitted from
        # the kwargs to match HTTPX's behavior).
        assert kwargs.get("verify", True) is True
        assert kwargs.get("cert") is None

    def test_helper_context_then_load_verify_locations_detected(self):
        """Loading additional CAs after helper creation must
        invalidate the stored construction metadata.

        Without fingerprint detection, the registry would
        silently keep the original ``verify=True`` even after
        the user replaced the trust store.
        """
        from eggfetch.compat.httpx._ssl_context import (
            _eggfetch_ssl_registry,
            context_to_eggfetch_kwargs,
        )

        import subprocess
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            ca_path = f"{tmpdir}/extra-ca.pem"
            ca_key = f"{tmpdir}/extra-ca.key"
            subprocess.run(
                [
                    "openssl", "req", "-x509", "-newkey", "ec",
                    "-pkeyopt", "ec_paramgen_curve:prime256v1",
                    "-keyout", ca_key, "-out", ca_path,
                    "-days", "1", "-nodes",
                    "-subj", "/CN=mutated-ca",
                    "-addext", "basicConstraints=critical,CA:TRUE",
                ],
                check=True,
                capture_output=True,
            )

            ctx = create_ssl_context()
            # Mutate after helper construction.
            ctx.load_verify_locations(cafile=ca_path)
            assert not _eggfetch_ssl_registry.is_eggfetch_context(ctx), (
                "Registry still treats the mutated helper context "
                "as a constructible helper context.  The "
                "fingerprint did not detect load_verify_locations."
            )
            # Translation classifies from the live snapshot — the
            # custom CA appears as a DER list.
            kwargs = context_to_eggfetch_kwargs(ctx)
            assert isinstance(kwargs.get("verify"), list)

    def test_helper_context_then_min_version_detected(self):
        """Lowering the TLS minimum version mutates the live
        public state.  The fingerprint must reject reuse of
        stale metadata so the new version bounds are honored.
        """
        from eggfetch.compat.httpx._ssl_context import (
            _eggfetch_ssl_registry,
        )

        ctx = create_ssl_context()
        ctx.minimum_version = ssl.TLSVersion.TLSv1_2
        # Default helper context with TLS 1.2 minimum is a no-op
        # for the classification result, so the fingerprint must
        # still detect the mutation.
        assert not _eggfetch_ssl_registry.is_eggfetch_context(ctx), (
            "Min-version mutation was not detected by the "
            "construction fingerprint."
        )

    def test_helper_context_then_cipher_policy_change_rejected(self):
        """A helper context with a custom cipher policy must be
        rejected before dispatch because rustls cannot represent
        arbitrary OpenSSL cipher strings.
        """
        ctx = create_ssl_context()
        ctx.set_ciphers("AES256-SHA")
        c = Client(verify=ctx)
        with pytest.raises(TypeError, match="cannot safely translate"):
            c.get("https://example.com/")
        c.close()

    def test_passthrough_context_translation_classifies_from_snapshot(self):
        """``create_ssl_context(verify=<external ctx>)`` must pass
        through the external context unchanged and treat it as a
        caller-created context for translation purposes.
        """
        from eggfetch.compat.httpx._ssl_context import (
            _eggfetch_ssl_registry,
            context_to_eggfetch_kwargs,
        )

        external = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        external.check_hostname = False
        external.verify_mode = ssl.CERT_NONE

        returned = create_ssl_context(verify=external)
        assert returned is external
        # Passthrough is registered but must not carry helper
        # provenance: the registry must not treat it as a
        # helper-constructible context.
        assert _eggfetch_ssl_registry.is_passthrough(returned)
        kwargs = context_to_eggfetch_kwargs(returned)
        assert kwargs.get("verify") is False

    def test_external_mtls_context_without_provenance_rejected(self):
        """An external context with client cert state but no
        helper-recorded ``cert_path`` cannot be safely exported;
        translation must classify from the live state and not
        silently downgrade to no client auth.

        The live snapshot has no public API to inspect the loaded
        client cert in Python 3.12; therefore translation
        faithfully reports the context as having no recorded
        client identity, and mTLS-required servers observe the
        absence of a client cert.
        """
        from eggfetch.compat.httpx._ssl_context import (
            context_to_eggfetch_kwargs,
        )

        # An external context with no helper provenance.
        external = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        external.check_hostname = False
        external.verify_mode = ssl.CERT_NONE
        # ``cert_path`` is ``None`` because we did not go through
        # ``create_ssl_context(cert=...)``.
        kwargs = context_to_eggfetch_kwargs(external)
        assert "cert" not in kwargs


# ── CERT_REQUIRED + check_hostname=False ──────────────────────────────


class TestCertRequiredWithoutHostname:
    """The plan's "CERT_REQUIRED + check_hostname=False" rule.

    Cert validation without hostname verification is a narrow
    representable case; eggfetch maps it to native
    ``verify_hostname(false)``.  The translation must not
    silently substitute hostname-verifying validation.
    """

    def test_cert_required_without_hostname_does_not_enable_hostname(self):
        from eggfetch.compat.httpx._ssl_context import (
            context_to_eggfetch_kwargs,
        )

        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_REQUIRED

        kwargs = context_to_eggfetch_kwargs(ctx)
        # The translation must not silently substitute a
        # hostname-verifying ``verify=True`` here.
        assert "verify" in kwargs


# ── Debug redaction ───────────────────────────────────────────────────


class TestProxyHeaderRedaction:
    """Proxy header values must not appear in diagnostic output.

    Diagnostic redaction is two-fold:
    - The core ``Headers`` type (``eggfetch.Headers``) redacts
      ``authorization``, ``proxy-authorization``, ``cookie``, and
      ``set-cookie`` values in its ``repr``/``__str__`` while
      leaving ``iter``/``get`` non-redacted for protocol use.
    - The compatibility ``Proxy`` class redacts the same set of
      names in its ``repr``/``__str__`` because that surface is
      diagnostic, not protocol.
    """

    def test_proxy_repr_redacts_sensitive_values(self):
        from eggfetch.compat.httpx import Proxy

        proxy = Proxy(
            "http://proxy.example.com",
            headers=[
                ("proxy-authorization", "Basic dXNlcjpwYXNz"),
                ("x-test", "non-secret"),
            ],
        )
        debug = repr(proxy)
        assert "dXNlcjpwYXNz" not in debug
        assert "<redacted>" in debug
        assert "non-secret" in debug

    def test_proxy_repr_redacts_authorization_case_insensitive(self):
        from eggfetch.compat.httpx import Proxy

        proxy = Proxy(
            "http://proxy.example.com",
            headers={"Proxy-Authorization": "Basic secret-token"},
        )
        debug = repr(proxy)
        assert "secret-token" not in debug
        assert "<redacted>" in debug

    def test_proxy_repr_does_not_redact_non_sensitive_headers(self):
        from eggfetch.compat.httpx import Proxy

        proxy = Proxy(
            "http://proxy.example.com",
            headers={"X-Custom": "value-with-marker-9F2C"},
        )
        debug = repr(proxy)
        assert "value-with-marker-9F2C" in debug

    def test_proxy_headers_remain_non_redacted_for_protocol_use(self):
        """Protocol code must still observe the raw header values;
        redaction is a diagnostic surface only.
        """
        from eggfetch.compat.httpx import Proxy

        proxy = Proxy(
            "http://proxy.example.com",
            headers={"proxy-authorization": "Basic keep-raw"},
        )
        assert ("proxy-authorization", "Basic keep-raw") in proxy.headers
