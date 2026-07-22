"""Tests for client event hooks."""
from __future__ import annotations

import pytest
from eggfetch.compat.httpx import (
    Client,
    AsyncClient,
    MockTransport,
    Request,
    Response,
)


class TestSyncHooks:
    def test_request_hook_called(self):
        calls = []

        def on_request(request):
            calls.append(("request", request.method))

        def handler(request):
            return Response(200)

        with Client(
            transport=MockTransport(handler),
            event_hooks={"request": [on_request], "response": []},
        ) as client:
            client.get("http://testserver/")

        assert len(calls) == 1
        assert calls[0] == ("request", "GET")

    def test_response_hook_called(self):
        calls = []

        def on_response(response):
            calls.append(("response", response.status_code))

        def handler(request):
            return Response(200)

        with Client(
            transport=MockTransport(handler),
            event_hooks={"request": [], "response": [on_response]},
        ) as client:
            client.get("http://testserver/")

        assert len(calls) == 1
        assert calls[0] == ("response", 200)

    def test_hook_ordering(self):
        calls = []

        def on_request(request):
            calls.append("request")

        def on_response(response):
            calls.append("response")

        def handler(request):
            return Response(200)

        with Client(
            transport=MockTransport(handler),
            event_hooks={"request": [on_request], "response": [on_response]},
        ) as client:
            client.get("http://testserver/")

        assert calls == ["request", "response"]

    def test_response_hook_error_closes_response(self):
        def bad_hook(response):
            raise RuntimeError("hook error")

        def handler(request):
            return Response(200, content=b"body")

        with Client(
            transport=MockTransport(handler),
            event_hooks={"request": [], "response": [bad_hook]},
        ) as client:
            with pytest.raises(RuntimeError, match="hook error"):
                client.get("http://testserver/")

    def test_multiple_hooks(self):
        calls = []

        def hook1(req):
            calls.append("hook1")

        def hook2(req):
            calls.append("hook2")

        def handler(request):
            return Response(200)

        with Client(
            transport=MockTransport(handler),
            event_hooks={"request": [hook1, hook2], "response": []},
        ) as client:
            client.get("http://testserver/")

        assert calls == ["hook1", "hook2"]

    def test_request_hook_can_modify_request(self):
        def add_header(request):
            request.headers["x-added"] = "yes"

        received = []

        def handler(request):
            received.append(request.headers.get("x-added"))
            return Response(200)

        with Client(
            transport=MockTransport(handler),
            event_hooks={"request": [add_header], "response": []},
        ) as client:
            client.get("http://testserver/")

        assert received == ["yes"]


class TestAsyncHooks:
    @pytest.mark.asyncio
    async def test_async_request_hook(self):
        calls = []

        async def on_request(request):
            calls.append("async-request")

        async def handler(request):
            return Response(200)

        async with AsyncClient(
            async_transport=MockTransport(handler),
            event_hooks={"request": [on_request], "response": []},
        ) as client:
            await client.get("http://testserver/")

        assert calls == ["async-request"]

    @pytest.mark.asyncio
    async def test_async_response_hook(self):
        calls = []

        async def on_response(response):
            calls.append("async-response")

        async def handler(request):
            return Response(200)

        async with AsyncClient(
            async_transport=MockTransport(handler),
            event_hooks={"request": [], "response": [on_response]},
        ) as client:
            await client.get("http://testserver/")

        assert calls == ["async-response"]

    @pytest.mark.asyncio
    async def test_sync_hook_in_async_client(self):
        calls = []

        def sync_hook(request):
            calls.append("sync")

        async def handler(request):
            return Response(200)

        async with AsyncClient(
            async_transport=MockTransport(handler),
            event_hooks={"request": [sync_hook], "response": []},
        ) as client:
            await client.get("http://testserver/")

        assert calls == ["sync"]

    @pytest.mark.asyncio
    async def test_mixed_sync_async_hooks(self):
        calls = []

        def sync_hook(request):
            calls.append("sync")

        async def async_hook(request):
            calls.append("async")

        async def handler(request):
            return Response(200)

        async with AsyncClient(
            async_transport=MockTransport(handler),
            event_hooks={"request": [sync_hook, async_hook], "response": []},
        ) as client:
            await client.get("http://testserver/")

        assert calls == ["sync", "async"]
