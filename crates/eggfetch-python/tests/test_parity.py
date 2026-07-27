"""Sync/async API parity tests.

Each test case exercises both the sync and async code paths with identical
assertions to ensure behavioral equivalence.
"""

import asyncio
import http.server
import json
import threading
import urllib.parse

import pytest

import eggfetch

from conftest import _ThreadingHTTPServer


# ---------------------------------------------------------------------------
# Local test server
# ---------------------------------------------------------------------------


class _Handler(http.server.BaseHTTPRequestHandler):
    """Minimal test server that echoes request details."""

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        body = json.dumps({
            "method": "GET",
            "path": parsed.path,
            "query": parsed.query,
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
        body = json.dumps({
            "method": "POST",
            "path": self.path,
            "headers": dict(self.headers),
            "body": raw.decode(errors="replace"),
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_PUT(self):
        length = int(self.headers.get("Content-Length", 0))
        raw = self.rfile.read(length) if length else b""
        body = json.dumps({
            "method": "PUT",
            "path": self.path,
            "headers": dict(self.headers),
            "body": raw.decode(errors="replace"),
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_PATCH(self):
        length = int(self.headers.get("Content-Length", 0))
        raw = self.rfile.read(length) if length else b""
        body = json.dumps({
            "method": "PATCH",
            "path": self.path,
            "headers": dict(self.headers),
            "body": raw.decode(errors="replace"),
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_DELETE(self):
        parsed = urllib.parse.urlparse(self.path)
        body = json.dumps({
            "method": "DELETE",
            "path": parsed.path,
            "query": parsed.query,
            "headers": dict(self.headers),
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_HEAD(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", "5")
        self.end_headers()

    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header("Allow", "GET, POST, OPTIONS")
        self.send_header("Content-Length", "0")
        self.end_headers()

    def log_message(self, format, *args):
        pass  # suppress server logs


@pytest.fixture(scope="module")
def server():
    srv = _ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# Sync/async parity: method dispatch
# ---------------------------------------------------------------------------


class TestMethodParity:
    """Verify that sync and async clients dispatch the same HTTP methods."""

    @pytest.mark.parametrize("method", ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"])
    def test_method_matches(self, server, method):
        # Sync
        sync_client = eggfetch.Client()
        sync_resp = sync_client.request(method, f"{server}/test")
        sync_client.close()

        # Async
        async def run_async():
            client = eggfetch.AsyncClient()
            resp = await client.request(method, f"{server}/test")
            client.close()
            return resp

        async_resp = asyncio.run(run_async())

        assert sync_resp.status_code == async_resp.status_code
        assert sync_resp.http_version == async_resp.http_version


# ---------------------------------------------------------------------------
# Sync/async parity: response properties
# ---------------------------------------------------------------------------


class TestResponsePropertyParity:
    """Verify that sync and async responses expose identical properties."""

    def test_json_response_properties(self, server):
        sync_client = eggfetch.Client()
        sync_resp = sync_client.get(f"{server}/json")
        sync_client.close()

        async def run_async():
            client = eggfetch.AsyncClient()
            resp = await client.get(f"{server}/json")
            client.close()
            return resp

        async_resp = asyncio.run(run_async())

        assert sync_resp.status_code == async_resp.status_code == 200
        assert sync_resp.reason_phrase == async_resp.reason_phrase
        assert sync_resp.http_version == async_resp.http_version
        assert sync_resp.url == async_resp.url
        assert sync_resp.encoding == async_resp.encoding
        assert sync_resp.content == async_resp.content
        assert sync_resp.text == async_resp.text
        assert sync_resp.history == async_resp.history

    def test_headers_match(self, server):
        sync_client = eggfetch.Client()
        sync_resp = sync_client.get(f"{server}/json")
        sync_client.close()

        async def run_async():
            client = eggfetch.AsyncClient()
            resp = await client.get(f"{server}/json")
            client.close()
            return resp

        async_resp = asyncio.run(run_async())

        # Both should have content-type header
        assert sync_resp.headers.get("content-type") is not None
        assert async_resp.headers.get("content-type") is not None
        assert (
            sync_resp.headers.get("content-type")
            == async_resp.headers.get("content-type")
        )


# ---------------------------------------------------------------------------
# Sync/async parity: error handling
# ---------------------------------------------------------------------------


class TestErrorParity:
    """Verify that sync and async clients raise the same exceptions."""

    def test_invalid_url(self):
        sync_client = eggfetch.Client()
        with pytest.raises(ValueError):
            sync_client.get("not-a-url")
        sync_client.close()

        async def run_async():
            client = eggfetch.AsyncClient()
            with pytest.raises(ValueError):
                await client.get("not-a-url")
            client.close()

        asyncio.run(run_async())

    def test_invalid_method(self):
        sync_client = eggfetch.Client()
        with pytest.raises(ValueError):
            sync_client.request("BAD METHOD", "http://example.com")
        sync_client.close()

        async def run_async():
            client = eggfetch.AsyncClient()
            with pytest.raises(ValueError):
                await client.request("BAD METHOD", "http://example.com")
            client.close()

        asyncio.run(run_async())


# ---------------------------------------------------------------------------
# Sync/async parity: close semantics
# ---------------------------------------------------------------------------


class TestCloseParity:
    """Verify that sync and async clients have identical close behavior."""

    def test_closed_client_raises(self):
        sync_client = eggfetch.Client()
        sync_client.close()
        with pytest.raises(ValueError, match="closed"):
            sync_client.get("http://example.com")

        async def run_async():
            client = eggfetch.AsyncClient()
            client.close()
            with pytest.raises(ValueError, match="closed"):
                await client.get("http://example.com")

        asyncio.run(run_async())

    def test_close_is_idempotent(self):
        sync_client = eggfetch.Client()
        sync_client.close()
        sync_client.close()  # should not raise
        assert sync_client.is_closed

        async def run_async():
            client = eggfetch.AsyncClient()
            client.close()
            client.close()  # should not raise
            assert client.is_closed

        asyncio.run(run_async())

    def test_context_manager_closes(self):
        with eggfetch.Client() as client:
            assert not client.is_closed
        assert client.is_closed

        async def run_async():
            async with eggfetch.AsyncClient() as client:
                assert not client.is_closed
            assert client.is_closed

        asyncio.run(run_async())


# ---------------------------------------------------------------------------
# Sync/async parity: request kwargs
# ---------------------------------------------------------------------------


class TestKwargsParity:
    """Verify that sync and async clients accept the same keyword arguments."""

    def test_headers_kwarg(self, server):
        sync_client = eggfetch.Client()
        sync_resp = sync_client.get(
            f"{server}/json", headers={"X-Custom": "sync-value"}
        )
        sync_client.close()

        async def run_async():
            client = eggfetch.AsyncClient()
            resp = await client.get(
                f"{server}/json", headers={"X-Custom": "async-value"}
            )
            client.close()
            return resp

        async_resp = asyncio.run(run_async())

        # Both should have sent the custom header
        sync_body = json.loads(sync_resp.text)
        async_body = json.loads(async_resp.text)
        assert sync_body["headers"]["x-custom"] == "sync-value"
        assert async_body["headers"]["x-custom"] == "async-value"

    def test_params_kwarg(self, server):
        sync_client = eggfetch.Client()
        sync_resp = sync_client.get(
            f"{server}/json", params={"key": "value"}
        )
        sync_client.close()

        async def run_async():
            client = eggfetch.AsyncClient()
            resp = await client.get(
                f"{server}/json", params={"key": "value"}
            )
            client.close()
            return resp

        async_resp = asyncio.run(run_async())

        sync_body = json.loads(sync_resp.text)
        async_body = json.loads(async_resp.text)
        assert sync_body["query"] == async_body["query"]
        assert "key=value" in sync_body["query"]

    def test_json_body(self, server):
        sync_client = eggfetch.Client()
        sync_resp = sync_client.post(
            f"{server}/json", json={"foo": "bar"}
        )
        sync_client.close()

        async def run_async():
            client = eggfetch.AsyncClient()
            resp = await client.post(
                f"{server}/json", json={"foo": "bar"}
            )
            client.close()
            return resp

        async_resp = asyncio.run(run_async())

        sync_body = json.loads(sync_resp.text)
        async_body = json.loads(async_resp.text)
        assert sync_body["method"] == "POST"
        assert async_body["method"] == "POST"

    def test_content_body(self, server):
        sync_client = eggfetch.Client()
        sync_resp = sync_client.post(
            f"{server}/json", content=b"raw bytes"
        )
        sync_client.close()

        async def run_async():
            client = eggfetch.AsyncClient()
            resp = await client.post(
                f"{server}/json", content=b"raw bytes"
            )
            client.close()
            return resp

        async_resp = asyncio.run(run_async())

        sync_body = json.loads(sync_resp.text)
        async_body = json.loads(async_resp.text)
        assert sync_body["method"] == "POST"
        assert async_body["method"] == "POST"


# ---------------------------------------------------------------------------
# Sync/async parity: redirect behavior
# ---------------------------------------------------------------------------


class _RedirectHandler(http.server.BaseHTTPRequestHandler):
    """Handler that supports redirect testing."""

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path == "/redirect":
            self.send_response(302)
            self.send_header("Location", "/target")
            self.send_header("Content-Length", "0")
            self.end_headers()
        elif parsed.path == "/target":
            body = b"redirected"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()

    def log_message(self, format, *args):
        pass


@pytest.fixture(scope="module")
def redirect_server():
    srv = _ThreadingHTTPServer(("127.0.0.1", 0), _RedirectHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


class TestRedirectParity:
    """Verify that sync and async clients handle redirects identically."""

    def test_follow_redirects(self, redirect_server):
        sync_client = eggfetch.Client(follow_redirects=True)
        sync_resp = sync_client.get(f"{redirect_server}/redirect")
        sync_client.close()

        async def run_async():
            client = eggfetch.AsyncClient(follow_redirects=True)
            resp = await client.get(f"{redirect_server}/redirect")
            client.close()
            return resp

        async_resp = asyncio.run(run_async())

        assert sync_resp.status_code == async_resp.status_code == 200
        assert sync_resp.text == async_resp.text == "redirected"
        assert len(sync_resp.history) == len(async_resp.history) == 1

    def test_no_follow_redirects(self, redirect_server):
        sync_client = eggfetch.Client()
        sync_resp = sync_client.get(f"{redirect_server}/redirect")
        sync_client.close()

        async def run_async():
            client = eggfetch.AsyncClient()
            resp = await client.get(f"{redirect_server}/redirect")
            client.close()
            return resp

        async_resp = asyncio.run(run_async())

        assert sync_resp.status_code == async_resp.status_code == 302
        assert len(sync_resp.history) == len(async_resp.history) == 0


# ---------------------------------------------------------------------------
# Sync/async parity: client constructor
# ---------------------------------------------------------------------------


class TestConstructorParity:
    """Verify that sync and async client constructors accept the same args."""

    def test_default_headers(self, server):
        sync_client = eggfetch.Client(headers={"X-Default": "sync"})
        sync_resp = sync_client.get(f"{server}/json")
        sync_client.close()

        async def run_async():
            client = eggfetch.AsyncClient(headers={"X-Default": "async"})
            resp = await client.get(f"{server}/json")
            client.close()
            return resp

        async_resp = asyncio.run(run_async())

        sync_body = json.loads(sync_resp.text)
        async_body = json.loads(async_resp.text)
        assert sync_body["headers"]["x-default"] == "sync"
        assert async_body["headers"]["x-default"] == "async"

    def test_max_redirects(self, redirect_server):
        sync_client = eggfetch.Client(
            follow_redirects=True, max_redirects=0
        )
        with pytest.raises(eggfetch.TooManyRedirects):
            sync_client.get(f"{redirect_server}/redirect")
        sync_client.close()

        async def run_async():
            client = eggfetch.AsyncClient(
                follow_redirects=True, max_redirects=0
            )
            with pytest.raises(eggfetch.TooManyRedirects):
                await client.get(f"{redirect_server}/redirect")
            client.close()

        asyncio.run(run_async())


# ---------------------------------------------------------------------------
# Enhanced handler for cookie, auth, and streaming tests
# ---------------------------------------------------------------------------


class _EnhancedHandler(http.server.BaseHTTPRequestHandler):
    """Handler with cookie, auth, and streaming support."""

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path == "/set-cookie":
            body = b"cookie set"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Set-Cookie", "session=abc123; Path=/")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif parsed.path == "/check-cookie":
            cookie_header = self.headers.get("Cookie", "")
            body = json.dumps({"cookie": cookie_header}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif parsed.path == "/redirect-with-cookie":
            self.send_response(302)
            self.send_header("Location", "/check-cookie")
            self.send_header("Set-Cookie", "redirected=true; Path=/")
            self.send_header("Content-Length", "0")
            self.end_headers()

        elif parsed.path == "/auth":
            auth_header = self.headers.get("Authorization", "")
            body = json.dumps({"authorization": auth_header}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif parsed.path == "/stream-data":
            body = b"streamed content"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        else:
            body = json.dumps({
                "method": "GET",
                "path": parsed.path,
                "query": parsed.query,
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
        parsed = urllib.parse.urlparse(self.path)

        if parsed.path == "/auth":
            auth_header = self.headers.get("Authorization", "")
            body = json.dumps({
                "authorization": auth_header,
                "body": raw.decode(errors="replace"),
            }).encode()
        else:
            body = json.dumps({
                "method": "POST",
                "path": self.path,
                "headers": dict(self.headers),
                "body": raw.decode(errors="replace"),
            }).encode()

        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        pass


@pytest.fixture(scope="module")
def enhanced_server():
    srv = _ThreadingHTTPServer(("127.0.0.1", 0), _EnhancedHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# Sync/async parity: cookie behavior
# ---------------------------------------------------------------------------


class TestCookieParity:
    """Verify that sync and async clients handle cookies identically."""

    def test_sync_cookies_set_and_send(self, enhanced_server):
        with eggfetch.Client() as client:
            resp = client.get(f"{enhanced_server}/set-cookie")
            assert resp.status_code == 200
            jar = client.cookies
            assert "session" in jar

            resp2 = client.get(f"{enhanced_server}/check-cookie")
            body = json.loads(resp2.text)
            assert "session=abc123" in body["cookie"]

    async def test_async_cookies_set_and_send(self, enhanced_server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.get(f"{enhanced_server}/set-cookie")
            assert resp.status_code == 200
            jar = client.cookies
            assert "session" in jar

            resp2 = await client.get(f"{enhanced_server}/check-cookie")
            body = json.loads(resp2.text)
            assert "session=abc123" in body["cookie"]

    def test_sync_redirect_cookies(self, enhanced_server):
        with eggfetch.Client(follow_redirects=True) as client:
            resp = client.get(f"{enhanced_server}/redirect-with-cookie")
            assert resp.status_code == 200
            jar = client.cookies
            assert "redirected" in jar

    async def test_async_redirect_cookies(self, enhanced_server):
        async with eggfetch.AsyncClient(follow_redirects=True) as client:
            resp = await client.get(f"{enhanced_server}/redirect-with-cookie")
            assert resp.status_code == 200
            jar = client.cookies
            assert "redirected" in jar

    def test_sync_kwarg_cookies_dont_leak(self, enhanced_server):
        with eggfetch.Client() as client:
            resp = client.get(
                f"{enhanced_server}/check-cookie",
                headers={"Cookie": "kwargs=only"},
            )
            body = json.loads(resp.text)
            assert "kwargs=only" in body["cookie"]
            assert "session" not in body["cookie"]

    async def test_async_kwarg_cookies_dont_leak(self, enhanced_server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.get(
                f"{enhanced_server}/check-cookie",
                headers={"Cookie": "kwargs=only"},
            )
            body = json.loads(resp.text)
            assert "kwargs=only" in body["cookie"]
            assert "session" not in body["cookie"]


# ---------------------------------------------------------------------------
# Sync/async parity: auth behavior
# ---------------------------------------------------------------------------


class TestAuthParity:
    """Verify that sync and async clients handle auth identically."""

    def test_sync_auth_basic(self, enhanced_server):
        with eggfetch.Client() as client:
            resp = client.get(
                f"{enhanced_server}/auth",
                auth=eggfetch.BasicAuth("user", "pass"),
            )
            body = json.loads(resp.text)
            assert body["authorization"].startswith("Basic ")

    async def test_async_auth_basic(self, enhanced_server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.get(
                f"{enhanced_server}/auth",
                auth=eggfetch.BasicAuth("user", "pass"),
            )
            body = json.loads(resp.text)
            assert body["authorization"].startswith("Basic ")

    def test_sync_auth_bearer(self, enhanced_server):
        with eggfetch.Client() as client:
            resp = client.get(
                f"{enhanced_server}/auth",
                auth=eggfetch.BearerAuth("mytoken"),
            )
            body = json.loads(resp.text)
            assert body["authorization"] == "Bearer mytoken"

    async def test_async_auth_bearer(self, enhanced_server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.get(
                f"{enhanced_server}/auth",
                auth=eggfetch.BearerAuth("mytoken"),
            )
            body = json.loads(resp.text)
            assert body["authorization"] == "Bearer mytoken"

    def test_sync_auth_none_no_header(self, enhanced_server):
        with eggfetch.Client() as client:
            resp = client.get(f"{enhanced_server}/auth")
            body = json.loads(resp.text)
            assert body["authorization"] == ""

    async def test_async_auth_none_no_header(self, enhanced_server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.get(f"{enhanced_server}/auth")
            body = json.loads(resp.text)
            assert body["authorization"] == ""

    def test_sync_auth_override(self, enhanced_server):
        with eggfetch.Client(auth=eggfetch.BasicAuth("default", "pwd")) as client:
            resp = client.get(
                f"{enhanced_server}/auth",
                auth=eggfetch.BearerAuth("override"),
            )
            body = json.loads(resp.text)
            assert body["authorization"] == "Bearer override"

    async def test_async_auth_override(self, enhanced_server):
        async with eggfetch.AsyncClient(
            auth=eggfetch.BasicAuth("default", "pwd")
        ) as client:
            resp = await client.get(
                f"{enhanced_server}/auth",
                auth=eggfetch.BearerAuth("override"),
            )
            body = json.loads(resp.text)
            assert body["authorization"] == "Bearer override"

    def test_sync_auth_post(self, enhanced_server):
        with eggfetch.Client() as client:
            resp = client.post(
                f"{enhanced_server}/auth",
                auth=eggfetch.BasicAuth("user", "pass"),
                content=b"hello",
            )
            body = json.loads(resp.text)
            assert body["authorization"].startswith("Basic ")
            assert body["body"] == "hello"

    async def test_async_auth_post(self, enhanced_server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.post(
                f"{enhanced_server}/auth",
                auth=eggfetch.BasicAuth("user", "pass"),
                content=b"hello",
            )
            body = json.loads(resp.text)
            assert body["authorization"].startswith("Basic ")
            assert body["body"] == "hello"


# ---------------------------------------------------------------------------
# Sync/async parity: streaming
# ---------------------------------------------------------------------------


class TestStreamingParity:
    """Verify that sync and async streaming produce identical results."""

    def test_sync_stream_iter_bytes(self, enhanced_server):
        with eggfetch.Client() as client:
            with client.stream("GET", f"{enhanced_server}/stream-data") as resp:
                result = b"".join(resp.iter_bytes())
        assert result == b"streamed content"

    async def test_async_stream_aiter_bytes(self, enhanced_server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{enhanced_server}/stream-data")
            async with resp:
                chunks = []
                async for chunk in resp.aiter_bytes():
                    chunks.append(chunk)
                result = b"".join(chunks)
        assert result == b"streamed content"

    def test_sync_stream_iter_text(self, enhanced_server):
        with eggfetch.Client() as client:
            with client.stream("GET", f"{enhanced_server}/stream-data") as resp:
                result = "".join(resp.iter_text())
        assert result == "streamed content"

    async def test_async_stream_aiter_text(self, enhanced_server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{enhanced_server}/stream-data")
            async with resp:
                chunks = []
                async for chunk in resp.aiter_text():
                    chunks.append(chunk)
                result = "".join(chunks)
        assert result == "streamed content"

    def test_sync_stream_iter_lines(self, enhanced_server):
        with eggfetch.Client() as client:
            with client.stream("GET", f"{enhanced_server}/stream-data") as resp:
                result = list(resp.iter_lines())
        assert result == ["streamed content"]

    async def test_async_stream_aiter_lines(self, enhanced_server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{enhanced_server}/stream-data")
            async with resp:
                result = []
                async for line in resp.aiter_lines():
                    result.append(line)
        assert result == ["streamed content"]

    def test_sync_stream_read(self, enhanced_server):
        with eggfetch.Client() as client:
            with client.stream("GET", f"{enhanced_server}/stream-data") as resp:
                result = resp.read()
        assert result == b"streamed content"

    async def test_async_stream_aread(self, enhanced_server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{enhanced_server}/stream-data")
            async with resp:
                result = await resp.aread()
        assert result == b"streamed content"

    def test_sync_stream_status(self, enhanced_server):
        with eggfetch.Client() as client:
            with client.stream("GET", f"{enhanced_server}/stream-data") as resp:
                assert resp.status_code == 200
                assert resp.is_success

    async def test_async_stream_status(self, enhanced_server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{enhanced_server}/stream-data")
            async with resp:
                assert resp.status_code == 200
                assert resp.is_success

    def test_sync_stream_headers(self, enhanced_server):
        with eggfetch.Client() as client:
            with client.stream("GET", f"{enhanced_server}/stream-data") as resp:
                assert "text/plain" in resp.headers.get("content-type", "")

    async def test_async_stream_headers(self, enhanced_server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{enhanced_server}/stream-data")
            async with resp:
                assert "text/plain" in resp.headers.get("content-type", "")

    def test_sync_stream_auto_drain(self, enhanced_server):
        with eggfetch.Client() as client:
            with client.stream("GET", f"{enhanced_server}/stream-data") as resp:
                pass
            # Should not raise; context manager drains

    async def test_async_stream_auto_drain(self, enhanced_server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{enhanced_server}/stream-data")
            async with resp:
                pass
            # Should not raise; context manager drains
