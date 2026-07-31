"""Track 4: Client state and mutable configuration.

Tests that client state transitions are enforced, property setters work
correctly, and default headers are present on built requests.
"""

import http.server
import json
import socketserver
import threading

import pytest

from eggfetch.compat.httpx import (
    Client,
    AsyncClient,
    Headers,
    Cookies,
    QueryParams,
    Timeout,
    URL,
    Auth,
    BasicAuth,
)


# ---------------------------------------------------------------------------
# Local test server
# ---------------------------------------------------------------------------

class _StateHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/get":
            body = b'{"ok": true}'
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


class _ThreadedHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True


@pytest.fixture(scope="module")
def server():
    srv = _ThreadedHTTPServer(("127.0.0.1", 0), _StateHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# Client state transitions
# ---------------------------------------------------------------------------

class TestClientState:
    def test_closed_client_cannot_reopen(self):
        client = Client()
        client.close()
        assert client.is_closed
        with pytest.raises(RuntimeError, match="Client is closed"):
            client._ensure_client()

    def test_closed_client_send_raises(self, server):
        client = Client()
        client.close()
        req = client.build_request("GET", f"{server}/get")
        with pytest.raises(RuntimeError, match="Client is closed"):
            client.send(req)

    def test_closed_client_stream_raises(self, server):
        client = Client()
        client.close()
        with pytest.raises(RuntimeError, match="Client is closed"):
            with client.stream("GET", f"{server}/get"):
                pass

    def test_close_is_idempotent(self):
        client = Client()
        client.close()
        client.close()
        client.close()
        assert client.is_closed

    def test_context_manager_prevents_reopen(self):
        with Client() as client:
            pass
        assert client.is_closed
        with pytest.raises(RuntimeError, match="Client is closed"):
            client._ensure_client()

    def test_enter_sets_opened_state(self):
        client = Client()
        assert not client.is_closed
        with client:
            assert not client.is_closed
        assert client.is_closed


# ---------------------------------------------------------------------------
# AsyncClient state transitions
# ---------------------------------------------------------------------------

class TestAsyncClientState:
    @pytest.mark.asyncio
    async def test_closed_client_cannot_reopen(self):
        client = AsyncClient()
        await client.close()
        assert client.is_closed
        with pytest.raises(RuntimeError, match="Client is closed"):
            client._ensure_client()

    @pytest.mark.asyncio
    async def test_closed_client_send_raises(self, server):
        client = AsyncClient()
        await client.close()
        req = client.build_request("GET", f"{server}/get")
        with pytest.raises(RuntimeError, match="Client is closed"):
            await client.send(req)

    @pytest.mark.asyncio
    async def test_closed_client_stream_raises(self, server):
        client = AsyncClient()
        await client.close()
        with pytest.raises(RuntimeError, match="Client is closed"):
            async with client.stream("GET", f"{server}/get"):
                pass

    @pytest.mark.asyncio
    async def test_aclose_is_idempotent(self):
        client = AsyncClient()
        await client.aclose()
        await client.aclose()
        assert client.is_closed

    @pytest.mark.asyncio
    async def test_context_manager_prevents_reopen(self):
        async with AsyncClient() as client:
            pass
        assert client.is_closed
        with pytest.raises(RuntimeError, match="Client is closed"):
            client._ensure_client()


# ---------------------------------------------------------------------------
# Property setters
# ---------------------------------------------------------------------------

class TestPropertySetters:
    def test_auth_setter(self):
        client = Client()
        client.auth = ("user", "pass")
        assert isinstance(client.auth, BasicAuth)

    def test_auth_setter_none(self):
        client = Client(auth=("user", "pass"))
        client.auth = None
        assert client.auth is None

    def test_base_url_setter_str(self):
        client = Client()
        client.base_url = "https://api.example.com"
        assert isinstance(client.base_url, URL)

    def test_base_url_setter_url(self):
        client = Client()
        url = URL("https://api.example.com")
        client.base_url = url
        assert client.base_url is url

    def test_base_url_setter_invalid_type(self):
        client = Client()
        with pytest.raises(TypeError, match="base_url"):
            client.base_url = 42

    def test_cookies_setter_dict(self):
        client = Client()
        client.cookies = {"session": "abc"}
        assert isinstance(client.cookies, Cookies)
        assert client.cookies["session"] == "abc"

    def test_cookies_setter_cookies(self):
        client = Client()
        c = Cookies({"a": "b"})
        client.cookies = c
        assert client.cookies is c

    def test_headers_setter_dict(self):
        client = Client()
        client.headers = {"x-custom": "val"}
        assert isinstance(client.headers, Headers)
        assert client.headers["x-custom"] == "val"

    def test_headers_setter_headers(self):
        client = Client()
        h = Headers({"x-a": "b"})
        client.headers = h
        assert client.headers is h

    def test_params_setter_dict(self):
        client = Client()
        client.params = {"q": "test"}
        assert isinstance(client.params, QueryParams)

    def test_params_setter_queryparams(self):
        client = Client()
        qp = QueryParams({"a": "b"})
        client.params = qp
        assert client.params is qp

    def test_timeout_setter_scalar(self):
        client = Client()
        client.timeout = 10.0
        assert isinstance(client.timeout, Timeout)
        assert client.timeout.total == 10.0

    def test_timeout_setter_timeout(self):
        client = Client()
        t = Timeout(20.0)
        client.timeout = t
        assert client.timeout is t

    def test_event_hooks_setter(self):
        client = Client()
        hooks = {"request": [lambda r: r], "response": []}
        client.event_hooks = hooks
        assert len(client.event_hooks["request"]) == 1

    def test_event_hooks_setter_copies(self):
        client = Client()
        hooks = {"request": [lambda r: r], "response": []}
        client.event_hooks = hooks
        # Modifying original should not affect client
        hooks["request"].append(lambda r: r)
        assert len(client.event_hooks["request"]) == 1

    def test_event_hooks_setter_invalid_type(self):
        client = Client()
        with pytest.raises(TypeError, match="event_hooks"):
            client.event_hooks = "invalid"


# ---------------------------------------------------------------------------
# Base URL trailing-slash semantics
# ---------------------------------------------------------------------------

class TestBaseUrlSemantics:
    def test_base_url_with_trailing_slash(self):
        client = Client(base_url="https://api.example.com/v1/")
        req = client.build_request("GET", "users")
        assert "v1" in str(req.url)

    def test_base_url_without_trailing_slash(self):
        client = Client(base_url="https://api.example.com/v1")
        req = client.build_request("GET", "users")
        # Without trailing slash, urljoin treats "v1" as a file, so "users"
        # replaces it entirely. This matches standard URL joining behavior.
        assert req.url.host == "api.example.com"

    def test_absolute_url_bypasses_base(self):
        client = Client(base_url="https://api.example.com/v1")
        req = client.build_request("GET", "https://other.com/override")
        assert req.url.host == "other.com"

    def test_request_with_leading_slash(self):
        client = Client(base_url="https://api.example.com/v1")
        req = client.build_request("GET", "/users")
        assert req.url.host == "api.example.com"


# ---------------------------------------------------------------------------
# Default headers
# ---------------------------------------------------------------------------

class TestDefaultHeaders:
    def test_client_has_default_headers(self):
        client = Client()
        assert "accept" in client.headers
        assert "accept-encoding" in client.headers
        assert "connection" in client.headers
        assert "user-agent" in client.headers

    def test_default_headers_include_accept(self):
        client = Client()
        assert client.headers["accept"] == "*/*"

    def test_user_headers_override_defaults(self):
        client = Client(headers={"accept": "application/json"})
        assert client.headers["accept"] == "application/json"
        # Other defaults still present
        assert "accept-encoding" in client.headers

    def test_user_headers_preserve_defaults(self):
        client = Client(headers={"x-custom": "val"})
        assert client.headers["x-custom"] == "val"
        assert client.headers["accept"] == "*/*"

    def test_async_client_has_default_headers(self):
        client = AsyncClient()
        assert "accept" in client.headers
        assert "accept-encoding" in client.headers

    def test_build_request_includes_default_headers(self):
        client = Client()
        req = client.build_request("GET", "https://example.com")
        assert "accept" in req.headers
        assert "accept-encoding" in req.headers
        assert "user-agent" in req.headers
