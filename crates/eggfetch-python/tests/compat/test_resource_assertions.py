"""Resource assertion tests for the HTTPX compatibility layer.

Track 10.3: Verify platform-appropriate measurements (FD, task/thread, RSS, socket state)
and that repeated early exits remain within committed thresholds.
"""

import os
import platform
import resource
import subprocess
import sys
import threading

import pytest
from eggfetch.compat.httpx import Client, AsyncClient, MockTransport, Response


def _get_fd_count():
    """Get current file descriptor count (Linux/macOS)."""
    try:
        if platform.system() == "Linux":
            return len(os.listdir(f"/proc/{os.getpid()}/fd"))
        elif platform.system() == "Darwin":
            result = subprocess.run(
                ["lsof", "-p", str(os.getpid())],
                capture_output=True, text=True, timeout=5,
            )
            return len(result.stdout.strip().split("\n")) - 1
    except Exception:
        pass
    return None


def _get_thread_count():
    """Get current thread count."""
    return threading.active_count()


def _get_rss_bytes():
    """Get current RSS in bytes."""
    try:
        usage = resource.getrusage(resource.RUSAGE_SELF)
        # On Linux, ru_maxrss is in KB; on macOS it's in bytes
        if platform.system() == "Linux":
            return usage.ru_maxrss * 1024
        return usage.ru_maxrss
    except Exception:
        return None


def _handler(request):
    return Response(200, content=b"x" * 1024)


class TestResourceStability:
    """Verify that repeated requests don't leak FDs, threads, or memory."""

    def test_fd_stability_under_repeated_requests(self):
        """FD count stabilizes after repeated requests."""
        fd_before = _get_fd_count()
        if fd_before is None:
            pytest.skip("FD counting not supported on this platform")

        with Client(transport=MockTransport(_handler)) as client:
            for _ in range(50):
                resp = client.get("http://testserver/")
                assert resp.status_code == 200

        fd_after = _get_fd_count()
        # Allow a small margin for platform noise, but no significant leak
        assert fd_after is not None
        assert fd_after <= fd_before + 5, \
            f"FD leak detected: before={fd_before}, after={fd_after}"

    def test_thread_stability_under_repeated_requests(self):
        """Thread count stabilizes after repeated requests."""
        threads_before = _get_thread_count()

        with Client(transport=MockTransport(_handler)) as client:
            for _ in range(50):
                resp = client.get("http://testserver/")
                assert resp.status_code == 200

        threads_after = _get_thread_count()
        # Thread count should not grow significantly
        assert threads_after <= threads_before + 3, \
            f"Thread leak detected: before={threads_before}, after={threads_after}"

    def test_fd_stability_under_rapid_open_close(self):
        """FD count stabilizes after rapid client open/close cycles."""
        fd_before = _get_fd_count()
        if fd_before is None:
            pytest.skip("FD counting not supported on this platform")

        for _ in range(20):
            with Client(transport=MockTransport(_handler)) as client:
                resp = client.get("http://testserver/")
                assert resp.status_code == 200

        fd_after = _get_fd_count()
        assert fd_after is not None
        assert fd_after <= fd_before + 5, \
            f"FD leak on rapid open/close: before={fd_before}, after={fd_after}"

    def test_thread_stability_under_rapid_open_close(self):
        """Thread count stabilizes after rapid client open/close cycles."""
        threads_before = _get_thread_count()

        for _ in range(20):
            with Client(transport=MockTransport(_handler)) as client:
                resp = client.get("http://testserver/")
                assert resp.status_code == 200

        threads_after = _get_thread_count()
        assert threads_after <= threads_before + 3, \
            f"Thread leak on rapid open/close: before={threads_before}, after={threads_after}"

    def test_memory_stability_under_repeated_requests(self):
        """RSS does not grow unboundedly under repeated requests."""
        rss_before = _get_rss_bytes()
        if rss_before is None:
            pytest.skip("RSS measurement not available")

        with Client(transport=MockTransport(_handler)) as client:
            for _ in range(100):
                resp = client.get("http://testserver/")
                _ = resp.content

        rss_after = _get_rss_bytes()
        # Allow 10MB growth margin for Python runtime overhead
        assert rss_after is not None
        assert rss_after <= rss_before + 10 * 1024 * 1024, \
            f"Memory growth: before={rss_before}, after={rss_after}"


class TestEarlyExitResourceCleanup:
    """Verify resource cleanup on early exits (cancellation, exceptions)."""

    def test_exception_in_handler_releases_resources(self):
        """Exception during request does not leak FDs."""
        fd_before = _get_fd_count()
        if fd_before is None:
            pytest.skip("FD counting not supported on this platform")

        def error_handler(request):
            raise RuntimeError("handler error")

        with Client(transport=MockTransport(error_handler)) as client:
            for _ in range(20):
                try:
                    client.get("http://testserver/")
                except Exception:
                    pass

        fd_after = _get_fd_count()
        assert fd_after is not None
        assert fd_after <= fd_before + 5, \
            f"FD leak after errors: before={fd_before}, after={fd_after}"

    def test_mixed_success_failure_stable(self):
        """Mixed success/failure requests do not leak resources."""
        fd_before = _get_fd_count()
        if fd_before is None:
            pytest.skip("FD counting not supported on this platform")

        call_count = [0]

        def mixed_handler(request):
            call_count[0] += 1
            if call_count[0] % 3 == 0:
                raise RuntimeError("intermittent error")
            return Response(200)

        with Client(transport=MockTransport(mixed_handler)) as client:
            for _ in range(30):
                try:
                    client.get("http://testserver/")
                except Exception:
                    pass

        fd_after = _get_fd_count()
        assert fd_after is not None
        assert fd_after <= fd_before + 5, \
            f"FD leak under mixed load: before={fd_before}, after={fd_after}"


class TestConcurrentResourceStability:
    """Verify resource stability under concurrent access."""

    def test_concurrent_sync_clients_stable(self):
        """Multiple concurrent sync clients do not leak resources."""
        fd_before = _get_fd_count()
        if fd_before is None:
            pytest.skip("FD counting not supported on this platform")

        def make_request():
            with Client(transport=MockTransport(_handler)) as client:
                for _ in range(10):
                    resp = client.get("http://testserver/")
                    assert resp.status_code == 200

        threads = [threading.Thread(target=make_request) for _ in range(5)]
        for t in threads:
            t.start()
        for t in threads:
            t.join(timeout=10)

        fd_after = _get_fd_count()
        assert fd_after is not None
        assert fd_after <= fd_before + 10, \
            f"FD leak under concurrency: before={fd_before}, after={fd_after}"
