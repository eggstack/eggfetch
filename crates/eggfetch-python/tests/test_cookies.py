"""Tests for the eggfetch Python cookie API."""

import http.server
import json
import threading
import time
import urllib.parse

import pytest

import eggfetch


# ---------------------------------------------------------------------------
# Local test server
# ---------------------------------------------------------------------------

class _CookieHandler(http.server.BaseHTTPRequestHandler):
    """Test server that echoes cookies and sets cookies."""

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        cookie_header = self.headers.get("Cookie", "")
        body = json.dumps({
            "path": parsed.path,
            "cookie": cookie_header,
            "headers": dict(self.headers),
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        raw = self.rfile.read(length) if length else b""
        cookie_header = self.headers.get("Cookie", "")
        body = json.dumps({
            "method": "POST",
            "path": self.path,
            "cookie": cookie_header,
            "headers": dict(self.headers),
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_HEAD(self):
        self.send_response(200)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header("Allow", "GET, POST, HEAD, OPTIONS")
        self.send_header("Content-Length", "0")
        self.end_headers()

    def log_message(self, format, *args):
        pass  # suppress logs


class _SetCookieHandler(http.server.BaseHTTPRequestHandler):
    """Test server that sends Set-Cookie headers."""

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        cookie_header = self.headers.get("Cookie", "")
        if parsed.path == "/set-single":
            body = json.dumps({"cookie": cookie_header}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Set-Cookie", "session_id=abc123; Path=/")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif parsed.path == "/set-multiple":
            body = json.dumps({"cookie": cookie_header}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Set-Cookie", "session_id=abc123; Path=/")
            self.send_header("Set-Cookie", "user=john; Path=/")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif parsed.path == "/set-domain":
            body = json.dumps({"cookie": cookie_header}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Set-Cookie", "tracked=xyz; Domain=localhost; Path=/")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif parsed.path == "/set-secure":
            body = json.dumps({"cookie": cookie_header}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Set-Cookie", "secure_token=secret; Secure; Path=/")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif parsed.path == "/set-httponly":
            body = json.dumps({"cookie": cookie_header}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Set-Cookie", "auth_token=topsecret; HttpOnly; Path=/")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif parsed.path == "/set-samesite":
            body = json.dumps({"cookie": cookie_header}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Set-Cookie", "ss=strict; SameSite=Strict; Path=/")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif parsed.path == "/set-expired":
            body = json.dumps({"cookie": cookie_header}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Set-Cookie", "old=cookie; Max-Age=0; Path=/")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif parsed.path == "/get":
            body = json.dumps({"cookie": cookie_header}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_error(404)

    def log_message(self, format, *args):
        pass


class _RedirectCookieHandler(http.server.BaseHTTPRequestHandler):
    """Test server that sets cookies during redirects."""

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        cookie_header = self.headers.get("Cookie", "")
        if parsed.path == "/redirect-to-set":
            body = b""
            self.send_response(302)
            self.send_header("Location", "/final")
            self.send_header("Set-Cookie", "redirect_cookie=from_redirect; Path=/")
            self.send_header("Content-Length", "0")
            self.end_headers()
        elif parsed.path == "/final":
            body = json.dumps({
                "path": parsed.path,
                "cookie": cookie_header,
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


def _start_server(handler_class):
    """Start a test server on a random port and return (server, base_url)."""
    server = http.server.HTTPServer(("127.0.0.1", 0), handler_class)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, f"http://127.0.0.1:{port}"


# ---------------------------------------------------------------------------
# Cookie object tests
# ---------------------------------------------------------------------------

class TestCookieObject:
    """Tests for Cookie object properties."""

    def test_cookie_from_response(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-single")
            assert resp.status_code == 200
            cookies = resp.cookies
            assert len(cookies) == 1
            cookie = cookies["session_id"]
            assert cookie.name == "session_id"
            assert cookie.value == "abc123"
            assert cookie.path == "/"
        finally:
            server.shutdown()

    def test_cookie_repr(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-single")
            cookie = resp.cookies["session_id"]
            assert "session_id" in repr(cookie)
        finally:
            server.shutdown()

    def test_cookie_str(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-single")
            cookie = resp.cookies["session_id"]
            assert str(cookie) == "session_id=abc123"
        finally:
            server.shutdown()

    def test_cookie_host_only_domain(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-single")
            cookie = resp.cookies["session_id"]
            assert cookie.domain == "127.0.0.1"
            assert cookie.is_host_only is True
        finally:
            server.shutdown()

    def test_cookie_secure_attribute(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-secure")
            cookie = resp.cookies["secure_token"]
            assert cookie.is_secure is True
        finally:
            server.shutdown()

    def test_cookie_httponly_attribute(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-httponly")
            cookie = resp.cookies["auth_token"]
            assert cookie.is_http_only is True
        finally:
            server.shutdown()

    def test_cookie_samesite_attribute(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-samesite")
            cookie = resp.cookies["ss"]
            assert cookie.same_site == "Strict"
        finally:
            server.shutdown()


# ---------------------------------------------------------------------------
# Cookies mapping tests
# ---------------------------------------------------------------------------

class TestCookiesMapping:
    """Tests for Cookies mapping protocol."""

    def test_len(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-multiple")
            assert len(resp.cookies) == 2
        finally:
            server.shutdown()

    def test_contains(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-multiple")
            assert "session_id" in resp.cookies
            assert "user" in resp.cookies
            assert "nonexistent" not in resp.cookies
        finally:
            server.shutdown()

    def test_getitem(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-multiple")
            assert resp.cookies["session_id"].value == "abc123"
            assert resp.cookies["user"].value == "john"
        finally:
            server.shutdown()

    def test_getitem_keyerror(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-single")
            with pytest.raises(KeyError):
                _ = resp.cookies["nonexistent"]
        finally:
            server.shutdown()

    def test_get_method(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-multiple")
            c = resp.cookies.get("session_id")
            assert c is not None
            assert c.value == "abc123"
            assert resp.cookies.get("nonexistent") is None
            assert resp.cookies.get("nonexistent", "default") == "default"
        finally:
            server.shutdown()

    def test_iter(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-multiple")
            names = list(resp.cookies)
            assert "session_id" in names
            assert "user" in names
        finally:
            server.shutdown()

    def test_values(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-multiple")
            vals = resp.cookies.values()
            values = [c.value for c in vals]
            assert "abc123" in values
            assert "john" in values
        finally:
            server.shutdown()

    def test_items(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-multiple")
            items = resp.cookies.items()
            d = {name: c.value for name, c in items}
            assert d["session_id"] == "abc123"
            assert d["user"] == "john"
        finally:
            server.shutdown()

    def test_repr(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-multiple")
            assert "Cookies(2)" == repr(resp.cookies)
        finally:
            server.shutdown()


# ---------------------------------------------------------------------------
# Client.cookies tests
# ---------------------------------------------------------------------------

class TestClientCookies:
    """Tests for client-level cookie jar."""

    def test_client_cookies_empty_initially(self):
        with eggfetch.Client() as client:
            assert len(client.cookies) == 0

    def test_client_cookies_update_from_response(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            with eggfetch.Client() as client:
                resp = client.get(f"{base}/set-single")
                assert resp.status_code == 200
                assert len(client.cookies) == 1
                assert client.cookies["session_id"].value == "abc123"
        finally:
            server.shutdown()

    def test_client_cookies_persist_across_requests(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            with eggfetch.Client() as client:
                client.get(f"{base}/set-single")
                resp2 = client.get(f"{base}/get")
                assert "session_id=abc123" in resp2.json()["cookie"]
        finally:
            server.shutdown()

    def test_client_cookies_initial_dict(self):
        with eggfetch.Client(cookies={"foo": "bar", "baz": "qux"}) as client:
            assert len(client.cookies) == 2
            assert client.cookies["foo"].value == "bar"
            assert client.cookies["baz"].value == "qux"

    def test_cookies_kwarg_on_get(self):
        server, base = _start_server(_CookieHandler)
        try:
            resp = eggfetch.get(f"{base}/get", cookies={"session_id": "from_kwarg"})
            assert "session_id=from_kwarg" in resp.json()["cookie"]
        finally:
            server.shutdown()

    def test_cookies_kwarg_on_client_get(self):
        server, base = _start_server(_CookieHandler)
        try:
            with eggfetch.Client(cookies={"session_id": "initial"}) as client:
                resp = client.get(f"{base}/get")
                assert "session_id=initial" in resp.json()["cookie"]
        finally:
            server.shutdown()


# ---------------------------------------------------------------------------
# Redirect cookie tests
# ---------------------------------------------------------------------------

class TestRedirectCookies:
    """Tests for cookie handling during redirects."""

    def test_set_cookie_on_redirect_hops(self):
        server, base = _start_server(_RedirectCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/redirect-to-set", follow_redirects=True)
            assert resp.status_code == 200
            assert resp.json()["cookie"] == "redirect_cookie=from_redirect"
        finally:
            server.shutdown()


# ---------------------------------------------------------------------------
# Response.cookies tests
# ---------------------------------------------------------------------------

class TestResponseCookies:
    """Tests for response-level cookies."""

    def test_response_cookies_from_headers(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-single")
            assert len(resp.cookies) == 1
            assert resp.cookies["session_id"].value == "abc123"
        finally:
            server.shutdown()

    def test_response_cookies_multiple(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            resp = eggfetch.get(f"{base}/set-multiple")
            assert len(resp.cookies) == 2
            vals = {c.name: c.value for c in resp.cookies.values()}
            assert vals["session_id"] == "abc123"
            assert vals["user"] == "john"
        finally:
            server.shutdown()

    def test_response_cookies_empty(self):
        server, base = _start_server(_CookieHandler)
        try:
            resp = eggfetch.get(f"{base}/get")
            assert len(resp.cookies) == 0
        finally:
            server.shutdown()

    def test_response_cookies_not_shared_with_client_jar(self):
        server, base = _start_server(_SetCookieHandler)
        try:
            with eggfetch.Client() as client:
                resp = client.get(f"{base}/set-single")
                assert len(resp.cookies) == 1
                assert len(client.cookies) == 1
                # Response cookies and client cookies should have the same cookie
                assert resp.cookies["session_id"].value == client.cookies["session_id"].value
        finally:
            server.shutdown()


# ---------------------------------------------------------------------------
# Cookie audit tests (Track B)
# ---------------------------------------------------------------------------


class TestCookieAudit:
    """Tests for cookie audit behaviors from the corrective-pass plan."""

    def test_raw_cookie_header_not_sent_cross_origin(self):
        """Host-only cookies set for one port are not sent to another host."""
        server, base = _start_server(_SetCookieHandler)
        try:
            with eggfetch.Client() as client:
                resp = client.get(f"{base}/set-single")
                cookie = resp.cookies["session_id"]
                assert cookie.is_host_only is True
                assert len(client.cookies) == 1
        finally:
            server.shutdown()

    def test_request_kwarg_cookies_not_persistent(self):
        """cookies= kwarg must not persist to the client jar."""
        server, base = _start_server(_SetCookieHandler)
        try:
            with eggfetch.Client() as client:
                resp = client.get(f"{base}/get", cookies={"ephemeral": "yes"})
                assert "ephemeral=yes" in resp.json()["cookie"]
                assert "ephemeral" not in client.cookies
        finally:
            server.shutdown()

    def test_request_kwarg_cookies_are_available_on_post_style_methods(self):
        server, base = _start_server(_CookieHandler)
        try:
            with eggfetch.Client() as client:
                resp = client.post(f"{base}/get", cookies={"ephemeral": "yes"})
                assert "ephemeral=yes" in resp.json()["cookie"]
                assert "ephemeral" not in client.cookies
        finally:
            server.shutdown()

    def test_client_cookies_sent_cross_origin(self):
        """Client-level cookies are sent to all matching origins."""
        srv1, base1 = _start_server(_SetCookieHandler)
        srv2, base2 = _start_server(_CookieHandler)
        try:
            with eggfetch.Client() as client:
                client.get(f"{base1}/set-single")
                assert "session_id" in client.cookies
                resp2 = client.get(f"{base2}/get")
                data = resp2.json()
                assert "session_id=abc123" in data["cookie"]
        finally:
            srv1.shutdown()
            srv2.shutdown()
