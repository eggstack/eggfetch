"""Tests for client event hooks."""
from __future__ import annotations

import pytest
from eggfetch.compat.httpx import (
    Client,
    AsyncClient,
    Auth,
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

    def test_request_hook_runs_after_auth_yields_request(self):
        """Per-hop ordering: auth yields the concrete Request, then hooks run.

        This verifies the correct ordering: auth → hooks → dispatch.
        """
        hook_saw_auth = []

        def check_auth(request):
            # The hook should see the request AFTER auth added the header
            hook_saw_auth.append("authorization" in request.headers)

        class TestAuth(Auth):
            def auth_flow(self, request):
                request.headers["authorization"] = "Bearer token"
                yield request

        def handler(request):
            return Response(200)

        with Client(
            transport=MockTransport(handler),
            auth=TestAuth(),
            event_hooks={"request": [check_auth], "response": []},
        ) as client:
            client.get("http://testserver/")

        # Hook saw the request AFTER auth added the header
        assert hook_saw_auth == [True]

    def test_response_hook_error_closes_stream(self):
        """When a response hook raises, the response stream is closed."""
        call_log = []

        def bad_hook(response):
            # Record the response state before raising
            call_log.append(("hook_called", response.status_code))
            raise RuntimeError("hook failed")

        def handler(request):
            return Response(200, content=b"data")

        with Client(
            transport=MockTransport(handler),
            event_hooks={"request": [], "response": [bad_hook]},
        ) as client:
            with pytest.raises(RuntimeError, match="hook failed"):
                client.get("http://testserver/")

        assert call_log == [("hook_called", 200)]

    def test_response_hook_can_modify_response(self):
        """Response hook can mutate the response and caller sees the mutation."""

        def add_header(response):
            response.headers["x-modified"] = "yes"

        def handler(request):
            return Response(200, content=b"data")

        with Client(
            transport=MockTransport(handler),
            event_hooks={"request": [], "response": [add_header]},
        ) as client:
            resp = client.get("http://testserver/")

        assert resp.headers["x-modified"] == "yes"

    def test_request_hook_error_propagates(self):
        """Request hook error prevents dispatch and propagates cleanly."""

        def bad_hook(request):
            raise RuntimeError("request hook failed")

        def handler(request):
            return Response(200, content=b"should-not-reach")

        with Client(
            transport=MockTransport(handler),
            event_hooks={"request": [bad_hook], "response": []},
        ) as client:
            with pytest.raises(RuntimeError, match="request hook failed"):
                client.get("http://testserver/")

    def test_response_hook_multiple_error_cleanup(self):
        """First response hook error closes response; second hook is not called."""
        call_log = []

        def first_hook(response):
            call_log.append("first")
            raise RuntimeError("first hook error")

        def second_hook(response):
            call_log.append("second")

        def handler(request):
            return Response(200, content=b"data")

        with Client(
            transport=MockTransport(handler),
            event_hooks={"request": [], "response": [first_hook, second_hook]},
        ) as client:
            with pytest.raises(RuntimeError, match="first hook error"):
                client.get("http://testserver/")

        # Second hook should not have been called
        assert call_log == ["first"]

    def test_per_hop_request_hook_on_auth_retry(self):
        """Request hook runs on every auth hop (per Track 4.2)."""
        hook_count = [0]

        def on_request(request):
            hook_count[0] += 1

        class RetryAuth(Auth):
            def auth_flow(self, request):
                request.headers["x-round"] = "1"
                yield request
                # After first response, yield a second request
                request.headers["x-round"] = "2"
                yield request

        def handler(request):
            step = request.headers.get("x-round", "")
            if step == "1":
                return Response(401)
            return Response(200)

        with Client(
            transport=MockTransport(handler),
            auth=RetryAuth(),
            event_hooks={"request": [on_request], "response": []},
        ) as client:
            resp = client.get("http://testserver/")

        # Hook ran once per hop (2 hops for auth retry)
        assert hook_count[0] == 2
        assert resp.status_code == 200

    def test_per_hop_response_hook_on_auth_retry(self):
        """Response hook runs on every hop before auth decides (per Track 4.2)."""
        response_codes = []

        def on_response(response):
            response_codes.append(response.status_code)

        class RetryAuth(Auth):
            def auth_flow(self, request):
                request.headers["x-round"] = "1"
                yield request
                request.headers["x-round"] = "2"
                yield request

        def handler(request):
            step = request.headers.get("x-round", "")
            if step == "1":
                return Response(401)
            return Response(200)

        with Client(
            transport=MockTransport(handler),
            auth=RetryAuth(),
            event_hooks={"request": [], "response": [on_response]},
        ) as client:
            resp = client.get("http://testserver/")

        # Response hook ran on each hop: 401 then 200
        assert response_codes == [401, 200]


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

    @pytest.mark.asyncio
    async def test_callable_object_returning_awaitable(self):
        """Callable objects returning awaitables are awaited (Track 4.4)."""
        calls = []

        class CallableHook:
            def __call__(self, request):
                # Returns an awaitable (coroutine-like)
                async def _hook():
                    calls.append("awaited")
                return _hook()

        async def handler(request):
            return Response(200)

        async with AsyncClient(
            async_transport=MockTransport(handler),
            event_hooks={"request": [CallableHook()], "response": []},
        ) as client:
            await client.get("http://testserver/")

        assert calls == ["awaited"]
