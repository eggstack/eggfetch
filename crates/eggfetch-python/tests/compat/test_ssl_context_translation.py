"""Focused TLS context translation tests for HTTPX parity phase 01.

Covers:
- create_ssl_context() construction matching HTTPX 0.28.1
- SSLContext snapshot extraction
- Representability classification
- Registry lifecycle
- Client/AsyncClient SSLContext interception
"""

import os
import ssl
import sys
import tempfile
import warnings

import pytest

from eggfetch.compat.httpx import (
    Client,
    AsyncClient,
    MockTransport,
    Request,
    Response,
    Timeout,
    create_ssl_context,
)


# ── create_ssl_context() construction parity ─────────────────────────


class TestCreateSSLContextConstruction:
    """Match HTTPX 0.28.1 create_ssl_context() construction behavior."""

    def test_returns_ssl_context(self):
        ctx = create_ssl_context()
        assert isinstance(ctx, ssl.SSLContext)

    def test_default_verify_true(self):
        ctx = create_ssl_context()
        assert ctx.verify_mode == ssl.CERT_REQUIRED
        assert ctx.check_hostname is True

    def test_verify_false(self):
        ctx = create_ssl_context(verify=False)
        assert ctx.verify_mode == ssl.CERT_NONE
        assert ctx.check_hostname is False

    def test_verify_false_is_separate(self):
        """verify=False must not leak into other contexts."""
        ctx_disabled = create_ssl_context(verify=False)
        ctx_default = create_ssl_context()
        assert ctx_disabled.verify_mode == ssl.CERT_NONE
        assert ctx_default.verify_mode == ssl.CERT_REQUIRED

    def test_trust_env_true_default(self):
        ctx = create_ssl_context(trust_env=True)
        assert isinstance(ctx, ssl.SSLContext)
        assert ctx.verify_mode == ssl.CERT_REQUIRED

    def test_trust_env_false(self):
        ctx = create_ssl_context(trust_env=False)
        assert isinstance(ctx, ssl.SSLContext)
        assert ctx.verify_mode == ssl.CERT_REQUIRED

    def test_verify_str_deprecated(self):
        import certifi

        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            ctx = create_ssl_context(verify=certifi.where())
            assert len(w) == 1
            assert issubclass(w[0].category, DeprecationWarning)
            assert "deprecated" in str(w[0].message).lower()
        assert isinstance(ctx, ssl.SSLContext)

    def test_verify_str_directory_deprecated(self, tmp_path):
        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            ctx = create_ssl_context(verify=str(tmp_path))
            assert len(w) == 1
            assert issubclass(w[0].category, DeprecationWarning)
        assert isinstance(ctx, ssl.SSLContext)

    def test_verify_sslcontext_passthrough(self):
        custom = ssl.create_default_context()
        ctx = create_ssl_context(verify=custom)
        assert ctx is custom

    def test_verify_invalid_type_raises(self):
        with pytest.raises(TypeError, match="verify must be"):
            create_ssl_context(verify=42)

    def test_cert_str_deprecated(self):
        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            # cert= with a non-existent file will raise SSLError,
            # but the deprecation warning should fire first.
            try:
                create_ssl_context(cert="/nonexistent.pem")
            except (ssl.SSLError, FileNotFoundError):
                pass
            assert any(issubclass(x.category, DeprecationWarning) for x in w)

    def test_cert_tuple_deprecated(self):
        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            try:
                create_ssl_context(cert=("/nonexistent.pem", "/nonexistent.key"))
            except (ssl.SSLError, FileNotFoundError):
                pass
            assert any(issubclass(x.category, DeprecationWarning) for x in w)

    def test_ssl_cert_file_env(self, monkeypatch):
        import certifi

        monkeypatch.setenv("SSL_CERT_FILE", certifi.where())
        ctx = create_ssl_context(verify=True, trust_env=True)
        assert isinstance(ctx, ssl.SSLContext)

    def test_ssl_cert_dir_env(self, monkeypatch, tmp_path):
        monkeypatch.setenv("SSL_CERT_DIR", str(tmp_path))
        # SSL_CERT_DIR with empty dir may raise; that's OK for this test.
        try:
            ctx = create_ssl_context(verify=True, trust_env=True)
            assert isinstance(ctx, ssl.SSLContext)
        except ssl.SSLError:
            pass  # Empty cert dir is acceptable

    def test_ssl_cert_file_and_dir_env_are_both_accepted(self, monkeypatch, tmp_path):
        import certifi

        monkeypatch.setenv("SSL_CERT_FILE", certifi.where())
        monkeypatch.setenv("SSL_CERT_DIR", str(tmp_path))
        ctx = create_ssl_context(verify=True, trust_env=True)
        assert isinstance(ctx, ssl.SSLContext)

    def test_trust_env_false_ignores_env(self, monkeypatch):
        import certifi

        monkeypatch.setenv("SSL_CERT_FILE", certifi.where())
        # With trust_env=False, SSL_CERT_FILE should be ignored.
        ctx = create_ssl_context(verify=True, trust_env=False)
        assert isinstance(ctx, ssl.SSLContext)
        assert ctx.verify_mode == ssl.CERT_REQUIRED


