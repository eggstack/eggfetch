"""Tests for HTTPX compat layer streaming support."""

import http.server
import threading

import pytest

from eggfetch.compat.httpx import Client, AsyncClient, Response, ByteStream, SyncByteStream, AsyncByteStream


class _StreamTestHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/hello":
            body = b"hello world"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
        elif self.path == "/lines":
            body = b"line1\nline2\nline3\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
        elif self.path == "/status":
            body = b"ok"
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
        self.do_GET()

    def log_message(self, format, *args):
        pass


@pytest.fixture(scope="module")
def server():
    srv = http.server.HTTPServer(("127.0.0.1", 0), _StreamTestHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


class TestStreamBaseClasses:
    def test_import_byte_stream(self):
        assert ByteStream is not None

    def test_import_sync_byte_stream(self):
        assert SyncByteStream is not None

    def test_import_async_byte_stream(self):
        assert AsyncByteStream is not None

    def test_byte_stream_yields_content(self):
        stream = ByteStream(b"hello")
        chunks = list(stream)
        assert chunks == [b"hello"]

    def test_byte_stream_empty(self):
        stream = ByteStream()
        chunks = list(stream)
        assert chunks == [b""]

    def test_byte_stream_close(self):
        stream = ByteStream(b"test")
        assert not stream._is_closed
        stream.close()
        assert stream._is_closed

    def test_byte_stream_context_manager(self):
        with ByteStream(b"test") as stream:
            assert not stream._is_closed
        assert stream._is_closed

    def test_sync_byte_stream_inherits(self):
        stream = SyncByteStream(b"test")
        assert isinstance(stream, ByteStream)
        chunks = list(stream)
        assert chunks == [b"test"]

    @pytest.mark.asyncio
    async def test_async_byte_stream_yields_content(self):
        stream = AsyncByteStream(b"hello")
        chunks = []
        async for chunk in stream:
            chunks.append(chunk)
        assert chunks == [b"hello"]

    @pytest.mark.asyncio
    async def test_async_byte_stream_close(self):
        stream = AsyncByteStream(b"test")
        assert not stream._is_closed
        await stream.aclose()
        assert stream._is_closed

    @pytest.mark.asyncio
    async def test_async_byte_stream_context_manager(self):
        async with AsyncByteStream(b"test") as stream:
            assert not stream._is_closed
        assert stream._is_closed


class TestClientStreamSync:
    def test_stream_returns_response(self, server):
        with Client() as client:
            with client.stream("GET", f"{server}/status") as resp:
                assert isinstance(resp, Response)
                assert resp.status_code == 200

    def test_stream_iter_bytes(self, server):
        with Client() as client:
            with client.stream("GET", f"{server}/hello") as resp:
                chunks = list(resp.iter_bytes())
                assert b"".join(chunks) == b"hello world"

    def test_stream_iter_text(self, server):
        with Client() as client:
            with client.stream("GET", f"{server}/hello") as resp:
                chunks = list(resp.iter_text())
                text = "".join(chunks)
                assert "hello world" in text

    def test_stream_iter_lines(self, server):
        with Client() as client:
            with client.stream("GET", f"{server}/lines") as resp:
                lines = list(resp.iter_lines())
                assert lines == ["line1", "line2", "line3"]

    def test_stream_iter_raw(self, server):
        with Client() as client:
            with client.stream("GET", f"{server}/hello") as resp:
                chunks = list(resp.iter_raw())
                assert b"".join(chunks) == b"hello world"

    def test_stream_read(self, server):
        with Client() as client:
            with client.stream("GET", f"{server}/hello") as resp:
                data = resp.read()
                assert data == b"hello world"

    def test_stream_close(self, server):
        with Client() as client:
            with client.stream("GET", f"{server}/hello") as resp:
                resp.close()
                assert resp._is_closed

    def test_stream_context_yields_response(self, server):
        with Client() as client:
            with client.stream("GET", f"{server}/hello") as resp:
                assert not resp._is_closed
                data = resp.read()
                assert data == b"hello world"


class TestClientStreamAsync:
    @pytest.mark.asyncio
    async def test_async_stream_returns_response(self, server):
        async with AsyncClient() as client:
            async with client.stream("GET", f"{server}/status") as resp:
                assert isinstance(resp, Response)
                assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_stream_aiter_bytes(self, server):
        async with AsyncClient() as client:
            async with client.stream("GET", f"{server}/hello") as resp:
                chunks = []
                async for chunk in resp.aiter_bytes():
                    chunks.append(chunk)
                assert b"".join(chunks) == b"hello world"

    @pytest.mark.asyncio
    async def test_async_stream_aiter_text(self, server):
        async with AsyncClient() as client:
            async with client.stream("GET", f"{server}/hello") as resp:
                chunks = []
                async for chunk in resp.aiter_text():
                    chunks.append(chunk)
                text = "".join(chunks)
                assert "hello world" in text

    @pytest.mark.asyncio
    async def test_async_stream_aiter_lines(self, server):
        async with AsyncClient() as client:
            async with client.stream("GET", f"{server}/lines") as resp:
                lines = []
                async for line in resp.aiter_lines():
                    lines.append(line)
                assert lines == ["line1", "line2", "line3"]

    @pytest.mark.asyncio
    async def test_async_stream_aiter_raw(self, server):
        async with AsyncClient() as client:
            async with client.stream("GET", f"{server}/hello") as resp:
                chunks = []
                async for chunk in resp.aiter_raw():
                    chunks.append(chunk)
                assert b"".join(chunks) == b"hello world"

    @pytest.mark.asyncio
    async def test_async_stream_aread(self, server):
        async with AsyncClient() as client:
            async with client.stream("GET", f"{server}/hello") as resp:
                data = await resp.aread()
                assert data == b"hello world"

    @pytest.mark.asyncio
    async def test_async_stream_aclose(self, server):
        async with AsyncClient() as client:
            async with client.stream("GET", f"{server}/hello") as resp:
                await resp.aclose()
                assert resp._is_closed
