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
    srv = http.server.HTTPServer(("127.0.0.1", 0), _Handler)
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
    srv = http.server.HTTPServer(("127.0.0.1", 0), _RedirectHandler)
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
