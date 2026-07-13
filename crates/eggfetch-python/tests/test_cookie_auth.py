"""Tests for cookie and auth interaction (Track D)."""

import http.server
import json
import threading
import urllib.parse
import base64

import pytest

import eggfetch


# ---------------------------------------------------------------------------
# Test servers
# ---------------------------------------------------------------------------


class _CookieAuthHandler(http.server.BaseHTTPRequestHandler):
    """Server that sets cookies, requires auth, and reports both."""

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        cookie_header = self.headers.get("Cookie", "")
        auth_header = self.headers.get("Authorization")

        if parsed.path == "/set-cookie-then-echo":
            body = json.dumps({
                "cookie": cookie_header,
                "auth": auth_header,
            }).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Set-Cookie", "session=abc123; Path=/")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif parsed.path == "/echo":
            body = json.dumps({
                "cookie": cookie_header,
                "auth": auth_header,
            }).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif parsed.path == "/set-cookie-redirect":
            self.send_response(302)
            self.send_header("Location", "/echo")
            self.send_header("Set-Cookie", "redirect_session=xyz; Path=/")
            self.send_header("Content-Length", "0")
            self.end_headers()

        elif parsed.path == "/set-cookie-redirect-to-other-origin":
            port = self.server.server_address[1]
            self.send_response(302)
            self.send_header("Location", f"http://127.0.0.1:{port + 1}/final")
            self.send_header("Set-Cookie", "from_origin_a=aaa; Path=/")
            self.send_header("Content-Length", "0")
            self.end_headers()

        elif parsed.path == "/final":
            body = json.dumps({
                "cookie": cookie_header,
                "auth": auth_header,
            }).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        else:
            self.send_error(404)

    def log_message(self, format, *args):
        pass


