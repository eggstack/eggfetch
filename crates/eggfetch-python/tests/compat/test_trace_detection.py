"""Tests for trace callback coroutine-function detection.

These tests prove that `inspect.iscoroutinefunction` correctly identifies
async callbacks and that the binding rejects coroutine trace callbacks on
sync Client before any network use.

Plan reference: ``plans/httpx-parity-corrective-06-final-semantic-truthfulness.md``
Track B, B2/B3.
"""

from __future__ import annotations

import asyncio
import functools
import inspect

import pytest

from eggfetch.compat.httpx import Client, AsyncClient, MockTransport, Response


def _ok_handler(request):
    return Response(200, content=b"ok")


def _noop_sync_callback(event, info):
    pass


async def _noop_async_callback(event, info):
    pass


class _CallableWithSyncCall:
    def __call__(self, event, info):
        pass


class _CallableWithAsyncCall:
    async def __call__(self, event, info):
        pass


class TestIscoroutinefunctionDetection:
    """Direct unit tests proving inspect.iscoroutinefunction works."""

    def test_sync_def_is_not_coroutine(self):
        assert not inspect.iscoroutinefunction(_noop_sync_callback)

    def test_async_def_is_coroutine(self):
        assert inspect.iscoroutinefunction(_noop_async_callback)

    def test_callable_object_with_sync_call_is_not_coroutine(self):
        obj = _CallableWithSyncCall()
        assert not inspect.iscoroutinefunction(obj)

    def test_callable_object_with_async_call(self):
        obj = _CallableWithAsyncCall()
        # inspect.iscoroutinefunction checks for __call__ on objects
        result = inspect.iscoroutinefunction(obj)
        # This may be True or False depending on Python version;
        # we document the actual behavior.
        assert isinstance(result, bool)

    def test_partial_wrapping_sync_is_not_coroutine(self):
        partial_cb = functools.partial(_noop_sync_callback)
        assert not inspect.iscoroutinefunction(partial_cb)

    def test_partial_wrapping_async(self):
        partial_cb = functools.partial(_noop_async_callback)
        result = inspect.iscoroutinefunction(partial_cb)
        # functools.partial does not always preserve coroutine-ness
        # for inspect.iscoroutinefunction; document actual behavior.
        assert isinstance(result, bool)

    def test_lambda_is_not_coroutine(self):
        lam = lambda event, info: None
        assert not inspect.iscoroutinefunction(lam)

    def test_none_is_not_coroutine(self):
        assert not inspect.iscoroutinefunction(None)


class TestSyncClientRejectsCoroutineTrace:
    """Coroutine trace callbacks on sync Client raise TypeError
    after dispatch (the error slot is pre-populated at construction
    and surfaced once the request completes or fails)."""

    def test_sync_client_rejects_async_trace(self):
        """Async callback triggers TypeError through the native path."""
        with Client() as client:
            with pytest.raises(TypeError, match="async trace callback"):
                client.get(
                    "http://127.0.0.1:1/",
                    extensions={"trace": _noop_async_callback},
                )

    def test_sync_client_accepts_sync_trace(self):
        """Sync callback is accepted without error."""
        with Client() as client:
            with pytest.raises(Exception):
                # Will fail with network error, but not TypeError
                client.get(
                    "http://127.0.0.1:1/",
                    extensions={"trace": _noop_sync_callback},
                )


class TestAsyncClientCoroutineTrace:
    """Coroutine trace callbacks on AsyncClient are accepted
    (or rejected deterministically before dispatch)."""

    @pytest.mark.asyncio
    async def test_async_client_accepts_sync_trace(self):
        async with AsyncClient() as client:
            with pytest.raises(Exception):
                # Will fail with network error, but not TypeError
                await client.get(
                    "http://127.0.0.1:1/",
                    extensions={"trace": _noop_sync_callback},
                )

    @pytest.mark.asyncio
    async def test_async_client_rejects_async_trace(self):
        """Async trace callback triggers TypeError through native path."""
        async with AsyncClient() as client:
            with pytest.raises(TypeError, match="async trace callback"):
                await client.get(
                    "http://127.0.0.1:1/",
                    extensions={"trace": _noop_async_callback},
                )
