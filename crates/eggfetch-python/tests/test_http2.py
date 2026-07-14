"""Tests for the HTTP/2 client option."""

import http.server
import json
import threading
import urllib.parse

import pytest

import eggfetch


# ---------------------------------------------------------------------------
# Local test server (HTTP/1.1 only)
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

    def log_message(self, format, *args):
        pass  # suppress logs during tests


@pytest.fixture(scope="module")
def server():
    """Start a local HTTP/1.1 server for the test module."""
    srv = http.server.HTTPServer(("127.0.0.1", 0), _Handler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# Sync client http2 option
# ---------------------------------------------------------------------------

class TestSyncClientHttp2:
    def test_client_construction_http2_true(self):
        client = eggfetch.Client(http2=True)
        assert client is not None

    def test_client_construction_http2_false(self):
        client = eggfetch.Client(http2=False)
        assert client is not None

    def test_client_construction_no_http2(self):
        client = eggfetch.Client()
        assert client is not None

    def test_http2_client_makes_request(self, server):
        """Client with http2=True can still make requests (falls back to h1)."""
        client = eggfetch.Client(http2=True)
        r = client.get(f"{server}/hello")
        assert r.status_code == 200
        data = json.loads(r.text)
        assert data["method"] == "GET"
        assert data["path"] == "/hello"

    def test_http2_client_post(self, server):
        client = eggfetch.Client(http2=True)
        r = client.post(f"{server}/echo", content=b"hello")
        assert r.status_code == 200
        data = json.loads(r.text)
        assert data["method"] == "POST"

    def test_http2_client_context_manager(self, server):
        with eggfetch.Client(http2=True) as client:
            r = client.get(f"{server}/hello")
            assert r.status_code == 200


# ---------------------------------------------------------------------------
# Async client http2 option
# ---------------------------------------------------------------------------

class TestAsyncClientHttp2:
    def test_async_client_construction_http2_true(self):
        client = eggfetch.AsyncClient(http2=True)
        assert client is not None

    def test_async_client_construction_http2_false(self):
        client = eggfetch.AsyncClient(http2=False)
        assert client is not None

    @pytest.mark.asyncio
    async def test_async_http2_client_makes_request(self, server):
        async with eggfetch.AsyncClient(http2=True) as client:
            r = await client.get(f"{server}/hello")
            assert r.status_code == 200
            data = json.loads(r.text)
            assert data["method"] == "GET"

    @pytest.mark.asyncio
    async def test_async_http2_client_post(self, server):
        async with eggfetch.AsyncClient(http2=True) as client:
            r = await client.post(f"{server}/echo", content=b"hello")
            assert r.status_code == 200
            data = json.loads(r.text)
            assert data["method"] == "POST"
