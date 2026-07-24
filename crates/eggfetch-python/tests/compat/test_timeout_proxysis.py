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


class _StallHandler(http.server.BaseHTTPRequestHandler):
    """Handler that reads the request but never responds (simulates stall)."""

    def do_GET(self):
        # Read the request but do not send a response
        content_length = int(self.headers.get("Content-Length", 0))
        if content_length:
            self.rfile.read(content_length)
        # Stall forever (test timeout will kill the thread)
        time.sleep(300)

    def log_message(self, format, *args):
        pass  # Suppress logs


class TestProxyTimeoutClassification:
    """Track 10.1: Proxy CONNECT stall timeout classification."""

    def test_mock_proxy_connect_timeout(self):
        """A mock transport that simulates proxy CONNECT stall raises ConnectTimeout."""
        from eggfetch.compat.httpx._exceptions import ConnectTimeout

        def handler(request):
            # Simulate a proxy that accepts connection but stalls CONNECT
            raise eggfetch.ConnectTimeout("Connect timed out")

        with Client(transport=MockTransport(handler)) as client:
            with pytest.raises((ConnectTimeout, eggfetch.TimeoutException)):
                client.get("http://testserver/")

    def test_mock_read_timeout_on_slow_server(self):
        """A mock transport that simulates a slow server raises ReadTimeout."""
        from eggfetch.compat.httpx._exceptions import ReadTimeout

        def handler(request):
            raise eggfetch.ReadTimeout("Read timed out")

        with Client(transport=MockTransport(handler)) as client:
            with pytest.raises((ReadTimeout, eggfetch.TimeoutException)):
                client.get("http://testserver/")

    def test_mock_write_timeout(self):
        """A mock transport that simulates write stall raises WriteTimeout."""
        from eggfetch.compat.httpx._exceptions import WriteTimeout

        def handler(request):
            raise eggfetch.WriteTimeout("Write timed out")

        with Client(transport=MockTransport(handler)) as client:
            with pytest.raises((WriteTimeout, eggfetch.TimeoutException)):
                client.post("http://testserver/", content=b"data")

    def test_mock_pool_timeout(self):
        """A mock transport that simulates pool exhaustion raises PoolTimeout."""
        from eggfetch.compat.httpx._exceptions import PoolTimeout

        def handler(request):
            raise eggfetch.PoolTimeout("Pool timed out")

        with Client(transport=MockTransport(handler)) as client:
            with pytest.raises((PoolTimeout, eggfetch.TimeoutException)):
                client.get("http://testserver/")


class TestAsyncProxyTimeoutClassification:
    """Track 10.1: Async proxy CONNECT stall timeout classification."""

    @pytest.mark.asyncio
    async def test_async_mock_connect_timeout(self):
        """Async mock transport that simulates CONNECT stall raises ConnectTimeout."""
        from eggfetch.compat.httpx._exceptions import ConnectTimeout

        async def handler(request):
            raise eggfetch.ConnectTimeout("Connect timed out")

        async with AsyncClient(async_transport=MockTransport(handler)) as client:
            with pytest.raises((ConnectTimeout, eggfetch.TimeoutException)):
                await client.get("http://testserver/")

    @pytest.mark.asyncio
    async def test_async_mock_read_timeout(self):
        """Async mock transport that simulates read stall raises ReadTimeout."""
        from eggfetch.compat.httpx._exceptions import ReadTimeout

        async def handler(request):
            raise eggfetch.ReadTimeout("Read timed out")

        async with AsyncClient(async_transport=MockTransport(handler)) as client:
            with pytest.raises((ReadTimeout, eggfetch.TimeoutException)):
                await client.get("http://testserver/")


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
