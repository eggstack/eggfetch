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

    def test_sha256_algorithm(self):
        auth = DigestAuth("user", "pass")
        request = Request("GET", "http://example.com/")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="abc", '
                    'qop="auth", algorithm=SHA-256',
                )
            ],
        )
        auth_request = flow.send(response)
        assert "algorithm=SHA-256" in auth_request.headers["authorization"]

    def test_qop_auth_int(self):
        auth = DigestAuth("user", "pass")
        request = Request("POST", "http://example.com/", content=b"body-data")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="abc", qop="auth-int"',
                )
            ],
        )
        auth_request = flow.send(response)
        assert "qop=auth-int" in auth_request.headers["authorization"]

    def test_nonce_count_increments(self):
        auth = DigestAuth("user", "pass")

        # First request
        request1 = Request("GET", "http://example.com/")
        flow1 = auth.auth_flow(request1)
        next(flow1)
        response1 = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="abc", qop="auth"',
                )
            ],
        )
        flow1.send(response1)
        assert auth._nonce_count == 1

        # Second request
        request2 = Request("GET", "http://example.com/")
        flow2 = auth.auth_flow(request2)
        next(flow2)
        response2 = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="abc", qop="auth"',
                )
            ],
        )
        auth_request2 = flow2.send(response2)
        assert "nc=00000002" in auth_request2.headers["authorization"]
        assert auth._nonce_count == 2

    def test_opaque_passthrough(self):
        auth = DigestAuth("user", "pass")
        request = Request("GET", "http://example.com/")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="abc", qop="auth", '
                    'opaque="my-opaque-value"',
                )
            ],
        )
        auth_request = flow.send(response)
        assert 'opaque="my-opaque-value"' in auth_request.headers["authorization"]

    def test_no_qop_old_style_response(self):
        auth = DigestAuth("user", "pass")
        request = Request("GET", "http://example.com/")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="abc"',
                )
            ],
        )
        auth_request = flow.send(response)
        # Old-style: no qop, nc, or cnonce
        auth_header = auth_request.headers["authorization"]
        assert "qop=" not in auth_header
        assert "nc=" not in auth_header

    def test_non_digest_challenge_ignored(self):
        auth = DigestAuth("user", "pass")
        request = Request("GET", "http://example.com/")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[("www-authenticate", 'Bearer realm="test"')],
        )
        with pytest.raises(StopIteration):
            flow.send(response)

    def test_repr(self):
        auth = DigestAuth("user", "pass")
        assert "user" in repr(auth)

    def test_sha256_auth_int_combo(self):
        auth = DigestAuth("user", "pass")
        request = Request("POST", "http://example.com/", content=b"body-data")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="abc", '
                    'qop="auth-int", algorithm=SHA-256',
                )
            ],
        )
        auth_request = flow.send(response)
        assert "algorithm=SHA-256" in auth_request.headers["authorization"]
        assert "qop=auth-int" in auth_request.headers["authorization"]

    def test_uri_with_query_string(self):
        auth = DigestAuth("user", "pass")
        request = Request("GET", "http://example.com/search?q=test&page=1")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="abc", qop="auth"',
                )
            ],
        )
        auth_request = flow.send(response)
        assert 'uri="/search?q=test&page=1"' in auth_request.headers["authorization"]

    def test_qop_list_negotiation(self):
        """Server advertises qop as comma-separated list; client picks first supported."""
        auth = DigestAuth("user", "pass")
        request = Request("GET", "http://example.com/")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="abc", '
                    'qop="auth-int,auth"',
                )
            ],
        )
        auth_request = flow.send(response)
        # Should pick auth-int (first supported from the list)
        assert "qop=auth-int" in auth_request.headers["authorization"]

    def test_qop_list_auth_first(self):
        """Server advertises qop as comma-separated list with auth first."""
        auth = DigestAuth("user", "pass")
        request = Request("GET", "http://example.com/")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="abc", '
                    'qop="auth,auth-int"',
                )
            ],
        )
        auth_request = flow.send(response)
        # Should pick auth (first supported)
        assert "qop=auth" in auth_request.headers["authorization"]

    def test_qop_list_no_supported_falls_back(self):
        """Server advertises only unsupported qop; client falls back to old-style."""
        auth = DigestAuth("user", "pass")
        request = Request("GET", "http://example.com/")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="abc", '
                    'qop="unknown"',
                )
            ],
        )
        auth_request = flow.send(response)
        # Should fall back to old-style (no qop/nc/cnonce)
        auth_header = auth_request.headers["authorization"]
        assert "qop=" not in auth_header
        assert "nc=" not in auth_header

    def test_cross_origin_redirect_preserves_auth(self):
        """Digest auth works across redirect to different host (new request)."""
        auth = DigestAuth("user", "pass")
        request = Request("GET", "http://host-a.example.com/")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="host-a.example.com", nonce="nonce1", qop="auth"',
                )
            ],
        )
        auth_request = flow.send(response)
        assert "authorization" in auth_request.headers

        # Simulate redirect: new request to host-b
        request2 = Request("GET", "http://host-b.example.com/")
        flow2 = auth.auth_flow(request2)
        next(flow2)

        response2 = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="host-b.example.com", nonce="nonce2", qop="auth"',
                )
            ],
        )
        auth_request2 = flow2.send(response2)
        assert "authorization" in auth_request2.headers
        assert 'realm="host-b.example.com"' in auth_request2.headers["authorization"]

    def test_body_hashing_with_auth_int(self):
        """auth-int includes entity body hash in response."""
        auth = DigestAuth("user", "pass")
        body = b"test body content"
        request = Request("POST", "http://example.com/", content=body)
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="abc", qop="auth-int"',
                )
            ],
        )
        auth_request = flow.send(response)
        auth_header = auth_request.headers["authorization"]
        assert "qop=auth-int" in auth_header
        # The header should contain a response hash
        assert "response=" in auth_header

    def test_repeated_challenge_increments_nonce(self):
        """Multiple challenges with same nonce keep incrementing nc."""
        auth = DigestAuth("user", "pass")

        for i in range(3):
            request = Request("GET", "http://example.com/")
            flow = auth.auth_flow(request)
            next(flow)

            response = Response(
                401,
                headers=[
                    (
                        "www-authenticate",
                        'Digest realm="test", nonce="abc", qop="auth"',
                    )
                ],
            )
            auth_request = flow.send(response)
            expected_nc = f"nc={i + 1:08d}"
            assert expected_nc in auth_request.headers["authorization"]

    def test_empty_body_with_auth_int(self):
        """auth-int with empty body (GET request) works."""
        auth = DigestAuth("user", "pass")
        request = Request("GET", "http://example.com/")
        flow = auth.auth_flow(request)
        next(flow)

        response = Response(
            401,
            headers=[
                (
                    "www-authenticate",
                    'Digest realm="test", nonce="abc", qop="auth-int"',
                )
            ],
        )
        auth_request = flow.send(response)
        assert "qop=auth-int" in auth_request.headers["authorization"]


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

    def test_netrc_with_account_field(self):
        """Account field is present but ignored during lookup."""
        content = "machine example.com login admin password secret account myaccount\n"
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

    def test_netrc_with_comments(self):
        """Comment lines (starting with #) are ignored."""
        content = (
            "# This is a comment\n"
            "machine example.com login admin password secret\n"
            "# Another comment\n"
        )
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

    def test_netrc_multiple_machines(self):
        """Multiple machine entries are parsed correctly."""
        content = (
            "machine host1.com login user1 password pass1\n"
            "machine host2.com login user2 password pass2\n"
        )
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".netrc", delete=False
        ) as f:
            f.write(content)
            f.flush()
            try:
                auth = NetRCAuth(f.name)
                assert auth._lookup_credentials("host1.com") == ("user1", "pass1")
                assert auth._lookup_credentials("host2.com") == ("user2", "pass2")
            finally:
                os.unlink(f.name)

    def test_netrc_default_entry_fallback(self):
        """Default entry is used when no exact match found."""
        content = (
            "machine example.com login specific password cred\n"
            "default login fallback password fallbackpass\n"
        )
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".netrc", delete=False
        ) as f:
            f.write(content)
            f.flush()
            try:
                auth = NetRCAuth(f.name)
                # Exact match
                assert auth._lookup_credentials("example.com") == ("specific", "cred")
                # Falls back to default
                assert auth._lookup_credentials("unknown.com") == ("fallback", "fallbackpass")
            finally:
                os.unlink(f.name)

    def test_netrc_empty_file(self):
        """Empty netrc file returns no credentials."""
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".netrc", delete=False
        ) as f:
            f.write("")
            f.flush()
            try:
                auth = NetRCAuth(f.name)
                creds = auth._lookup_credentials("example.com")
                assert creds is None
            finally:
                os.unlink(f.name)

    @pytest.mark.skipif(os.name == "nt", reason="Unix permission check")
    def test_netrc_strict_permissions_accepted(self):
        """File with 0600 permissions is accepted."""
        content = "machine example.com login admin password secret\n"
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".netrc", delete=False
        ) as f:
            f.write(content)
            f.flush()
            try:
                os.chmod(f.name, 0o600)
                auth = NetRCAuth(f.name)
                creds = auth._lookup_credentials("example.com")
                assert creds == ("admin", "secret")
            finally:
                os.unlink(f.name)

    @pytest.mark.skipif(os.name == "nt", reason="Unix permission check")
    def test_netrc_group_readable_rejected(self):
        """File with 0640 permissions (group-readable) is rejected."""
        content = "machine example.com login admin password secret\n"
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".netrc", delete=False
        ) as f:
            f.write(content)
            f.flush()
            try:
                os.chmod(f.name, 0o640)
                auth = NetRCAuth(f.name)
                creds = auth._lookup_credentials("example.com")
                assert creds is None
            finally:
                os.unlink(f.name)

    def test_netrc_no_login_entry_skipped(self):
        """Entry with no login is skipped (returns None)."""
        content = "machine example.com password secret\n"
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".netrc", delete=False
        ) as f:
            f.write(content)
            f.flush()
            try:
                auth = NetRCAuth(f.name)
                creds = auth._lookup_credentials("example.com")
                assert creds is None
            finally:
                os.unlink(f.name)

    def test_netrc_auth_flow_passthrough_without_credentials(self):
        """Auth flow passes through unchanged when no credentials found."""
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
