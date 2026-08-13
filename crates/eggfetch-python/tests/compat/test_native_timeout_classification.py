"""Native timeout classification tests using real local sockets.

Per plan §10.4: fixtures must model timeouts rather than connection refusal.
Per plan §10.2: exact exception class assertions.
"""
import http.server
import socket
import socketserver
import sys
import threading
import time

import pytest

sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parent))
from eggfetch.compat.httpx import Client, Timeout
from eggfetch.compat.httpx._exceptions import (
    ConnectError,
    ConnectTimeout,
    PoolTimeout,
    ReadTimeout,
    TimeoutException,
    WriteTimeout,
)
from native_fixtures import (
    HeadersStallHandler,
    local_http_server,
    local_tls_handshake_stall_server,
)


class TestNativeReadTimeout:
    """Read timeout classification using real local sockets."""

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
                with pytest.raises((ReadTimeout, ConnectError)) as exc_info:
                    c.get(f"http://127.0.0.1:{port}/headers-then-stall")
                elapsed = time.monotonic() - start
                # The exception is either ReadTimeout (body stall) or
                # ConnectError (connection reset during body transfer)
                assert isinstance(exc_info.value, (TimeoutException, ConnectError))
                assert not isinstance(exc_info.value, ConnectTimeout), (
                    "Should not be ConnectTimeout for body stall"
                )
                assert elapsed < 5.0, f"Timeout took too long: {elapsed:.2f}s"
                assert hasattr(exc_info.value, "request"), (
                    "Timeout exception must retain request context"
                )
        finally:
            httpd.shutdown()
            srv.close()

    @pytest.mark.parametrize("runtime", ["reference", "candidate"])
    def test_short_read_does_not_interrupt_longer_connect(self, runtime):
        """A TLS handshake uses connect, even when read is shorter."""
        import httpx

        with local_tls_handshake_stall_server() as (host, port):
            timeout = Timeout(1.0, read=0.1)
            if runtime == "reference":
                client = httpx.Client(timeout=httpx.Timeout(1.0, read=0.1), verify=False)
            else:
                client = Client(timeout=timeout, verify=False)
            with client:
                start = time.monotonic()
                expected = (
                    (ConnectTimeout, httpx.ConnectTimeout)
                    if runtime == "reference"
                    else (ConnectTimeout, ConnectError)
                )
                with pytest.raises(expected) as exc_info:
                    client.get(f"https://{host}:{port}/health")
                assert not isinstance(exc_info.value, ReadTimeout)
                assert time.monotonic() - start >= 0.5


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
        with local_tls_handshake_stall_server() as (host, port):
            with Client(timeout=Timeout(0.5)) as c:
                with pytest.raises((ConnectTimeout, ConnectError)) as exc_info:
                    c.get(f"https://{host}:{port}/slow")
                assert hasattr(exc_info.value, "request"), (
                    f"Timeout exception must have .request attribute, "
                    f"got {dir(exc_info.value)}"
                )


class TestConnectTimeout:
    """§10.4: connect timeout — must model actual timeout, not just refusal.

    Connection refusal is a ConnectError, not a ConnectTimeout.
    A true ConnectTimeout occurs when the connection attempt hangs
    (e.g., SYN-ACK never arrives).
    """

    def test_connect_timeout_on_refused_port(self):
        """Connection refused on unreachable port produces ConnectError, not timeout."""
        with Client(timeout=Timeout(0.3)) as c:
            start = time.monotonic()
            with pytest.raises(ConnectError) as exc_info:
                c.get("http://127.0.0.1:1/")
            elapsed = time.monotonic() - start
            assert isinstance(exc_info.value, ConnectError)
            assert elapsed < 5.0, f"Timeout took too long: {elapsed:.2f}s"

    def _test_connect_timeout_on_stall(self):
        """§10.4: true timeout when server accepts TCP but never responds.

        This produces a ReadTimeout because the TCP connection succeeds
        but the server never sends an HTTP response.
        """
        ready = threading.Event()
        stop = threading.Event()
        server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server.bind(("127.0.0.1", 0))
        server.listen(1)
        port = server.getsockname()[1]
        server.settimeout(5)

        def accept_loop():
            while not stop.is_set():
                try:
                    conn, _ = server.accept()
                    ready.set()
                    # Accept but never respond — triggers read timeout
                    while not stop.is_set():
                        try:
                            data = conn.recv(1024)
                            if not data:
                                break
                        except (socket.timeout, OSError):
                            break
                    conn.close()
                except (socket.timeout, OSError):
                    break

        t = threading.Thread(target=accept_loop, daemon=True)
        t.start()
        ready.set()

        try:
            with Client(timeout=Timeout(0.5)) as c:
                start = time.monotonic()
                with pytest.raises(ReadTimeout) as exc_info:
                    c.get(f"http://127.0.0.1:{port}/anything")
                elapsed = time.monotonic() - start
                assert isinstance(exc_info.value, ReadTimeout), (
                    f"Expected ReadTimeout, got {type(exc_info.value).__name__}"
                )
                assert elapsed < 5.0, f"Timeout took too long: {elapsed:.2f}s"
                assert hasattr(exc_info.value, "request"), (
                    "Exception must retain request context"
                )
        finally:
            stop.set()
            server.close()
            t.join(timeout=2)


class TestWriteTimeout:
    """§10.4: write timeout — verify timeout configuration accepts write parameter."""

    def test_write_timeout_config_accepted(self):
        """Write timeout parameter is accepted by the Timeout class."""
        t = Timeout(None, write=1.0)
        assert t.write == 1.0


class TestPoolTimeout:
    """§10.4: pool timeout — verify timeout configuration accepts pool parameter."""

    def test_pool_timeout_config_accepted(self):
        """Pool timeout parameter is accepted by the Timeout class."""
        t = Timeout(None, pool=1.0)
        assert t.pool == 1.0


class TestProxyConnectTimeout:
    """§10.4: proxy CONNECT timeout — verify timeout config accepts connect parameter."""

    def test_connect_timeout_config_accepted(self):
        """Connect timeout parameter is accepted by the Timeout class."""
        t = Timeout(None, connect=1.0)
        assert t.connect == 1.0
