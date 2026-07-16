"""Differential compatibility tests comparing eggfetch against requests/HTTPX.

These tests verify that eggfetch behaves identically to requests/HTTPX for
shared features, and documents intentional differences. Tests are skipped if
the comparison library is not installed.
"""

import http.server
import threading
import urllib.parse

import pytest

import eggfetch

try:
    import requests as _requests

    HAS_REQUESTS = True
except ImportError:
    HAS_REQUESTS = False

try:
    import httpx as _httpx

    HAS_HTTPX = True
except ImportError:
    HAS_HTTPX = False


# ---------------------------------------------------------------------------
# Test server
# ---------------------------------------------------------------------------


class _DifferentialHandler(http.server.BaseHTTPRequestHandler):
    """Test server for differential compatibility tests."""

    def _send_json(self, data: dict, status: int = 200):
        import json

        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_text(self, text: str, status: int = 200):
        body = text.encode()
        self.send_response(status)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        port = self.server.server_address[1]
        base = f"http://127.0.0.1:{port}"

        if path == "/get":
            self._send_json({"method": "GET", "path": path})

        elif path == "/redirect/302":
            self.send_response(302)
            self.send_header("Location", f"{base}/get")
            self.end_headers()

        elif path == "/redirect/301":
            self.send_response(301)
            self.send_header("Location", f"{base}/get")
            self.end_headers()

        elif path == "/redirect/307":
            self.send_response(307)
            self.send_header("Location", f"{base}/get")
            self.end_headers()

        elif path == "/redirect/308":
            self.send_response(308)
            self.send_header("Location", f"{base}/get")
            self.end_headers()

        elif path == "/headers":
            headers = {k: v for k, v in self.headers.items()}
            self._send_json({"headers": headers})

        elif path == "/status/200":
            self._send_text("ok")

        elif path == "/status/404":
            self._send_text("not found", 404)

        elif path == "/status/500":
            self._send_text("server error", 500)

        elif path == "/basic-auth":
            import base64

            auth_header = self.headers.get("Authorization", "")
            if auth_header.startswith("Basic "):
                decoded = base64.b64decode(auth_header[6:]).decode()
                user, password = decoded.split(":", 1)
                self._send_json({"authenticated": True, "user": user})
            else:
                self.send_response(401)
                self.send_header("WWW-Authenticate", 'Basic realm="test"')
                self.end_headers()

        elif path == "/bearer-auth":
            auth_header = self.headers.get("Authorization", "")
            if auth_header.startswith("Bearer "):
                token = auth_header[7:]
                self._send_json({"authenticated": True, "token": token})
            else:
                self.send_response(401)
                self.end_headers()

        elif path == "/json":
            self._send_json({"key": "value", "number": 42})

        elif path == "/echo":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            self.send_response(200)
            self.send_header("Content-Type", self.headers.get("Content-Type", "application/octet-stream"))
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path

        if path == "/post":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            self._send_json({"method": "POST", "body": body.decode(errors="replace")})

        elif path == "/json":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            import json

            data = json.loads(body)
            self._send_json({"received": data})

        elif path == "/form":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            import urllib.parse as up

            data = up.parse_qs(body.decode())
            self._send_json({"received": {k: v[0] if len(v) == 1 else v for k, v in data.items()}})

        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        pass  # Suppress request logging


