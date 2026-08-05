"""Phase 2 Track 5: Response content, encoding, and stream state parity tests."""

import json
import pytest

from eggfetch.compat.httpx import Response
from eggfetch.compat.httpx._exceptions import (
    ResponseNotRead,
    StreamClosed,
    StreamConsumed,
)


class TestStreamExceptions:
    """5.1 Raise HTTPX stream exceptions."""

    def test_content_raises_before_read_streaming(self):
        def gen():
            yield b"data"

        resp = Response(200, stream=gen())
        with pytest.raises(ResponseNotRead):
            _ = resp.content

    def test_text_raises_before_read_streaming(self):
        def gen():
            yield b"data"

        resp = Response(200, stream=gen())
        with pytest.raises(ResponseNotRead):
            _ = resp.text

    def test_json_raises_before_read_streaming(self):
        def gen():
            yield b"data"

        resp = Response(200, stream=gen())
        with pytest.raises(ResponseNotRead):
            resp.json()

    def test_iter_bytes_raises_before_read_streaming(self):
        def gen():
            yield b"data"

        resp = Response(200, stream=gen())
        assert list(resp.iter_bytes()) == [b"data"]

    def test_content_after_read_streaming(self):
        def gen():
            yield b"data"

        resp = Response(200, stream=gen())
        resp.read()
        assert resp.content == b"data"

    def test_content_available_for_buffered(self):
        resp = Response(200, content=b"hello")
        assert resp.content == b"hello"


class TestStreamState:
    """5.2 Expose stream state."""

    def test_is_closed_default_false(self):
        resp = Response(200, content=b"ok")
        assert resp.is_closed

    def test_is_closed_after_close(self):
        resp = Response(200, content=b"ok")
        resp.close()
        assert resp.is_closed

    def test_is_stream_consumed_default_false(self):
        def gen():
            yield b"data"

        resp = Response(200, stream=gen())
        assert not resp.is_stream_consumed

    def test_is_stream_consumed_after_read(self):
        def gen():
            yield b"data"

        resp = Response(200, stream=gen())
        resp.read()
        assert resp.is_stream_consumed

    def test_num_bytes_downloaded_buffered(self):
        resp = Response(200, content=b"hello")
        assert resp.num_bytes_downloaded == 0

    def test_num_bytes_downloaded_after_read(self):
        def gen():
            yield b"chunk1"
            yield b"chunk2"

        resp = Response(200, stream=gen())
        resp.read()
        assert resp.num_bytes_downloaded == 12


class TestRawVsDecodedIteration:
    """5.3 Preserve raw versus decoded iteration."""

    def test_iter_bytes_yields_bytes(self):
        resp = Response(200, content=b"hello world")
        chunks = list(resp.iter_bytes(chunk_size=5))
        assert chunks == [b"hello", b" worl", b"d"]

    def test_iter_text_yields_strings(self):
        resp = Response(200, text="hello world")
        chunks = list(resp.iter_text(chunk_size=5))
        assert chunks == ["hello", " worl", "d"]

    def test_iter_lines(self):
        resp = Response(200, text="line1\nline2\nline3")
        lines = list(resp.iter_lines())
        assert lines == ["line1", "line2", "line3"]

    def test_iter_lines_crlf(self):
        resp = Response(200, text="line1\r\nline2\r\n")
        lines = list(resp.iter_lines())
        assert lines == ["line1", "line2"]

    def test_iter_raw_yields_bytes(self):
        resp = Response(200, stream=iter([b"raw data"]))
        chunks = list(resp.iter_raw(chunk_size=4))
        assert chunks == [b"raw ", b"data"]


class TestEncodingBehavior:
    """5.4 Match encoding behavior."""

    def test_encoding_override(self):
        resp = Response(200, content="héllo".encode("utf-8"))
        resp.encoding = "utf-8"
        assert resp.text == "héllo"

    def test_encoding_charset_from_content_type(self):
        resp = Response(
            200,
            content="héllo".encode("utf-8"),
            headers={"Content-Type": "text/plain; charset=utf-8"},
        )
        assert resp.text == "héllo"

    def test_encoding_setter_raises_after_text_access(self):
        resp = Response(200, content=b"hello")
        _ = resp.text  # Access text first
        with pytest.raises(ValueError, match="cannot be set"):
            resp.encoding = "utf-8"

    def test_default_encoding_callable(self):
        def custom_encoding(content: bytes) -> str:
            return "utf-8"

        resp = Response(200, content=b"hello", default_encoding=custom_encoding)
        assert resp.text == "hello"

    def test_json_decoding(self):
        data = {"key": "value", "num": 42}
        resp = Response(
            200,
            content=json.dumps(data).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        assert resp.json() == data


class TestBodyContentAcrossWrappers:
    """5.5 Preserve body content across wrappers."""

    def test_buffered_content_survives(self):
        resp = Response(200, content=b"buffered data")
        assert resp.content == b"buffered data"
        assert resp.num_bytes_downloaded == 0

    def test_streaming_content_after_read(self):
        def gen():
            yield b"streaming "
            yield b"data"

        resp = Response(200, stream=gen())
        resp.read()
        assert resp.content == b"streaming data"
        assert resp.num_bytes_downloaded == 14

    def test_text_encoding_preserved(self):
        resp = Response(
            200,
            content="héllo wörld".encode("utf-8"),
            headers={"Content-Type": "text/plain; charset=utf-8"},
        )
        assert resp.text == "héllo wörld"

    def test_history_survives(self):
        h1 = Response(301)
        resp = Response(200, content=b"ok", history=[h1])
        assert len(resp.history) == 1
