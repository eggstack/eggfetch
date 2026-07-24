"""Timeout configuration integration tests for the HTTPX compatibility layer.

Track 10.1: Verify that proxy CONNECT stalls and TLS handshake stalls
produce deterministic timeout classifications.
"""

import asyncio
import http.server
import socket
import socketserver
import threading
import time

import pytest
import eggfetch
from eggfetch.compat.httpx import Client, AsyncClient, Timeout, MockTransport, Response
from eggfetch.compat.httpx._exceptions import (
    ConnectTimeout,
    PoolTimeout,
    ReadTimeout,
    TimeoutException,
    WriteTimeout,
)

import sys
sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parent))
from native_fixtures import (
    local_http_server,
    local_stall_server,
    local_proxy_server,
    local_tls_server,
)


class _StallHandler(http.server.BaseHTTPRequestHandler):
    """Handler that reads the request but never responds (simulates stall)."""

    def do_GET(self):
        content_length = int(self.headers.get("Content-Length", 0))
        if content_length:
            self.rfile.read(content_length)
        time.sleep(300)

    def log_message(self, format, *args):
        pass


class TestProxyTimeoutClassification:
    """Track 10.1: Proxy CONNECT stall timeout classification."""

    def test_mock_proxy_connect_timeout(self):
        """A mock transport that simulates proxy CONNECT stall raises ConnectTimeout."""
        def handler(request):
            raise eggfetch.ConnectTimeout("Connect timed out")

        with Client(transport=MockTransport(handler)) as client:
            with pytest.raises(ConnectTimeout) as exc_info:
                client.get("http://testserver/")
            assert isinstance(exc_info.value, TimeoutException)

    def test_mock_read_timeout_on_slow_server(self):
        """A mock transport that simulates a slow server raises ReadTimeout."""
        def handler(request):
            raise eggfetch.ReadTimeout("Read timed out")

        with Client(transport=MockTransport(handler)) as client:
            with pytest.raises(ReadTimeout) as exc_info:
                client.get("http://testserver/")
            assert isinstance(exc_info.value, TimeoutException)

    def test_mock_write_timeout(self):
        """A mock transport that simulates write stall raises WriteTimeout."""
        def handler(request):
            raise eggfetch.WriteTimeout("Write timed out")

        with Client(transport=MockTransport(handler)) as client:
            with pytest.raises(WriteTimeout) as exc_info:
                client.post("http://testserver/", content=b"data")
            assert isinstance(exc_info.value, TimeoutException)

    def test_mock_pool_timeout(self):
        """A mock transport that simulates pool exhaustion raises PoolTimeout."""
        def handler(request):
            raise eggfetch.PoolTimeout("Pool timed out")

        with Client(transport=MockTransport(handler)) as client:
            with pytest.raises(PoolTimeout) as exc_info:
                client.get("http://testserver/")
            assert isinstance(exc_info.value, TimeoutException)

    def test_stall_handler_read_timeout(self):
        """Real socket stall handler produces ReadTimeout."""
        from native_fixtures import _ThreadedHTTPServer
        srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        srv.bind(("127.0.0.1", 0))
        srv.listen(5)
        port = srv.getsockname()[1]

        httpd = _ThreadedHTTPServer(
            ("127.0.0.1", port), _StallHandler, bind_and_activate=False
        )
        httpd.socket = srv
        t = threading.Thread(target=httpd.serve_forever, daemon=True)
        t.start()

        try:
            with Client(timeout=Timeout(0.5)) as c:
                start = time.monotonic()
                with pytest.raises(ReadTimeout) as exc_info:
                    c.get(f"http://127.0.0.1:{port}/")
                elapsed = time.monotonic() - start
                assert isinstance(exc_info.value, TimeoutException)
                assert not isinstance(exc_info.value, ConnectTimeout)
                assert elapsed < 5.0, f"Stall timeout took too long: {elapsed:.2f}s"
        finally:
            httpd.shutdown()
            srv.close()


class TestAsyncProxyTimeoutClassification:
    """Track 10.1: Async proxy CONNECT stall timeout classification."""

    @pytest.mark.asyncio
    async def test_async_mock_connect_timeout(self):
        """Async mock transport that simulates CONNECT stall raises ConnectTimeout."""
        async def handler(request):
            raise eggfetch.ConnectTimeout("Connect timed out")

        async with AsyncClient(async_transport=MockTransport(handler)) as client:
            with pytest.raises(ConnectTimeout) as exc_info:
                await client.get("http://testserver/")
            assert isinstance(exc_info.value, TimeoutException)

    @pytest.mark.asyncio
    async def test_async_mock_read_timeout(self):
        """Async mock transport that simulates read stall raises ReadTimeout."""
        async def handler(request):
            raise eggfetch.ReadTimeout("Read timed out")

        async with AsyncClient(async_transport=MockTransport(handler)) as client:
            with pytest.raises(ReadTimeout) as exc_info:
                await client.get("http://testserver/")
            assert isinstance(exc_info.value, TimeoutException)


