"""Phase 2 Track 3: Response protocol metadata parity tests."""

import pytest

from eggfetch.compat.httpx import Response, Request, URL


class TestHttpVersionFromExtensions:
    """3.1 Treat response extensions as the source of protocol metadata."""

    def test_http_version_from_bytes_extension(self):
        resp = Response(
            200,
            content=b"ok",
            extensions={"http_version": b"HTTP/2"},
        )
        assert resp.http_version == "HTTP/2"

    def test_http_version_from_string_extension(self):
        resp = Response(
            200,
            content=b"ok",
            extensions={"http_version": "HTTP/1.1"},
        )
        assert resp.http_version == "HTTP/1.1"

    def test_http_version_default(self):
        resp = Response(200, content=b"ok")
        assert resp.http_version == "HTTP/1.1"

    def test_reason_phrase_from_bytes_extension(self):
        resp = Response(
            200,
            content=b"ok",
            extensions={"reason_phrase": b"Custom Reason"},
        )
        assert resp.reason_phrase == "Custom Reason"

    def test_reason_phrase_from_string_extension(self):
        resp = Response(
            200,
            content=b"ok",
            extensions={"reason_phrase": "Custom Reason"},
        )
        assert resp.reason_phrase == "Custom Reason"

    def test_reason_phrase_default_200(self):
        resp = Response(200, content=b"ok")
        assert resp.reason_phrase == "OK"

    def test_reason_phrase_default_404(self):
        resp = Response(404)
        assert resp.reason_phrase == "Not Found"


class TestRequestAttachment:
    """3.2 Preserve request and URL attachment."""

    def test_request_attached(self):
        req = Request("GET", "https://example.com/path")
        resp = Response(200, content=b"ok", request=req)
        assert resp.request is req

    def test_url_from_request(self):
        req = Request("GET", "https://example.com/path")
        resp = Response(200, content=b"ok", request=req)
        assert str(resp.url) == "https://example.com/path"

    def test_url_no_request(self):
        resp = Response(200, content=b"ok")
        # URL("") creates a non-absolute URL with empty parts
        assert not resp.url.is_absolute_url

    def test_request_setter(self):
        req1 = Request("GET", "https://example.com/a")
        req2 = Request("GET", "https://example.com/b")
        resp = Response(200, content=b"ok", request=req1)
        resp.request = req2
        assert resp.request is req2
        assert str(resp.url) == "https://example.com/b"


class TestElapsedTiming:
    """3.3 Measure elapsed time."""

    def test_elapsed_unavailable_before_read_streaming(self):
        def gen():
            yield b"data"

        resp = Response(200, stream=gen())
        with pytest.raises(RuntimeError, match="not available until"):
            _ = resp.elapsed

    def test_elapsed_available_buffered(self):
        resp = Response(200, content=b"data")
        assert resp.elapsed is not None

    def test_elapsed_after_read_streaming(self):
        def gen():
            yield b"data"

        resp = Response(200, stream=gen())
        resp.read()
        assert resp.elapsed is not None

    def test_elapsed_non_negative(self):
        resp = Response(200, content=b"data")
        from datetime import timedelta

        assert resp.elapsed >= timedelta(0)
