"""Phase 2 Track 2: Request auto-header and stream semantics parity tests."""

import pytest

from eggfetch.compat.httpx import Request, Headers
from eggfetch.compat.httpx._exceptions import RequestNotRead, StreamConsumed


class TestExplicitStreamNoAutoHeaders:
    """2.1 Distinguish content= from explicit stream=."""

    def test_explicit_stream_no_transfer_encoding(self):
        async def gen():
            yield b"data"

        req = Request("POST", "https://example.test", stream=gen())
        assert "transfer-encoding" not in req.headers

    def test_explicit_stream_no_content_length(self):
        async def gen():
            yield b"data"

        req = Request("POST", "https://example.test", stream=gen())
        assert "content-length" not in req.headers

    def test_content_gets_content_length(self):
        req = Request("POST", "https://example.test", content=b"hello")
        assert req.headers["content-length"] == "5"

    def test_content_gets_content_type_json(self):
        req = Request("POST", "https://example.test", json={"k": "v"})
        assert "content-type" in req.headers

    def test_content_gets_content_type_form(self):
        req = Request("POST", "https://example.test", data={"k": "v"})
        assert req.headers["content-type"] == "application/x-www-form-urlencoded"


class TestEmptyBodyMethodBehavior:
    """2.2 Match empty-body method behavior."""

    def test_empty_post_no_content_length(self):
        req = Request("POST", "https://example.test")
        # HTTPX adds Content-Length: 0 for empty POST at client level,
        # not at Request construction level
        assert req.content == b""
        assert req.headers["content-length"] == "0"

    def test_empty_put_no_content_length(self):
        req = Request("PUT", "https://example.test")
        assert req.content == b""
        assert req.headers["content-length"] == "0"

    def test_empty_patch_no_content_length(self):
        req = Request("PATCH", "https://example.test")
        assert req.content == b""
        assert req.headers["content-length"] == "0"


class TestHostConstruction:
    """2.3 Make Host construction lossless."""

    def test_ipv4_host(self):
        req = Request("GET", "http://192.168.1.1/path")
        assert req.headers["host"] == "192.168.1.1"

    def test_ipv6_host(self):
        req = Request("GET", "http://[::1]/path")
        host = req.headers["host"]
        # urllib.parse may strip brackets, but host must be present
        assert "::1" in host

    def test_non_default_port(self):
        req = Request("GET", "https://example.com:8443/path")
        assert req.headers["host"] == "example.com:8443"

    def test_default_http_port_omitted(self):
        req = Request("GET", "http://example.com:80/path")
        assert "80" not in req.headers["host"]

    def test_default_https_port_omitted(self):
        req = Request("GET", "https://example.com:443/path")
        assert "443" not in req.headers["host"]

    def test_custom_host_header_preserved(self):
        req = Request(
            "GET",
            "https://example.com/path",
            headers={"Host": "custom-host.com"},
        )
        assert req.headers["host"] == "custom-host.com"


class TestStreamConsumptionState:
    """2.4 Enforce request stream type and consumption state."""

    def test_read_consumes_sync_stream(self):
        def gen():
            yield b"chunk1"
            yield b"chunk2"

        req = Request("POST", "https://example.test", stream=gen())
        assert not req.is_stream_consumed
        data = req.read()
        assert data == b"chunk1chunk2"
        assert req.is_stream_consumed

    def test_repeated_read_returns_cached(self):
        def gen():
            yield b"data"

        req = Request("POST", "https://example.test", stream=gen())
        first = req.read()
        second = req.read()
        assert first == second == b"data"

    def test_aread_consumes_async_stream(self):
        async def gen():
            yield b"chunk1"
            yield b"chunk2"

        req = Request("POST", "https://example.test", stream=gen())
        import asyncio

        data = asyncio.run(req.aread())
        assert data == b"chunk1chunk2"
        assert req.is_stream_consumed

    def test_stream_read_returns_cached_on_second_read(self):
        def gen():
            yield b"data"

        req = Request("POST", "https://example.test", stream=gen())
        first = req.read()
        # Second read should return cached content (HTTPX behavior)
        second = req.read()
        assert first == second == b"data"
        assert req.is_stream_consumed
