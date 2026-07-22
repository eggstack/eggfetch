"""Tests for WSGITransport."""
from __future__ import annotations

import pytest
from eggfetch.compat.httpx import Client, WSGITransport


def simple_app(environ, start_response):
    start_response("200 OK", [("Content-Type", "text/plain")])
    method = environ["REQUEST_METHOD"]
    path = environ["PATH_INFO"]
    return [f"{method} {path}".encode()]


def header_echo_app(environ, start_response):
    start_response("200 OK", [("Content-Type", "text/plain")])
    headers = {k: v for k, v in environ.items() if k.startswith("HTTP_")}
    body = "\n".join(f"{k}={v}" for k, v in sorted(headers.items()))
    return [body.encode()]


def error_app(environ, start_response):
    raise RuntimeError("app error")


def post_body_app(environ, start_response):
    body = environ["wsgi.input"].read()
    start_response("200 OK", [("Content-Type", "text/plain")])
    return [body]


class TestWSGITransport:
    def test_simple_get(self):
        with Client(transport=WSGITransport(simple_app)) as client:
            resp = client.get("http://testserver/path")
            assert resp.status_code == 200
            assert resp.content == b"GET /path"

    def test_post_method(self):
        with Client(transport=WSGITransport(simple_app)) as client:
            resp = client.post("http://testserver/data")
            assert resp.content == b"POST /data"

    def test_headers_passed(self):
        with Client(transport=WSGITransport(header_echo_app)) as client:
            resp = client.get(
                "http://testserver/",
                headers={"X-Custom": "test-value"},
            )
            assert resp.status_code == 200
            text = resp.text
            assert "HTTP_X_CUSTOM=test-value" in text

    def test_request_body(self):
        with Client(transport=WSGITransport(post_body_app)) as client:
            resp = client.post("http://testserver/echo", content=b"hello world")
            assert resp.content == b"hello world"

    def test_query_string(self):
        def qs_app(environ, start_response):
            start_response("200 OK", [])
            return [environ["QUERY_STRING"].encode()]

        with Client(transport=WSGITransport(qs_app)) as client:
            resp = client.get("http://testserver/?foo=bar&baz=1")
            assert resp.content == b"foo=bar&baz=1"

    def test_app_error_raises(self):
        with pytest.raises(RuntimeError, match="app error"):
            with Client(transport=WSGITransport(error_app)) as client:
                client.get("http://testserver/")

    def test_app_error_suppressed(self):
        transport = WSGITransport(error_app, raise_app_exceptions=False)
        with Client(transport=transport) as client:
            resp = client.get("http://testserver/")
            assert resp.status_code == 500

    def test_context_manager(self):
        with WSGITransport(simple_app) as transport:
            assert transport is not None

    def test_script_name(self):
        def script_app(environ, start_response):
            start_response("200 OK", [])
            return [environ["SCRIPT_NAME"].encode()]

        with Client(transport=WSGITransport(script_app, script_name="/api")) as client:
            resp = client.get("http://testserver/test")
            assert resp.content == b"/api"