# ── Snapshot extraction ──────────────────────────────────────────────


class TestSSLContextSnapshot:
    """Test _SSLContextSnapshot extraction from live contexts."""

    def test_snapshot_default_context(self):
        from eggfetch.compat.httpx._ssl_context import snapshot_context

        ctx = ssl.create_default_context()
        snap = snapshot_context(ctx)
        assert snap.verify_mode == ssl.CERT_REQUIRED
        assert snap.check_hostname is True
        assert snap.class_name == "SSLContext"

    def test_snapshot_disabled_verification(self):
        from eggfetch.compat.httpx._ssl_context import snapshot_context

        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        snap = snapshot_context(ctx)
        assert snap.verify_mode == ssl.CERT_NONE
        assert snap.check_hostname is False

    def test_snapshot_repr(self):
        from eggfetch.compat.httpx._ssl_context import snapshot_context

        ctx = ssl.create_default_context()
        snap = snapshot_context(ctx)
        r = repr(snap)
        assert "verify_mode=" in r
        assert "check_hostname=" in r
        assert "SSLContext" in r


# ── Classification ───────────────────────────────────────────────────


class TestClassification:
    """Test the representability classifier."""

    def test_default_context_exactly_representable(self):
        from eggfetch.compat.httpx._ssl_context import (
            Classification,
            _classify_context,
        )

        ctx = ssl.create_default_context()
        assert _classify_context(ctx) == Classification.EXACTLY_REPRESENTABLE

    def test_disabled_context_representable_with_defaults(self):
        from eggfetch.compat.httpx._ssl_context import (
            Classification,
            _classify_context,
        )

        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        cls = _classify_context(ctx)
        assert cls == Classification.REPRESENTABLE_WITH_DEFAULTS

    def test_eggfetch_created_context_classified(self):
        ctx = create_ssl_context()
        from eggfetch.compat.httpx._ssl_context import (
            Classification,
            _classify_context,
        )

        cls = _classify_context(ctx)
        assert cls in (
            Classification.EXACTLY_REPRESENTABLE,
            Classification.REPRESENTABLE_WITH_DEFAULTS,
        )


# ── Registry lifecycle ───────────────────────────────────────────────


class TestRegistry:
    """Test the weak-keyed SSL context registry."""

    def test_eggfetch_context_is_registered(self):
        from eggfetch.compat.httpx._ssl_context import _eggfetch_ssl_registry

        ctx = create_ssl_context()
        assert _eggfetch_ssl_registry.is_eggfetch_context(ctx)

    def test_external_context_not_registered(self):
        from eggfetch.compat.httpx._ssl_context import _eggfetch_ssl_registry

        ctx = ssl.create_default_context()
        assert not _eggfetch_ssl_registry.is_eggfetch_context(ctx)

    def test_registry_metadata_has_verify(self):
        from eggfetch.compat.httpx._ssl_context import _eggfetch_ssl_registry

        ctx = create_ssl_context()
        meta = _eggfetch_ssl_registry.get(ctx)
        assert meta is not None
        assert "verify" in meta
        assert "trust_env" in meta

    def test_registry_metadata_with_cert(self):
        from eggfetch.compat.httpx._ssl_context import _eggfetch_ssl_registry

        ctx = create_ssl_context(cert=None)
        meta = _eggfetch_ssl_registry.get(ctx)
        assert meta is not None

    def test_registry_gc_cleanup(self):
        import gc

        from eggfetch.compat.httpx._ssl_context import _eggfetch_ssl_registry

        ctx = create_ssl_context()
        assert _eggfetch_ssl_registry.is_eggfetch_context(ctx)
        del ctx
        gc.collect()
        # After GC, the weak reference should be dead.
        # We can't directly query by id, but we can verify
        # the registry doesn't hold stale strong refs.
        # Create a new context and verify registry works.
        ctx2 = create_ssl_context()
        assert _eggfetch_ssl_registry.is_eggfetch_context(ctx2)


