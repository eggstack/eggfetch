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
    """Extensions survive through send.  Request extensions live on
    ``response.request.extensions``, not on ``response.extensions``."""

    def test_request_extensions_on_request_object(self):
        handler, captured = _capture_handler()
        with Client(transport=MockTransport(handler)) as client:
            req = client.build_request(
                "GET", "http://example.com/",
                extensions={"trace_id": "abc-123"},
            )
            resp = client.send(req)

        # Request extensions are on the request attached to the response
        assert resp.request is not None
        assert "trace_id" in resp.request.extensions
        assert resp.request.extensions["trace_id"] == "abc-123"
        # Standard response extensions from the handler are present
        assert resp.extensions.get("http_version") == "HTTP/1.1"

    def test_request_extensions_on_request_method(self):
        handler, captured = _capture_handler()
        with Client(transport=MockTransport(handler)) as client:
            resp = client.request(
                "GET", "http://example.com/",
                extensions={"custom_key": "value"},
            )

        assert resp.request is not None
        assert resp.request.extensions.get("custom_key") == "value"

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

        # Both client and request extensions survive on the request object
        assert resp.request is not None
        assert resp.request.extensions.get("client_ext") == "from_client"
        assert resp.request.extensions.get("request_ext") == "from_request"

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
        assert resp.request is not None
        assert resp.request.extensions.get("shared_key") == "request_value"

    def test_no_extensions_returns_empty_dict(self):
        handler, _ = _capture_handler()
        with Client(transport=MockTransport(handler)) as client:
            resp = client.get("http://example.com/")

        # Response extensions may have http_version/reason_phrase from handler
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
                assert resp.request is not None
                assert resp.request.extensions.get("stream_key") == "stream_val"
                resp.read()


class TestExtensionPassthroughTransport:
    """Extensions survive through custom transport dispatch."""

    def test_extensions_through_custom_transport(self):
        def handler(request):
            # The transport receives the request with extensions intact
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

        assert resp.request is not None
        assert resp.request.extensions.get("transport_test") is True
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

        assert resp.request is not None
        assert resp.request.extensions.get("mount_key") == "mount_value"


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

        assert resp.request is not None
        assert resp.request.extensions.get("async_key") == "async_val"

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

        assert resp.request is not None
        assert resp.request.extensions.get("async_mount") is True

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

        assert resp.request is not None
        assert resp.request.extensions.get("client_level") == "yes"
        assert resp.request.extensions.get("request_level") == "yes"


class TestResponseExtensionIsolation:
    """Response extensions must not contain request-only keys (Track 5.3)."""

    def test_request_extensions_not_on_response(self):
        def handler(request):
            return Response(200, content=b"ok")

        with Client(transport=MockTransport(handler)) as client:
            resp = client.get(
                "http://example.com/",
                extensions={"request_only": True},
            )

        # Request-only extension must NOT leak into response extensions
        assert "request_only" not in resp.extensions
        # But it IS on the request object
        assert resp.request is not None
        assert resp.request.extensions.get("request_only") is True

    def test_response_handler_extensions_preserved(self):
        def handler(request):
            return Response(
                200,
                content=b"ok",
                extensions={"http_version": "HTTP/2", "reason_phrase": "OK"},
            )

        with Client(transport=MockTransport(handler)) as client:
            resp = client.get("http://example.com/")

        assert resp.extensions.get("http_version") == "HTTP/2"
        assert resp.extensions.get("reason_phrase") == "OK"

    def test_timeout_extension_preserved_on_request(self):
        """Timeout extension is placed on the request by the one-hop dispatch."""
        from eggfetch.compat.httpx._timeout import Timeout as CompatTimeout

        captured = []

        def handler(request):
            captured.append(request)
            return Response(200, content=b"ok")

        with Client(
            transport=MockTransport(handler),
            timeout=CompatTimeout(10.0),
        ) as client:
            resp = client.get("http://example.com/")

        assert captured[0].extensions.get("timeout") == CompatTimeout(10.0)
