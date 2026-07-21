"""Required HTTPX compatibility tests.

These tests MUST pass against httpx==0.28.1. They cannot be skipped.
If httpx is not installed or the wrong version is present, the test
session will fail rather than skip.
"""

import base64
import http.server
import json
import threading
import urllib.parse

import httpx
import pytest

import eggfetch

assert httpx.__version__ == "0.28.1", (
    f"Expected httpx 0.28.1, got {httpx.__version__}"
)


class _CompatHandler(http.server.BaseHTTPRequestHandler):
    """Minimal test server for required compatibility tests."""

    def _send_json(self, data, status=200):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_text(self, text, status=200):
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
            self._send_json({"method": "GET"})
        elif path == "/json":
            self._send_json({"key": "value", "number": 42})
        elif path == "/headers":
            headers = {k: v for k, v in self.headers.items()}
            self._send_json({"headers": headers})
        elif path == "/redirect/302":
            self.send_response(302)
            self.send_header("Location", f"{base}/get")
            self.end_headers()
        elif path == "/basic-auth":
            auth_header = self.headers.get("Authorization", "")
            if auth_header.startswith("Basic "):
                decoded = base64.b64decode(auth_header[6:]).decode()
                user, _password = decoded.split(":", 1)
                self._send_json({"authenticated": True, "user": user})
            else:
                self.send_response(401)
                self.send_header("WWW-Authenticate", 'Basic realm="test"')
                self.end_headers()
        elif path == "/status/404":
            self._send_text("not found", 404)
        elif path == "/status/500":
            self._send_text("server error", 500)
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        if path == "/json":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            data = json.loads(body)
            self._send_json({"received": data})
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        pass


@pytest.fixture(scope="module")
def compat_server():
    """Start a test server for required compatibility tests."""
    server = http.server.HTTPServer(("127.0.0.1", 0), _CompatHandler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    yield f"http://127.0.0.1:{port}"
    server.shutdown()


class TestBasicClientShape:
    """Verify eggfetch Client has the same constructor shape as httpx.Client."""

    def test_client_context_manager(self, compat_server):
        with eggfetch.Client() as client:
            resp = client.get(f"{compat_server}/get")
            assert resp.status_code == 200

    def test_async_client_context_manager(self, compat_server):
        import asyncio

        async def _test():
            async with eggfetch.AsyncClient() as client:
                resp = await client.get(f"{compat_server}/get")
                assert resp.status_code == 200

        asyncio.run(_test())

    def test_client_has_expected_methods(self):
        client = eggfetch.Client()
        for method in (
            "get", "post", "put", "patch", "delete", "head",
            "options", "request", "stream",
        ):
            assert hasattr(client, method), f"Client missing method: {method}"

    def test_async_client_has_expected_methods(self):
        client = eggfetch.AsyncClient()
        for method in (
            "get", "post", "put", "patch", "delete", "head",
            "options", "request", "stream",
        ):
            assert hasattr(client, method), f"AsyncClient missing method: {method}"


class TestRequestMethods:
    """Verify HTTP methods work identically to httpx."""

    @pytest.mark.parametrize(
        "method", ["get", "post", "put", "patch", "delete", "head", "options"]
    )
    def test_method_exists(self, method):
        assert callable(getattr(eggfetch, method))

    def test_get_returns_response(self, compat_server):
        resp = eggfetch.get(f"{compat_server}/get")
        assert resp.status_code == 200

    def test_json_response(self, compat_server):
        resp = eggfetch.get(f"{compat_server}/json")
        assert resp.json() == {"key": "value", "number": 42}

    def test_post_json(self, compat_server):
        resp = eggfetch.post(f"{compat_server}/json", json={"data": "test"})
        assert resp.status_code == 200
        assert resp.json()["received"]["data"] == "test"


class TestRedirectBehavior:
    """Verify redirect behavior matches httpx (intentional difference documented)."""

    def test_no_follow_by_default(self, compat_server):
        resp = eggfetch.get(f"{compat_server}/redirect/302")
        assert resp.status_code == 302

    def test_follow_with_flag(self, compat_server):
        resp = eggfetch.get(
            f"{compat_server}/redirect/302", follow_redirects=True
        )
        assert resp.status_code == 200


class TestErrorHandling:
    """Verify error handling matches httpx exception hierarchy."""

    def test_4xx_raises_on_status(self, compat_server):
        resp = eggfetch.get(f"{compat_server}/status/404")
        with pytest.raises(eggfetch.HTTPStatusError):
            resp.raise_for_status()

    def test_5xx_raises_on_status(self, compat_server):
        resp = eggfetch.get(f"{compat_server}/status/500")
        with pytest.raises(eggfetch.HTTPStatusError):
            resp.raise_for_status()

    def test_base_exception_is_eggfetch_error(self):
        assert issubclass(eggfetch.HTTPStatusError, eggfetch.EggfetchError)
        assert issubclass(eggfetch.TimeoutException, eggfetch.EggfetchError)
        assert issubclass(eggfetch.NetworkError, eggfetch.EggfetchError)


class TestTimeout:
    """Verify Timeout object API matches httpx."""

    def test_timeout_float(self, compat_server):
        resp = eggfetch.get(f"{compat_server}/get", timeout=5.0)
        assert resp.status_code == 200

    def test_timeout_object(self, compat_server):
        t = eggfetch.Timeout(5.0)
        resp = eggfetch.get(f"{compat_server}/get", timeout=t)
        assert resp.status_code == 200

    def test_timeout_properties(self):
        t = eggfetch.Timeout(3.0)
        assert t.pool == 3.0
        assert t.connect == 3.0
        assert t.read == 3.0
        assert t.write == 3.0


class TestAuthShape:
    """Verify auth API shape is compatible with httpx."""

    def test_basic_auth_object(self):
        auth = eggfetch.BasicAuth("user", "pass")
        assert auth is not None

    def test_bearer_auth_object(self):
        auth = eggfetch.BearerAuth("token")
        assert auth is not None

    def test_noauth(self):
        assert eggfetch.NOAUTH is not None


class TestResponseShape:
    """Verify response object has expected attributes."""

    def test_status_code(self, compat_server):
        resp = eggfetch.get(f"{compat_server}/get")
        assert hasattr(resp, "status_code")
        assert resp.status_code == 200

    def test_headers(self, compat_server):
        resp = eggfetch.get(f"{compat_server}/get")
        assert hasattr(resp, "headers")
        assert isinstance(resp.headers, eggfetch.Headers)

    def test_text(self, compat_server):
        resp = eggfetch.get(f"{compat_server}/json")
        assert hasattr(resp, "text")
        assert isinstance(resp.text, str)

    def test_json_method(self, compat_server):
        resp = eggfetch.get(f"{compat_server}/json")
        assert callable(resp.json)

    def test_cookies(self, compat_server):
        resp = eggfetch.get(f"{compat_server}/get")
        assert hasattr(resp, "cookies")


class TestCookieShape:
    """Verify cookie API is compatible."""

    def test_cookies_object(self):
        cookies = eggfetch.Cookies()
        assert hasattr(cookies, "set")
        assert hasattr(cookies, "get")
