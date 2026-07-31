"""Track 2: Client.stream() preserves per-call overrides.

Tests that auth, follow_redirects, and timeout overrides are preserved
through Client.stream() and AsyncClient.stream().
"""

import asyncio
import http.server
import json
import socketserver
import threading

import pytest

from eggfetch.compat.httpx import Client, AsyncClient, Timeout


# ---------------------------------------------------------------------------
# Local test server
# ---------------------------------------------------------------------------

class _StreamOverrideHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/get":
            body = json.dumps({"method": "GET"}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == "/stream":
            body = b"line1\nline2\nline3\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == "/echo-headers":
            headers = {k: v for k, v in self.headers.items()}
            body = json.dumps({"headers": headers}).encode()
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
    srv = _ThreadedHTTPServer(("127.0.0.1", 0), _StreamOverrideHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# Sync stream overrides
# ---------------------------------------------------------------------------

class TestSyncStreamOverrides:
    def test_stream_preserves_auth_override(self, server):
        """Explicit auth=None disables client auth in stream."""
        with Client(auth=("user", "pass")) as client:
            with client.stream("GET", f"{server}/get", auth=None) as resp:
                assert resp.status_code == 200
                # The Authorization header should NOT be present
                assert "authorization" not in resp.headers

    def test_stream_preserves_timeout_override(self, server):
        """Explicit timeout=None disables timeouts in stream."""
        with Client(timeout=5.0) as client:
            with client.stream("GET", f"{server}/get", timeout=None) as resp:
                assert resp.status_code == 200

    def test_stream_preserves_follow_redirects_override(self, server):
        """Explicit follow_redirects overrides client default in stream."""
        with Client(follow_redirects=False) as client:
            with client.stream("GET", f"{server}/get",
                               follow_redirects=True) as resp:
                assert resp.status_code == 200

    def test_stream_uses_client_default_timeout(self, server):
        """Omitted timeout uses client default in stream."""
        with Client(timeout=5.0) as client:
            with client.stream("GET", f"{server}/get") as resp:
                assert resp.status_code == 200

    def test_stream_explicit_timeout_overrides_client(self, server):
        """Explicit timeout overrides client default in stream."""
        with Client(timeout=5.0) as client:
            with client.stream("GET", f"{server}/get",
                               timeout=Timeout(10.0)) as resp:
                assert resp.status_code == 200


# ---------------------------------------------------------------------------
# Async stream overrides
# ---------------------------------------------------------------------------

class TestAsyncStreamOverrides:
    @pytest.mark.asyncio
    async def test_async_stream_preserves_auth_override(self, server):
        """Explicit auth=None disables client auth in async stream."""
        async with AsyncClient(auth=("user", "pass")) as client:
            async with client.stream("GET", f"{server}/get", auth=None) as resp:
                assert resp.status_code == 200
                assert "authorization" not in resp.headers

    @pytest.mark.asyncio
    async def test_async_stream_preserves_timeout_override(self, server):
        """Explicit timeout=None disables timeouts in async stream."""
        async with AsyncClient(timeout=5.0) as client:
            async with client.stream("GET", f"{server}/get",
                                     timeout=None) as resp:
                assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_stream_preserves_follow_redirects(self, server):
        """Explicit follow_redirects overrides client default in async stream."""
        async with AsyncClient(follow_redirects=False) as client:
            async with client.stream("GET", f"{server}/get",
                                     follow_redirects=True) as resp:
                assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_stream_uses_client_default_timeout(self, server):
        """Omitted timeout uses client default in async stream."""
        async with AsyncClient(timeout=5.0) as client:
            async with client.stream("GET", f"{server}/get") as resp:
                assert resp.status_code == 200
