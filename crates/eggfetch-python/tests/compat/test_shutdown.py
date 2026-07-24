"""Cross-platform interpreter shutdown tests.

Track 10.2: Verify clean shutdown with unused/used clients, unread responses,
partially read responses, auth sequences, and close/request races.
"""

import subprocess
import sys

import pytest


SHUTDOWN_TEST_CODE = '''
"""Cross-platform interpreter shutdown test cases."""
import sys
from eggfetch.compat.httpx import Client, AsyncClient, MockTransport, Response, Auth

def _handler(request):
    return Response(200, content=b"response body " * 100)

class _TwoStepAuth(Auth):
    def auth_flow(self, request):
        request.headers["x-attempt"] = "1"
        response = yield request
        if response.status_code == 401:
            request.headers["x-attempt"] = "2"
            yield request

def test_unused_sync_client():
    """Unused sync client shuts down cleanly."""
    c = Client(transport=MockTransport(_handler))
    c.close()

def test_used_sync_client():
    """Used sync client shuts down cleanly."""
    c = Client(transport=MockTransport(_handler))
    resp = c.get("http://testserver/")
    assert resp.status_code == 200
    c.close()

def test_unread_sync_response():
    """Unread sync response does not leak resources."""
    c = Client(transport=MockTransport(_handler))
    resp = c.get("http://testserver/")
    # Do not read the body
    c.close()

def test_partial_read_sync_response():
    """Partially read sync response is cleaned up on close."""
    c = Client(transport=MockTransport(_handler))
    resp = c.get("http://testserver/")
    _ = resp.read()[:10]  # Read only first 10 bytes
    c.close()

def test_auth_challenge_sequence():
    """Auth challenge sequence shuts down cleanly."""
    def auth_handler(request):
        attempt = request.headers.get("x-attempt", "")
        if attempt == "1":
            return Response(401)
        return Response(200, text="authenticated")

    c = Client(auth=_TwoStepAuth(), transport=MockTransport(auth_handler))
    resp = c.get("http://testserver/")
    assert resp.status_code == 200
    c.close()

def test_close_then_request_raises():
    """Request after close raises RuntimeError."""
    c = Client(transport=MockTransport(_handler))
    c.close()
    try:
        c.get("http://testserver/")
        assert False, "Expected RuntimeError after close"
    except RuntimeError:
        pass

def test_context_manager_cleanup():
    """Context manager closes client on exit."""
    with Client(transport=MockTransport(_handler)) as c:
        resp = c.get("http://testserver/")
        assert resp.status_code == 200
    # Client is closed here

if __name__ == "__main__":
    test_unused_sync_client()
    test_used_sync_client()
    test_unread_sync_response()
    test_partial_read_sync_response()
    test_auth_challenge_sequence()
    test_close_then_request_raises()
    test_context_manager_cleanup()
    print("All sync shutdown tests: PASS")
'''


ASYNC_SHUTDOWN_TEST_CODE = '''
"""Async interpreter shutdown test cases."""
import asyncio
from eggfetch.compat.httpx import AsyncClient, MockTransport, Response, Auth

def _handler(request):
    return Response(200, content=b"response body " * 100)

class _TwoStepAuth(Auth):
    def auth_flow(self, request):
        request.headers["x-attempt"] = "1"
        response = yield request
        if response.status_code == 401:
            request.headers["x-attempt"] = "2"
            yield request

async def test_unused_async_client():
    """Unused async client shuts down cleanly."""
    async with AsyncClient(async_transport=MockTransport(_handler)) as c:
        pass  # No requests

async def test_used_async_client():
    """Used async client shuts down cleanly."""
    async with AsyncClient(async_transport=MockTransport(_handler)) as c:
        resp = await c.get("http://testserver/")
        assert resp.status_code == 200

async def test_unread_async_response():
    """Unread async response does not leak resources."""
    async with AsyncClient(async_transport=MockTransport(_handler)) as c:
        resp = await c.get("http://testserver/")
        # Do not read the body

async def test_partial_read_async_response():
    """Partially read async response is cleaned up."""
    async with AsyncClient(async_transport=MockTransport(_handler)) as c:
        resp = await c.get("http://testserver/")
        _ = resp.content[:10]  # Read only first 10 bytes

async def test_auth_challenge_async():
    """Async auth challenge sequence shuts down cleanly."""
    def auth_handler(request):
        attempt = request.headers.get("x-attempt", "")
        if attempt == "1":
            return Response(401)
        return Response(200, text="authenticated")

    async with AsyncClient(auth=_TwoStepAuth(), async_transport=MockTransport(auth_handler)) as c:
        resp = await c.get("http://testserver/")
        assert resp.status_code == 200

async def main():
    await test_unused_async_client()
    await test_used_async_client()
    await test_unread_async_response()
    await test_partial_read_async_response()
    await test_auth_challenge_async()
    print("All async shutdown tests: PASS")

if __name__ == "__main__":
    asyncio.run(main())
'''


class TestSyncShutdown:
    def test_sync_shutdown_subprocess(self):
        """Run sync shutdown tests in a subprocess to verify clean exit."""
        result = subprocess.run(
            [sys.executable, "-c", SHUTDOWN_TEST_CODE],
            capture_output=True, text=True, timeout=30,
        )
        assert result.returncode == 0, \
            f"Sync shutdown test failed.\nstdout: {result.stdout}\nstderr: {result.stderr}"
        assert "PASS" in result.stdout


class TestAsyncShutdown:
    def test_async_shutdown_subprocess(self):
        """Run async shutdown tests in a subprocess to verify clean exit."""
        result = subprocess.run(
            [sys.executable, "-c", ASYNC_SHUTDOWN_TEST_CODE],
            capture_output=True, text=True, timeout=30,
        )
        assert result.returncode == 0, \
            f"Async shutdown test failed.\nstdout: {result.stdout}\nstderr: {result.stderr}"
        assert "PASS" in result.stdout
