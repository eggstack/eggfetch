"""Tests for ASGITransport."""
from __future__ import annotations

import pytest
from eggfetch.compat.httpx import AsyncClient, ASGITransport


async def simple_app(scope, receive, send):
    assert scope["type"] == "http"
    method = scope["method"]
    path = scope["path"]

    body = b""
    while True:
        message = await receive()
        body += message.get("body", b"")
        if not message.get("more_body", False):
            break

    response_body = f"{method} {path}".encode()
    if body:
        response_body += b" " + body

    await send({
        "type": "http.response.start",
        "status": 200,
        "headers": [[b"content-type", b"text/plain"]],
    })
    await send({
        "type": "http.response.body",
        "body": response_body,
    })


async def header_echo_app(scope, receive, send):
    headers = {}
    for name, value in scope.get("headers", []):
        headers[name.decode()] = value.decode()

    body = "\n".join(f"{k}={v}" for k, v in sorted(headers.items()))

    await send({
        "type": "http.response.start",
        "status": 200,
        "headers": [[b"content-type", b"text/plain"]],
    })
    await send({
        "type": "http.response.body",
        "body": body.encode(),
    })


async def error_app(scope, receive, send):
    raise RuntimeError("asgi error")


async def streaming_app(scope, receive, send):
    await send({
        "type": "http.response.start",
        "status": 200,
        "headers": [],
    })
    await send({"type": "http.response.body", "body": b"chunk1"})
    await send({"type": "http.response.body", "body": b"chunk2"})
    await send({"type": "http.response.body", "body": b""})


class TestASGITransport:
    @pytest.mark.asyncio
    async def test_simple_get(self):
        async with AsyncClient(
            async_transport=ASGITransport(simple_app)
        ) as client:
            resp = await client.get("http://testserver/path")
            assert resp.status_code == 200
            assert resp.content == b"GET /path"

    @pytest.mark.asyncio
    async def test_post_with_body(self):
        async with AsyncClient(
            async_transport=ASGITransport(simple_app)
        ) as client:
            resp = await client.post(
                "http://testserver/data",
                content=b"body-data",
            )
            assert b"body-data" in resp.content

    @pytest.mark.asyncio
    async def test_headers_passed(self):
        async with AsyncClient(
            async_transport=ASGITransport(header_echo_app)
        ) as client:
            resp = await client.get(
                "http://testserver/",
                headers={"X-Custom": "test-value"},
            )
            text = resp.text
            assert "x-custom=test-value" in text

    @pytest.mark.asyncio
    async def test_scope_fields(self):
        scope_captured = []

        async def capture_app(scope, receive, send):
            scope_captured.append(scope)
            await send({"type": "http.response.start", "status": 200, "headers": []})
            await send({"type": "http.response.body", "body": b""})

        async with AsyncClient(
            async_transport=ASGITransport(capture_app)
        ) as client:
            await client.get("http://testserver/path?q=1")

        scope = scope_captured[0]
        assert scope["type"] == "http"
        assert scope["method"] == "GET"
        assert scope["path"] == "/path"
        assert scope["scheme"] == "http"
        assert scope["query_string"] == b"q=1"
        assert scope["asgi"]["version"] == "3.0"

    @pytest.mark.asyncio
    async def test_error_app_raises(self):
        async with AsyncClient(
            async_transport=ASGITransport(error_app)
        ) as client:
            with pytest.raises(RuntimeError, match="asgi error"):
                await client.get("http://testserver/")

    @pytest.mark.asyncio
    async def test_error_suppressed(self):
        async with AsyncClient(
            async_transport=ASGITransport(error_app, raise_app_exceptions=False)
        ) as client:
            resp = await client.get("http://testserver/")
            assert resp.status_code == 500

    @pytest.mark.asyncio
    async def test_streaming_response(self):
        async with AsyncClient(
            async_transport=ASGITransport(streaming_app)
        ) as client:
            resp = await client.get("http://testserver/")
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_root_path(self):
        async def root_app(scope, receive, send):
            await send({"type": "http.response.start", "status": 200, "headers": []})
            await send({"type": "http.response.body", "body": scope["root_path"].encode()})

        async with AsyncClient(
            async_transport=ASGITransport(root_app, root_path="/app")
        ) as client:
            resp = await client.get("http://testserver/test")
            assert resp.content == b"/app"