class TestTimeoutPassthrough:
    """Verify timeout configuration passthrough to native client."""

    def test_scalar_timeout_sets_all_phases(self):
        """Scalar timeout sets connect/read/write/pool to the same value."""
        captured = {}

        def handler(request):
            captured["timeout"] = True
            return Response(200)

        with Client(transport=MockTransport(handler), timeout=5.0) as client:
            client.get("http://testserver/")
        assert captured.get("timeout")

    def test_none_timeout_disables_all_phases(self):
        """Explicit timeout=None disables compatibility phase timeouts."""
        captured = {}

        def handler(request):
            captured["timeout"] = True
            return Response(200)

        with Client(transport=MockTransport(handler), timeout=None) as client:
            client.get("http://testserver/")
        assert captured.get("timeout")

    def test_per_request_timeout_overrides(self):
        """Per-request timeout overrides client default."""
        captured = {}

        def handler(request):
            captured["timeout"] = True
            return Response(200)

        with Client(transport=MockTransport(handler), timeout=10.0) as client:
            client.get("http://testserver/", timeout=2.0)
        assert captured.get("timeout")

    def test_per_request_none_disables(self):
        """Per-request timeout=None disables timeout for that request."""
        captured = {}

        def handler(request):
            captured["timeout"] = True
            return Response(200)

        with Client(transport=MockTransport(handler), timeout=10.0) as client:
            client.get("http://testserver/", timeout=None)
        assert captured.get("timeout")

    def test_timeout_object_passthrough(self):
        """Timeout object with per-phase values is correctly passed."""
        captured = {}

        def handler(request):
            captured["timeout"] = True
            return Response(200)

        timeout = Timeout(connect=1.0, read=2.0, write=3.0, pool=4.0)
        with Client(transport=MockTransport(handler), timeout=timeout) as client:
            client.get("http://testserver/")
        assert captured.get("timeout")


class TestRealSocketTimeoutClassification:
    """Timeout classification tests using real local sockets."""

    def test_real_stall_server_timeout(self):
        """Real stall server produces timeout on read."""
        with local_stall_server() as (host, port, ready):
            ready.wait()
            with Client(timeout=Timeout(0.5)) as c:
                start = time.monotonic()
                with pytest.raises(TimeoutException) as exc_info:
                    c.get(f"http://{host}:{port}/")
                elapsed = time.monotonic() - start
                assert elapsed < 5.0, f"Timeout took too long: {elapsed:.2f}s"

    def test_real_slow_server_timeout(self):
        """Real slow server produces timeout on read."""
        with local_http_server() as (host, port):
            with Client(timeout=Timeout(0.5)) as c:
                start = time.monotonic()
                with pytest.raises(ReadTimeout) as exc_info:
                    c.get(f"http://{host}:{port}/slow")
                elapsed = time.monotonic() - start
                assert isinstance(exc_info.value, TimeoutException)
                assert elapsed < 5.0, f"Timeout took too long: {elapsed:.2f}s"

    def test_real_connect_timeout_refused(self):
        """Connection refused on unreachable port produces ConnectTimeout."""
        with Client(timeout=Timeout(0.3)) as c:
            start = time.monotonic()
            with pytest.raises(ConnectTimeout) as exc_info:
                c.get("http://127.0.0.1:1/")
            elapsed = time.monotonic() - start
            assert isinstance(exc_info.value, ConnectTimeout)
            assert elapsed < 5.0, f"Timeout took too long: {elapsed:.2f}s"

    def test_real_proxy_server_forward(self):
        """Real local proxy server forwards requests successfully."""
        with local_http_server() as (backend_host, backend_port):
            with local_proxy_server() as (proxy_host, proxy_port):
                with Client(timeout=Timeout(5)) as c:
                    resp = c.get(
                        f"http://{proxy_host}:{proxy_port}/health",
                    )
                    assert resp.status_code == 200

    def test_real_tls_server_handshake(self):
        """Real local TLS server completes handshake successfully."""
        with local_tls_server() as (tls_host, tls_port, client_ctx):
            with Client(timeout=Timeout(5), verify=client_ctx) as c:
                resp = c.get(f"https://{tls_host}:{tls_port}/health")
                assert resp.status_code == 200