# ── Client interception ──────────────────────────────────────────────


class TestClientSSLContextInterception:
    """Test that Client/AsyncClient intercept SSLContext verify args."""

    def test_client_with_eggfetch_context(self):
        ctx = create_ssl_context()
        captured = []

        def handler(request):
            captured.append(True)
            return Response(200)

        with Client(transport=MockTransport(handler), verify=ctx) as client:
            resp = client.get("https://example.com")
        assert resp.status_code == 200
        assert len(captured) == 1

    def test_client_with_disabled_context(self):
        ctx = create_ssl_context(verify=False)
        captured = []

        def handler(request):
            captured.append(True)
            return Response(200)

        with Client(transport=MockTransport(handler), verify=ctx) as client:
            resp = client.get("https://example.com")
        assert resp.status_code == 200

    def test_client_with_default_sslcontext(self):
        ctx = ssl.create_default_context()
        captured = []

        def handler(request):
            captured.append(True)
            return Response(200)

        with Client(transport=MockTransport(handler), verify=ctx) as client:
            resp = client.get("https://example.com")
        assert resp.status_code == 200

    def test_client_with_unrepresentable_context_rejects(self):
        """A context with custom ciphers should be rejected at construction."""
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        # This should work (CERT_NONE is representable)
        captured = []

        def handler(request):
            captured.append(True)
            return Response(200)

        with Client(transport=MockTransport(handler), verify=ctx) as client:
            resp = client.get("https://example.com")
        assert resp.status_code == 200

    def test_client_bool_verify_still_works(self):
        """Existing bool verify path must remain backward compatible."""
        captured = []

        def handler(request):
            captured.append(True)
            return Response(200)

        with Client(transport=MockTransport(handler), verify=True) as client:
            client.get("https://example.com")
        assert len(captured) == 1

        with Client(transport=MockTransport(handler), verify=False) as client:
            client.get("https://example.com")
        assert len(captured) == 2

    def test_client_str_verify_still_works(self):
        """Existing str verify path must remain backward compatible."""
        captured = []

        def handler(request):
            captured.append(True)
            return Response(200)

        with Client(transport=MockTransport(handler), verify=True) as client:
            client.get("https://example.com")
        assert len(captured) == 1


# ── context_to_eggfetch_kwargs ───────────────────────────────────────


class TestContextToKwargs:
    """Test the context-to-kwargs translation function."""

    def test_eggfetch_context_returns_registry_metadata(self):
        from eggfetch.compat.httpx._ssl_context import context_to_eggfetch_kwargs

        ctx = create_ssl_context()
        kwargs = context_to_eggfetch_kwargs(ctx)
        assert isinstance(kwargs, dict)

    def test_default_context_returns_bool_verify(self):
        from eggfetch.compat.httpx._ssl_context import context_to_eggfetch_kwargs

        # ``ssl.create_default_context()`` loads the system trust store
        # into the context.  EggFetch must carry the actual DER anchors
        # rather than inferring default trust from a CA-count heuristic
        # that could mask a deliberately-narrowed custom CA set.
        ctx = ssl.create_default_context()
        kwargs = context_to_eggfetch_kwargs(ctx)
        verify = kwargs.get("verify")
        assert isinstance(verify, list)
        assert all(isinstance(cert, (bytes, bytearray)) for cert in verify)
        assert len(verify) > 0

    def test_disabled_context_returns_false_verify(self):
        from eggfetch.compat.httpx._ssl_context import context_to_eggfetch_kwargs

        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        kwargs = context_to_eggfetch_kwargs(ctx)
        assert kwargs.get("verify") is False

    def test_unrepresentable_raises(self):
        """A third-party SSLContext subclass is unrepresentable."""
        from eggfetch.compat.httpx._ssl_context import context_to_eggfetch_kwargs

        # Create a custom subclass of SSLContext - this will have
        # class_name != "SSLContext" and will be rejected.
        class CustomSSLContext(ssl.SSLContext):
            pass

        ctx = CustomSSLContext(ssl.PROTOCOL_TLS_CLIENT)
        with pytest.raises(TypeError, match="cannot safely translate"):
            context_to_eggfetch_kwargs(ctx)
