"""Lifecycle and resource-cleanup tests for the HTTPX compatibility layer.

These tests verify that Python-level objects (Client, AsyncClient, Response)
clean up correctly: close/aclose idempotency, context manager guarantees,
and proper cleanup after partial/abandoned streams.

These are *not* network-level lifecycle tests (those live in Rust tests);
they exercise the Python wrapper layer only.
"""

import asyncio
import http.server
import socketserver
import threading
import time

import pytest

from eggfetch.compat.httpx import Client, AsyncClient, Response


# ---------------------------------------------------------------------------
# Local test server
# ---------------------------------------------------------------------------

class _LifecycleHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/hello":
            body = b"hello world"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
        elif self.path == "/stream":
            body = b"chunk1\nchunk2\nchunk3\nchunk4\nchunk5\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
        elif self.path == "/slow":
            body = b"".join(f"slow{i}\n".encode() for i in range(10))
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        pass


class _ThreadedHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True


@pytest.fixture(scope="module")
def server():
    srv = _ThreadedHTTPServer(("127.0.0.1", 0), _LifecycleHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# Sync client lifecycle
# ---------------------------------------------------------------------------

class TestSyncClientLifecycle:
    def test_clean_shutdown_after_request(self, server):
        """Client closes cleanly after a normal request."""
        client = Client()
        resp = client.get(f"{server}/hello")
        assert resp.status_code == 200
        client.close()
        assert client.is_closed

    def test_clean_shutdown_after_request_context_manager(self, server):
        """Context manager closes client cleanly after a request."""
        with Client() as client:
            resp = client.get(f"{server}/hello")
            assert resp.status_code == 200
        assert client.is_closed

    def test_close_is_idempotent(self, server):
        """Calling close() multiple times does not raise."""
        client = Client()
        client.get(f"{server}/hello")
        client.close()
        client.close()  # second call
        client.close()  # third call
        assert client.is_closed

    def test_close_without_request(self, server):
        """Client closes cleanly without ever making a request."""
        client = Client()
        client.close()
        assert client.is_closed

    def test_close_after_partial_stream(self, server):
        """Client closes cleanly after reading a partial stream."""
        with Client() as client:
            with client.stream("GET", f"{server}/slow") as resp:
                # Read just one chunk then abandon
                for chunk in resp.iter_bytes():
                    assert len(chunk) > 0
                    break
        assert client.is_closed

    def test_close_after_unread_stream(self, server):
        """Client closes cleanly when a stream response is never read."""
        with Client() as client:
            resp = client.get(f"{server}/stream")
            # Don't read content — let context manager close
        assert client.is_closed

    def test_context_manager_cleans_up_on_exception(self, server):
        """Context manager closes client even when exception occurs."""
        try:
            with Client() as client:
                client.get(f"{server}/hello")
                raise RuntimeError("intentional error")
        except RuntimeError:
            pass
        assert client.is_closed


# ---------------------------------------------------------------------------
# Async client lifecycle
# ---------------------------------------------------------------------------

class TestAsyncClientLifecycle:
    @pytest.mark.asyncio
    async def test_clean_shutdown_after_request(self, server):
        """AsyncClient closes cleanly after a normal request."""
        client = AsyncClient()
        resp = await client.get(f"{server}/hello")
        assert resp.status_code == 200
        await client.close()
        assert client.is_closed

    @pytest.mark.asyncio
    async def test_clean_shutdown_context_manager(self, server):
        """Async context manager closes client cleanly."""
        async with AsyncClient() as client:
            resp = await client.get(f"{server}/hello")
            assert resp.status_code == 200
        assert client.is_closed

    @pytest.mark.asyncio
    async def test_aclose_is_idempotent(self, server):
        """Calling aclose() multiple times does not raise."""
        client = AsyncClient()
        await client.get(f"{server}/hello")
        await client.aclose()
        await client.aclose()  # second call
        await client.aclose()  # third call
        assert client.is_closed

    @pytest.mark.asyncio
    async def test_close_is_idempotent(self, server):
        """Calling close() (async) multiple times does not raise."""
        client = AsyncClient()
        await client.get(f"{server}/hello")
        await client.close()
        await client.close()
        assert client.is_closed

    @pytest.mark.asyncio
    async def test_close_without_request(self, server):
        """AsyncClient closes cleanly without ever making a request."""
        client = AsyncClient()
        await client.close()
        assert client.is_closed

    @pytest.mark.asyncio
    async def test_close_after_partial_stream(self, server):
        """AsyncClient closes cleanly after reading a partial stream."""
        async with AsyncClient() as client:
            async with client.stream("GET", f"{server}/slow") as resp:
                async for chunk in resp.aiter_bytes():
                    assert len(chunk) > 0
                    break
        assert client.is_closed

    @pytest.mark.asyncio
    async def test_close_after_unread_stream(self, server):
        """AsyncClient closes cleanly when a stream response is never read."""
        async with AsyncClient() as client:
            resp = await client.get(f"{server}/stream")
            # Don't read content — let context manager close
        assert client.is_closed

    @pytest.mark.asyncio
    async def test_context_manager_cleans_up_on_exception(self, server):
        """Async context manager closes client even when exception occurs."""
        try:
            async with AsyncClient() as client:
                await client.get(f"{server}/hello")
                raise RuntimeError("intentional error")
        except RuntimeError:
            pass
        assert client.is_closed


# ---------------------------------------------------------------------------
# Response lifecycle
# ---------------------------------------------------------------------------

class TestResponseLifecycle:
    def test_response_close_is_idempotent(self, server):
        """Response.close() can be called multiple times without error."""
        with Client() as client:
            resp = client.get(f"{server}/hello")
            resp.close()
            resp.close()
            resp.close()
            assert resp._is_closed

    @pytest.mark.asyncio
    async def test_response_aclose_is_idempotent(self, server):
        """Response.aclose() can be called multiple times without error."""
        async with AsyncClient() as client:
            resp = await client.get(f"{server}/hello")
            await resp.aclose()
            await resp.aclose()
            await resp.aclose()
            assert resp._is_closed

    def test_streaming_response_close_after_partial_read(self, server):
        """Streaming response closes cleanly after partial read."""
        with Client() as client:
            with client.stream("GET", f"{server}/slow") as resp:
                first = next(resp.iter_bytes())
                assert len(first) > 0
                resp.close()
                resp.close()  # idempotent

    @pytest.mark.asyncio
    async def test_async_streaming_response_aclose_after_partial_read(self, server):
        """Async streaming response acloses cleanly after partial read."""
        async with AsyncClient() as client:
            async with client.stream("GET", f"{server}/slow") as resp:
                first = await resp.aiter_bytes().__anext__()
                assert len(first) > 0
                await resp.aclose()
                await resp.aclose()  # idempotent

    def test_context_manager_stream_closes_response(self, server):
        """stream() context manager closes response on exit."""
        with Client() as client:
            with client.stream("GET", f"{server}/slow") as resp:
                for chunk in resp.iter_bytes():
                    assert len(chunk) > 0
                    break
            assert resp._is_closed

    @pytest.mark.asyncio
    async def test_async_context_manager_stream_closes_response(self, server):
        """async stream() context manager closes response on exit."""
        async with AsyncClient() as client:
            async with client.stream("GET", f"{server}/slow") as resp:
                async for chunk in resp.aiter_bytes():
                    assert len(chunk) > 0
                    break
            assert resp._is_closed


# ---------------------------------------------------------------------------
# Multiple clients — no resource leaks
# ---------------------------------------------------------------------------

class TestMultipleClientLifecycle:
    def test_repeated_create_and_close(self, server):
        """Repeatedly creating and closing clients does not leak."""
        for _ in range(3):
            client = Client(timeout=10.0)
            resp = client.get(f"{server}/hello")
            assert resp.status_code == 200
            client.close()
            time.sleep(0.05)

    @pytest.mark.asyncio
    async def test_repeated_async_create_and_close(self, server):
        """Repeatedly creating and closing async clients does not leak."""
        for _ in range(3):
            client = AsyncClient(timeout=10.0)
            resp = await client.get(f"{server}/hello")
            assert resp.status_code == 200
            await client.close()
            await asyncio.sleep(0.05)