class _EchoHandler(http.server.BaseHTTPRequestHandler):
    """Simple echo server for cross-origin redirect target."""

    def do_GET(self):
        cookie_header = self.headers.get("Cookie", "")
        auth_header = self.headers.get("Authorization")
        body = json.dumps({
            "cookie": cookie_header,
            "auth": auth_header,
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        pass


def _start_server(handler_class):
    """Start a test server on a random port and return (server, base_url)."""
    server = http.server.HTTPServer(("127.0.0.1", 0), handler_class)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, f"http://127.0.0.1:{port}"


# ---------------------------------------------------------------------------
# Track D: Cookie + Auth interaction
# ---------------------------------------------------------------------------


class TestBasicAuthSessionCookie:
    """Test 13: Server sets a session cookie and requires Basic auth."""

    def test_auth_and_cookie_sent_together(self):
        server, base = _start_server(_CookieAuthHandler)
        try:
            with eggfetch.Client(
                auth=eggfetch.BasicAuth("user", "pass")
            ) as client:
                # First request: server sets session cookie, we send auth.
                resp = client.get(f"{base}/set-cookie-then-echo")
                data = resp.json()
                assert data["auth"] == "Basic dXNlcjpwYXNz"

                # Second request: client jar has cookie, client has auth.
                resp2 = client.get(f"{base}/echo")
                data2 = resp2.json()
                assert "session=abc123" in data2["cookie"]
                assert data2["auth"] == "Basic dXNlcjpwYXNz"
        finally:
            server.shutdown()


class TestRedirectPreservesBoth:
    """Test 14: Redirect within same origin preserves both cookie and auth."""

    def test_redirect_preserves_cookie_and_auth(self):
        server, base = _start_server(_CookieAuthHandler)
        try:
            resp = eggfetch.get(
                f"{base}/set-cookie-redirect",
                auth=eggfetch.BearerAuth("my-token"),
                follow_redirects=True,
            )
            data = resp.json()
            # Cookie set on the redirect response should be sent on the final.
            assert "redirect_session=xyz" in data["cookie"]
            assert data["auth"] == "Bearer my-token"
        finally:
            server.shutdown()


class TestCrossOriginRedirectStripsBoth:
    """Test 15: Cross-origin redirect behavior for cookie and auth.

    The Rust unit test ``build_redirect_cross_origin_strips_auth`` verifies
    that ``build_redirect_request`` strips Authorization, Cookie, and
    Proxy-Authorization headers on cross-origin redirects.

    At the Python integration level, client-level auth is suppressed on
    cross-origin redirect hops.  Cookie headers are stripped but the
    cookie jar re-injects cookies for the target domain.  Since all local
    test servers share the same IP (127.0.0.1), the jar re-sends cookies.

    This test verifies that:
    1. The redirect completes successfully to a different origin.
    2. No Authorization header arrives at the second server.
    """

    def test_cross_origin_redirect_completes_and_strips_auth(self):
        server_b, base_b = _start_server(_EchoHandler)
        port_b = server_b.server_address[1]

        class _RedirectToOriginBHandler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                parsed = urllib.parse.urlparse(self.path)
                if parsed.path == "/redirect-to-b":
                    self.send_response(302)
                    self.send_header(
                        "Location",
                        f"http://127.0.0.1:{port_b}/final"
                    )
                    self.send_header("Content-Length", "0")
                    self.end_headers()
                else:
                    self.send_error(404)

            def log_message(self, format, *args):
                pass

        srv_a = http.server.HTTPServer(
            ("127.0.0.1", 0), _RedirectToOriginBHandler
        )
        base_a = f"http://127.0.0.1:{srv_a.server_address[1]}"
        t = threading.Thread(target=srv_a.serve_forever, daemon=True)
        t.start()

        try:
            resp = eggfetch.get(
                f"{base_a}/redirect-to-b",
                auth=eggfetch.BearerAuth("secret-token"),
                follow_redirects=True,
            )
            data = resp.json()
            # Redirect completed successfully to origin B.
            # Auth must NOT have been forwarded cross-origin.
            assert data["auth"] is None
        finally:
            srv_a.shutdown()
            server_b.shutdown()

    def test_client_sensitive_defaults_and_request_cookies_are_not_reintroduced(self):
        server_b, base_b = _start_server(_EchoHandler)
        port_b = server_b.server_address[1]

        class _RedirectToOriginBHandler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                self.send_response(302)
                self.send_header(
                    "Location",
                    f"http://127.0.0.1:{port_b}/final",
                )
                self.send_header("Content-Length", "0")
                self.end_headers()

            def log_message(self, format, *args):
                pass

        srv_a = http.server.HTTPServer(("127.0.0.1", 0), _RedirectToOriginBHandler)
        base_a = f"http://127.0.0.1:{srv_a.server_address[1]}"
        thread = threading.Thread(target=srv_a.serve_forever, daemon=True)
        thread.start()

        try:
            with eggfetch.Client(
                headers={
                    "Authorization": "Bearer default-secret",
                    "Cookie": "raw-secret=yes",
                },
                follow_redirects=True,
            ) as client:
                response = client.get(
                    f"{base_a}/redirect-to-b",
                    cookies={"request-secret": "yes"},
                )
            data = response.json()
            assert data["auth"] is None
            assert "raw-secret" not in data["cookie"]
            assert "request-secret" not in data["cookie"]
        finally:
            srv_a.shutdown()
            server_b.shutdown()


class TestSameOriginRedirectPreservesBoth:
    """Test 16: Same-origin redirect preserves both cookie and auth."""

    def test_same_origin_redirect_preserves_both(self):
        server, base = _start_server(_CookieAuthHandler)
        try:
            resp = eggfetch.get(
                f"{base}/set-cookie-redirect",
                auth=eggfetch.BearerAuth("same-origin-token"),
                follow_redirects=True,
            )
            data = resp.json()
            # Both cookie and auth should be present on the final request.
            assert "redirect_session=xyz" in data["cookie"]
            assert data["auth"] == "Bearer same-origin-token"
        finally:
            server.shutdown()


class TestAuthDisableDoesNotDisableCookies:
    """Disabling auth must not disable cookie handling."""

    def test_noauth_still_sends_cookies(self):
        server, base = _start_server(_CookieAuthHandler)
        try:
            with eggfetch.Client(
                auth=eggfetch.NOAUTH
            ) as client:
                resp = client.get(f"{base}/set-cookie-then-echo")
                data = resp.json()
                assert data["auth"] is None
                resp2 = client.get(f"{base}/echo")
                data2 = resp2.json()
                assert "session=abc123" in data2["cookie"]
        finally:
            server.shutdown()
