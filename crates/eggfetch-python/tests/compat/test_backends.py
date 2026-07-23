"""Tests for async backend behaviour (asyncio detection, error messages)."""

from __future__ import annotations

import asyncio
import sys
import typing

import pytest

from eggfetch.compat.httpx import Client, AsyncClient, Request, Response
from eggfetch.compat.httpx._mock import MockTransport, _build_response
from eggfetch.compat.httpx._exceptions import RequestError


# ---------------------------------------------------------------------------
# Async context manager
# ---------------------------------------------------------------------------


class TestAsyncContextManager:
    """AsyncClient must work as an async context manager."""

    @pytest.mark.asyncio
    async def test_async_context_manager_enter_exit(self):
        async with AsyncClient() as client:
            assert not client.is_closed
        assert client.is_closed

    @pytest.mark.asyncio
    async def test_async_context_manager_with_transport(self):
        def handler(request):
            return _build_response(200, text="ok")

        async with AsyncClient(transport=MockTransport(handler)) as client:
            resp = await client.get("http://test/")
            assert resp.status_code == 200


# ---------------------------------------------------------------------------
# Sync handler with AsyncClient
# ---------------------------------------------------------------------------


class TestSyncHandlerAsyncClient:
    """AsyncClient should accept sync handlers (called directly)."""

    @pytest.mark.asyncio
    async def test_sync_handler_via_async_client(self):
        def handler(request):
            return _build_response(200, text="sync-ok")

        async with AsyncClient(transport=MockTransport(handler)) as client:
            resp = await client.get("http://test/")
            assert resp.status_code == 200
            assert resp.text == "sync-ok"


# ---------------------------------------------------------------------------
# Async handler with AsyncClient
# ---------------------------------------------------------------------------


class TestAsyncHandlerAsyncClient:
    """AsyncClient with async_transport should await async handlers."""

    @pytest.mark.asyncio
    async def test_async_handler_via_async_client(self):
        async def handler(request):
            return _build_response(200, text="async-ok")

        async with AsyncClient(async_transport=MockTransport(handler)) as client:
            resp = await client.get("http://test/")
            assert resp.status_code == 200
            assert resp.text == "async-ok"


# ---------------------------------------------------------------------------
# Sync client rejects async handler
# ---------------------------------------------------------------------------


class TestSyncClientRejectsAsyncHandler:
    """Client (sync) with an async handler should raise RuntimeError."""

    def test_sync_client_with_async_handler_raises(self):
        async def handler(request):
            return _build_response(200)

        with pytest.raises(RuntimeError, match="async"):
            with Client(transport=MockTransport(handler)) as client:
                client.get("http://test/")


# ---------------------------------------------------------------------------
# Closed client detection
# ---------------------------------------------------------------------------


class TestClosedClientDetection:
    """Operations on a closed client raise RuntimeError."""

    @pytest.mark.asyncio
    async def test_async_send_after_close_raises(self):
        async with AsyncClient() as client:
            pass
        with pytest.raises(RuntimeError, match="closed"):
            req = Request("GET", "http://test/")
            await client.send(req)

    def test_sync_send_after_close_raises(self):
        with Client() as client:
            pass
        with pytest.raises(RuntimeError, match="closed"):
            req = Request("GET", "http://test/")
            client.send(req)


# ---------------------------------------------------------------------------
# Event loop detection (sync path)
# ---------------------------------------------------------------------------


class TestEventLoopDetection:
    """Sync client must not fail if called from an existing event loop."""

    def test_sync_client_works_standalone(self):
        def handler(request):
            return _build_response(200, text="standalone")

        with Client(transport=MockTransport(handler)) as client:
            resp = client.get("http://test/")
            assert resp.text == "standalone"


# ---------------------------------------------------------------------------
# send() type validation
# ---------------------------------------------------------------------------


class TestSendTypeValidation:
    """send() must reject non-Request objects."""

    def test_sync_send_rejects_string(self):
        with Client() as client:
            with pytest.raises(TypeError, match="Request"):
                client.send("not-a-request")  # type: ignore[arg-type]

    @pytest.mark.asyncio
    async def test_async_send_rejects_string(self):
        async with AsyncClient() as client:
            with pytest.raises(TypeError, match="Request"):
                await client.send("not-a-request")  # type: ignore[arg-type]


