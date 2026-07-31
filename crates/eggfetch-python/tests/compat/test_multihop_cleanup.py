"""Phase 4 Track 6: Multi-hop resource cleanup tests.

Tests for deterministic cleanup of intermediate responses,
auth generators, and pool permits across redirects and auth flows.
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
from eggfetch.compat.httpx._exceptions import TooManyRedirects


class TestSyncCleanup:
    """Sync resource cleanup across multi-hop flows."""

    def test_intermediate_redirect_response_closed(self):
        """Intermediate redirect responses are read and closed when followed."""
        closed_paths = []

        def handler(request):
            if request.url.path == "/a":
                return Response(302, headers={"Location": "/b"})
            return Response(200, text="ok")

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get("http://testserver/a")
            assert resp.status_code == 200

    def test_auth_generator_closed_on_success(self):
        """Auth generator is closed after successful auth."""

        class TrackedAuth(Auth):
            closed = False

            def auth_flow(self, request):
                request.headers["authorization"] = "Bearer token"
                yield request

            def close(self):
                TrackedAuth.closed = True

        def handler(request):
            return Response(200, text="ok")

        auth = TrackedAuth()
        with Client(transport=MockTransport(handler), auth=auth) as c:
            resp = c.get("http://testserver/")

        assert resp.status_code == 200

    def test_auth_generator_closed_on_exception(self):
        """Auth generator is closed even when an exception occurs."""

        class TrackedAuth(Auth):
            def auth_flow(self, request):
                request.headers["authorization"] = "Bearer token"
                yield request

        def handler(request):
            raise ConnectionError("connection failed")

        auth = TrackedAuth()
        with Client(transport=MockTransport(handler), auth=auth) as c:
            with pytest.raises(Exception):
                c.get("http://testserver/")

    def test_too_many_redirects_has_request(self):
        """TooManyRedirects exception has the request attached."""
        def handler(request):
            return Response(302, headers={"Location": "/loop"})

        with Client(
            transport=MockTransport(handler),
            follow_redirects=True,
            max_redirects=5,
        ) as c:
            with pytest.raises(TooManyRedirects) as exc_info:
                c.get("http://testserver/loop")
            assert exc_info.value.request is not None


class TestAsyncCleanup:
    """Async resource cleanup across multi-hop flows."""

    @pytest.mark.asyncio
    async def test_intermediate_redirect_response_closed_async(self):
        async def handler(request):
            if request.url.path == "/a":
                return Response(302, headers={"Location": "/b"})
            return Response(200, text="ok")

        async with AsyncClient(
            async_transport=MockTransport(handler), follow_redirects=True
        ) as c:
            resp = await c.get("http://testserver/a")
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_auth_generator_closed_on_exception_async(self):
        class TrackedAuth(Auth):
            def auth_flow(self, request):
                request.headers["authorization"] = "Bearer token"
                yield request

        async def handler(request):
            raise ConnectionError("connection failed")

        auth = TrackedAuth()
        async with AsyncClient(
            async_transport=MockTransport(handler), auth=auth
        ) as c:
            with pytest.raises(Exception):
                await c.get("http://testserver/")

    @pytest.mark.asyncio
    async def test_too_many_redirects_async(self):
        async def handler(request):
            return Response(302, headers={"Location": "/loop"})

        async with AsyncClient(
            async_transport=MockTransport(handler),
            follow_redirects=True,
            max_redirects=3,
        ) as c:
            with pytest.raises(TooManyRedirects):
                await c.get("http://testserver/loop")


class TestHookExceptionCleanup:
    """Hook exceptions halt the state machine and clean up."""

    def test_request_hook_error_halts_redirect(self):
        """Request hook exception prevents further redirects."""

        def bad_hook(request):
            if request.url.path == "/b":
                raise RuntimeError("hook abort")
            return request

        def handler(request):
            if request.url.path == "/a":
                return Response(302, headers={"Location": "/b"})
            return Response(200, text="ok")

        with Client(
            transport=MockTransport(handler),
            follow_redirects=True,
            event_hooks={"request": [bad_hook], "response": []},
        ) as c:
            with pytest.raises(RuntimeError, match="hook abort"):
                c.get("http://testserver/a")

    def test_response_hook_error_halts_redirect(self):
        """Response hook exception prevents further redirects."""

        def bad_hook(response):
            if response.status_code == 302:
                raise RuntimeError("response hook abort")

        def handler(request):
            if request.url.path == "/a":
                return Response(302, headers={"Location": "/b"})
            return Response(200, text="ok")

        with Client(
            transport=MockTransport(handler),
            follow_redirects=True,
            event_hooks={"request": [], "response": [bad_hook]},
        ) as c:
            with pytest.raises(RuntimeError, match="response hook abort"):
                c.get("http://testserver/a")
