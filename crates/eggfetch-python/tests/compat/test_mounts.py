"""Tests for URL-pattern mount routing."""

from __future__ import annotations

import pytest

from eggfetch.compat.httpx import (
    Client,
    AsyncClient,
    MockTransport,
    Request,
    Response,
)


def _make_handler(response_text: str):
    def handler(request):
        return Response(200, content=response_text.encode())

    return handler


class TestMountRouting:
    def test_exact_scheme_match(self):
        http_handler = _make_handler("http")
        https_handler = _make_handler("https")

        with Client(
            mounts={
                "http://": MockTransport(http_handler),
                "https://": MockTransport(https_handler),
            }
        ) as client:
            resp = client.get("http://example.com/")
            assert resp.content == b"http"

    def test_longer_prefix_wins(self):
        general = _make_handler("general")
        specific = _make_handler("specific")

        with Client(
            mounts={
                "http://": MockTransport(general),
                "http://specific.example.com": MockTransport(specific),
            }
        ) as client:
            resp = client.get("http://specific.example.com/path")
            assert resp.content == b"specific"

    def test_no_match_falls_through(self):
        mock_resp = Response(200, content=b"default")

        def handler(request):
            return mock_resp

        with Client(transport=MockTransport(handler)) as client:
            resp = client.get("http://example.com/")
            assert resp.status_code == 200

    def test_mount_close_on_client_close(self):
        closed = []

        class TrackingTransport:
            def handle_request(self, request):
                return Response(200)

            def close(self):
                closed.append(True)

        client = Client(mounts={"http://": TrackingTransport()})
        client.close()
        assert len(closed) == 1


class TestAsyncMountRouting:
    @pytest.mark.asyncio
    async def test_async_mount_dispatch(self):
        async def handler(request):
            return Response(200, content=b"async-mount")

        async with AsyncClient(
            mounts={"http://": MockTransport(handler)}
        ) as client:
            resp = await client.get("http://example.com/")
            assert resp.content == b"async-mount"

    @pytest.mark.asyncio
    async def test_async_transport_constructor(self):
        async def handler(request):
            return Response(200, content=b"async-transport")

        async with AsyncClient(
            async_transport=MockTransport(handler)
        ) as client:
            resp = await client.get("http://example.com/")
            assert resp.content == b"async-transport"