# ---------------------------------------------------------------------------
# Asyncio leaked task detection
# ---------------------------------------------------------------------------


class TestAsyncioLeakedTaskDetection:
    """Verify async operations don't leak tasks."""

    @pytest.mark.asyncio
    async def test_no_leaked_tasks_after_send(self):
        """A single send must not leave orphan tasks behind."""
        def handler(request):
            return _build_response(200, text="ok")

        before_tasks = asyncio.all_tasks()

        async with AsyncClient(
            async_transport=MockTransport(handler)
        ) as client:
            resp = await client.get("http://test/")
            assert resp.status_code == 200

        after_tasks = asyncio.all_tasks()
        # No new tasks should remain after the client closes
        new_tasks = after_tasks - before_tasks
        assert len(new_tasks) == 0, f"Leaked tasks: {new_tasks}"

    @pytest.mark.asyncio
    async def test_no_leaked_tasks_after_multiple_requests(self):
        """Multiple requests must not leak tasks."""
        def handler(request):
            return _build_response(200, text="ok")

        before_tasks = asyncio.all_tasks()

        async with AsyncClient(
            async_transport=MockTransport(handler)
        ) as client:
            for _ in range(5):
                resp = await client.get("http://test/")
                assert resp.status_code == 200

        after_tasks = asyncio.all_tasks()
        new_tasks = after_tasks - before_tasks
        assert len(new_tasks) == 0, f"Leaked tasks: {new_tasks}"

    @pytest.mark.asyncio
    async def test_no_leaked_tasks_after_error(self):
        """An error in send must not leak tasks."""
        def handler(request):
            raise ValueError("intentional error")

        before_tasks = asyncio.all_tasks()

        async with AsyncClient(
            async_transport=MockTransport(handler)
        ) as client:
            with pytest.raises(ValueError):
                await client.get("http://test/")

        after_tasks = asyncio.all_tasks()
        new_tasks = after_tasks - before_tasks
        assert len(new_tasks) == 0, f"Leaked tasks: {new_tasks}"


# ---------------------------------------------------------------------------
# Event loop blocking verification
# ---------------------------------------------------------------------------


class TestEventLoopNonBlocking:
    """Verify async operations don't block the event loop."""

    @pytest.mark.asyncio
    async def test_concurrent_requests_dont_block(self):
        """Multiple concurrent requests should complete without blocking."""
        import time

        call_times = []

        async def handler(request):
            call_times.append(time.monotonic())
            return _build_response(200, text="ok")

        async with AsyncClient(
            async_transport=MockTransport(handler)
        ) as client:
            # Fire 3 requests concurrently
            import asyncio as _asyncio
            results = await _asyncio.gather(
                client.get("http://test/1"),
                client.get("http://test/2"),
                client.get("http://test/3"),
            )

        assert len(results) == 3
        for r in results:
            assert r.status_code == 200

    @pytest.mark.asyncio
    async def test_async_client_works_in_running_loop(self):
        """AsyncClient must work correctly inside an already-running loop."""
        def handler(request):
            return _build_response(200, text="in-loop")

        async with AsyncClient(
            async_transport=MockTransport(handler)
        ) as client:
            resp = await client.get("http://test/")
            assert resp.text == "in-loop"


# ---------------------------------------------------------------------------
# Client extensions parameter
# ---------------------------------------------------------------------------


class TestClientExtensions:
    """Client-level extensions parameter behavior."""

    def test_sync_client_extensions_stored(self):
        with Client(extensions={"ext_key": "ext_val"}) as client:
            assert client._extensions == {"ext_key": "ext_val"}

    def test_async_client_extensions_stored(self):
        client = AsyncClient(extensions={"ext_key": "ext_val"})
        assert client._extensions == {"ext_key": "ext_val"}

    def test_client_default_extensions_empty(self):
        with Client() as client:
            assert client._extensions == {}

    def test_async_client_default_extensions_empty(self):
        client = AsyncClient()
        assert client._extensions == {}
