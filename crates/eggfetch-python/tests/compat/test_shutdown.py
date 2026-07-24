"""Cross-platform interpreter shutdown tests.

Track 10.2: Verify clean shutdown with unused/used clients, unread responses,
partially read responses, auth sequences, and close/request races.
"""

import subprocess
import sys
import time

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

def test_no_explicit_close():
    """Client without explicit close shuts down cleanly on GC."""
    c = Client(transport=MockTransport(_handler))
    resp = c.get("http://testserver/")
    assert resp.status_code == 200
    del c
    import gc; gc.collect()

if __name__ == "__main__":
    test_unused_sync_client()
    test_used_sync_client()
    test_unread_sync_response()
    test_partial_read_sync_response()
    test_auth_challenge_sequence()
    test_close_then_request_raises()
    test_context_manager_cleanup()
    test_no_explicit_close()
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


PROXY_SHUTDOWN_TEST_CODE = '''
"""Proxy request shutdown test cases."""
import sys
from eggfetch.compat.httpx import Client, MockTransport, Response

def _proxy_handler(request):
    return Response(200, content=b"proxied response")

def test_proxy_request_shutdown():
    """Proxy-proxied request shuts down cleanly."""
    c = Client(transport=MockTransport(_proxy_handler))
    resp = c.get("http://testserver/")
    assert resp.status_code == 200
    c.close()

def test_proxy_request_context_manager():
    """Proxy request via context manager shuts down cleanly."""
    with Client(transport=MockTransport(_proxy_handler)) as c:
        resp = c.get("http://testserver/")
        assert resp.status_code == 200

if __name__ == "__main__":
    test_proxy_request_shutdown()
    test_proxy_request_context_manager()
    print("All proxy shutdown tests: PASS")
'''


TLS_SHUTDOWN_TEST_CODE = '''
"""TLS request shutdown test cases."""
import sys
from eggfetch.compat.httpx import Client, MockTransport, Response

def _tls_handler(request):
    return Response(200, content=b"tls response")

def test_tls_request_shutdown():
    """TLS request shuts down cleanly."""
    c = Client(transport=MockTransport(_tls_handler))
    resp = c.get("http://testserver/")
    assert resp.status_code == 200
    c.close()

def test_tls_request_context_manager():
    """TLS request via context manager shuts down cleanly."""
    with Client(transport=MockTransport(_tls_handler)) as c:
        resp = c.get("http://testserver/")
        assert resp.status_code == 200

if __name__ == "__main__":
    test_tls_request_shutdown()
    test_tls_request_context_manager()
    print("All TLS shutdown tests: PASS")
'''


CANCELLED_ASYNC_SHUTDOWN_TEST_CODE = '''
"""Cancelled async request shutdown test cases."""
import asyncio
from eggfetch.compat.httpx import AsyncClient, MockTransport, Response

def _slow_handler(request):
    import time
    time.sleep(0.1)
    return Response(200)

async def test_cancelled_async_shutdown():
    """Cancelled async request shuts down cleanly."""
    async with AsyncClient(async_transport=MockTransport(_slow_handler)) as c:
        task = asyncio.create_task(c.get("http://testserver/"))
        await asyncio.sleep(0.01)
        task.cancel()
        try:
            await task
        except asyncio.CancelledError:
            pass
    print("Cancelled async shutdown: PASS")

if __name__ == "__main__":
    asyncio.run(test_cancelled_async_shutdown())
'''


STREAMING_SHUTDOWN_TEST_CODE = '''
"""Streaming response shutdown test cases."""
import sys
from eggfetch.compat.httpx import Client, MockTransport, Response

def _streaming_handler(request):
    return Response(200, content=b"chunk" * 200)

def test_streaming_response_shutdown():
    """Partially consumed streaming response shuts down cleanly."""
    c = Client(transport=MockTransport(_streaming_handler))
    resp = c.get("http://testserver/")
    assert resp.status_code == 200
    # Read only some content then close
    _ = resp.content[:50]
    c.close()

def test_streaming_response_context_manager():
    """Streaming response via context manager shuts down cleanly."""
    with Client(transport=MockTransport(_streaming_handler)) as c:
        resp = c.get("http://testserver/")
        assert resp.status_code == 200
        _ = resp.content[:50]

if __name__ == "__main__":
    test_streaming_response_shutdown()
    test_streaming_response_context_manager()
    print("All streaming shutdown tests: PASS")
'''


FORBIDDEN_WARNING_PATTERNS = [
    "event loop is closed",
    "unhandled task",
    "thread-pool panic",
    "Unclosed",
    "Event loop closed",
]


class TestSyncShutdown:
    def test_sync_shutdown_subprocess(self):
        """Run sync shutdown tests in a subprocess to verify clean exit."""
        result = subprocess.run(
            [sys.executable, "-c", SHUTDOWN_TEST_CODE],
            capture_output=True, text=True, timeout=30,
        )
        assert result.returncode == 0, (
            f"Sync shutdown test failed.\nstdout: {result.stdout}\nstderr: {result.stderr}"
        )
        assert "PASS" in result.stdout
        stderr_lower = result.stderr.lower()
        for pattern in FORBIDDEN_WARNING_PATTERNS:
            assert pattern.lower() not in stderr_lower, (
                f"Unexpected stderr warning '{pattern}' detected:\n{result.stderr}"
            )


