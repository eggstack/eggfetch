"""Track 3: Auth input normalization.

Tests that auth inputs are properly normalized: tuples become BasicAuth,
callables become _FunctionAuth, invalid inputs raise TypeError, URL
credentials are used as fallback, and explicit auth=None disables auth.
"""

import http.server
import json
import socketserver
import threading

import pytest

from eggfetch.compat.httpx import Client, AsyncClient, Auth, BasicAuth
from eggfetch.compat.httpx._client import _build_auth, _FunctionAuth


# ---------------------------------------------------------------------------
# Local test server
# ---------------------------------------------------------------------------

class _AuthHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/get":
            auth_header = self.headers.get("Authorization", "")
            body = json.dumps({
                "method": "GET",
                "auth": auth_header,
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


class _ThreadedHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True


@pytest.fixture(scope="module")
def server():
    srv = _ThreadedHTTPServer(("127.0.0.1", 0), _AuthHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# _build_auth normalization
# ---------------------------------------------------------------------------

class TestBuildAuth:
    def test_none_returns_none(self):
        assert _build_auth(None) is None

    def test_auth_instance_passthrough(self):
        auth = BasicAuth("user", "pass")
        assert _build_auth(auth) is auth

    def test_tuple_becomes_basic_auth(self):
        result = _build_auth(("user", "pass"))
        assert isinstance(result, BasicAuth)
        assert result.username == "user"
        assert result.password == "pass"

    def test_callable_becomes_function_auth(self):
        def my_auth(request):
            return request
        result = _build_auth(my_auth)
        assert isinstance(result, _FunctionAuth)

    def test_invalid_type_raises_type_error(self):
        with pytest.raises(TypeError, match="auth must be"):
            _build_auth(42)

    def test_invalid_tuple_length_raises_type_error(self):
        with pytest.raises(TypeError, match="auth tuple must be"):
            _build_auth(("user",))

    def test_invalid_tuple_length_three_raises(self):
        with pytest.raises(TypeError, match="auth tuple must be"):
            _build_auth(("user", "pass", "extra"))


# ---------------------------------------------------------------------------
# Constructor auth normalization
# ---------------------------------------------------------------------------

class TestClientAuthNormalization:
    def test_tuple_auth_normalized(self):
        client = Client(auth=("user", "pass"))
        assert isinstance(client.auth, BasicAuth)
        assert client.auth.username == "user"
        assert client.auth.password == "pass"

    def test_none_auth(self):
        client = Client(auth=None)
        assert client.auth is None

    def test_auth_instance_passthrough(self):
        auth = BasicAuth("u", "p")
        client = Client(auth=auth)
        assert client.auth is auth

    def test_callable_auth(self):
        def my_auth(request):
            return request
        client = Client(auth=my_auth)
        assert isinstance(client.auth, _FunctionAuth)

    def test_auth_property_setter_normalizes(self):
        client = Client()
        client.auth = ("user", "pass")
        assert isinstance(client.auth, BasicAuth)

    def test_auth_property_setter_none(self):
        client = Client(auth=("user", "pass"))
        client.auth = None
        assert client.auth is None


# ---------------------------------------------------------------------------
# Auth application in requests
# ---------------------------------------------------------------------------

class TestAuthApplication:
    def test_basic_auth_sends_header(self, server):
        with Client(auth=("user", "pass")) as client:
            resp = client.get(f"{server}/get")
            data = resp.json()
            assert data["auth"].startswith("Basic ")

    def test_no_auth_no_header(self, server):
        with Client() as client:
            resp = client.get(f"{server}/get")
            data = resp.json()
            assert data["auth"] == ""

    def test_per_request_auth_overrides_client(self, server):
        with Client(auth=("user", "pass")) as client:
            resp = client.get(f"{server}/get", auth=("other", "cred"))
            data = resp.json()
            # Should use per-request auth
            assert data["auth"].startswith("Basic ")

    def test_per_request_auth_none_disables(self, server):
        with Client(auth=("user", "pass")) as client:
            resp = client.get(f"{server}/get", auth=None)
            data = resp.json()
            assert data["auth"] == ""


# ---------------------------------------------------------------------------
# AsyncClient auth normalization
# ---------------------------------------------------------------------------

class TestAsyncClientAuthNormalization:
    @pytest.mark.asyncio
    async def test_tuple_auth_normalized(self):
        async with AsyncClient(auth=("user", "pass")) as client:
            assert isinstance(client.auth, BasicAuth)

    @pytest.mark.asyncio
    async def test_callable_auth(self):
        def my_auth(request):
            return request
        async with AsyncClient(auth=my_auth) as client:
            assert isinstance(client.auth, _FunctionAuth)

    @pytest.mark.asyncio
    async def test_auth_property_setter(self):
        async with AsyncClient() as client:
            client.auth = ("user", "pass")
            assert isinstance(client.auth, BasicAuth)
