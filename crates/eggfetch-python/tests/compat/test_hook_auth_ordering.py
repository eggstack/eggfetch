"""Hook and auth ordering tests.

Track 4.5: Verify the exact ordering:
  request hook -> auth mutation -> transport dispatch -> intermediate response -> final response hook.
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
        """Verify: request hook -> auth -> transport -> intermediate -> response hook."""
        event_log = []

        def on_request(request):
            event_log.append(("request_hook", request.headers.get("x-step", "none")))
            # Hook sees the request BEFORE auth modifies it
            assert "x-step" not in request.headers

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
        # Expected order:
        # 1. request_hook (before auth, x-step not set yet)
        # 2. transport dispatch (step 1, gets 401)
        # 3. transport dispatch (step 2, gets 200)
        # 4. response_hook (final response)
        assert event_log[0] == ("request_hook", "none")
        assert event_log[-1] == ("response_hook", 200)
        # The request hook should only be called once (before auth)
        assert sum(1 for e in event_log if e[0] == "request_hook") == 1

    def test_auth_modifies_request_between_hook_and_transport(self):
        """Auth adds headers after request hooks run."""
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

        # Hook saw the request before auth
        assert hook_saw == ["missing"]
        # Transport saw the auth header
        assert resp.text == "Bearer token"

    def test_response_hook_after_auth_flow_completes(self):
        """Response hook runs after the full auth flow (all rounds)."""
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

        # Response hook should only see the final response (200), not the intermediate 401
        assert response_log == [200]


class TestAsyncHookAuthOrdering:
    @pytest.mark.asyncio
    async def test_full_ordering_async(self):
        """Verify async: request hook -> auth -> transport -> response hook."""
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
        assert event_log[0] == ("request_hook", "none")
        assert event_log[-1] == ("response_hook", 200)
        assert sum(1 for e in event_log if e[0] == "request_hook") == 1

    @pytest.mark.asyncio
    async def test_async_auth_modifies_request_after_hook(self):
        """Async auth adds headers after request hooks run."""
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

        assert hook_saw == ["missing"]
        assert resp.text == "Bearer async-token"
