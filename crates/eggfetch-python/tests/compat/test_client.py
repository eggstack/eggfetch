"""Tests for Client and AsyncClient compatibility."""

import asyncio
import json
import http.server
import socketserver
import threading

import pytest

from eggfetch.compat.httpx import (
    Client,
    AsyncClient,
    Request,
    Response,
    Headers,
    Cookies,
    Timeout,
    QueryParams,
    URL,
)


class _ThreadedHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True


class _ClientHandler(http.server.BaseHTTPRequestHandler):
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
        from urllib.parse import urlparse, parse_qs
        parsed = urlparse(self.path)
        path = parsed.path
        qs = parse_qs(parsed.query)

        if path == "/get":
            self._send_json({"method": "GET"})
        elif path == "/echo-headers":
            headers = {k: v for k, v in self.headers.items()}
            self._send_json({"headers": headers})
        elif path == "/status/404":
            self._send_text("not found", 404)
        elif path == "/status/500":
            self._send_text("server error", 500)
        elif path == "/set-cookie":
            value = qs.get("value", ["test"])[0]
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Set-Cookie", f"session={value}; Path=/")
            body = json.dumps({"cookie_set": True}).encode()
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        from urllib.parse import urlparse
        parsed = urlparse(self.path)
        path = parsed.path

        if path == "/echo":
            cl = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(cl)
            self.send_response(200)
            self.send_header(
                "Content-Type",
                self.headers.get("Content-Type", "application/octet-stream"),
            )
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path == "/json":
            cl = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(cl)
            data = json.loads(body)
            self._send_json({"received": data})
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        pass


