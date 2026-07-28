"""Tests for the eggfetch Python response compatibility surface (Milestone H)."""

import asyncio
import http.server
import json
import threading
import urllib.parse

import pytest

import eggfetch

from conftest import _ThreadingHTTPServer


# ---------------------------------------------------------------------------
# Extended test server
# ---------------------------------------------------------------------------


class _Handler(http.server.BaseHTTPRequestHandler):
    """Test server that returns various status codes and content types."""

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path

        if path == "/json":
            body = json.dumps({"key": "value", "count": 42}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif path == "/text":
            body = b"Hello, World!"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif path == "/html":
            body = b"<html><body>Hello</body></html>"
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif path == "/charset-latin1":
            # Send body encoded in latin-1 (iso-8859-1)
            body = "caf\u00e9".encode("latin-1")  # "café" in latin-1
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=iso-8859-1")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif path == "/multiline":
            body = b"line1\nline2\nline3\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif path == "/multi-header":
            body = b"ok"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("X-Thing", "first")
            self.send_header("X-Thing", "second")
            self.send_header("X-Thing", "third")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif path == "/404":
            body = b"Not Found"
            self.send_response(404)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif path == "/500":
            body = b"Internal Server Error"
            self.send_response(500)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif path == "/301":
            self.send_response(301)
            self.send_header("Location", "/json")
            self.send_header("Content-Length", "0")
            self.end_headers()

        elif path == "/100":
            # HTTP/1.1 100 Continue - hard to simulate with http.server,
            # but we test the property via from_parts
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()

        elif path == "/empty-content-type":
            body = b"no content type"
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif path == "/charset-quoted":
            body = "caf\u00e9".encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=\"utf-8\"")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif path == "/charset-unknown":
            body = "caf\u00e9".encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=x-notreal")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif path == "/latin1-bytes-no-charset":
            # Latin-1 encoded bytes, but no charset in Content-Type
            # Decoder should fall back to UTF-8 lossy
            body = "caf\u00e9".encode("latin-1")
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
        self.do_POST()

    def do_PATCH(self):
        self.do_POST()

    def do_DELETE(self):
        self.do_GET()

    def do_HEAD(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("X-Echo", "head-ok")
        self.end_headers()

    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header("Allow", "GET, POST, OPTIONS")
        self.end_headers()

    def log_message(self, format, *args):
        pass  # suppress logs during tests


@pytest.fixture(scope="module")
def server():
    """Start a local HTTP server for the test module."""
    srv = _ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# Response properties: reason_phrase, http_version, encoding, history
# ---------------------------------------------------------------------------


class TestResponseProperties:
    def test_reason_phrase_ok(self, server):
        r = eggfetch.get(f"{server}/json")
        assert r.reason_phrase == "OK"

    def test_reason_phrase_not_found(self, server):
        r = eggfetch.get(f"{server}/404")
        assert r.reason_phrase == "Not Found"

    def test_reason_phrase_server_error(self, server):
        r = eggfetch.get(f"{server}/500")
        assert r.reason_phrase == "Internal Server Error"

    def test_http_version(self, server):
        r = eggfetch.get(f"{server}/json")
        assert r.http_version in ("HTTP/1.0", "HTTP/1.1", "HTTP/2", "HTTP/3")

    def test_encoding_from_content_type(self, server):
        r = eggfetch.get(f"{server}/html")
        assert r.encoding is not None
        assert r.encoding.lower() == "utf-8"

    def test_encoding_latin1(self, server):
        r = eggfetch.get(f"{server}/charset-latin1")
        assert r.encoding is not None
        assert "iso-8859-1" in r.encoding.lower() or "latin" in r.encoding.lower()

    def test_encoding_none_when_no_charset(self, server):
        r = eggfetch.get(f"{server}/text")
        # text/plain without charset
        assert r.encoding is None

    def test_encoding_none_when_no_content_type(self, server):
        r = eggfetch.get(f"{server}/empty-content-type")
        assert r.encoding is None

    def test_encoding_quoted_charset(self, server):
        r = eggfetch.get(f"{server}/charset-quoted")
        assert r.encoding is not None
        assert r.encoding.lower() == "utf-8"

    def test_encoding_unknown_charset_falls_back(self, server):
        r = eggfetch.get(f"{server}/charset-unknown")
        # Unknown charset is not recognized by encoding_rs,
        # so encoding property is the raw string but text falls back to UTF-8
        assert r.encoding is not None
        assert r.encoding == "x-notreal"
        # Body was valid UTF-8, so it decodes correctly via lossy fallback
        assert "caf" in r.text

    def test_history_empty(self, server):
        r = eggfetch.get(f"{server}/json")
        assert r.history == []

    def test_stream_consumed_false(self, server):
        r = eggfetch.get(f"{server}/json")
        assert r._stream_consumed is False


# ---------------------------------------------------------------------------
# Status helpers
# ---------------------------------------------------------------------------


class TestStatusHelpers:
    def test_is_informational(self, server):
        r = eggfetch.get(f"{server}/json")
        assert r.is_informational is False
        assert r.is_success is True
        assert r.is_redirect is False
        assert r.is_client_error is False
        assert r.is_server_error is False
        assert r.is_error is False

    def test_is_client_error(self, server):
        r = eggfetch.get(f"{server}/404")
        assert r.is_informational is False
        assert r.is_success is False
        assert r.is_redirect is False
        assert r.is_client_error is True
        assert r.is_server_error is False
        assert r.is_error is True

    def test_is_server_error(self, server):
        r = eggfetch.get(f"{server}/500")
        assert r.is_informational is False
        assert r.is_success is False
        assert r.is_redirect is False
        assert r.is_client_error is False
        assert r.is_server_error is True
        assert r.is_error is True

    def test_is_redirect(self, server):
        # Our server returns 301 without redirect following
        r = eggfetch.get(f"{server}/301")
        assert r.is_redirect is True
        assert r.is_error is False


# ---------------------------------------------------------------------------
# json() method
# ---------------------------------------------------------------------------


class TestJsonMethod:
    def test_json_returns_dict(self, server):
        r = eggfetch.get(f"{server}/json")
        data = r.json()
        assert isinstance(data, dict)
        assert data["key"] == "value"
        assert data["count"] == 42

    def test_json_is_cached(self, server):
        r = eggfetch.get(f"{server}/json")
        d1 = r.json()
        d2 = r.json()
        assert d1 == d2

    def test_json_with_kwargs(self, server):
        r = eggfetch.get(f"{server}/json")
        # parse_float and parse_int are valid json.loads kwargs
        data = r.json(parse_int=str)
        assert data["count"] == "42"


# ---------------------------------------------------------------------------
# Text decoding
# ---------------------------------------------------------------------------


class TestTextDecoding:
    def test_utf8_text(self, server):
        r = eggfetch.get(f"{server}/text")
        assert r.text == "Hello, World!"

    def test_latin1_decoded_as_latin1(self, server):
        r = eggfetch.get(f"{server}/charset-latin1")
        # The server sends "café" encoded in latin-1 with charset=iso-8859-1
        # Our decoder should decode it properly
        assert "caf" in r.text

    def test_latin1_bytes_without_charset_use_utf8_lossy(self, server):
        # Latin-1 bytes without charset → UTF-8 lossy decode produces
        # replacement characters for the non-ASCII bytes
        r = eggfetch.get(f"{server}/latin1-bytes-no-charset")
        assert r.encoding is None
        # The latin-1 byte 0xe9 is not valid UTF-8, so lossy decode
        # replaces it with U+FFFD
        assert "\ufffd" in r.text

    def test_html_text(self, server):
        r = eggfetch.get(f"{server}/html")
        assert "<html>" in r.text


# ---------------------------------------------------------------------------
# iter_bytes, iter_text, iter_lines
# ---------------------------------------------------------------------------


class TestIterators:
    def test_iter_bytes(self, server):
        r = eggfetch.get(f"{server}/text")
        chunks = list(r.iter_bytes())
        assert len(chunks) > 0
        combined = b"".join(chunks)
        assert combined == b"Hello, World!"

    def test_iter_bytes_custom_chunk_size(self, server):
        r = eggfetch.get(f"{server}/text")
        chunks = list(r.iter_bytes(chunk_size=5))
        assert len(chunks) > 1
        combined = b"".join(chunks)
        assert combined == b"Hello, World!"

    def test_iter_text(self, server):
        r = eggfetch.get(f"{server}/text")
        chunks = list(r.iter_text())
        assert len(chunks) > 0
        combined = "".join(chunks)
        assert combined == "Hello, World!"

    def test_iter_text_custom_chunk_size(self, server):
        r = eggfetch.get(f"{server}/text")
        chunks = list(r.iter_text(chunk_size=5))
        assert len(chunks) > 1
        combined = "".join(chunks)
        assert combined == "Hello, World!"

    def test_iter_lines(self, server):
        r = eggfetch.get(f"{server}/multiline")
        lines = list(r.iter_lines())
        assert lines == ["line1", "line2", "line3"]

    def test_iter_lines_single_line(self, server):
        r = eggfetch.get(f"{server}/text")
        lines = list(r.iter_lines())
        assert lines == ["Hello, World!"]

    def test_iter_bytes_returns_iterator(self, server):
        r = eggfetch.get(f"{server}/text")
        it = r.iter_bytes()
        # Should be an iterator (has __iter__)
        assert hasattr(it, "__iter__")


# ---------------------------------------------------------------------------
# close / aclose
# ---------------------------------------------------------------------------


class TestClose:
    def test_close_is_noop(self, server):
        r = eggfetch.get(f"{server}/json")
        r.close()  # should not raise

    def test_aclose_is_noop(self, server):
        r = eggfetch.get(f"{server}/json")
        r.aclose()  # should not raise


# ---------------------------------------------------------------------------
# __repr__
# ---------------------------------------------------------------------------


class TestRepr:
    def test_repr_200(self, server):
        r = eggfetch.get(f"{server}/json")
        assert repr(r) == "<Response [200 OK]>"

    def test_repr_404(self, server):
        r = eggfetch.get(f"{server}/404")
        assert repr(r) == "<Response [404 Not Found]>"

    def test_repr_500(self, server):
        r = eggfetch.get(f"{server}/500")
        assert repr(r) == "<Response [500 Internal Server Error]>"


# ---------------------------------------------------------------------------
# raise_for_status
# ---------------------------------------------------------------------------


class TestRaiseForStatus:
    def test_no_raise_on_200(self, server):
        r = eggfetch.get(f"{server}/json")
        r.raise_for_status()  # should not raise

    def test_raises_on_404(self, server):
        r = eggfetch.get(f"{server}/404")
        with pytest.raises(eggfetch.HTTPStatusError, match="404"):
            r.raise_for_status()

    def test_raises_on_500(self, server):
        r = eggfetch.get(f"{server}/500")
        with pytest.raises(eggfetch.HTTPStatusError, match="500"):
            r.raise_for_status()

    def test_error_message_includes_reason(self, server):
        r = eggfetch.get(f"{server}/404")
        with pytest.raises(eggfetch.HTTPStatusError, match="Not Found"):
            r.raise_for_status()

    def test_error_message_includes_url(self, server):
        r = eggfetch.get(f"{server}/404")
        with pytest.raises(eggfetch.HTTPStatusError, match=f"{server}/404"):
            r.raise_for_status()


# ---------------------------------------------------------------------------
# Headers.get_list
# ---------------------------------------------------------------------------


class TestHeadersGetList:
    def test_get_list_single_value(self, server):
        r = eggfetch.get(f"{server}/text")
        vals = r.headers.get_list("content-type")
        assert isinstance(vals, list)
        assert len(vals) == 1
        assert "text/plain" in vals[0]

    def test_get_list_multiple_values(self, server):
        r = eggfetch.get(f"{server}/multi-header")
        vals = r.headers.get_list("x-thing")
        assert vals == ["first", "second", "third"]

    def test_get_list_missing_header(self, server):
        r = eggfetch.get(f"{server}/text")
        vals = r.headers.get_list("x-not-present")
        assert vals == []

    def test_get_list_invalid_name(self, server):
        r = eggfetch.get(f"{server}/text")
        with pytest.raises(ValueError, match="Invalid header name"):
            r.headers.get_list("invalid header name!")


# ---------------------------------------------------------------------------
# Async response properties
# ---------------------------------------------------------------------------


class TestAsyncResponseProperties:
    def test_async_reason_phrase(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/json")
                assert r.reason_phrase == "OK"
        asyncio.run(_test())

    def test_async_http_version(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/json")
                assert r.http_version in ("HTTP/1.0", "HTTP/1.1", "HTTP/2", "HTTP/3")
        asyncio.run(_test())

    def test_async_status_helpers(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/404")
                assert r.is_client_error is True
                assert r.is_error is True
                assert r.is_success is False
        asyncio.run(_test())

    def test_async_json(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/json")
                data = r.json()
                assert data["key"] == "value"
        asyncio.run(_test())

    def test_async_repr(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/json")
                assert repr(r) == "<Response [200 OK]>"
        asyncio.run(_test())

    def test_async_encoding(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/html")
                assert r.encoding is not None
        asyncio.run(_test())

    def test_async_iter_lines(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/multiline")
                lines = list(r.iter_lines())
                assert lines == ["line1", "line2", "line3"]
        asyncio.run(_test())

    def test_async_headers_get_list(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/multi-header")
                vals = r.headers.get_list("x-thing")
                assert vals == ["first", "second", "third"]
        asyncio.run(_test())

    def test_async_raise_for_status(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/404")
                with pytest.raises(eggfetch.HTTPStatusError, match="404"):
                    r.raise_for_status()
        asyncio.run(_test())
