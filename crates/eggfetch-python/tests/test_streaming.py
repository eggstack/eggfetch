"""Tests for the eggfetch Python streaming response API."""

import http.server
import json
import threading
import time

import pytest

import eggfetch


# ---------------------------------------------------------------------------
# Local test server
# ---------------------------------------------------------------------------

class _StreamingHandler(http.server.BaseHTTPRequestHandler):
    """Test server that supports streaming endpoints."""

    def do_GET(self):
        if self.path == "/stream-bytes":
            body = b"hello world"
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()

        elif self.path == "/stream-text":
            body = b"line1\nline2\nline3\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()

        elif self.path == "/stream-lines":
            body = b"first\nsecond\nthird\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()

        elif self.path == "/status":
            body = b"ok"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("X-Status", "ok")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()

        elif self.path == "/no-newline":
            body = b"no newline here"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()

        elif self.path == "/json-stream":
            body = b'{"key":"value"}'
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
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
    srv = http.server.HTTPServer(("127.0.0.1", 0), _StreamingHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

class TestStreamingResponseBasics:
    def test_import(self):
        assert hasattr(eggfetch, "StreamingResponse")

    def test_stream_returns_streaming_response(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/status")
            assert isinstance(resp, eggfetch.StreamingResponse)
            assert resp.status_code == 200

    def test_stream_url(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/status")
            assert resp.url == f"{server}/status"

    def test_stream_headers(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/status")
            assert "content-type" in resp.headers

    def test_stream_is_success(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/status")
            assert resp.is_success

    def test_stream_encoding(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/stream-text")
            assert resp.encoding == "utf-8"

    def test_stream_context_manager(self, server):
        with eggfetch.Client() as client:
            with client.stream("GET", f"{server}/status") as resp:
                assert resp.status_code == 200

    def test_stream_repr(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/status")
            assert "StreamingResponse" in repr(resp)
            assert "200" in repr(resp)


class TestSyncIterBytes:
    def test_iter_bytes_yields_chunks(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/stream-bytes")
            chunks = list(resp.iter_bytes())
            assert len(chunks) > 0
            combined = b"".join(chunks)
            assert combined == b"hello world"

    def test_iter_bytes_type(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/stream-bytes")
            for chunk in resp.iter_bytes():
                assert isinstance(chunk, bytes)


class TestSyncIterText:
    def test_iter_text_yields_strings(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/stream-text")
            chunks = list(resp.iter_text())
            assert len(chunks) > 0
            combined = "".join(chunks)
            assert "line1" in combined
            assert "line2" in combined

    def test_iter_text_type(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/stream-text")
            for chunk in resp.iter_text():
                assert isinstance(chunk, str)


class TestSyncIterLines:
    def test_iter_lines_yields_complete_lines(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/stream-lines")
            lines = list(resp.iter_lines())
            assert lines == ["first", "second", "third"]

    def test_iter_lines_strips_newlines(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/stream-text")
            lines = list(resp.iter_lines())
            assert lines == ["line1", "line2", "line3"]

    def test_iter_lines_no_trailing_newline(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/no-newline")
            lines = list(resp.iter_lines())
            assert lines == ["no newline here"]

    def test_iter_lines_type(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/stream-lines")
            for line in resp.iter_lines():
                assert isinstance(line, str)


class TestRead:
    def test_read_returns_bytes(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/stream-bytes")
            data = resp.read()
            assert isinstance(data, bytes)
            assert data == b"hello world"

    def test_text_returns_string(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/stream-text")
            data = resp.text()
            assert isinstance(data, str)
            assert "line1" in data


class TestContextManager:
    def test_sync_context_manager_drains(self, server):
        with eggfetch.Client() as client:
            with client.stream("GET", f"{server}/stream-bytes") as resp:
                assert resp.status_code == 200
                data = resp.read()
                assert data == b"hello world"

    def test_sync_context_manager_closes_on_exception(self, server):
        with eggfetch.Client() as client:
            try:
                with client.stream("GET", f"{server}/stream-bytes") as resp:
                    raise ValueError("test exception")
            except ValueError:
                pass


class TestStreamMethod:
    def test_stream_get(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/status")
            assert resp.status_code == 200

    def test_stream_post(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("POST", f"{server}/status")
            assert resp.status_code == 200

    def test_stream_with_headers(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/status", headers={"X-Test": "value"})
            assert resp.status_code == 200


# ---------------------------------------------------------------------------
# Async streaming tests
# ---------------------------------------------------------------------------


class TestAsyncStreamingResponseBasics:
    async def test_async_stream_returns_streaming_response(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/status")
            assert isinstance(resp, eggfetch.StreamingResponse)
            assert resp.status_code == 200

    async def test_async_stream_url(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/status")
            assert resp.url == f"{server}/status"

    async def test_async_stream_headers(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/status")
            assert "content-type" in resp.headers

    async def test_async_stream_is_success(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/status")
            assert resp.is_success

    async def test_async_stream_encoding(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/stream-text")
            assert resp.encoding == "utf-8"

    async def test_async_stream_context_manager(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/status")
            async with resp:
                assert resp.status_code == 200

    async def test_async_stream_repr(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/status")
            assert "StreamingResponse" in repr(resp)
            assert "200" in repr(resp)


class TestAsyncIterBytes:
    async def test_aiter_bytes_yields_chunks(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/stream-bytes")
            async with resp:
                chunks = []
                async for chunk in resp.aiter_bytes():
                    chunks.append(chunk)
                assert len(chunks) > 0
                combined = b"".join(chunks)
                assert combined == b"hello world"

    async def test_aiter_bytes_type(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/stream-bytes")
            async with resp:
                async for chunk in resp.aiter_bytes():
                    assert isinstance(chunk, bytes)


class TestAsyncIterText:
    async def test_aiter_text_yields_strings(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/stream-text")
            async with resp:
                chunks = []
                async for chunk in resp.aiter_text():
                    chunks.append(chunk)
                assert len(chunks) > 0
                combined = "".join(chunks)
                assert "line1" in combined
                assert "line2" in combined

    async def test_aiter_text_type(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/stream-text")
            async with resp:
                async for chunk in resp.aiter_text():
                    assert isinstance(chunk, str)


class TestAsyncIterLines:
    async def test_aiter_lines_yields_complete_lines(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/stream-lines")
            async with resp:
                lines = []
                async for line in resp.aiter_lines():
                    lines.append(line)
                assert lines == ["first", "second", "third"]

    async def test_aiter_lines_strips_newlines(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/stream-text")
            async with resp:
                lines = []
                async for line in resp.aiter_lines():
                    lines.append(line)
                assert lines == ["line1", "line2", "line3"]

    async def test_aiter_lines_no_trailing_newline(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/no-newline")
            async with resp:
                lines = []
                async for line in resp.aiter_lines():
                    lines.append(line)
                assert lines == ["no newline here"]

    async def test_aiter_lines_type(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/stream-lines")
            async with resp:
                async for line in resp.aiter_lines():
                    assert isinstance(line, str)


class TestAsyncRead:
    async def test_aread_returns_bytes(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/stream-bytes")
            async with resp:
                data = await resp.aread()
                assert isinstance(data, bytes)
                assert data == b"hello world"

    async def test_async_text_returns_string(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/stream-text")
            async with resp:
                data = await resp.aread()
                assert isinstance(data, bytes)
                text = resp.text()
                assert isinstance(text, str)
                assert "line1" in text


class TestAsyncContextManager:
    async def test_async_context_manager_drains(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/stream-bytes")
            async with resp:
                assert resp.status_code == 200
                data = await resp.aread()
                assert data == b"hello world"

    async def test_async_context_manager_closes_on_exception(self, server):
        async with eggfetch.AsyncClient() as client:
            try:
                resp = await client.stream("GET", f"{server}/stream-bytes")
                async with resp:
                    raise ValueError("test exception")
            except ValueError:
                pass


class TestAsyncStreamMethod:
    async def test_async_stream_get(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/status")
            assert resp.status_code == 200

    async def test_async_stream_post(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("POST", f"{server}/status")
            assert resp.status_code == 200

    async def test_async_stream_with_headers(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream(
                "GET", f"{server}/status", headers={"X-Test": "value"}
            )
            assert resp.status_code == 200


class TestAsyncStreamingErrors:
    async def test_async_stream_after_body_consumed(self, server):
        async with eggfetch.AsyncClient() as client:
            resp = await client.stream("GET", f"{server}/stream-bytes")
            async with resp:
                await resp.aread()
                with pytest.raises(ValueError):
                    [c async for c in resp.aiter_bytes()]

    async def test_async_stream_closed_client(self, server):
        client = eggfetch.AsyncClient()
        client.close()
        with pytest.raises(ValueError, match="closed"):
            await client.stream("GET", f"{server}/status")


# ---------------------------------------------------------------------------
# Sync-only error tests
# ---------------------------------------------------------------------------


class TestStreamingErrors:
    def test_stream_after_body_consumed(self, server):
        with eggfetch.Client() as client:
            resp = client.stream("GET", f"{server}/stream-bytes")
            resp.read()
            with pytest.raises(ValueError):
                list(resp.iter_bytes())

    def test_stream_closed_client(self, server):
        client = eggfetch.Client()
        client.close()
        with pytest.raises(ValueError, match="closed"):
            client.stream("GET", f"{server}/status")
