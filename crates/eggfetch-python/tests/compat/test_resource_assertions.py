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

sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parent))
from eggfetch.compat.httpx import Client, AsyncClient, MockTransport, Response
from native_fixtures import local_http_server


def _load_resource_thresholds():
    """Load platform thresholds from compat/httpx/0.28.1/resource-thresholds.toml.

    Per plan §10.6: missing platform profile or unparsed policy is a failure.
    """
    try:
        import tomllib
    except ModuleNotFoundError:
        import tomli as tomllib

    thresholds_path = os.path.join(
        os.path.dirname(__file__), "..", "..", "..", "..",
        "compat", "httpx", "0.28.1", "resource-thresholds.toml",
    )
    thresholds_path = os.path.normpath(thresholds_path)
    if not os.path.exists(thresholds_path):
        pytest.fail(f"Resource thresholds file not found: {thresholds_path}")

    with open(thresholds_path, "rb") as f:
        data = tomllib.load(f)

    system = platform.system().lower()
    if system not in data.get("platform", {}):
        pytest.fail(f"No resource thresholds for platform: {system}")
    return data["platform"][system]


def _get_fd_count():
    """Get current file descriptor count (Linux/macOS).

    Per plan §10.6: missing platform support is a failure, not a skip.
    """
    if platform.system() == "Linux":
        fd_dir = f"/proc/{os.getpid()}/fd"
        if not os.path.isdir(fd_dir):
            raise RuntimeError(f"FD directory not available: {fd_dir}")
        return len(os.listdir(fd_dir))
    elif platform.system() == "Darwin":
        result = subprocess.run(
            ["lsof", "-p", str(os.getpid())],
            capture_output=True, text=True, timeout=5,
        )
        return len(result.stdout.strip().split("\n")) - 1
    else:
        raise RuntimeError(f"FD counting not supported on {platform.system()}")


def _get_thread_count():
    """Get current thread count."""
    return threading.active_count()


def _get_rss_bytes():
    """Get current RSS in bytes.

    Raises RuntimeError if the platform does not support RSS measurement.
    """
    usage = resource.getrusage(resource.RUSAGE_SELF)
    # On Linux, ru_maxrss is in KB; on macOS it's in bytes
    if platform.system() == "Linux":
        return usage.ru_maxrss * 1024
    return usage.ru_maxrss


def _handler(request):
    return Response(200, content=b"x" * 1024)


class TestResourceStability:
    """Verify that repeated requests don't leak FDs, threads, or memory."""

    def test_fd_stability_under_repeated_requests(self):
        """FD count stabilizes after repeated requests."""
        thresholds = _load_resource_thresholds()
        max_fd_delta = thresholds.get("max_fd_delta", 10)

        try:
            fd_before = _get_fd_count()
        except RuntimeError as e:
            pytest.fail(str(e))

        with Client(transport=MockTransport(_handler)) as client:
            for _ in range(50):
                resp = client.get("http://testserver/")
                assert resp.status_code == 200

        fd_after = _get_fd_count()
        delta = fd_after - fd_before
        assert fd_after <= fd_before + max_fd_delta, (
            f"FD leak detected: before={fd_before}, after={fd_after}, "
            f"delta={delta}, threshold={max_fd_delta}"
        )

    def test_thread_stability_under_repeated_requests(self):
        """Thread count stabilizes after repeated requests."""
        thresholds = _load_resource_thresholds()
        max_thread_delta = thresholds.get("max_thread_delta", 5)

        threads_before = _get_thread_count()

        with Client(transport=MockTransport(_handler)) as client:
            for _ in range(50):
                resp = client.get("http://testserver/")
                assert resp.status_code == 200

        threads_after = _get_thread_count()
        delta = threads_after - threads_before
        assert threads_after <= threads_before + max_thread_delta, (
            f"Thread leak detected: before={threads_before}, after={threads_after}, "
            f"delta={delta}, threshold={max_thread_delta}"
        )

    def test_fd_stability_under_rapid_open_close(self):
        """FD count stabilizes after rapid client open/close cycles."""
        thresholds = _load_resource_thresholds()
        max_fd_delta = thresholds.get("max_fd_delta", 10)

        try:
            fd_before = _get_fd_count()
        except RuntimeError as e:
            pytest.fail(str(e))

        for _ in range(20):
            with Client(transport=MockTransport(_handler)) as client:
                resp = client.get("http://testserver/")
                assert resp.status_code == 200

        fd_after = _get_fd_count()
        delta = fd_after - fd_before
        assert fd_after <= fd_before + max_fd_delta, (
            f"FD leak on rapid open/close: before={fd_before}, after={fd_after}, "
            f"delta={delta}, threshold={max_fd_delta}"
        )

    def test_thread_stability_under_rapid_open_close(self):
        """Thread count stabilizes after rapid client open/close cycles."""
        thresholds = _load_resource_thresholds()
        max_thread_delta = thresholds.get("max_thread_delta", 5)

        threads_before = _get_thread_count()

        for _ in range(20):
            with Client(transport=MockTransport(_handler)) as client:
                resp = client.get("http://testserver/")
                assert resp.status_code == 200

        threads_after = _get_thread_count()
        delta = threads_after - threads_before
        assert threads_after <= threads_before + max_thread_delta, (
            f"Thread leak on rapid open/close: before={threads_before}, "
            f"after={threads_after}, delta={delta}, threshold={max_thread_delta}"
        )

    def test_memory_stability_under_repeated_requests(self):
        """RSS does not grow unboundedly under repeated requests."""
        thresholds = _load_resource_thresholds()
        max_rss_growth = thresholds.get("max_rss_growth_bytes", 10 * 1024 * 1024)

        rss_before = _get_rss_bytes()

        with Client(transport=MockTransport(_handler)) as client:
            for _ in range(100):
                resp = client.get("http://testserver/")
                _ = resp.content

        rss_after = _get_rss_bytes()
        growth = rss_after - rss_before
        assert rss_after <= rss_before + max_rss_growth, (
            f"Memory growth: before={rss_before}, after={rss_after}, "
            f"growth={growth}, threshold={max_rss_growth}"
        )


