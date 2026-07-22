"""Tests for HTTPX-compatible transport protocols."""

from __future__ import annotations

import pytest

from eggfetch.compat.httpx import (
    BaseTransport,
    AsyncBaseTransport,
    HTTPTransport,
    AsyncHTTPTransport,
    MockTransport,
    Request,
    Response,
    URL,
)


class TestBaseTransport:
    def test_handle_request_not_implemented(self):
        transport = BaseTransport()
        request = Request("GET", "http://example.com/")
        with pytest.raises(NotImplementedError):
            transport.handle_request(request)

    def test_close_is_noop(self):
        transport = BaseTransport()
        transport.close()

    def test_context_manager(self):
        with BaseTransport() as transport:
            assert isinstance(transport, BaseTransport)

    def test_close_idempotent(self):
        transport = BaseTransport()
        transport.close()
        transport.close()


class TestAsyncBaseTransport:
    @pytest.mark.asyncio
    async def test_handle_async_request_not_implemented(self):
        transport = AsyncBaseTransport()
        request = Request("GET", "http://example.com/")
        with pytest.raises(NotImplementedError):
            await transport.handle_async_request(request)

    @pytest.mark.asyncio
    async def test_aclose_is_noop(self):
        transport = AsyncBaseTransport()
        await transport.aclose()

    @pytest.mark.asyncio
    async def test_async_context_manager(self):
        async with AsyncBaseTransport() as transport:
            assert isinstance(transport, AsyncBaseTransport)


class TestHTTPTransport:
    def test_constructor_defaults(self):
        transport = HTTPTransport()
        assert transport._verify is True
        assert transport._http1 is True
        assert transport._http2 is False

    def test_constructor_custom_params(self):
        transport = HTTPTransport(
            verify=False,
            http2=True,
            retries=3,
        )
        assert transport._verify is False
        assert transport._http2 is True
        assert transport._retries == 3

    def test_context_manager(self):
        with HTTPTransport() as transport:
            assert isinstance(transport, HTTPTransport)

    def test_close_idempotent(self):
        transport = HTTPTransport()
        transport.close()
        transport.close()


class TestAsyncHTTPTransport:
    def test_constructor_defaults(self):
        transport = AsyncHTTPTransport()
        assert transport._verify is True
        assert transport._http1 is True

    @pytest.mark.asyncio
    async def test_async_context_manager(self):
        async with AsyncHTTPTransport() as transport:
            assert isinstance(transport, AsyncHTTPTransport)

    @pytest.mark.asyncio
    async def test_aclose_idempotent(self):
        transport = AsyncHTTPTransport()
        await transport.aclose()
        await transport.aclose()


class TestTransportDispatch:
    def test_custom_transport_overrides_native(self):
        def handler(request):
            return Response(200, content=b"from transport")

        transport = MockTransport(handler)

        from eggfetch.compat.httpx import Client

        client = Client(transport=transport)
        response = client.get("http://example.com/")
        assert response.status_code == 200
        assert response.content == b"from transport"
        client.close()

    def test_transport_receives_full_request(self):
        received = []

        def handler(request):
            received.append(request)
            return Response(200, content=b"ok")

        from eggfetch.compat.httpx import Client

        client = Client(
            transport=MockTransport(handler),
            headers={"x-custom": "value"},
        )
        client.get("http://example.com/test")
        assert len(received) == 1
        req = received[0]
        assert req.method == "GET"
        assert "x-custom" in req.headers
        client.close()
