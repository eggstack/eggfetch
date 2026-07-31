"""Phase 2 Track 6: Public representations and missing exports parity tests."""

import pytest

from eggfetch.compat.httpx import (
    Request,
    Response,
    URL,
    Headers,
    QueryParams,
    Cookies,
    Timeout,
    Limits,
    Proxy,
    Auth,
    BasicAuth,
    Client,
    AsyncClient,
    codes,
)


class TestReprBehavior:
    """6.1 Match stable repr behavior."""

    def test_request_repr(self):
        req = Request("GET", "https://example.com/path")
        r = repr(req)
        assert "GET" in r
        assert "example.com" in r

    def test_request_repr_method_and_url(self):
        req = Request("POST", "https://api.example.com/data")
        r = repr(req)
        assert "POST" in r
        assert "api.example.com" in r

    def test_response_repr_status_and_reason(self):
        resp = Response(200, content=b"ok")
        r = repr(resp)
        assert "200" in r
        assert "OK" in r

    def test_response_repr_404(self):
        resp = Response(404)
        r = repr(resp)
        assert "404" in r
        assert "Not Found" in r

    def test_url_repr_redacts_password(self):
        url = URL("https://user:secret@example.com/path")
        r = repr(url)
        assert "secret" not in r
        assert "***" in r


class TestMissingExports:
    """6.2 Add or explicitly classify missing public exports."""

    def test_main_exists(self):
        from eggfetch.compat.httpx import main
        assert callable(main)

    def test_create_ssl_context_exists(self):
        from eggfetch.compat.httpx import create_ssl_context
        assert callable(create_ssl_context)

    def test_codes_exists(self):
        assert codes.OK == 200
        assert codes.NOT_FOUND == 404
