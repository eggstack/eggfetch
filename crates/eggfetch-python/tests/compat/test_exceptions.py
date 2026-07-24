"""Tests for exception hierarchy."""

import pytest

from eggfetch.compat.httpx import (
    CloseError,
    ConnectError,
    ConnectTimeout,
    CookieConflict,
    DecodingError,
    HTTPError,
    HTTPStatusError,
    InvalidURL,
    LocalProtocolError,
    NetworkError,
    PoolTimeout,
    ProtocolError,
    ProxyError,
    ReadError,
    ReadTimeout,
    RemoteProtocolError,
    RequestError,
    RequestNotRead,
    ResponseNotRead,
    StreamClosed,
    StreamConsumed,
    StreamError,
    TimeoutException,
    TooManyRedirects,
    TransportError,
    UnsupportedProtocol,
    WriteError,
    WriteTimeout,
    Request,
    Response,
    URL,
)


# ── MRO / hierarchy ────────────────────────────────────────────────────

class TestExceptionHierarchy:
    def test_httperror_is_exception(self):
        assert issubclass(HTTPError, Exception)

    def test_status_error_mro(self):
        assert issubclass(HTTPStatusError, HTTPError)
        assert issubclass(HTTPStatusError, Exception)

    def test_request_error_mro(self):
        assert issubclass(RequestError, HTTPError)

    def test_transport_error_mro(self):
        assert issubclass(TransportError, RequestError)
        assert issubclass(TransportError, HTTPError)

    def test_timeout_exception_mro(self):
        assert issubclass(TimeoutException, TransportError)
        assert issubclass(TimeoutException, RequestError)
        assert issubclass(TimeoutException, HTTPError)

    def test_connect_timeout_mro(self):
        assert issubclass(ConnectTimeout, TimeoutException)
        assert issubclass(ConnectTimeout, TransportError)

    def test_read_timeout_mro(self):
        assert issubclass(ReadTimeout, TimeoutException)

    def test_write_timeout_mro(self):
        assert issubclass(WriteTimeout, TimeoutException)

    def test_pool_timeout_mro(self):
        assert issubclass(PoolTimeout, TimeoutException)

    def test_network_error_mro(self):
        assert issubclass(NetworkError, TransportError)

    def test_close_error_mro(self):
        assert issubclass(CloseError, NetworkError)
        assert issubclass(CloseError, TransportError)

    def test_connect_error_mro(self):
        assert issubclass(ConnectError, NetworkError)

    def test_read_error_mro(self):
        assert issubclass(ReadError, NetworkError)

    def test_write_error_mro(self):
        assert issubclass(WriteError, NetworkError)

    def test_protocol_error_mro(self):
        assert issubclass(ProtocolError, TransportError)

    def test_local_protocol_error_mro(self):
        assert issubclass(LocalProtocolError, ProtocolError)

    def test_remote_protocol_error_mro(self):
        assert issubclass(RemoteProtocolError, ProtocolError)

    def test_proxy_error_mro(self):
        assert issubclass(ProxyError, TransportError)

    def test_unsupported_protocol_mro(self):
        assert issubclass(UnsupportedProtocol, TransportError)

    def test_decoding_error_mro(self):
        assert issubclass(DecodingError, RequestError)

    def test_too_many_redirects_mro(self):
        assert issubclass(TooManyRedirects, RequestError)


class TestStreamErrorHierarchy:
    def test_stream_error_is_exception(self):
        assert issubclass(StreamError, Exception)

    def test_stream_error_not_http_error(self):
        assert not issubclass(StreamError, HTTPError)

    def test_request_not_read(self):
        assert issubclass(RequestNotRead, StreamError)

    def test_response_not_read(self):
        assert issubclass(ResponseNotRead, StreamError)

    def test_stream_closed(self):
        assert issubclass(StreamClosed, StreamError)

    def test_stream_consumed(self):
        assert issubclass(StreamConsumed, StreamError)


class TestStandaloneExceptions:
    def test_invalid_url(self):
        assert issubclass(InvalidURL, Exception)
        assert not issubclass(InvalidURL, HTTPError)

    def test_cookie_conflict(self):
        assert issubclass(CookieConflict, Exception)
        assert not issubclass(CookieConflict, HTTPError)


# ── Constructor signatures ─────────────────────────────────────────────

class TestExceptionConstructors:
    def test_http_error_with_request(self):
        req = Request("GET", "https://example.com")
        exc = HTTPError("error")
        exc._request = req
        assert exc.request is req

    def test_http_error_with_message(self):
        exc = HTTPError(message="test error")
        assert "test error" in str(exc)

    def test_http_status_error(self):
        req = Request("GET", "https://example.com")
        resp = Response(404, request=req)
        exc = HTTPStatusError("Not Found", request=req, response=resp)
        assert exc.request is req
        assert exc.response is resp
        assert "Not Found" in str(exc)

    def test_request_error_has_request(self):
        req = Request("GET", "https://example.com")
        exc = RequestError(request=req, message="fail")
        assert exc.request is req

    def test_transport_error_has_request(self):
        req = Request("GET", "https://example.com")
        exc = TransportError(request=req, message="fail")
        assert exc.request is req

    def test_timeout_exception_has_request(self):
        req = Request("GET", "https://example.com")
        exc = TimeoutException(request=req, message="timeout")
        assert exc.request is req

    def test_connect_timeout_has_request(self):
        req = Request("GET", "https://example.com")
        exc = ConnectTimeout(request=req, message="connect timeout")
        assert exc.request is req

    def test_invalid_url_message(self):
        exc = InvalidURL("bad url")
        assert "bad url" in str(exc)

    def test_cookie_conflict_message(self):
        exc = CookieConflict("conflict")
        assert "conflict" in str(exc)


# ── raise_for_status ───────────────────────────────────────────────────

class TestRaiseForStatus:
    def test_success_returns_self(self):
        resp = Response(200)
        result = resp.raise_for_status()
        assert result is resp

    def test_informational_returns_self(self):
        resp = Response(100)
        result = resp.raise_for_status()
        assert result is resp

    def test_4xx_raises(self):
        resp = Response(404, request=Request("GET", "https://example.com"))
        with pytest.raises(HTTPStatusError) as exc_info:
            resp.raise_for_status()
        assert exc_info.value.response is resp
        assert exc_info.value.request is resp.request

    def test_5xx_raises(self):
        resp = Response(500, request=Request("GET", "https://example.com"))
        with pytest.raises(HTTPStatusError) as exc_info:
            resp.raise_for_status()
        assert exc_info.value.response is resp

    def test_redirect_not_error(self):
        resp = Response(301)
        result = resp.raise_for_status()
        assert result is resp
