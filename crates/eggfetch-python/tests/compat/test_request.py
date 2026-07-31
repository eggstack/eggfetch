"""Tests for Request compatibility."""

import json
import asyncio

import pytest

from eggfetch.compat.httpx import Request, URL, Headers, QueryParams, Cookies


class TestRequestConstruction:
    def test_method_and_url(self):
        req = Request("GET", "https://example.com/path")
        assert req.method == "GET"
        assert str(req.url) == "https://example.com/path"

    def test_method_uppercased(self):
        req = Request("get", "https://example.com")
        assert req.method == "GET"

    def test_url_object(self):
        url = URL("https://example.com/path")
        req = Request("GET", url)
        assert req.url is url

    def test_empty_body(self):
        req = Request("GET", "https://example.com")
        assert req.content is None


class TestBodyMutualExclusion:
    def test_content_only(self):
        req = Request("POST", "https://example.com", content=b"hello")
        assert req.content == b"hello"

    def test_json_only(self):
        req = Request("POST", "https://example.com", json={"key": "value"})
        assert req.content == json.dumps({"key": "value"}, separators=(",", ":"), ensure_ascii=False).encode()

    def test_data_dict(self):
        req = Request("POST", "https://example.com", data={"key": "value"})
        assert req.content is not None

    def test_data_string(self):
        req = Request("POST", "https://example.com", data="raw body")
        assert req.content == b"raw body"

    def test_content_and_json_raises(self):
        with pytest.raises(ValueError, match="mutually exclusive"):
            Request("POST", "https://example.com", content=b"body", json={"k": "v"})

    def test_content_and_data_raises(self):
        with pytest.raises(ValueError, match="mutually exclusive"):
            Request("POST", "https://example.com", content=b"body", data={"k": "v"})


class TestAutoHeaders:
    def test_host_header(self):
        req = Request("GET", "https://example.com:8443/path")
        assert req.headers["host"] == "example.com:8443"

    def test_host_default_port(self):
        req = Request("GET", "https://example.com/path")
        assert req.headers["host"] == "example.com"

    def test_content_length_json(self):
        req = Request("POST", "https://example.com", json={"k": "v"})
        assert "content-length" in req.headers

    def test_content_type_json(self):
        req = Request("POST", "https://example.com", json={"k": "v"})
        assert req.headers["content-type"] == "application/json"

    def test_content_type_form(self):
        req = Request("POST", "https://example.com", data={"k": "v"})
        assert req.headers["content-type"] == "application/x-www-form-urlencoded"

    def test_content_type_not_overwritten(self):
        req = Request(
            "POST",
            "https://example.com",
            json={"k": "v"},
            headers={"Content-Type": "application/custom"},
        )
        assert req.headers["content-type"] == "application/custom"

    def test_no_transfer_encoding_for_explicit_stream(self):
        async def gen():
            yield b"chunk"

        req = Request("POST", "https://example.com", stream=gen())
        assert "transfer-encoding" not in req.headers


class TestParamsMerging:
    def test_params_object(self):
        qp = QueryParams({"q": "test"})
        req = Request("GET", "https://example.com", params=qp)
        assert req.params["q"] == "test"

    def test_params_dict(self):
        req = Request("GET", "https://example.com", params={"q": "test"})
        assert req.params["q"] == "test"


class TestRead:
    def test_read_content(self):
        req = Request("POST", "https://example.com", content=b"hello")
        assert req.read() == b"hello"

    def test_read_empty(self):
        req = Request("GET", "https://example.com")
        assert req.read() == b""

    def test_read_stream(self):
        def gen():
            yield b"chunk1"
            yield b"chunk2"

        req = Request("POST", "https://example.com", stream=gen())
        assert req.read() == b"chunk1chunk2"
        assert req.is_stream_consumed

    @pytest.mark.asyncio
    async def test_aread_stream(self):
        async def gen():
            yield b"chunk1"
            yield b"chunk2"

        req = Request("POST", "https://example.com", stream=gen())
        result = await req.aread()
        assert result == b"chunk1chunk2"
        assert req.is_stream_consumed


class TestStreamConsumed:
    def test_initially_false(self):
        req = Request("GET", "https://example.com")
        assert not req.is_stream_consumed


class TestHttpVersion:
    def test_default_version(self):
        req = Request("GET", "https://example.com")
        assert req.http_version == "HTTP/1.1"


class TestExtensions:
    def test_default_empty(self):
        req = Request("GET", "https://example.com")
        assert req.extensions == {}

    def test_custom_extensions(self):
        req = Request("GET", "https://example.com", extensions={"timeout": 5.0})
        assert req.extensions == {"timeout": 5.0}


class TestCookies:
    def test_cookies_object(self):
        c = Cookies({"session": "abc"})
        req = Request("GET", "https://example.com", cookies=c)
        assert req.cookies["session"] == "abc"

    def test_cookies_dict(self):
        req = Request("GET", "https://example.com", cookies={"session": "abc"})
        assert req.cookies["session"] == "abc"


class TestRequestRepr:
    def test_repr(self):
        req = Request("GET", "https://example.com")
        r = repr(req)
        assert "GET" in r
        assert "example.com" in r