@pytest.fixture(scope="module")
def diff_server():
    """Start a test server for differential tests."""
    server = http.server.HTTPServer(("127.0.0.1", 0), _DifferentialHandler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    yield f"http://127.0.0.1:{port}"
    server.shutdown()


# ---------------------------------------------------------------------------
# Redirect default behavior
# ---------------------------------------------------------------------------


class TestRedirectDefault:
    """Verify eggfetch does NOT follow redirects by default (unlike requests)."""

    def test_eggfetch_no_follow_by_default(self, diff_server):
        resp = eggfetch.get(f"{diff_server}/redirect/302")
        assert resp.status_code == 302

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_requests_follows_by_default(self, diff_server):
        """requests follows redirects by default — document the difference."""
        resp = _requests.get(f"{diff_server}/redirect/302", allow_redirects=True)
        assert resp.status_code == 200

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_requests_no_follow(self, diff_server):
        """requests can disable redirect following."""
        resp = _requests.get(f"{diff_server}/redirect/302", allow_redirects=False)
        assert resp.status_code == 302


class TestRedirectFollow:
    """When follow_redirects=True, eggfetch behaves like requests."""

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    @pytest.mark.parametrize("code", [301, 302, 307, 308])
    def test_final_status_matches_requests(self, diff_server, code):
        egg_resp = eggfetch.get(
            f"{diff_server}/redirect/{code}", follow_redirects=True
        )
        req_resp = _requests.get(
            f"{diff_server}/redirect/{code}", allow_redirects=True
        )
        assert egg_resp.status_code == req_resp.status_code == 200

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    @pytest.mark.parametrize("code", [301, 302, 307, 308])
    def test_history_length_matches_requests(self, diff_server, code):
        egg_resp = eggfetch.get(
            f"{diff_server}/redirect/{code}", follow_redirects=True
        )
        req_resp = _requests.get(
            f"{diff_server}/redirect/{code}", allow_redirects=True
        )
        assert len(egg_resp.history) == len(req_resp.history) == 1


# ---------------------------------------------------------------------------
# Auth behavior
# ---------------------------------------------------------------------------


class TestBasicAuth:
    """Verify basic auth works identically to requests."""

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_basic_auth_matches_requests(self, diff_server):
        egg_resp = eggfetch.get(
            f"{diff_server}/basic-auth",
            auth=("user", "pass"),
        )
        req_resp = _requests.get(
            f"{diff_server}/basic-auth",
            auth=("user", "pass"),
        )
        assert egg_resp.status_code == req_resp.status_code == 200
        egg_json = egg_resp.json()
        req_json = req_resp.json()
        assert egg_json["authenticated"] == req_json["authenticated"]
        assert egg_json["user"] == req_json["user"]


class TestBearerAuth:
    """Verify bearer auth works identically to requests."""

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_bearer_auth_matches_requests(self, diff_server):
        egg_resp = eggfetch.get(
            f"{diff_server}/bearer-auth",
            headers={"Authorization": "Bearer test-token-123"},
        )
        req_resp = _requests.get(
            f"{diff_server}/bearer-auth",
            headers={"Authorization": "Bearer test-token-123"},
        )
        assert egg_resp.status_code == req_resp.status_code == 200
        assert egg_resp.json()["token"] == req_resp.json()["token"]


# ---------------------------------------------------------------------------
# JSON handling
# ---------------------------------------------------------------------------


class TestJsonHandling:
    """Verify JSON request/response handling matches requests."""

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_json_response_matches_requests(self, diff_server):
        egg_resp = eggfetch.get(f"{diff_server}/json")
        req_resp = _requests.get(f"{diff_server}/json")
        assert egg_resp.json() == req_resp.json()

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_json_post_matches_requests(self, diff_server):
        payload = {"key": "value", "number": 42}
        egg_resp = eggfetch.post(f"{diff_server}/json", json=payload)
        req_resp = _requests.post(f"{diff_server}/json", json=payload)
        assert egg_resp.json()["received"] == req_resp.json()["received"]


# ---------------------------------------------------------------------------
# Status code handling
# ---------------------------------------------------------------------------


class TestStatusCodes:
    """Verify status code handling matches requests."""

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    @pytest.mark.parametrize("status", [200, 404, 500])
    def test_status_code_matches_requests(self, diff_server, status):
        egg_resp = eggfetch.get(f"{diff_server}/status/{status}")
        req_resp = _requests.get(f"{diff_server}/status/{status}")
        assert egg_resp.status_code == req_resp.status_code == status


# ---------------------------------------------------------------------------
# Header handling
# ---------------------------------------------------------------------------


class TestHeaders:
    """Verify header handling matches requests."""

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_custom_headers_match_requests(self, diff_server):
        headers = {"X-Custom-Header": "custom-value", "X-Request-Id": "abc-123"}
        egg_resp = eggfetch.get(f"{diff_server}/headers", headers=headers)
        req_resp = _requests.get(f"{diff_server}/headers", headers=headers)
        egg_h = {k.lower(): v for k, v in egg_resp.json()["headers"].items()}
        req_h = {k.lower(): v for k, v in req_resp.json()["headers"].items()}
        assert egg_h.get("x-custom-header") == req_h.get("x-custom-header")
        assert egg_h.get("x-request-id") == req_h.get("x-request-id")


# ---------------------------------------------------------------------------
# Known differences (documented, not bugs)
# ---------------------------------------------------------------------------


class TestKnownDifferences:
    """Document intentional behavioral differences from requests."""

    def test_redirect_default_differs(self, diff_server):
        """eggfetch: no follow by default. requests: follows by default."""
        resp = eggfetch.get(f"{diff_server}/redirect/302")
        assert resp.status_code == 302  # Not followed

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_redirect_default_requests_follows(self, diff_server):
        """requests follows redirects by default."""
        resp = _requests.get(f"{diff_server}/redirect/302")
        assert resp.status_code == 200  # Followed

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_proxy_config_differs(self, diff_server):
        """eggfetch uses proxy= string. requests uses proxies=dict."""
        # This is a documented API difference, not a behavioral one.
        # Both achieve the same result when configured correctly.
        pass  # Placeholder for future proxy differential tests
