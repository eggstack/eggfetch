"""Hook and auth ordering tests.

Track 4.5: Verify the exact per-hop ordering:
  auth yields Request → request hook → transport → response hook → auth/redirect decision.
"""

import pytest
from eggfetch.compat.httpx import (
    Auth,
    AsyncClient,
    Client,
    MockTransport,
    Request,
    Response,
)


class MultiStepAuth(Auth):
    """Auth that yields two requests, verifying intermediate responses."""

    def auth_flow(self, request):
        request.headers["x-step"] = "1"
        response = yield request
        # Intermediate response is fed back
        if response.status_code == 401:
            request.headers["x-step"] = "2"
            response = yield request


class TestSyncHookAuthOrdering:
    def test_full_ordering_sync(self):
        """Verify per-hop: auth → request hook → transport → response hook → auth decision."""
        event_log = []

        def on_request(request):
            event_log.append(("request_hook", request.headers.get("x-step", "none")))

        def on_response(response):
            event_log.append(("response_hook", response.status_code))

        def handler(request):
            step = request.headers.get("x-step", "")
            if step == "1":
                return Response(401)
            return Response(200, text="done")

        with Client(
            auth=MultiStepAuth(),
            transport=MockTransport(handler),
            event_hooks={"request": [on_request], "response": [on_response]},
        ) as client:
            resp = client.get("http://testserver/")

        assert resp.status_code == 200
        # Per-hop ordering (Track 4.2):
        # Hop 1: auth yields x-step=1 → request_hook sees "1" → dispatch 401 → response_hook sees 401
        # Hop 2: auth yields x-step=2 → request_hook sees "2" → dispatch 200 → response_hook sees 200
        assert event_log == [
            ("request_hook", "1"),
            ("response_hook", 401),
            ("request_hook", "2"),
            ("response_hook", 200),
        ]

    def test_auth_modifies_request_before_hook(self):
        """Auth yields the concrete Request, then hook sees it (Track 4.2)."""
        hook_saw = []

        def on_request(request):
            hook_saw.append(request.headers.get("authorization", "missing"))

        class SimpleAuth(Auth):
            def auth_flow(self, request):
                request.headers["authorization"] = "Bearer token"
                yield request

        def handler(request):
            return Response(200, text=request.headers.get("authorization", "none"))

        with Client(
            auth=SimpleAuth(),
            transport=MockTransport(handler),
            event_hooks={"request": [on_request], "response": []},
        ) as client:
            resp = client.get("http://testserver/")

        # Hook sees the request AFTER auth yields it
        assert hook_saw == ["Bearer token"]
        # Transport saw the auth header
        assert resp.text == "Bearer token"

    def test_response_hook_on_every_hop(self):
        """Response hook runs on every hop before auth decides (Track 4.2)."""
        response_log = []

        def on_response(response):
            response_log.append(response.status_code)

        def handler(request):
            step = request.headers.get("x-step", "")
            if step == "1":
                return Response(401)
            return Response(200)

        with Client(
            auth=MultiStepAuth(),
            transport=MockTransport(handler),
            event_hooks={"request": [], "response": [on_response]},
        ) as client:
            resp = client.get("http://testserver/")

        # Response hook runs on every hop: 401 then 200
        assert response_log == [401, 200]

    def test_request_hook_error_prevents_dispatch(self):
        """If request hook raises, no transport dispatch occurs."""

        def bad_hook(request):
            raise RuntimeError("hook abort")

        def handler(request):
            return Response(200, content=b"should-not-reach")

        with Client(
            transport=MockTransport(handler),
            event_hooks={"request": [bad_hook], "response": []},
        ) as client:
            with pytest.raises(RuntimeError, match="hook abort"):
                client.get("http://testserver/")


class TestAsyncHookAuthOrdering:
    @pytest.mark.asyncio
    async def test_full_ordering_async(self):
        """Verify async per-hop: auth → request hook → transport → response hook."""
        event_log = []

        async def on_request(request):
            event_log.append(("request_hook", request.headers.get("x-step", "none")))

        async def on_response(response):
            event_log.append(("response_hook", response.status_code))

        async def handler(request):
            step = request.headers.get("x-step", "")
            if step == "1":
                return Response(401)
            return Response(200, text="done")

        async with AsyncClient(
            auth=MultiStepAuth(),
            async_transport=MockTransport(handler),
            event_hooks={"request": [on_request], "response": [on_response]},
        ) as client:
            resp = await client.get("http://testserver/")

        assert resp.status_code == 200
        assert event_log == [
            ("request_hook", "1"),
            ("response_hook", 401),
            ("request_hook", "2"),
            ("response_hook", 200),
        ]

    @pytest.mark.asyncio
    async def test_async_auth_modifies_request_before_hook(self):
        """Async auth yields Request, then hook sees it (Track 4.2)."""
        hook_saw = []

        async def on_request(request):
            hook_saw.append(request.headers.get("authorization", "missing"))

        class SimpleAuth(Auth):
            def auth_flow(self, request):
                request.headers["authorization"] = "Bearer async-token"
                yield request

        async def handler(request):
            return Response(200, text=request.headers.get("authorization", "none"))

        async with AsyncClient(
            auth=SimpleAuth(),
            async_transport=MockTransport(handler),
            event_hooks={"request": [on_request], "response": []},
        ) as client:
            resp = await client.get("http://testserver/")

        assert hook_saw == ["Bearer async-token"]
        assert resp.text == "Bearer async-token"

    @pytest.mark.asyncio
    async def test_async_response_hook_on_every_hop(self):
        """Response hook runs on every hop in async (Track 4.2)."""
        response_log = []

        async def on_response(response):
            response_log.append(response.status_code)

        async def handler(request):
            step = request.headers.get("x-step", "")
            if step == "1":
                return Response(401)
            return Response(200)

        async with AsyncClient(
            auth=MultiStepAuth(),
            async_transport=MockTransport(handler),
            event_hooks={"request": [], "response": [on_response]},
        ) as client:
            resp = await client.get("http://testserver/")

        assert response_log == [401, 200]