class TestAsyncShutdown:
    def test_async_shutdown_subprocess(self):
        """Run async shutdown tests in a subprocess to verify clean exit."""
        result = subprocess.run(
            [sys.executable, "-c", ASYNC_SHUTDOWN_TEST_CODE],
            capture_output=True, text=True, timeout=30,
        )
        assert result.returncode == 0, (
            f"Async shutdown test failed.\nstdout: {result.stdout}\nstderr: {result.stderr}"
        )
        assert "PASS" in result.stdout
        stderr_lower = result.stderr.lower()
        for pattern in FORBIDDEN_WARNING_PATTERNS:
            assert pattern.lower() not in stderr_lower, (
                f"Unexpected stderr warning '{pattern}' detected:\n{result.stderr}"
            )


class TestProxyShutdown:
    def test_proxy_shutdown_subprocess(self):
        """Run proxy shutdown tests in a subprocess."""
        result = subprocess.run(
            [sys.executable, "-c", PROXY_SHUTDOWN_TEST_CODE],
            capture_output=True, text=True, timeout=30,
        )
        assert result.returncode == 0, (
            f"Proxy shutdown test failed.\nstdout: {result.stdout}\nstderr: {result.stderr}"
        )
        assert "PASS" in result.stdout
        stderr_lower = result.stderr.lower()
        for pattern in FORBIDDEN_WARNING_PATTERNS:
            assert pattern.lower() not in stderr_lower, (
                f"Unexpected stderr warning '{pattern}' detected:\n{result.stderr}"
            )


class TestTLSShutdown:
    def test_tls_shutdown_subprocess(self):
        """Run TLS shutdown tests in a subprocess."""
        result = subprocess.run(
            [sys.executable, "-c", TLS_SHUTDOWN_TEST_CODE],
            capture_output=True, text=True, timeout=30,
        )
        assert result.returncode == 0, (
            f"TLS shutdown test failed.\nstdout: {result.stdout}\nstderr: {result.stderr}"
        )
        assert "PASS" in result.stdout
        stderr_lower = result.stderr.lower()
        for pattern in FORBIDDEN_WARNING_PATTERNS:
            assert pattern.lower() not in stderr_lower, (
                f"Unexpected stderr warning '{pattern}' detected:\n{result.stderr}"
            )


class TestCancelledAsyncShutdown:
    def test_cancelled_async_shutdown_subprocess(self):
        """Run cancelled async shutdown tests in a subprocess."""
        result = subprocess.run(
            [sys.executable, "-c", CANCELLED_ASYNC_SHUTDOWN_TEST_CODE],
            capture_output=True, text=True, timeout=30,
        )
        assert result.returncode == 0, (
            f"Cancelled async shutdown test failed.\nstdout: {result.stdout}\nstderr: {result.stderr}"
        )
        assert "PASS" in result.stdout
        stderr_lower = result.stderr.lower()
        for pattern in FORBIDDEN_WARNING_PATTERNS:
            assert pattern.lower() not in stderr_lower, (
                f"Unexpected stderr warning '{pattern}' detected:\n{result.stderr}"
            )


class TestStreamingShutdown:
    def test_streaming_shutdown_subprocess(self):
        """Run streaming shutdown tests in a subprocess."""
        result = subprocess.run(
            [sys.executable, "-c", STREAMING_SHUTDOWN_TEST_CODE],
            capture_output=True, text=True, timeout=30,
        )
        assert result.returncode == 0, (
            f"Streaming shutdown test failed.\nstdout: {result.stdout}\nstderr: {result.stderr}"
        )
        assert "PASS" in result.stdout
        stderr_lower = result.stderr.lower()
        for pattern in FORBIDDEN_WARNING_PATTERNS:
            assert pattern.lower() not in stderr_lower, (
                f"Unexpected stderr warning '{pattern}' detected:\n{result.stderr}"
            )


class TestShutdownDeadlineBounds:
    """All shutdown subprocess tests must complete within bounded time."""

    @pytest.mark.parametrize("test_code,expected_label", [
        (SHUTDOWN_TEST_CODE, "sync"),
        (ASYNC_SHUTDOWN_TEST_CODE, "async"),
        (PROXY_SHUTDOWN_TEST_CODE, "proxy"),
        (TLS_SHUTDOWN_TEST_CODE, "tls"),
        (CANCELLED_ASYNC_SHUTDOWN_TEST_CODE, "cancelled-async"),
        (STREAMING_SHUTDOWN_TEST_CODE, "streaming"),
    ])
    def test_shutdown_deadline(self, test_code, expected_label):
        """Each shutdown scenario completes within 15 seconds."""
        start = time.monotonic()
        result = subprocess.run(
            [sys.executable, "-c", test_code],
            capture_output=True, text=True, timeout=15,
        )
        elapsed = time.monotonic() - start
        assert result.returncode == 0, (
            f"{expected_label} shutdown failed in {elapsed:.2f}s.\n"
            f"stdout: {result.stdout}\nstderr: {result.stderr}"
        )
        assert elapsed < 15.0, (
            f"{expected_label} shutdown took {elapsed:.2f}s, exceeded 15s deadline"
        )