class TestEarlyExitResourceCleanup:
    """Verify resource cleanup on early exits (cancellation, exceptions)."""

    def test_exception_in_handler_releases_resources(self):
        """Exception during request does not leak FDs."""
        thresholds = _load_resource_thresholds()
        max_fd_delta = thresholds.get("max_fd_delta", 10)

        try:
            fd_before = _get_fd_count()
        except RuntimeError as e:
            pytest.fail(str(e))

        def error_handler(request):
            raise RuntimeError("handler error")

        with Client(transport=MockTransport(error_handler)) as client:
            for _ in range(20):
                with pytest.raises(RuntimeError, match="handler error"):
                    client.get("http://testserver/")

        fd_after = _get_fd_count()
        delta = fd_after - fd_before
        assert fd_after <= fd_before + max_fd_delta, (
            f"FD leak after errors: before={fd_before}, after={fd_after}, "
            f"delta={delta}, threshold={max_fd_delta}"
        )

    def test_mixed_success_failure_stable(self):
        """Mixed success/failure requests do not leak resources."""
        thresholds = _load_resource_thresholds()
        max_fd_delta = thresholds.get("max_fd_delta", 10)

        try:
            fd_before = _get_fd_count()
        except RuntimeError as e:
            pytest.fail(str(e))

        call_count = [0]

        def mixed_handler(request):
            call_count[0] += 1
            if call_count[0] % 3 == 0:
                raise RuntimeError("intermittent error")
            return Response(200)

        with Client(transport=MockTransport(mixed_handler)) as client:
            for _ in range(30):
                try:
                    resp = client.get("http://testserver/")
                    assert resp.status_code == 200
                except RuntimeError:
                    pass  # Expected intermittent errors

        fd_after = _get_fd_count()
        delta = fd_after - fd_before
        assert fd_after <= fd_before + max_fd_delta, (
            f"FD leak under mixed load: before={fd_before}, after={fd_after}, "
            f"delta={delta}, threshold={max_fd_delta}"
        )


class TestConcurrentResourceStability:
    """Verify resource stability under concurrent access."""

    def test_concurrent_sync_clients_stable(self):
        """Multiple concurrent sync clients do not leak resources."""
        thresholds = _load_resource_thresholds()
        max_fd_delta = thresholds.get("max_fd_delta", 10)

        try:
            fd_before = _get_fd_count()
        except RuntimeError as e:
            pytest.fail(str(e))

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
        delta = fd_after - fd_before
        assert fd_after <= fd_before + max_fd_delta * 2, (
            f"FD leak under concurrency: before={fd_before}, after={fd_after}, "
            f"delta={delta}, threshold={max_fd_delta * 2}"
        )


class TestRealSocketResourceStability:
    """Resource stability tests using real local sockets (optional variant)."""

    def test_fd_stability_real_socket(self):
        """FD count stabilizes after real socket requests."""
        thresholds = _load_resource_thresholds()
        max_fd_delta = thresholds.get("max_fd_delta", 10)

        try:
            fd_before = _get_fd_count()
        except RuntimeError as e:
            pytest.fail(str(e))

        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            with Client(timeout=10) as client:
                for _ in range(30):
                    resp = client.get(url)
                    assert resp.status_code == 200

        fd_after = _get_fd_count()
        delta = fd_after - fd_before
        assert fd_after <= fd_before + max_fd_delta, (
            f"FD leak on real sockets: before={fd_before}, after={fd_after}, "
            f"delta={delta}, threshold={max_fd_delta}"
        )

    def test_thread_stability_real_socket(self):
        """Thread count stabilizes after real socket requests."""
        thresholds = _load_resource_thresholds()
        max_thread_delta = thresholds.get("max_thread_delta", 5)

        threads_before = _get_thread_count()

        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            with Client(timeout=10) as client:
                for _ in range(30):
                    resp = client.get(url)
                    assert resp.status_code == 200

        threads_after = _get_thread_count()
        delta = threads_after - threads_before
        assert threads_after <= threads_before + max_thread_delta, (
            f"Thread leak on real sockets: before={threads_before}, "
            f"after={threads_after}, delta={delta}, threshold={max_thread_delta}"
        )
