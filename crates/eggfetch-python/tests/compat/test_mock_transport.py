"""Tests for MockTransport."""
from __future__ import annotations

import asyncio
import pytest
from eggfetch.compat.httpx import (
    Client,
    AsyncClient,
    MockTransport,
    Request,
    Response,
    _build_response,
)


class TestMockTransportSync:
    def test_basic_handler(self):
        def handler(request):
            return Response(200, content=b"Hello")

        with Client(transport=MockTransport(handler)) as client:
            resp = client.get("http://testserver/")
            assert resp.status_code == 200
            assert resp.content == b"Hello"

    def test_handler_receives_request(self):
        received = []

        def handler(request):
            received.append(request)
            return Response(200)

        with Client(transport=MockTransport(handler)) as client:
            client.post("http://testserver/data", content=b"body")

        assert len(received) == 1
        assert received[0].method == "POST"
        assert received[0].content == b"body"

    def test_response_request_attached(self):
        def handler(request):
            return Response(200, content=b"ok")

        with Client(transport=MockTransport(handler)) as client:
            resp = client.get("http://testserver/")
            assert resp.request is not None
            assert resp.request.method == "GET"

    def test_handler_exception_propagates(self):
        def handler(request):
            raise ValueError("test error")

        with Client(transport=MockTransport(handler)) as client:
            with pytest.raises(ValueError, match="test error"):
                client.get("http://testserver/")

    def test_mock_with_status_codes(self):
        def handler(request):
            if "404" in str(request.url):
                return Response(404, content=b"Not Found")
            return Response(200, content=b"OK")

        with Client(transport=MockTransport(handler)) as client:
            resp = client.get("http://testserver/404")
            assert resp.status_code == 404

    def test_close_idempotent(self):
        def handler(request):
            return Response(200)

        transport = MockTransport(handler)
        transport.close()
        transport.close()

    def test_context_manager(self):
        def handler(request):
            return Response(200)

        with MockTransport(handler) as transport:
            assert not transport._is_closed

    def test_handler_returns_json(self):
        def handler(request):
            return Response(200, json={"key": "value"})

        with Client(transport=MockTransport(handler)) as client:
            resp = client.get("http://testserver/")
            assert resp.json() == {"key": "value"}

    def test_sync_client_rejects_async_handler(self):
        async def handler(request):
            return Response(200)

        with pytest.raises(RuntimeError, match="async"):
            with Client(transport=MockTransport(handler)) as client:
                client.get("http://testserver/")

    def test_closed_transport_raises(self):
        def handler(request):
            return Response(200)

        transport = MockTransport(handler)
        transport.close()
        with pytest.raises(RuntimeError, match="closed"):
            transport.handle_request(Request("GET", "http://test/"))


class TestMockTransportAsync:
    @pytest.mark.asyncio
    async def test_async_handler(self):
        async def handler(request):
            return Response(200, content=b"async")

        async with AsyncClient(
            async_transport=MockTransport(handler)
        ) as client:
            resp = await client.get("http://testserver/")
            assert resp.content == b"async"

    @pytest.mark.asyncio
    async def test_sync_handler_in_async_client(self):
        def handler(request):
            return Response(200, content=b"sync-in-async")

        async with AsyncClient(
            async_transport=MockTransport(handler)
        ) as client:
            resp = await client.get("http://testserver/")
            assert resp.content == b"sync-in-async"


class TestBuildResponse:
    def test_build_with_content(self):
        resp = _build_response(201, content=b"created")
        assert resp.status_code == 201
        assert resp.content == b"created"

    def test_build_with_text(self):
        resp = _build_response(200, text="hello")
        assert resp.text == "hello"

    def test_build_with_json(self):
        resp = _build_response(200, json={"a": 1})
        assert resp.json() == {"a": 1}

    def test_build_with_headers(self):
        resp = _build_response(200, headers={"x-test": "yes"})
        assert resp.headers["x-test"] == "yes"
