"""Tests for extension passthrough across client send paths."""
from __future__ import annotations

import pytest

from eggfetch.compat.httpx import (
    Client,
    AsyncClient,
    MockTransport,
    Request,
    Response,
)


def _capture_handler():
    """Return a handler that captures the request and a canned response."""
    captured = []

    def handler(request):
        captured.append(request)
        return Response(
            200,
            content=b"ok",
            extensions={"http_version": "HTTP/1.1", "reason_phrase": "OK"},
        )

    return handler, captured


class TestExtensionPassthrough:
    """Extensions set on the request survive through send to the response."""

    def test_extensions_on_build_request_survive_to_response(self):
        handler, captured = _capture_handler()
        with Client(transport=MockTransport(handler)) as client:
            req = client.build_request(
                "GET", "http://example.com/",
                extensions={"trace_id": "abc-123"},
            )
            resp = client.send(req)

        # Response should contain the request extension
        assert "trace_id" in resp.extensions
        assert resp.extensions["trace_id"] == "abc-123"
        # Standard keys from the handler response should also be present
        assert resp.extensions.get("http_version") == "HTTP/1.1"

    def test_extensions_on_request_method(self):
        handler, captured = _capture_handler()
        with Client(transport=MockTransport(handler)) as client:
            resp = client.request(
                "GET", "http://example.com/",
                extensions={"custom_key": "value"},
            )

        assert resp.extensions.get("custom_key") == "value"

    def test_client_level_extensions_merge_with_request(self):
        handler, captured = _capture_handler()
        with Client(
            transport=MockTransport(handler),
            extensions={"client_ext": "from_client"},
        ) as client:
            req = client.build_request(
                "GET", "http://example.com/",
                extensions={"request_ext": "from_request"},
            )
            resp = client.send(req)

        # Both client and request extensions should be present
        assert resp.extensions.get("client_ext") == "from_client"
        assert resp.extensions.get("request_ext") == "from_request"

    def test_request_extensions_override_client(self):
        handler, captured = _capture_handler()
        with Client(
            transport=MockTransport(handler),
            extensions={"shared_key": "client_value"},
        ) as client:
            req = client.build_request(
                "GET", "http://example.com/",
                extensions={"shared_key": "request_value"},
            )
            resp = client.send(req)

        # Request extensions override client extensions for same key
        assert resp.extensions.get("shared_key") == "request_value"

    def test_no_extensions_returns_empty_dict(self):
        handler, _ = _capture_handler()
        with Client(transport=MockTransport(handler)) as client:
            resp = client.get("http://example.com/")

        # Even with no user extensions, http_version/reason_phrase may be present
        # The response should not crash
        assert isinstance(resp.extensions, dict)

    def test_extensions_through_streaming_path(self):
        def stream_handler(request):
            def body_iter():
                yield b"chunk"
            return Response(200, stream=body_iter())

        with Client(transport=MockTransport(stream_handler)) as client:
            with client.stream(
                "GET", "http://example.com/",
                extensions={"stream_key": "stream_val"},
            ) as resp:
                assert resp.extensions.get("stream_key") == "stream_val"
                resp.read()


class TestExtensionPassthroughTransport:
    """Extensions survive through custom transport dispatch."""

    def test_extensions_through_custom_transport(self):
        def handler(request):
            # Verify the transport received the request with extensions
            return Response(
                200,
                content=b"transport-ok",
                extensions={"http_version": "HTTP/1.1"},
            )

        with Client(transport=MockTransport(handler)) as client:
            req = client.build_request(
                "GET", "http://example.com/",
                extensions={"transport_test": True},
            )
            resp = client.send(req)

        assert resp.extensions.get("transport_test") is True
        assert resp.extensions.get("http_version") == "HTTP/1.1"

    def test_extensions_through_mount_dispatch(self):
        def api_handler(request):
            return Response(
                200,
                content=b"api",
                extensions={"http_version": "HTTP/1.1"},
            )

        def default_handler(request):
            return Response(200, content=b"default")

        with Client(
            mounts={
                "http://api.example.com": MockTransport(api_handler),
                "all://": MockTransport(default_handler),
            }
        ) as client:
            resp = client.get(
                "http://api.example.com/data",
                extensions={"mount_key": "mount_value"},
            )

        assert resp.extensions.get("mount_key") == "mount_value"


class TestExtensionPassthroughAsync:
    """Extension passthrough through async client paths."""

    @pytest.mark.asyncio
    async def test_extensions_through_async_transport(self):
        async def handler(request):
            return Response(
                200,
                content=b"async-ok",
                extensions={"http_version": "HTTP/1.1"},
            )

        async with AsyncClient(
            async_transport=MockTransport(handler)
        ) as client:
            req = client.build_request(
                "GET", "http://example.com/",
                extensions={"async_key": "async_val"},
            )
            resp = await client.send(req)

        assert resp.extensions.get("async_key") == "async_val"

    @pytest.mark.asyncio
    async def test_extensions_through_async_mount(self):
        async def handler(request):
            return Response(
                200,
                content=b"mounted",
                extensions={"http_version": "HTTP/1.1"},
            )

        async with AsyncClient(
            mounts={"http://": MockTransport(handler)}
        ) as client:
            resp = await client.get(
                "http://example.com/",
                extensions={"async_mount": True},
            )

        assert resp.extensions.get("async_mount") is True

    @pytest.mark.asyncio
    async def test_client_extensions_merge_async(self):
        async def handler(request):
            return Response(
                200,
                content=b"ok",
                extensions={"http_version": "HTTP/1.1"},
            )

        async with AsyncClient(
            async_transport=MockTransport(handler),
            extensions={"client_level": "yes"},
        ) as client:
            resp = await client.get(
                "http://example.com/",
                extensions={"request_level": "yes"},
            )

        assert resp.extensions.get("client_level") == "yes"
        assert resp.extensions.get("request_level") == "yes"
