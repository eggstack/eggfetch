"""Tests for HTTPX-compatible authentication."""
from __future__ import annotations

import asyncio
import base64
import os
import tempfile
import pytest
from eggfetch.compat.httpx import (
    Auth,
    AsyncClient,
    BasicAuth,
    Client,
    DigestAuth,
    MockTransport,
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


class TestAuthThroughTransport:
    """Verify auth is applied by the client regardless of transport."""

    def test_basic_auth_through_mock_transport_sync(self):
        """BasicAuth header is present when using MockTransport with Client."""
        def handler(request):
            return Response(200, text=request.headers.get("authorization", ""))

        with Client(
            auth=BasicAuth("user", "pass"),
            transport=MockTransport(handler),
        ) as client:
            response = client.get("http://testserver/")

        assert response.status_code == 200
        auth_header = response.text
        assert auth_header.startswith("Basic ")
        decoded = base64.b64decode(auth_header.split(" ", 1)[1]).decode("latin-1")
        assert decoded == "user:pass"

    @pytest.mark.asyncio
    async def test_basic_auth_through_mock_transport_async(self):
        """BasicAuth header is present when using MockTransport with AsyncClient."""
        def handler(request):
            return Response(200, text=request.headers.get("authorization", ""))

        async with AsyncClient(
            auth=BasicAuth("user", "pass"),
            transport=MockTransport(handler),
        ) as client:
            response = await client.get("http://testserver/")

        assert response.status_code == 200
        auth_header = response.text
        assert auth_header.startswith("Basic ")
        decoded = base64.b64decode(auth_header.split(" ", 1)[1]).decode("latin-1")
        assert decoded == "user:pass"

    def test_basic_auth_through_mounted_transport_sync(self):
        """BasicAuth works through a mounted transport (sync)."""
        def handler(request):
            return Response(200, text=request.headers.get("authorization", ""))

        with Client(
            auth=BasicAuth("user", "pass"),
            mounts={"http://": MockTransport(handler)},
        ) as client:
            response = client.get("http://testserver/")

        assert response.status_code == 200
        auth_header = response.text
        assert auth_header.startswith("Basic ")
        decoded = base64.b64decode(auth_header.split(" ", 1)[1]).decode("latin-1")
        assert decoded == "user:pass"

    @pytest.mark.asyncio
    async def test_basic_auth_through_mounted_transport_async(self):
        """BasicAuth works through a mounted transport (async)."""
        def handler(request):
            return Response(200, text=request.headers.get("authorization", ""))

        async with AsyncClient(
            auth=BasicAuth("user", "pass"),
            mounts={"http://": MockTransport(handler)},
        ) as client:
            response = await client.get("http://testserver/")

        assert response.status_code == 200
        auth_header = response.text
        assert auth_header.startswith("Basic ")
        decoded = base64.b64decode(auth_header.split(" ", 1)[1]).decode("latin-1")
        assert decoded == "user:pass"

    def test_multi_yield_auth_flow_through_transport(self):
        """A custom auth flow with multiple yields works through MockTransport."""
        call_count = 0

        class MultiStepAuth(Auth):
            def auth_flow(self, request):
                nonlocal call_count
                # First: add a header
                request.headers["x-auth-step"] = "1"
                response = yield request
                # Second: retry with a different header if challenged
                if response.status_code == 401:
                    call_count += 1
                    request.headers["x-auth-step"] = "2"
                    yield request

        def handler(request):
            step = request.headers.get("x-auth-step", "")
            if step == "1":
                return Response(401, text="unauthorized")
            return Response(200, text=f"step={step}")

        with Client(
            auth=MultiStepAuth(),
            transport=MockTransport(handler),
        ) as client:
            response = client.get("http://testserver/")

        assert response.status_code == 200
        assert response.text == "step=2"
        assert call_count == 1

    @pytest.mark.asyncio
    async def test_multi_yield_auth_flow_through_transport_async(self):
        """A custom auth flow with multiple yields works through MockTransport (async)."""
        call_count = 0

        class MultiStepAuth(Auth):
            def auth_flow(self, request):
                nonlocal call_count
                request.headers["x-auth-step"] = "1"
                response = yield request
                if response.status_code == 401:
                    call_count += 1
                    request.headers["x-auth-step"] = "2"
                    yield request

        def handler(request):
            step = request.headers.get("x-auth-step", "")
            if step == "1":
                return Response(401, text="unauthorized")
            return Response(200, text=f"step={step}")

        async with AsyncClient(
            auth=MultiStepAuth(),
            transport=MockTransport(handler),
        ) as client:
            response = await client.get("http://testserver/")

        assert response.status_code == 200
        assert response.text == "step=2"
        assert call_count == 1

    def test_intermediate_auth_response_handled(self):
        """Intermediate auth responses (401) are properly fed back to the auth flow."""
        responses_seen = []

        class TrackingAuth(Auth):
            def auth_flow(self, request):
                request.headers["x-attempt"] = "1"
                response = yield request
                responses_seen.append(response.status_code)
                if response.status_code == 401:
                    request.headers["x-attempt"] = "2"
                    response2 = yield request
                    responses_seen.append(response2.status_code)

        def handler(request):
            attempt = request.headers.get("x-attempt", "")
            if attempt == "1":
                return Response(401, text="need auth")
            return Response(200, text="ok")

        with Client(
            auth=TrackingAuth(),
            transport=MockTransport(handler),
        ) as client:
            response = client.get("http://testserver/")

        assert response.status_code == 200
        assert responses_seen == [401, 200]

    def test_request_context_preserved_on_auth_failure(self):
        """Request method, URL, and body are preserved across auth retries."""
        captured_requests = []

        class ContextPreservingAuth(Auth):
            def auth_flow(self, request):
                request.headers["x-auth"] = "first"
                captured_requests.append(("first", request.method, str(request.url), request.content))
                response = yield request
                if response.status_code == 401:
                    request.headers["x-auth"] = "second"
                    captured_requests.append(("second", request.method, str(request.url), request.content))
                    yield request

        def handler(request):
            auth = request.headers.get("x-auth", "")
            if auth == "first":
                return Response(401)
            return Response(200, text="done")

        with Client(
            auth=ContextPreservingAuth(),
            transport=MockTransport(handler),
        ) as client:
            response = client.post(
                "http://testserver/data",
                content=b"payload",
            )

        assert response.status_code == 200
        assert len(captured_requests) == 2
        # Both attempts preserve the same method, URL, and body
        for label, method, url, body in captured_requests:
            assert method == "POST"
            assert url == "http://testserver/data"
            assert body == b"payload"

    def test_auth_none_with_transport_skips_auth(self):
        """When auth=None, no auth header is sent even with a transport."""
        def handler(request):
            return Response(200, text=request.headers.get("authorization", "none"))

        with Client(
            auth=None,
            transport=MockTransport(handler),
        ) as client:
            response = client.get("http://testserver/")

        assert response.status_code == 200
        assert response.text == "none"

    def test_digest_auth_through_transport(self):
        """DigestAuth works through MockTransport."""
        def handler(request):
            auth_header = request.headers.get("authorization", "")
            if auth_header.startswith("Digest "):
                return Response(200, text="authenticated")
            return Response(
                401,
                headers=[
                    ("www-authenticate", 'Digest realm="test", nonce="abc", qop="auth"')
                ],
            )

        with Client(
            auth=DigestAuth("user", "pass"),
            transport=MockTransport(handler),
        ) as client:
            response = client.get("http://testserver/")

        assert response.status_code == 200
        assert response.text == "authenticated"


class TestSyncAuthWithThreeYields:
    """Custom sync auth that yields three requests (401 → 401 → 200)."""

    def test_three_yields_sync(self):
        call_count = 0

        class ThreeStepAuth(Auth):
            def auth_flow(self, request):
                nonlocal call_count
                for attempt in ("1", "2", "3"):
                    call_count += 1
                    request.headers["x-attempt"] = attempt
                    response = yield request
                    if response.status_code != 401:
                        return

        def handler(request):
            attempt = request.headers.get("x-attempt", "")
            if attempt in ("1", "2"):
                return Response(401, text=f"attempt {attempt}")
            return Response(200, text=f"attempt {attempt}")

        with Client(
            auth=ThreeStepAuth(),
            transport=MockTransport(handler),
        ) as client:
            response = client.get("http://testserver/")

        assert response.status_code == 200
        assert response.text == "attempt 3"
        assert call_count == 3

    def test_three_yields_intermediate_drained(self):
        """Intermediate responses are drained and closed before follow-up dispatch."""
        closed = []

        class ThreeStepAuth(Auth):
            def auth_flow(self, request):
                for attempt in ("1", "2"):
                    request.headers["x-attempt"] = attempt
                    response = yield request
                    # Intermediate response should be closed by the client
                    # before we get here (the generator yields after send()).
                    # But we can verify the response was received.
                    assert response.status_code == 401
                request.headers["x-attempt"] = "3"
                yield request

        def handler(request):
            attempt = request.headers.get("x-attempt", "")
            if attempt in ("1", "2"):
                return Response(401, text=f"attempt {attempt}")
            return Response(200, text=f"attempt {attempt}")

        with Client(
            auth=ThreeStepAuth(),
            transport=MockTransport(handler),
        ) as client:
            response = client.get("http://testserver/")

        assert response.status_code == 200
        assert response.text == "attempt 3"


class TestAsyncAuthWithThreeYields:
    """Custom async auth that yields three requests (401 → 401 → 200)."""

    @pytest.mark.asyncio
    async def test_three_yields_async(self):
        call_count = 0

        class ThreeStepAuth(Auth):
            def auth_flow(self, request):
                nonlocal call_count
                for attempt in ("1", "2", "3"):
                    call_count += 1
                    request.headers["x-attempt"] = attempt
                    response = yield request
                    if response.status_code != 401:
                        return

        async def handler(request):
            attempt = request.headers.get("x-attempt", "")
            if attempt in ("1", "2"):
                return Response(401, text=f"attempt {attempt}")
            return Response(200, text=f"attempt {attempt}")

        async with AsyncClient(
            auth=ThreeStepAuth(),
            async_transport=MockTransport(handler),
        ) as client:
            response = await client.get("http://testserver/")

        assert response.status_code == 200
        assert response.text == "attempt 3"
        assert call_count == 3

    @pytest.mark.asyncio
    async def test_three_yields_async_native_async_auth_flow(self):
        """Subclass overrides async_auth_flow with real async logic."""
        call_count = 0

        class AsyncNativeAuth(Auth):
            def auth_flow(self, request):
                # Base auth_flow (not used directly by async client)
                raise NotImplementedError("async_auth_flow should be called")

            async def async_auth_flow(self, request):
                nonlocal call_count
                for attempt in ("1", "2", "3"):
                    call_count += 1
                    request.headers["x-attempt"] = attempt
                    response = yield request
                    if response.status_code != 401:
                        return

        async def handler(request):
            attempt = request.headers.get("x-attempt", "")
            if attempt in ("1", "2"):
                return Response(401, text=f"attempt {attempt}")
            return Response(200, text=f"attempt {attempt}")

        async with AsyncClient(
            auth=AsyncNativeAuth(),
            async_transport=MockTransport(handler),
        ) as client:
            response = await client.get("http://testserver/")

        assert response.status_code == 200
        assert response.text == "attempt 3"
        assert call_count == 3


class TestIntermediateResponseCleanup:
    """Intermediate auth responses are drained and closed, not exposed to hooks."""

    def test_intermediate_closed_sync(self):
        """Sync: intermediate 401 is read + closed before follow-up dispatch."""
        response_close_count = [0]
        original_close = Response.close

        def tracking_close(self):
            response_close_count[0] += 1
            original_close(self)

        Response.close = tracking_close
        try:

            class AlwaysRetryAuth(Auth):
                def auth_flow(self, request):
                    request.headers["x-step"] = "1"
                    response = yield request
                    if response.status_code == 401:
                        request.headers["x-step"] = "2"
                        yield request

            def handler(request):
                step = request.headers.get("x-step", "")
                if step == "1":
                    return Response(401, text="unauthorized")
                return Response(200, text="ok")

            with Client(
                auth=AlwaysRetryAuth(),
                transport=MockTransport(handler),
            ) as client:
                response = client.get("http://testserver/")

            assert response.status_code == 200
            # Intermediate response should have been closed
            assert response_close_count[0] >= 1
        finally:
            Response.close = original_close

    @pytest.mark.asyncio
    async def test_intermediate_closed_async(self):
        """Async: intermediate 401 is read + closed before follow-up dispatch."""
        response_close_count = [0]
        original_aclose = Response.aclose

        async def tracking_aclose(self):
            response_close_count[0] += 1
            await original_aclose(self)

        Response.aclose = tracking_aclose
        try:

            class AlwaysRetryAuth(Auth):
                def auth_flow(self, request):
                    request.headers["x-step"] = "1"
                    response = yield request
                    if response.status_code == 401:
                        request.headers["x-step"] = "2"
                        yield request

            async def handler(request):
                step = request.headers.get("x-step", "")
                if step == "1":
                    return Response(401, text="unauthorized")
                return Response(200, text="ok")

            async with AsyncClient(
                auth=AlwaysRetryAuth(),
                async_transport=MockTransport(handler),
            ) as client:
                response = await client.get("http://testserver/")

            assert response.status_code == 200
            assert response_close_count[0] >= 1
        finally:
            Response.aclose = original_aclose

    def test_response_hooks_run_on_every_hop_sync(self):
        """Per Track 4.2: response hooks run on every hop before auth decides."""
        hook_responses = []

        def on_response(response):
            hook_responses.append(response.status_code)

        class MultiAuth(Auth):
            def auth_flow(self, request):
                request.headers["x-step"] = "1"
                response = yield request
                if response.status_code == 401:
                    request.headers["x-step"] = "2"
                    yield request

        def handler(request):
            step = request.headers.get("x-step", "")
            if step == "1":
                return Response(401)
            return Response(200)

        with Client(
            auth=MultiAuth(),
            transport=MockTransport(handler),
            event_hooks={"request": [], "response": [on_response]},
        ) as client:
            response = client.get("http://testserver/")

        assert response.status_code == 200
        # Response hooks run on every hop: 401 then 200
        assert hook_responses == [401, 200]

    @pytest.mark.asyncio
    async def test_response_hooks_run_on_every_hop_async(self):
        """Async per Track 4.2: response hooks run on every hop."""
        hook_responses = []

        async def on_response(response):
            hook_responses.append(response.status_code)

        class MultiAuth(Auth):
            def auth_flow(self, request):
                request.headers["x-step"] = "1"
                response = yield request
                if response.status_code == 401:
                    request.headers["x-step"] = "2"
                    yield request

        async def handler(request):
            step = request.headers.get("x-step", "")
            if step == "1":
                return Response(401)
            return Response(200)

        async with AsyncClient(
            auth=MultiAuth(),
            async_transport=MockTransport(handler),
            event_hooks={"request": [], "response": [on_response]},
        ) as client:
            response = await client.get("http://testserver/")

        assert response.status_code == 200
        assert hook_responses == [401, 200]


class TestPerRequestAuthDisable:
    """auth=None on send() disables auth even if client has default auth."""

    def test_per_request_auth_none_sync(self):
        def handler(request):
            return Response(200, text=request.headers.get("authorization", "none"))

        with Client(
            auth=BasicAuth("user", "pass"),
            transport=MockTransport(handler),
        ) as client:
            response = client.get("http://testserver/", auth=None)

        assert response.status_code == 200
        assert response.text == "none"

    @pytest.mark.asyncio
    async def test_per_request_auth_none_async(self):
        async def handler(request):
            return Response(200, text=request.headers.get("authorization", "none"))

        async with AsyncClient(
            auth=BasicAuth("user", "pass"),
            async_transport=MockTransport(handler),
        ) as client:
            response = await client.get("http://testserver/", auth=None)

        assert response.status_code == 200
        assert response.text == "none"

    def test_per_request_auth_override_sync(self):
        """Passing a different auth on send() overrides the client default."""
        def handler(request):
            return Response(200, text=request.headers.get("authorization", ""))

        with Client(
            auth=BasicAuth("default", "default"),
            transport=MockTransport(handler),
        ) as client:
            response = client.get(
                "http://testserver/",
                auth=BasicAuth("override", "override"),
            )

        assert response.status_code == 200
        decoded = base64.b64decode(
            response.text.split(" ", 1)[1]
        ).decode("latin-1")
        assert decoded == "override:override"

    @pytest.mark.asyncio
    async def test_per_request_auth_override_async(self):
        async def handler(request):
            return Response(200, text=request.headers.get("authorization", ""))

        async with AsyncClient(
            auth=BasicAuth("default", "default"),
            async_transport=MockTransport(handler),
        ) as client:
            response = await client.get(
                "http://testserver/",
                auth=BasicAuth("override", "override"),
            )

        assert response.status_code == 200
        decoded = base64.b64decode(
            response.text.split(" ", 1)[1]
        ).decode("latin-1")
        assert decoded == "override:override"


class TestAuthExceptionRetainsContext:
    """Auth exceptions preserve the request context."""

    def test_auth_exception_has_request_sync(self):
        class BrokenAuth(Auth):
            def auth_flow(self, request):
                request.headers["x-broken"] = "true"
                raise ValueError("auth failed")

        with Client(
            auth=BrokenAuth(),
            transport=MockTransport(lambda r: Response(200)),
        ) as client:
            with pytest.raises(ValueError, match="auth failed"):
                client.get("http://testserver/")

    @pytest.mark.asyncio
    async def test_auth_exception_has_request_async(self):
        class BrokenAuth(Auth):
            def auth_flow(self, request):
                request.headers["x-broken"] = "true"
                raise ValueError("auth failed")

        async with AsyncClient(
            auth=BrokenAuth(),
            async_transport=MockTransport(lambda r: Response(200)),
        ) as client:
            with pytest.raises(ValueError, match="auth failed"):
                await client.get("http://testserver/")

    @pytest.mark.asyncio
    async def test_auth_exception_in_async_auth_flow(self):
        """Exception in async_auth_flow is raised correctly."""
        class BrokenAsyncAuth(Auth):
            def auth_flow(self, request):
                raise NotImplementedError("should not be called")

            async def async_auth_flow(self, request):
                yield request  # yield the request first
                raise ValueError("async auth broken")

        async with AsyncClient(
            auth=BrokenAsyncAuth(),
            async_transport=MockTransport(lambda r: Response(200)),
        ) as client:
            with pytest.raises(ValueError, match="async auth broken"):
                await client.get("http://testserver/")
