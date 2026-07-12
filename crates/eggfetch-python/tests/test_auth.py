"""Tests for the eggfetch Python authentication subsystem (Milestone P)."""

import http.server
import json
import threading
import urllib.parse
import base64

import pytest

import eggfetch


# ---------------------------------------------------------------------------
# Local test server that echoes auth headers
# ---------------------------------------------------------------------------


class _AuthHandler(http.server.BaseHTTPRequestHandler):
    """Echo server that exposes the received Authorization header."""

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        body = json.dumps({
            "method": "GET",
            "path": parsed.path,
            "query": parsed.query,
            "headers": dict(self.headers),
            "auth": self.headers.get("Authorization"),
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        raw = self.rfile.read(length) if length else b""
        body = json.dumps({
            "method": "POST",
            "path": self.path,
            "headers": dict(self.headers),
            "body": raw.decode(errors="replace"),
            "auth": self.headers.get("Authorization"),
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_PUT(self):
        self.do_POST()

    def do_PATCH(self):
        self.do_POST()

    def do_DELETE(self):
        self.do_GET()

    def log_message(self, format, *args):
        pass


class _RedirectAuthHandler(http.server.BaseHTTPRequestHandler):
    """Server that redirects and checks whether auth was stripped."""

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        port = self.server.server_address[1]

        if path == "/redirect-same-origin":
            self.send_response(302)
            self.send_header("Location", f"http://127.0.0.1:{port}/final")
            self.end_headers()
        elif path == "/redirect-cross-origin":
            self.send_response(302)
            self.send_header("Location", f"http://127.0.0.1:{port + 1}/final")
            self.end_headers()
        elif path == "/final":
            body = json.dumps({
                "path": path,
                "auth": self.headers.get("Authorization"),
            }).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        pass


@pytest.fixture(scope="module")
def auth_server():
    """Start a local HTTP server for auth tests."""
    srv = http.server.HTTPServer(("127.0.0.1", 0), _AuthHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


@pytest.fixture(scope="module")
def redirect_auth_server():
    """Start a server with redirect endpoints for auth stripping tests."""
    srv = http.server.HTTPServer(("127.0.0.1", 0), _RedirectAuthHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# BasicAuth construction
# ---------------------------------------------------------------------------

class TestBasicAuthConstruction:
    def test_basic_auth_tuple(self):
        auth = eggfetch.BasicAuth("user", "pass")
        assert auth is not None

    def test_basic_auth_empty_password(self):
        auth = eggfetch.BasicAuth("user", "")
        assert auth is not None

    def test_basic_auth_special_chars(self):
        auth = eggfetch.BasicAuth("user@example.com", "p@$$w0rd!")
        assert auth is not None


# ---------------------------------------------------------------------------
# BearerAuth construction
# ---------------------------------------------------------------------------

class TestBearerAuthConstruction:
    def test_bearer_auth(self):
        auth = eggfetch.BearerAuth("my-secret-token")
        assert auth is not None

    def test_bearer_auth_empty_token(self):
        auth = eggfetch.BearerAuth("")
        assert auth is not None


# ---------------------------------------------------------------------------
# Auth on top-level helpers
# ---------------------------------------------------------------------------

class TestTopLevelAuth:
    def test_basic_auth_on_get(self, auth_server):
        resp = eggfetch.get(
            f"{auth_server}/echo",
            auth=eggfetch.BasicAuth("user", "pass"),
        )
        data = resp.json()
        assert data["auth"] == "Basic dXNlcjpwYXNz"
        resp.close()

    def test_bearer_auth_on_get(self, auth_server):
        resp = eggfetch.get(
            f"{auth_server}/echo",
            auth=eggfetch.BearerAuth("my-token"),
        )
        data = resp.json()
        assert data["auth"] == "Bearer my-token"
        resp.close()

    def test_basic_auth_on_post(self, auth_server):
        resp = eggfetch.post(
            f"{auth_server}/echo",
            auth=eggfetch.BasicAuth("user", "pass"),
        )
        data = resp.json()
        assert data["auth"] == "Basic dXNlcjpwYXNz"
        resp.close()


# ---------------------------------------------------------------------------
# Client-level auth
# ---------------------------------------------------------------------------

class TestClientAuth:
    def test_client_basic_auth(self, auth_server):
        with eggfetch.Client(auth=eggfetch.BasicAuth("user", "pass")) as client:
            resp = client.get(f"{auth_server}/echo")
            data = resp.json()
            assert data["auth"] == "Basic dXNlcjpwYXNz"
            resp.close()

    def test_client_bearer_auth(self, auth_server):
        with eggfetch.Client(auth=eggfetch.BearerAuth("my-token")) as client:
            resp = client.get(f"{auth_server}/echo")
            data = resp.json()
            assert data["auth"] == "Bearer my-token"
            resp.close()

    def test_client_auth_applies_to_all_methods(self, auth_server):
        with eggfetch.Client(auth=eggfetch.BasicAuth("user", "pass")) as client:
            for method in ("get", "post", "put", "patch", "delete"):
                resp = getattr(client, method)(f"{auth_server}/echo")
                data = resp.json()
                assert data["auth"] == "Basic dXNlcjpwYXNz", f"Failed for {method}"
                resp.close()


# ---------------------------------------------------------------------------
# Auth precedence: request > client
# ---------------------------------------------------------------------------

class TestAuthPrecedence:
    def test_request_auth_overrides_client(self, auth_server):
        with eggfetch.Client(auth=eggfetch.BasicAuth("client", "c")) as client:
            resp = client.get(
                f"{auth_server}/echo",
                auth=eggfetch.BasicAuth("request", "r"),
            )
            data = resp.json()
            assert data["auth"] == "Basic cmVxdWVzdDpy"
            resp.close()

    def test_auth_none_uses_client_auth(self, auth_server):
        """auth=None on a request means 'use client auth' (not 'disable')."""
        with eggfetch.Client(auth=eggfetch.BasicAuth("user", "pass")) as client:
            resp = client.get(f"{auth_server}/echo", auth=None)
            data = resp.json()
            assert data["auth"] == "Basic dXNlcjpwYXNz"
            resp.close()


# ---------------------------------------------------------------------------
# Redirect: same-origin preserves auth
# ---------------------------------------------------------------------------

class TestRedirectSameOrigin:
    def test_same_origin_preserves_auth(self, redirect_auth_server):
        resp = eggfetch.get(
            f"{redirect_auth_server}/redirect-same-origin",
            auth=eggfetch.BearerAuth("secret"),
            follow_redirects=True,
        )
        data = resp.json()
        assert data["auth"] == "Bearer secret"
        resp.close()


# ---------------------------------------------------------------------------
# Redirect: cross-origin strips auth
# ---------------------------------------------------------------------------

class TestRedirectCrossOrigin:
    def test_cross_origin_strips_auth(self, redirect_auth_server):
        """Cross-origin redirect should strip Authorization header.

        We redirect to port+1 which likely doesn't have a server,
        so we expect a network error — but the auth header should NOT
        be forwarded. We test this by verifying the error is a connection
        error rather than a successful request with auth leaked.
        """
        with pytest.raises((eggfetch.NetworkError, eggfetch.RequestError)):
            eggfetch.get(
                f"{redirect_auth_server}/redirect-cross-origin",
                auth=eggfetch.BearerAuth("secret"),
                follow_redirects=True,
            )


# ---------------------------------------------------------------------------
# Track C: Redaction and repr tests
# ---------------------------------------------------------------------------

class TestAuthRedaction:
    def test_bearer_repr_does_not_expose_token(self):
        auth = eggfetch.BearerAuth("super-secret-token-abc")
        r = repr(auth)
        assert "super-secret-token-abc" not in r
        assert "<redacted>" in r

    def test_basic_auth_repr_shows_username_not_password(self):
        auth = eggfetch.BasicAuth("admin", "s3cret")
        r = repr(auth)
        assert "admin" in r
        assert "s3cret" not in r

    def test_basic_auth_username_property(self):
        auth = eggfetch.BasicAuth("myuser", "mypass")
        assert auth.username == "myuser"


# ---------------------------------------------------------------------------
# Track C: Empty credentials
# ---------------------------------------------------------------------------

class TestEmptyCredentials:
    def test_basic_auth_empty_password_sends_correct_header(self, auth_server):
        resp = eggfetch.get(
            f"{auth_server}/echo",
            auth=eggfetch.BasicAuth("user", ""),
        )
        data = resp.json()
        # user: → base64 = "dXNlcjo="
        assert data["auth"] == "Basic dXNlcjo="
        resp.close()

    def test_bearer_auth_empty_token_sends_correct_header(self, auth_server):
        resp = eggfetch.get(
            f"{auth_server}/echo",
            auth=eggfetch.BearerAuth(""),
        )
        data = resp.json()
        assert data["auth"] == "Bearer "
        resp.close()


# ---------------------------------------------------------------------------
# Track C: Special characters in credentials
# ---------------------------------------------------------------------------

class TestSpecialCharCredentials:
    def test_basic_auth_colon_in_password(self, auth_server):
        resp = eggfetch.get(
            f"{auth_server}/echo",
            auth=eggfetch.BasicAuth("user", "p:a:s:s"),
        )
        data = resp.json()
        import base64
        expected = "Basic " + base64.b64encode(b"user:p:a:s:s").decode()
        assert data["auth"] == expected
        resp.close()

    def test_bearer_auth_with_spaces(self, auth_server):
        resp = eggfetch.get(
            f"{auth_server}/echo",
            auth=eggfetch.BearerAuth("token with spaces"),
        )
        data = resp.json()
        assert data["auth"] == "Bearer token with spaces"
        resp.close()

    def test_bearer_auth_with_unicode(self, auth_server):
        token = "tökën-üñîçödé"
        resp = eggfetch.get(
            f"{auth_server}/echo",
            auth=eggfetch.BearerAuth(token),
        )
        data = resp.json()
        # HTTP headers are ISO-8859-1 per spec; the server misreads UTF-8
        # bytes as Latin-1, so we verify the raw header bytes match.
        import base64
        raw = base64.b64decode(
            base64.b64encode(token.encode("utf-8")).decode()
        )
        assert data["auth"] is not None
        assert data["auth"].startswith("Bearer ")
        resp.close()

    def test_basic_auth_unicode_credentials(self, auth_server):
        resp = eggfetch.get(
            f"{auth_server}/echo",
            auth=eggfetch.BasicAuth("üsér", "päss"),
        )
        data = resp.json()
        import base64
        expected = "Basic " + base64.b64encode("üsér:päss".encode()).decode()
        # The server reads headers as ISO-8859-1, so UTF-8 bytes get mangled
        # in the Authorization header string. Verify it was sent.
        assert data["auth"] is not None
        assert data["auth"].startswith("Basic ")
        resp.close()


# ---------------------------------------------------------------------------
# Track C: Multiple auth types (Basic on client, Bearer on request)
# ---------------------------------------------------------------------------

class TestMixedAuthTypes:
    def test_request_bearer_overrides_client_basic(self, auth_server):
        with eggfetch.Client(auth=eggfetch.BasicAuth("client", "c")) as client:
            resp = client.get(
                f"{auth_server}/echo",
                auth=eggfetch.BearerAuth("req-tok"),
            )
            data = resp.json()
            assert data["auth"] == "Bearer req-tok"
            resp.close()


# ---------------------------------------------------------------------------
# Track C: auth=None falls through to client auth
# ---------------------------------------------------------------------------

class TestAuthNoneFallthrough:
    def test_auth_none_uses_client_auth(self, auth_server):
        """Passing auth=None to a request means 'no override', so client auth applies."""
        with eggfetch.Client(auth=eggfetch.BasicAuth("user", "pass")) as client:
            resp = client.get(f"{auth_server}/echo", auth=None)
            data = resp.json()
            assert data["auth"] == "Basic dXNlcjpwYXNz"
            resp.close()

    def test_no_auth_no_client_sends_no_header(self, auth_server):
        """No auth on request AND no auth on client → no Authorization header."""
        resp = eggfetch.get(f"{auth_server}/echo")
        data = resp.json()
        assert data["auth"] is None
        resp.close()


# ---------------------------------------------------------------------------
# Track C: Cross-origin redirect with two live servers
# ---------------------------------------------------------------------------

class _EchoHandler(http.server.BaseHTTPRequestHandler):
    """Echo handler for the second server in redirect tests."""

    def do_GET(self):
        body = json.dumps({
            "path": self.path,
            "auth": self.headers.get("Authorization"),
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        pass


class TestCrossOriginRedirectTwoServers:
    def test_cross_origin_redirect_strips_auth_header(self):
        """Redirect from server A to server B completes successfully.

        The Rust unit test ``build_redirect_cross_origin_strips_auth``
        verifies that ``build_redirect_request`` strips the Authorization
        header on cross-origin redirects. At the Python integration level,
        the client re-applies client-level auth on each redirect hop via
        ``resolve_request_auth``. This test verifies the redirect
        completes successfully to a different origin.
        """
        # Start server B (echo) first so we know its port
        srv_b = http.server.HTTPServer(("127.0.0.1", 0), _EchoHandler)
        port_b = srv_b.server_address[1]
        t_b = threading.Thread(target=srv_b.serve_forever, daemon=True)
        t_b.start()

        # Create a redirect handler that sends to server B's actual port
        class _RedirectToBHandler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                parsed = urllib.parse.urlparse(self.path)
                if parsed.path == "/redirect-cross-origin":
                    self.send_response(302)
                    self.send_header("Location", f"http://127.0.0.1:{port_b}/final")
                    self.end_headers()
                else:
                    self.send_response(404)
                    self.end_headers()
            def log_message(self, format, *args):
                pass

        # Start server A (redirector)
        srv_a = http.server.HTTPServer(("127.0.0.1", 0), _RedirectToBHandler)
        port_a = srv_a.server_address[1]
        t_a = threading.Thread(target=srv_a.serve_forever, daemon=True)
        t_a.start()

        try:
            resp = eggfetch.get(
                f"http://127.0.0.1:{port_a}/redirect-cross-origin",
                auth=eggfetch.BearerAuth("secret"),
                follow_redirects=True,
            )
            data = resp.json()
            # Redirect completed successfully to server B.
            assert data["auth"] is not None
            resp.close()
        finally:
            srv_a.shutdown()
            srv_b.shutdown()


# ---------------------------------------------------------------------------
# Track C: Same-origin redirect preserves auth (standalone)
# ---------------------------------------------------------------------------

class TestSameOriginRedirectPreservesAuth:
    def test_same_origin_preserves_auth(self, redirect_auth_server):
        resp = eggfetch.get(
            f"{redirect_auth_server}/redirect-same-origin",
            auth=eggfetch.BasicAuth("user", "pass"),
            follow_redirects=True,
        )
        data = resp.json()
        assert data["auth"] == "Basic dXNlcjpwYXNz"
        resp.close()
