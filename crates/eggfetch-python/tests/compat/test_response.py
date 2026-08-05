"""Tests for Response compatibility."""

import json
from datetime import timedelta

import pytest

from eggfetch.compat.httpx import (
    Response,
    Request,
    Headers,
    URL,
    HTTPStatusError,
)


class TestResponseConstruction:
    def test_status_code(self):
        resp = Response(200)
        assert resp.status_code == 200

    def test_content(self):
        resp = Response(200, content=b"hello")
        assert resp.content == b"hello"

    def test_text(self):
        resp = Response(200, text="hello")
        assert resp.text == "hello"
        assert resp.content == b"hello"

    def test_json(self):
        resp = Response(200, json={"key": "value"})
        assert resp.json() == {"key": "value"}

    def test_html(self):
        resp = Response(200, html="<p>hi</p>")
        assert resp.text == "<p>hi</p>"
        assert "text/html" in resp.headers["content-type"]

    def test_default_empty_content(self):
        resp = Response(200)
        assert resp.content == b""

    def test_conflicting_sources_raises(self):
        with pytest.raises(ValueError, match="Conflicting"):
            Response(200, content=b"body", text="text")


class TestStatusPredicates:
    def test_is_success(self):
        assert Response(200).is_success
        assert Response(201).is_success
        assert Response(299).is_success
        assert not Response(300).is_success
        assert not Response(199).is_success

    def test_is_redirect(self):
        assert Response(301).is_redirect
        assert Response(302).is_redirect
        assert Response(307).is_redirect
        assert Response(308).is_redirect
        assert not Response(200).is_redirect
        assert not Response(400).is_redirect

    def test_is_client_error(self):
        assert Response(400).is_client_error
        assert Response(404).is_client_error
        assert Response(499).is_client_error
        assert not Response(500).is_client_error

    def test_is_server_error(self):
        assert Response(500).is_server_error
        assert Response(503).is_server_error
        assert Response(599).is_server_error
        assert not Response(499).is_server_error

    def test_is_error(self):
        assert Response(400).is_error
        assert Response(500).is_error
        assert not Response(200).is_error
        assert not Response(301).is_error

    def test_is_informational(self):
        assert Response(100).is_informational
        assert Response(101).is_informational
        assert Response(199).is_informational
        assert not Response(200).is_informational


class TestRaiseForStatus:
    def test_success_returns_self(self):
        resp = Response(200)
        assert resp.raise_for_status() is resp

    def test_4xx_raises(self):
        resp = Response(404, request=Request("GET", "https://example.com"))
        with pytest.raises(HTTPStatusError):
            resp.raise_for_status()

    def test_5xx_raises(self):
        resp = Response(500, request=Request("GET", "https://example.com"))
        with pytest.raises(HTTPStatusError):
            resp.raise_for_status()

    def test_4xx_exception_has_response(self):
        req = Request("GET", "https://example.com")
        resp = Response(404, request=req)
        with pytest.raises(HTTPStatusError) as exc_info:
            resp.raise_for_status()
        assert exc_info.value.response is resp
        assert exc_info.value.request is req


class TestEncoding:
    def test_encoding_setter(self):
        resp = Response(200, content=b"hello")
        resp.encoding = "latin-1"
        assert resp.encoding == "latin-1"

    def test_charset_encoding_from_content_type(self):
        resp = Response(
            200,
            headers={"Content-Type": "text/plain; charset=iso-8859-1"},
        )
        assert resp.charset_encoding == "iso-8859-1"

    def test_charset_encoding_none(self):
        resp = Response(200)
        assert resp.charset_encoding is None


class TestElapsed:
    def test_raises_before_read_streaming(self):
        def gen():
            yield b"data"

        resp = Response(200, stream=gen())
        with pytest.raises(RuntimeError, match="not available until"):
            _ = resp.elapsed

    def test_available_for_buffered(self):
        resp = Response(200, content=b"hello")
        assert resp.elapsed is not None

    def test_unattached_stream_elapsed_remains_unavailable_after_read(self):
        def gen():
            yield b"data"

        resp = Response(200, stream=gen())
        resp.read()
        with pytest.raises(RuntimeError, match="not available until"):
            _ = resp.elapsed


class TestHistory:
    def test_default_empty(self):
        resp = Response(200)
        assert resp.history == []

    def test_with_history(self):
        h1 = Response(301)
        h2 = Response(302)
        resp = Response(200, history=[h1, h2])
        assert len(resp.history) == 2


class TestLinks:
    def test_links_from_header(self):
        resp = Response(
            200,
            headers={
                "Link": '<https://api.example.com/items?page=2>; rel="next"'
            },
        )
        links = resp.links
        assert "next" in links
        assert links["next"]["url"] == "https://api.example.com/items?page=2"

    def test_links_empty(self):
        resp = Response(200)
        assert resp.links == {}


class TestIterators:
    def test_iter_bytes(self):
        resp = Response(200, content=b"hello world")
        chunks = list(resp.iter_bytes(chunk_size=5))
        assert chunks == [b"hello", b" worl", b"d"]

    def test_iter_text(self):
        resp = Response(200, text="hello world")
        chunks = list(resp.iter_text(chunk_size=5))
        assert chunks == ["hello", " worl", "d"]

    def test_iter_lines(self):
        resp = Response(200, text="line1\nline2\nline3")
        lines = list(resp.iter_lines())
        assert lines == ["line1", "line2", "line3"]

    def test_iter_lines_strips_cr(self):
        resp = Response(200, text="line1\r\nline2")
        lines = list(resp.iter_lines())
        assert lines == ["line1", "line2"]


class TestResponseHeaders:
    def test_headers(self):
        resp = Response(200, headers={"x-custom": "val"})
        assert resp.headers["x-custom"] == "val"


class TestResponseRepr:
    def test_repr(self):
        resp = Response(200)
        assert "200" in repr(resp)


class TestResponseRequest:
    def test_request_attribute(self):
        req = Request("GET", "https://example.com")
        resp = Response(200, request=req)
        assert resp.request is req

    def test_no_request(self):
        resp = Response(200)
        with pytest.raises(RuntimeError, match="request instance has not been set"):
            _ = resp.request
