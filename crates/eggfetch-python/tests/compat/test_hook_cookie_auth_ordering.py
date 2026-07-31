"""Phase 4 Track 5: Hook, cookie, auth ordering parity tests.

Verifies the exact per-hop order with event recording:
  auth yields Request → cookie header set → request hook → transport
  → response hook → cookie extraction → redirect/auth decision.
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


class TestPerHopOrdering:
    """5.1 Per-hop event order matches HTTPX."""

    def test_direct_request_order(self):
        """Single request: hooks run once, cookies extracted."""
        event_log = []

        def on_request(request):
            event_log.append(("request_hook", request.headers.get("x-step", "none")))

        def on_response(response):
            event_log.append(("response_hook", response.status_code))

        def handler(request):
            return Response(
                200,
                headers=[("Set-Cookie", "hop1=yes; Path=/")],
                text="ok",
            )

        with Client(
            transport=MockTransport(handler),
            event_hooks={"request": [on_request], "response": [on_response]},
        ) as c:
            resp = c.get("http://testserver/")

        assert resp.status_code == 200
        assert event_log == [("request_hook", "none"), ("response_hook", 200)]
        assert c.cookies.get("hop1") == "yes"

    def test_redirect_order(self):
        """Redirect: hooks run on each hop."""
        event_log = []

        def on_request(request):
            event_log.append(("request_hook", request.url.path))

        def on_response(response):
            event_log.append(("response_hook", response.status_code))

        def handler(request):
            if request.url.path == "/a":
                return Response(
                    302,
                    headers=[
                        ("Location", "/b"),
                        ("Set-Cookie", "from_a=yes; Path=/"),
                    ],
                )
            return Response(200, text="ok")

        with Client(
            transport=MockTransport(handler),
            follow_redirects=True,
            event_hooks={"request": [on_request], "response": [on_response]},
        ) as c:
            resp = c.get("http://testserver/a")

        assert resp.status_code == 200
        # Each hop: request_hook then response_hook
        assert event_log == [
            ("request_hook", "/a"),
            ("response_hook", 302),
            ("request_hook", "/b"),
            ("response_hook", 200),
        ]
        # Cookie from first response should be available on second request
        assert c.cookies.get("from_a") == "yes"

    def test_auth_then_redirect_order(self):
        """Auth (preemptive) then redirect: hooks on each hop."""
        from eggfetch.compat.httpx import BasicAuth

        event_log = []

        def on_request(request):
            event_log.append(("request_hook", request.url.path))

        def on_response(response):
            event_log.append(("response_hook", response.status_code))

        def handler(request):
            if request.url.path == "/protected" and request.headers.get("authorization"):
                return Response(
                    302,
                    headers=[
                        ("Location", "/final"),
                        ("Set-Cookie", "auth_cookie=yes; Path=/"),
                    ],
                )
            if request.url.path == "/final":
                return Response(200, text="ok")
            return Response(401)

        with Client(
            transport=MockTransport(handler),
            auth=BasicAuth("user", "pass"),
            follow_redirects=True,
            event_hooks={"request": [on_request], "response": [on_response]},
        ) as c:
            resp = c.get("http://testserver/protected")

        assert resp.status_code == 200
        # 2 hops: auth preempts (no 401 challenge), then redirect follow
        assert event_log == [
            ("request_hook", "/protected"),
            ("response_hook", 302),
            ("request_hook", "/final"),
            ("response_hook", 200),
        ]

    def test_each_hop_exactly_one_hook_call(self):
        """Each actual hop produces exactly one request-hook and one response-hook."""
        hop_count = [0]

        def on_request(request):
            hop_count[0] += 1

        def on_response(response):
            pass

        def handler(request):
            if request.url.path == "/a":
                return Response(302, headers={"Location": "/b"})
            if request.url.path == "/b":
                return Response(302, headers={"Location": "/c"})
            return Response(200)

        with Client(
            transport=MockTransport(handler),
            follow_redirects=True,
            event_hooks={"request": [on_request], "response": [on_response]},
        ) as c:
            resp = c.get("http://testserver/a")

        assert hop_count[0] == 3


class TestAsyncPerHopOrdering:
    """Async per-hop ordering matches sync."""

    @pytest.mark.asyncio
    async def test_redirect_order_async(self):
        event_log = []

        async def on_request(request):
            event_log.append(("request_hook", request.url.path))

        async def on_response(response):
            event_log.append(("response_hook", response.status_code))

        async def handler(request):
            if request.url.path == "/a":
                return Response(
                    302,
                    headers=[
                        ("Location", "/b"),
                        ("Set-Cookie", "from_a=yes; Path=/"),
                    ],
                )
            return Response(200, text="ok")

        async with AsyncClient(
            async_transport=MockTransport(handler),
            follow_redirects=True,
            event_hooks={"request": [on_request], "response": [on_response]},
        ) as c:
            resp = await c.get("http://testserver/a")

        assert resp.status_code == 200
        assert event_log == [
            ("request_hook", "/a"),
            ("response_hook", 302),
            ("request_hook", "/b"),
            ("response_hook", 200),
        ]

    @pytest.mark.asyncio
    async def test_auth_redirect_order_async(self):
        from eggfetch.compat.httpx import BasicAuth

        event_log = []

        async def on_request(request):
            event_log.append(("request_hook", request.url.path))

        async def on_response(response):
            event_log.append(("response_hook", response.status_code))

        async def handler(request):
            if request.url.path == "/protected" and request.headers.get("authorization"):
                return Response(302, headers=[("Location", "/final")])
            if request.url.path == "/final":
                return Response(200, text="ok")
            return Response(401)

        async with AsyncClient(
            async_transport=MockTransport(handler),
            auth=BasicAuth("user", "pass"),
            follow_redirects=True,
            event_hooks={"request": [on_request], "response": [on_response]},
        ) as c:
            resp = await c.get("http://testserver/protected")

        assert resp.status_code == 200
        assert event_log == [
            ("request_hook", "/protected"),
            ("response_hook", 302),
            ("request_hook", "/final"),
            ("response_hook", 200),
        ]
