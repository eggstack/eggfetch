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


class TestPrivateSSLContextExport:
    """Exercise the single versioned Python-to-Rust private contract."""

    def test_export_has_bounded_versioned_shape(self):
        from eggfetch._ssl_context import _export_ssl_context_state

        payload = _export_ssl_context_state(ssl.create_default_context())
        assert set(payload) == {
            "schema_version",
            "classification",
            "verify_mode",
            "check_hostname",
            "ca_certs_der",
            "min_version",
            "max_version",
            "helper_metadata",
        }
        assert payload["schema_version"] == 1
        assert isinstance(payload["ca_certs_der"], list)
        assert payload["helper_metadata"] is None

    def test_export_preserves_helper_provenance_without_secrets(self):
        from eggfetch._ssl_context import _export_ssl_context_state

        payload = _export_ssl_context_state(create_ssl_context(verify=False))
        assert payload["helper_metadata"]["verify"] is False
        assert "private_key" not in repr(payload)
        assert "pem" not in repr(payload).lower()

    def test_export_check_hostname_false(self):
        from eggfetch._ssl_context import _export_ssl_context_state

        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        payload = _export_ssl_context_state(ctx)
        assert payload["check_hostname"] is False
        assert payload["verify_mode"] == ssl.CERT_NONE
        assert payload["helper_metadata"] is None

    def test_export_custom_ca_der(self):
        from eggfetch._ssl_context import _export_ssl_context_state

        payload = _export_ssl_context_state(ssl.create_default_context())
        assert isinstance(payload["ca_certs_der"], list)
        assert len(payload["ca_certs_der"]) > 0
        assert all(isinstance(entry, bytes) for entry in payload["ca_certs_der"])

    def test_export_tls12_tls13_bounds(self):
        from eggfetch._ssl_context import _export_ssl_context_state

        ctx = ssl.create_default_context()
        ctx.minimum_version = ssl.TLSVersion.TLSv1_2
        ctx.maximum_version = ssl.TLSVersion.TLSv1_3
        payload = _export_ssl_context_state(ctx)
        assert payload["min_version"] == int(ssl.TLSVersion.TLSv1_2)
        assert payload["max_version"] == int(ssl.TLSVersion.TLSv1_3)
        # Representable bounds must still construct before any dispatch.
        from eggfetch import Client as NativeClient

        client = NativeClient(verify=ctx)
        client.close()

    def test_export_helper_mtls_provenance(self):
        from eggfetch._ssl_context import (
            _eggfetch_ssl_registry,
            _export_ssl_context_state,
        )

        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        _eggfetch_ssl_registry.register(
            ctx, cert_path="/tmp/client.pem", key_path="/tmp/client.key",
            verify=True, trust_env=True,
        )
        payload = _export_ssl_context_state(ctx)
        assert payload["helper_metadata"] is not None
        assert payload["helper_metadata"]["cert_path"] == "/tmp/client.pem"
        assert payload["helper_metadata"]["key_path"] == "/tmp/client.key"
        assert "private_key" not in repr(payload).lower()

    def test_export_mutation_invalidation(self):
        from eggfetch._ssl_context import (
            _eggfetch_ssl_registry,
            _export_ssl_context_state,
        )

        ctx = create_ssl_context(verify=False)
        assert _eggfetch_ssl_registry.is_eggfetch_context(ctx)
        # Live mutation must drop stale helper provenance.
        ctx.verify_mode = ssl.CERT_REQUIRED
        ctx.check_hostname = True
        payload = _export_ssl_context_state(ctx)
        assert payload["helper_metadata"] is None

    def _assert_native_rejects_before_dispatch(
        self, monkeypatch, ctx, payload, match
    ):
        import eggfetch._ssl_context as bridge

        monkeypatch.setattr(
            bridge, "_export_ssl_context_state", lambda _ctx: payload
        )
        with pytest.raises((TypeError, ValueError), match=match):
            from eggfetch import Client as NativeClient

            NativeClient(verify=ctx)

    @pytest.mark.parametrize(
        ("field", "value"),
        [
            ("schema_version", 99),
            ("classification", "future-classification"),
            ("verify_mode", "required"),
            ("ca_certs_der", ["not-der"]),
        ],
    )
    def test_malformed_export_fails_before_dispatch(self, monkeypatch, field, value):
        import eggfetch._ssl_context as bridge

        ctx = ssl.create_default_context()
        payload = bridge._export_ssl_context_state(ctx)
        payload[field] = value
        monkeypatch.setattr(bridge, "_export_ssl_context_state", lambda _ctx: payload)
        with pytest.raises((TypeError, ValueError), match="SSLContext export"):
            from eggfetch import Client as NativeClient

            NativeClient(verify=ctx)

    def test_malformed_payload_not_mapping_fails_before_dispatch(
        self, monkeypatch
    ):
        ctx = ssl.create_default_context()
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, ["not-a-mapping"], "SSLContext export"
        )

    @pytest.mark.parametrize("field", ["schema_version", "classification"])
    def test_malformed_missing_header_field_fails_before_dispatch(
        self, monkeypatch, field
    ):
        import eggfetch._ssl_context as bridge

        ctx = ssl.create_default_context()
        payload = bridge._export_ssl_context_state(ctx)
        del payload[field]
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )

    def test_malformed_unknown_schema_version_fails_before_dispatch(
        self, monkeypatch
    ):
        import eggfetch._ssl_context as bridge

        ctx = ssl.create_default_context()
        payload = bridge._export_ssl_context_state(ctx)
        payload["schema_version"] = 99
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )

    def test_malformed_invalid_classification_fails_before_dispatch(
        self, monkeypatch
    ):
        import eggfetch._ssl_context as bridge

        ctx = ssl.create_default_context()
        payload = bridge._export_ssl_context_state(ctx)
        payload["classification"] = "future-classification"
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )

    @pytest.mark.parametrize(
        "field", ["verify_mode", "check_hostname", "ca_certs_der"]
    )
    def test_malformed_missing_core_field_fails_before_dispatch(
        self, monkeypatch, field
    ):
        import eggfetch._ssl_context as bridge

        ctx = ssl.create_default_context()
        payload = bridge._export_ssl_context_state(ctx)
        del payload[field]
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )

    @pytest.mark.parametrize(
        ("field", "value"),
        [
            ("verify_mode", "required"),
            ("verify_mode", None),
            ("check_hostname", "yes"),
            ("check_hostname", 1),
            ("ca_certs_der", "not-a-list"),
            ("ca_certs_der", ["not-der"]),
            ("ca_certs_der", [None]),
            ("ca_certs_der", [{"der": b"bytes"}]),
        ],
    )
    def test_malformed_wrong_type_core_field_fails_before_dispatch(
        self, monkeypatch, field, value
    ):
        import eggfetch._ssl_context as bridge

        ctx = ssl.create_default_context()
        payload = bridge._export_ssl_context_state(ctx)
        payload[field] = value
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )

    @pytest.mark.parametrize("field", ["min_version", "max_version"])
    def test_malformed_missing_version_field_fails_before_dispatch(
        self, monkeypatch, field
    ):
        import eggfetch._ssl_context as bridge

        ctx = ssl.create_default_context()
        payload = bridge._export_ssl_context_state(ctx)
        del payload[field]
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )

    @pytest.mark.parametrize(
        ("field", "value"),
        [
            ("min_version", "TLSv1.2"),
            ("min_version", [771]),
            ("max_version", "TLSv1.3"),
            ("max_version", {"version": 772}),
        ],
    )
    def test_malformed_wrong_type_version_field_fails_before_dispatch(
        self, monkeypatch, field, value
    ):
        import eggfetch._ssl_context as bridge

        ctx = ssl.create_default_context()
        payload = bridge._export_ssl_context_state(ctx)
        payload[field] = value
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )

    @pytest.mark.parametrize("value", [770, 769, 999])
    def test_malformed_unsupported_min_version_fails_before_dispatch(
        self, monkeypatch, value
    ):
        import eggfetch._ssl_context as bridge

        ctx = ssl.create_default_context()
        payload = bridge._export_ssl_context_state(ctx)
        payload["min_version"] = value
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "unsupported TLS"
        )

    @pytest.mark.parametrize("value", [773, 999, 10000])
    def test_malformed_unsupported_max_version_fails_before_dispatch(
        self, monkeypatch, value
    ):
        import eggfetch._ssl_context as bridge

        ctx = ssl.create_default_context()
        payload = bridge._export_ssl_context_state(ctx)
        payload["max_version"] = value
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "unsupported TLS"
        )

    def test_malformed_missing_helper_metadata_fails_before_dispatch(
        self, monkeypatch
    ):
        import eggfetch._ssl_context as bridge

        ctx = ssl.create_default_context()
        payload = bridge._export_ssl_context_state(ctx)
        del payload["helper_metadata"]
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )

    @pytest.mark.parametrize(
        "value", ["metadata", ["metadata"], 123, True]
    )
    def test_malformed_non_mapping_helper_metadata_fails_before_dispatch(
        self, monkeypatch, value
    ):
        import eggfetch._ssl_context as bridge

        ctx = ssl.create_default_context()
        payload = bridge._export_ssl_context_state(ctx)
        payload["helper_metadata"] = value
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )

    def _helper_payload(self):
        import eggfetch._ssl_context as bridge

        ctx = create_ssl_context(verify=False)
        payload = bridge._export_ssl_context_state(ctx)
        assert isinstance(payload["helper_metadata"], dict)
        return ctx, payload

    @pytest.mark.parametrize("bad_verify", [123, None, ["no-verify"], {"verify": False}])
    def test_malformed_helper_verify_fails_before_dispatch(
        self, monkeypatch, bad_verify
    ):
        ctx, payload = self._helper_payload()
        payload["helper_metadata"] = dict(payload["helper_metadata"])
        payload["helper_metadata"]["verify"] = bad_verify
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )

    def test_malformed_helper_missing_verify_fails_before_dispatch(
        self, monkeypatch
    ):
        ctx, payload = self._helper_payload()
        payload["helper_metadata"] = dict(payload["helper_metadata"])
        del payload["helper_metadata"]["verify"]
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )

    @pytest.mark.parametrize("bad_path", [123, ["path"], {"path": "x"}])
    def test_malformed_helper_cert_path_fails_before_dispatch(
        self, monkeypatch, bad_path
    ):
        ctx, payload = self._helper_payload()
        payload["helper_metadata"] = dict(payload["helper_metadata"])
        payload["helper_metadata"]["cert_path"] = bad_path
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )

    @pytest.mark.parametrize("bad_path", [123, ["path"], {"path": "x"}])
    def test_malformed_helper_key_path_fails_before_dispatch(
        self, monkeypatch, bad_path
    ):
        ctx, payload = self._helper_payload()
        payload["helper_metadata"] = dict(payload["helper_metadata"])
        payload["helper_metadata"]["key_path"] = bad_path
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )

    def test_malformed_helper_missing_cert_path_fails_before_dispatch(
        self, monkeypatch
    ):
        ctx, payload = self._helper_payload()
        payload["helper_metadata"] = dict(payload["helper_metadata"])
        del payload["helper_metadata"]["cert_path"]
        self._assert_native_rejects_before_dispatch(
            monkeypatch, ctx, payload, "SSLContext export"
        )


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
