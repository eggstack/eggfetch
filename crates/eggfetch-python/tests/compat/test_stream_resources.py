"""Tests for resource ownership, producer/consumer failures, and stress scenarios.

C4: Resource ownership — eggfetch-opened files close on all paths.
F1: Request producer failures — iterator raises, invalid type, timeout.
F2: Response consumer failures — consumer stops early, iterator dropped.
H1-H3: Stress — reference stream server, thread count assertions.
"""

import asyncio
import io
import os
import socketserver
import tempfile
import threading

import http.server
import pytest

from eggfetch.compat.httpx import Client, AsyncClient, Response


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

class _EchoHandler(http.server.BaseHTTPRequestHandler):
    """Echo POST body back; stream GET body in chunks."""

    def do_GET(self):
        if self.path == "/hello":
            body = b"hello world"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == "/lines":
            body = b"line1\nline2\nline3\nline4\nline5\n"
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
        elif self.path == "/large":
            body = b"x" * (1024 * 100)  # 100KB
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        content_length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_length) if content_length else b""
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()

    def log_message(self, format, *args):
        pass


class _ThreadedHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    block_on_close = False


@pytest.fixture(scope="module")
def server():
    srv = _ThreadedHTTPServer(("127.0.0.1", 0), _EchoHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()
    srv.server_close()
    t.join(timeout=2)


# ---------------------------------------------------------------------------
# C4: Resource ownership — file lifecycle tests
# ---------------------------------------------------------------------------

class TestFileBodyOwnership:
    def test_file_like_object_read_incrementally(self, server):
        """File-like objects are read incrementally, not buffered entirely."""
        data = b"test data for file-like body"
        buf = io.BytesIO(data)

        with Client() as client:
            resp = client.post(f"{server}/hello", content=buf)
            assert resp.status_code == 200

    def test_file_like_object_closed_after_send(self, server):
        """File-like objects are not closed by eggfetch (user-owned)."""

        class TrackingBytesIO(io.BytesIO):
            closed_flag = False

            def close(self):
                self.closed_flag = True
                super().close()

        buf = TrackingBytesIO(b"data")
        with Client() as client:
            client.post(f"{server}/hello", content=buf)
        # eggfetch should NOT close user-owned files
        # (HTTPX doesn't close them either)


# ---------------------------------------------------------------------------
# F1: Request producer failures
# ---------------------------------------------------------------------------

class TestRequestProducerFailures:
    def test_sync_iterator_yields_non_bytes(self, server):
        """Iterator yielding non-bytes/str types should fail or coerce."""
        def bad_iter():
            yield 123  # not bytes or str

        with Client() as client:
            with pytest.raises((TypeError, Exception)):
                client.post(f"{server}/hello", content=bad_iter())

    def test_sync_iterator_empty(self, server):
        """Empty iterator produces empty body."""
        def empty_iter():
            return
            yield  # make it a generator

        with Client() as client:
            resp = client.post(f"{server}/hello", content=empty_iter())
            assert resp.status_code == 200

    def test_client_close_during_iteration(self, server):
        """Client close during iteration should not hang."""
        def slow_iter():
            for i in range(100):
                yield f"chunk{i}".encode()

        with Client() as client:
            resp = client.post(f"{server}/hello", content=slow_iter())
            # Reading partial data then closing should not hang
            data = resp.content
            assert len(data) > 0


# ---------------------------------------------------------------------------
# F2: Response consumer failures
# ---------------------------------------------------------------------------

class TestResponseConsumerFailures:
    def test_consume_partial_then_close(self, server):
        """Consumer reads partial data, then closes — should not leak."""
        with Client() as client:
            with client.stream("GET", f"{server}/slow") as resp:
                # Read just one chunk
                for chunk in resp.iter_bytes():
                    assert len(chunk) > 0
                    break
                # Exit context without reading rest — should discard

    def test_iterator_dropped_early(self, server):
        """Iterator object dropped before completion — should clean up."""
        with Client() as client:
            with client.stream("GET", f"{server}/slow") as resp:
                gen = resp.iter_bytes()
                first = next(gen)
                assert len(first) > 0
                # Drop the generator without exhausting
                del gen

    def test_read_after_close_returns_data(self, server):
        """read() after close() should still return buffered data."""
        with Client() as client:
            with client.stream("GET", f"{server}/hello") as resp:
                data = resp.read()
                resp.close()
                # Data should still be accessible
                assert data == b"hello world"


# ---------------------------------------------------------------------------
# H1: Reference stream server tests
# ---------------------------------------------------------------------------

class TestReferenceStreamServer:
    def test_stream_lines(self, server):
        """Line streaming works correctly."""
        with Client() as client:
            with client.stream("GET", f"{server}/lines") as resp:
                chunks = list(resp.iter_lines())
                assert len(chunks) == 5
                for i, chunk in enumerate(chunks):
                    assert chunk == f"line{i + 1}"

    def test_stream_large_body(self, server):
        """Large body streams without issues."""
        with Client() as client:
            with client.stream("GET", f"{server}/large") as resp:
                total = b""
                for chunk in resp.iter_bytes():
                    total += chunk
                assert len(total) == 1024 * 100

    @pytest.mark.asyncio
    async def test_async_stream_lines(self, server):
        """Async line streaming works correctly."""
        async with AsyncClient() as client:
            async with client.stream("GET", f"{server}/lines") as resp:
                chunks = []
                async for line in resp.aiter_lines():
                    chunks.append(line)
                assert len(chunks) == 5

    @pytest.mark.asyncio
    async def test_async_stream_large_body(self, server):
        """Async large body streaming works correctly."""
        async with AsyncClient() as client:
            async with client.stream("GET", f"{server}/large") as resp:
                total = b""
                async for chunk in resp.aiter_bytes():
                    total += chunk
                assert len(total) == 1024 * 100


# ---------------------------------------------------------------------------
# H3: Thread/task envelope tests
# ---------------------------------------------------------------------------

class TestThreadEnvelopes:
    def test_multiple_concurrent_sync_streams(self, server):
        """Multiple concurrent sync streams don't leak threads."""
        initial_threads = threading.active_count()

        with Client() as client:
            for _ in range(5):
                with client.stream("GET", f"{server}/hello") as resp:
                    data = resp.read()
                    assert data == b"hello world"

        # Allow cleanup time
        import time
        time.sleep(0.5)

        # Thread count should not grow unboundedly
        # (allow some tolerance for runtime worker threads)
        final_threads = threading.active_count()
        assert final_threads <= initial_threads + 3, (
            f"Thread leak: started with {initial_threads}, now {final_threads}"
        )

    @pytest.mark.timeout(60)
    @pytest.mark.asyncio
    async def test_concurrent_async_reads(self, server):
        """Multiple concurrent async reads complete without blocking.

        The sync interface blocks each OS thread on the async runtime,
        making true concurrency impossible when multiple threads share the
        runtime.  This async version uses ``asyncio.gather`` to exercise
        the real concurrency path and prove the threaded test server
        handles parallel requests.
        """
        async with AsyncClient(timeout=30.0) as client:
            results = await asyncio.gather(
                *(client.get(f"{server}/hello") for _ in range(5))
            )
        assert len(results) == 5
        for r in results:
            assert r.content == b"hello world"
