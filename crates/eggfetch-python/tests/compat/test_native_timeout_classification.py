"""Native timeout classification tests using real local sockets."""
import http.server
import socket
import sys
import threading
import time

import pytest

sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parent))
from eggfetch.compat.httpx import Client, Timeout
from eggfetch.compat.httpx._exceptions import (
    ConnectTimeout,
    ReadTimeout,
    TimeoutException,
)
from native_fixtures import local_http_server, local_stall_server, HeadersStallHandler


class TestNativeReadTimeout:
    """Read timeout classification using real local sockets."""

    def test_total_timeout_on_slow_endpoint(self):
        """Total timeout fires when server delays response."""
        with local_http_server() as (host, port):
            with Client(timeout=Timeout(0.5)) as c:
                start = time.monotonic()
                with pytest.raises(TimeoutException) as exc_info:
                    c.get(f"http://{host}:{port}/slow")
                elapsed = time.monotonic() - start
                assert isinstance(exc_info.value, ReadTimeout), (
                    f"Expected ReadTimeout, got {type(exc_info.value).__name__}"
                )
                assert "total timeout" in str(exc_info.value).lower()
                assert elapsed < 5.0, f"Timeout took too long: {elapsed:.2f}s"

    def test_read_timeout_on_headers_then_stall(self):
        """Server sends headers then stalls on body; client detects error."""
        srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        srv.bind(("127.0.0.1", 0))
        srv.listen(5)
        port = srv.getsockname()[1]

        httpd = http.server.HTTPServer(
            ("127.0.0.1", port), HeadersStallHandler, bind_and_activate=False
        )
        httpd.socket = srv
        t = threading.Thread(target=httpd.serve_forever, daemon=True)
        t.start()

        try:
            with Client(timeout=Timeout(0.5)) as c:
                start = time.monotonic()
                with pytest.raises(ReadTimeout) as exc_info:
                    c.get(f"http://127.0.0.1:{port}/headers-then-stall")
                elapsed = time.monotonic() - start
                assert isinstance(exc_info.value, TimeoutException)
                assert not isinstance(exc_info.value, ConnectTimeout), (
                    "Should be ReadTimeout, not ConnectTimeout"
                )
                assert elapsed < 5.0, f"Timeout took too long: {elapsed:.2f}s"
                assert hasattr(exc_info.value, "request"), (
                    "Timeout exception must retain request context"
                )
        finally:
            httpd.shutdown()
            srv.close()

    def test_read_timeout_on_stall_server(self):
        """Read timeout fires when server accepts but never responds."""
        with local_stall_server() as (host, port, ready):
            ready.wait()
            with Client(timeout=Timeout(0.5)) as c:
                start = time.monotonic()
                with pytest.raises(TimeoutException) as exc_info:
                    c.get(f"http://{host}:{port}/anything")
                elapsed = time.monotonic() - start
                assert isinstance(exc_info.value, TimeoutException)
                assert elapsed < 5.0, f"Timeout took too long: {elapsed:.2f}s"

    def test_read_timeout_is_not_connect_error(self):
        """Read timeout is not classified as ConnectTimeout."""
        with local_http_server() as (host, port):
            with Client(timeout=Timeout(0.5)) as c:
                with pytest.raises(ReadTimeout) as exc_info:
                    c.get(f"http://{host}:{port}/slow")
                assert not isinstance(exc_info.value, ConnectTimeout)


class TestNativeTimeoutPassthrough:
    """Verify timeout configuration reaches the native engine."""

    def test_scalar_timeout_allows_fast_requests(self):
        """Fast requests succeed well within timeout."""
        with local_http_server() as (host, port):
            with Client(timeout=Timeout(5.0)) as c:
                resp = c.get(f"http://{host}:{port}/health")
                assert resp.status_code == 200

    def test_per_request_timeout_override(self):
        """Per-request timeout overrides client default."""
        with local_http_server() as (host, port):
            with Client(timeout=Timeout(5.0)) as c:
                resp = c.get(
                    f"http://{host}:{port}/health", timeout=Timeout(10.0)
                )
                assert resp.status_code == 200

    def test_timeout_none_disables_phases(self):
        """timeout=None disables all timeout phases."""
        with local_http_server() as (host, port):
            with Client(timeout=Timeout(5.0)) as c:
                resp = c.get(f"http://{host}:{port}/health", timeout=None)
                assert resp.status_code == 200

    def test_timeout_retains_request_context(self):
        """Timeout exceptions retain the original request reference."""
        with local_stall_server() as (host, port, ready):
            ready.wait()
            with Client(timeout=Timeout(0.5)) as c:
                with pytest.raises(TimeoutException) as exc_info:
                    c.get(f"http://{host}:{port}/slow")
                assert hasattr(exc_info.value, "request"), (
                    f"Timeout exception must have .request attribute, "
                    f"got {dir(exc_info.value)}"
                )

    def test_connect_timeout_on_refused_port(self):
        """Connect timeout fires on unreachable host."""
        with Client(timeout=Timeout(0.3)) as c:
            start = time.monotonic()
            with pytest.raises(ConnectTimeout) as exc_info:
                c.get("http://127.0.0.1:1/")
            elapsed = time.monotonic() - start
            assert isinstance(exc_info.value, ConnectTimeout)
            assert elapsed < 5.0, f"Timeout took too long: {elapsed:.2f}s"