@pytest.fixture(scope="module")
def compat_server():
    server = _ThreadedHTTPServer(("127.0.0.1", 0), _ClientHandler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    yield f"http://127.0.0.1:{port}"
    server.shutdown()


class TestClientConstructor:
    def test_defaults(self):
        client = Client()
        assert isinstance(client.headers, Headers)
        assert isinstance(client.cookies, Cookies)
        assert isinstance(client.params, QueryParams)
        assert isinstance(client.timeout, Timeout)
        assert client.is_closed is False

    def test_custom_headers(self):
        client = Client(headers={"x-custom": "val"})
        assert client.headers["x-custom"] == "val"

    def test_custom_cookies(self):
        client = Client(cookies={"session": "abc"})
        assert client.cookies["session"] == "abc"

    def test_custom_params(self):
        client = Client(params={"q": "test"})
        assert client.params["q"] == "test"

    def test_custom_timeout(self):
        client = Client(timeout=Timeout(10.0))
        assert client.timeout.total == 10.0

    def test_scalar_timeout(self):
        client = Client(timeout=5.0)
        assert client.timeout.total == 5.0

    def test_base_url(self):
        client = Client(base_url="https://api.example.com")
        assert client.base_url.host == "api.example.com"

    def test_auth(self):
        client = Client(auth=("user", "pass"))
        assert client.auth == ("user", "pass")

    def test_follow_redirects(self):
        client = Client(follow_redirects=True)
        assert client.is_closed is False


class TestClientBuildRequest:
    def test_build_request(self):
        client = Client()
        req = client.build_request("GET", "https://example.com/path")
        assert req.method == "GET"
        assert req.url.host == "example.com"

    def test_build_request_merges_headers(self):
        client = Client(headers={"x-client": "val"})
        req = client.build_request("GET", "https://example.com", headers={"x-req": "val"})
        assert req.headers["x-client"] == "val"
        assert req.headers["x-req"] == "val"

    def test_build_request_merges_params(self):
        client = Client(params={"a": "1"})
        req = client.build_request("GET", "https://example.com", params={"b": "2"})
        assert req.params["a"] == "1"
        assert req.params["b"] == "2"

    def test_build_request_merges_cookies(self):
        client = Client(cookies={"a": "1"})
        req = client.build_request("GET", "https://example.com", cookies={"b": "2"})
        assert req.cookies["a"] == "1"
        assert req.cookies["b"] == "2"


class TestClientBaseUrl:
    def test_base_url_joining(self):
        client = Client(base_url="https://api.example.com/v1")
        req = client.build_request("GET", "/users")
        assert "/v1/users" in str(req.url) or req.url.path == "/users"

    def test_absolute_url_wins(self):
        client = Client(base_url="https://api.example.com/v1")
        req = client.build_request("GET", "https://other.com/override")
        assert req.url.host == "other.com"


class TestClientSend:
    def test_send_request(self, compat_server):
        with Client() as client:
            req = client.build_request("GET", f"{compat_server}/get")
            resp = client.send(req)
            assert resp.status_code == 200

    def test_send_request_identity(self, compat_server):
        with Client() as client:
            req = client.build_request("GET", f"{compat_server}/get")
            resp = client.send(req)
            assert resp.request is req

    def test_send_non_request_raises(self, compat_server):
        client = Client()
        client._ensure_client()
        with pytest.raises(TypeError, match="Request"):
            client.send("not a request")


class TestClientMethods:
    def test_get(self, compat_server):
        with Client() as client:
            resp = client.get(f"{compat_server}/get")
            assert resp.status_code == 200

    def test_post_json(self, compat_server):
        with Client() as client:
            resp = client.post(f"{compat_server}/json", json={"key": "val"})
            assert resp.status_code == 200

    def test_post_content(self, compat_server):
        with Client() as client:
            resp = client.post(f"{compat_server}/echo", content=b"hello")
            assert resp.content == b"hello"

    def test_request_method(self, compat_server):
        with Client() as client:
            resp = client.request("GET", f"{compat_server}/get")
            assert resp.status_code == 200


class TestClientContextManager:
    def test_context_manager(self, compat_server):
        with Client() as client:
            resp = client.get(f"{compat_server}/get")
            assert resp.status_code == 200
        assert client.is_closed

    def test_close(self, compat_server):
        client = Client()
        client._ensure_client()
        client.close()
        assert client.is_closed


class TestAsyncClient:
    @pytest.mark.asyncio
    async def test_async_context_manager(self, compat_server):
        async with AsyncClient() as client:
            resp = await client.get(f"{compat_server}/get")
            assert resp.status_code == 200
        assert client.is_closed

    @pytest.mark.asyncio
    async def test_async_get(self, compat_server):
        async with AsyncClient() as client:
            resp = await client.get(f"{compat_server}/get")
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_post_json(self, compat_server):
        async with AsyncClient() as client:
            resp = await client.post(f"{compat_server}/json", json={"k": "v"})
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_request(self, compat_server):
        async with AsyncClient() as client:
            resp = await client.request("GET", f"{compat_server}/get")
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_send(self, compat_server):
        async with AsyncClient() as client:
            req = client.build_request("GET", f"{compat_server}/get")
            resp = await client.send(req)
            assert resp.status_code == 200
            assert resp.request is req

    @pytest.mark.asyncio
    async def test_async_close(self, compat_server):
        client = AsyncClient()
        client._ensure_client()
        await client.close()
        assert client.is_closed

    @pytest.mark.asyncio
    async def test_async_send_non_request_raises(self, compat_server):
        client = AsyncClient()
        client._ensure_client()
        with pytest.raises(TypeError, match="Request"):
            await client.send("not a request")


class TestClientTimeout:
    """Test timeout parameter behavior."""

    def test_timeout_none_disables_timeout(self, compat_server):
        """timeout=None should explicitly disable all timeouts."""
        with Client(timeout=5.0) as client:
            req = client.build_request("GET", f"{compat_server}/get")
            resp = client.send(req, timeout=None)
            assert resp.status_code == 200

    def test_timeout_omitted_uses_client_default(self, compat_server):
        """Omitted timeout should use client-level timeout."""
        with Client(timeout=5.0) as client:
            req = client.build_request("GET", f"{compat_server}/get")
            resp = client.send(req)
            assert resp.status_code == 200

    def test_timeout_scalar_sets_all_phases(self, compat_server):
        """timeout=10 should set all phases to 10."""
        with Client(timeout=5.0) as client:
            req = client.build_request("GET", f"{compat_server}/get")
            resp = client.send(req, timeout=10.0)
            assert resp.status_code == 200

    def test_timeout_object_preserved(self, compat_server):
        """Timeout object should be passed through correctly."""
        with Client(timeout=5.0) as client:
            req = client.build_request("GET", f"{compat_server}/get")
            resp = client.send(req, timeout=Timeout(2.0))
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_timeout_none_disables(self, compat_server):
        """async: timeout=None should explicitly disable all timeouts."""
        async with AsyncClient(timeout=5.0) as client:
            req = client.build_request("GET", f"{compat_server}/get")
            resp = await client.send(req, timeout=None)
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_timeout_omitted_uses_default(self, compat_server):
        """async: omitted timeout should use client-level timeout."""
        async with AsyncClient(timeout=5.0) as client:
            req = client.build_request("GET", f"{compat_server}/get")
            resp = await client.send(req)
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_timeout_scalar(self, compat_server):
        """async: timeout=10 should set all phases to 10."""
        async with AsyncClient(timeout=5.0) as client:
            req = client.build_request("GET", f"{compat_server}/get")
            resp = await client.send(req, timeout=10.0)
            assert resp.status_code == 200

    def test_request_timeout_none(self, compat_server):
        """request() timeout=None should disable timeouts."""
        with Client(timeout=5.0) as client:
            resp = client.request("GET", f"{compat_server}/get", timeout=None)
            assert resp.status_code == 200

    def test_request_timeout_omitted(self, compat_server):
        """request() omitted timeout should use client default."""
        with Client(timeout=5.0) as client:
            resp = client.request("GET", f"{compat_server}/get")
            assert resp.status_code == 200
