"""Tests for HTTPX-compatible authentication."""
from __future__ import annotations

import base64
import os
import tempfile
import pytest
from eggfetch.compat.httpx import (
    Auth,
    BasicAuth,
    DigestAuth,
    NetRCAuth,
    Request,
    Response,
)


class TestAuth:
    def test_base_class_not_implemented(self):
        auth = Auth()
        request = Request("GET", "http://example.com/")
        with pytest.raises(NotImplementedError):
            next(auth.auth_flow(request))


class TestBasicAuth:
    def test_stores_credentials(self):
        auth = BasicAuth("user", "pass")
        assert auth.username == "user"
        assert auth.password == "pass"
        assert auth.encoding == "latin-1"

    def test_custom_encoding(self):
        auth = BasicAuth("user", "pass", encoding="utf-8")
        assert auth.encoding == "utf-8"

    def test_auth_flow_adds_header(self):
        auth = BasicAuth("user", "pass")
        request = Request("GET", "http://example.com/")
        flow = auth.auth_flow(request)
        modified_request = next(flow)

        auth_header = modified_request.headers["authorization"]
        assert auth_header.startswith("Basic ")

        encoded = auth_header.split(" ", 1)[1]
        decoded = base64.b64decode(encoded).decode("latin-1")
        assert decoded == "user:pass"

    def test_auth_flow_single_yield(self):
        auth = BasicAuth("user", "pass")
        request = Request("GET", "http://example.com/")
        flow = auth.auth_flow(request)
        next(flow)
        with pytest.raises(StopIteration):
            next(flow)

    def test_repr(self):
        auth = BasicAuth("admin")
        assert "admin" in repr(auth)

    def test_empty_credentials(self):
        auth = BasicAuth()
        request = Request("GET", "http://example.com/")
        flow = auth.auth_flow(request)
        modified = next(flow)
        auth_header = modified.headers["authorization"]
        assert auth_header == "Basic " + base64.b64encode(b":").decode()


class TestDigestAuth:
    def test_stores_credentials(self):
        auth = DigestAuth("user", "pass")
        assert auth.username == "user"
        assert auth.password == "pass"

    def test_auth_flow_uses_challenge(self):
        auth = DigestAuth("admin", "password123")
        request = Request("GET", "http://example.com/private")
        flow = auth.auth_flow(request)

        first_request = next(flow)
        assert first_request.method == "GET"

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test@example.com", '
                    'nonce="abc123", '
                    'qop="auth", '
                    "algorithm=MD5",
                )
            ],
        )

        auth_request = flow.send(response)
        assert "authorization" in auth_request.headers
        assert auth_request.headers["authorization"].startswith("Digest ")

    def test_auth_flow_no_challenge_passthrough(self):
        auth = DigestAuth("user", "pass")
        request = Request("GET", "http://example.com/")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(200)
        with pytest.raises(StopIteration):
            flow.send(response)

    def test_stale_nonce_resets_count(self):
        auth = DigestAuth("user", "pass")
        auth._nonce_count = 5
        request = Request("GET", "http://example.com/")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="xyz", qop="auth", stale=TRUE',
                )
            ],
        )
        flow.send(response)
        assert auth._nonce_count == 1

    def test_repr(self):
        auth = DigestAuth("user", "pass")
        assert "user" in repr(auth)


class TestNetRCAuth:
    def test_parse_netrc_basic(self):
        content = "machine example.com login admin password secret\n"
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".netrc", delete=False
        ) as f:
            f.write(content)
            f.flush()
            try:
                auth = NetRCAuth(f.name)
                creds = auth._lookup_credentials("example.com")
                assert creds == ("admin", "secret")
            finally:
                os.unlink(f.name)

    def test_parse_netrc_default_entry(self):
        content = "default login fallback password pass123\n"
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".netrc", delete=False
        ) as f:
            f.write(content)
            f.flush()
            try:
                auth = NetRCAuth(f.name)
                creds = auth._lookup_credentials("any-host.com")
                assert creds == ("fallback", "pass123")
            finally:
                os.unlink(f.name)

    def test_no_match_returns_none(self):
        content = "machine example.com login admin password secret\n"
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".netrc", delete=False
        ) as f:
            f.write(content)
            f.flush()
            try:
                auth = NetRCAuth(f.name)
                creds = auth._lookup_credentials("other.com")
                assert creds is None
            finally:
                os.unlink(f.name)

    def test_missing_file(self):
        auth = NetRCAuth("/nonexistent/.netrc")
        creds = auth._lookup_credentials("example.com")
        assert creds is None

    def test_auth_flow_with_credentials(self):
        content = "machine testserver.com login user password pass\n"
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".netrc", delete=False
        ) as f:
            f.write(content)
            f.flush()
            try:
                auth = NetRCAuth(f.name)
                request = Request("GET", "http://testserver.com/")
                flow = auth.auth_flow(request)
                modified = next(flow)
                assert "authorization" in modified.headers
                assert modified.headers["authorization"].startswith("Basic ")
            finally:
                os.unlink(f.name)

    def test_auth_flow_no_match_passthrough(self):
        content = "machine other.com login x password y\n"
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".netrc", delete=False
        ) as f:
            f.write(content)
            f.flush()
            try:
                auth = NetRCAuth(f.name)
                request = Request("GET", "http://example.com/")
                flow = auth.auth_flow(request)
                req = next(flow)
                assert "authorization" not in req.headers
            finally:
                os.unlink(f.name)

    def test_repr(self):
        auth = NetRCAuth("/tmp/test.netrc")
        assert "/tmp/test.netrc" in repr(auth)

    @pytest.mark.skipif(os.name == "nt", reason="Unix permission check")
    def test_rejects_permissive_permissions(self):
        content = "machine example.com login admin password secret\n"
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".netrc", delete=False
        ) as f:
            f.write(content)
            f.flush()
            try:
                os.chmod(f.name, 0o644)
                auth = NetRCAuth(f.name)
                creds = auth._lookup_credentials("example.com")
                assert creds is None
            finally:
                os.unlink(f.name)
