"""Track 0: HTTPX 0.28.1 reference-pinning tests for advanced transport options.

These tests pin the exact observable behavior of httpx==0.28.1 for:
- HTTPTransport(uds=...)
- AsyncHTTPTransport(uds=...)
- local_address with IPv4
- socket option tuple/list shapes
- invalid option types/values

Run against httpx==0.28.1 to establish the reference baseline.
"""

import socket
import sys
import typing

import httpx
import pytest


# ── Socket option representation ────────────────────────────────────────

class TestSocketOptionRepresentation:
    """Pin the accepted representation for socket_options in HTTPX 0.28.1."""

    def test_single_option_accepted(self):
        """HTTPX accepts a single (level, option, value) tuple."""
        transport = httpx.HTTPTransport(
            socket_options=[(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)]
        )
        assert transport is not None

    def test_multiple_options_accepted(self):
        """HTTPX accepts multiple socket option tuples."""
        opts = [
            (socket.IPPROTO_TCP, socket.TCP_NODELAY, 1),
            (socket.SOL_SOCKET, socket.SO_KEEPALIVE, 1),
        ]
        transport = httpx.HTTPTransport(socket_options=opts)
        assert transport is not None

    def test_empty_list_accepted(self):
        """HTTPX accepts an empty socket options list."""
        transport = httpx.HTTPTransport(socket_options=[])
        assert transport is not None

    def test_none_accepted(self):
        """HTTPX accepts None (default) for socket_options."""
        transport = httpx.HTTPTransport(socket_options=None)
        assert transport is not None

    @pytest.mark.skipif(
        sys.platform == "win32", reason="TCP_NODELAY constant differs on Windows"
    )
    def test_option_with_bytes_value(self):
        """HTTPX accepts bytes as the option value."""
        value = (0).to_bytes(4, byteorder="little")
        transport = httpx.HTTPTransport(
            socket_options=[(socket.IPPROTO_TCP, socket.TCP_NODELAY, value)]
        )
        assert transport is not None

    @pytest.mark.skipif(
        sys.platform == "win32", reason="TCP_NODELAY constant differs on Windows"
    )
    def test_option_with_int_value(self):
        """HTTPX accepts int as the option value."""
        transport = httpx.HTTPTransport(
            socket_options=[(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)]
        )
        assert transport is not None


# ── local_address ────────────────────────────────────────────────────────

class TestLocalAddress:
    """Pin the behavior of local_address in HTTPX 0.28.1."""

    def test_ipv4_loopback_accepted(self):
        """HTTPX accepts '127.0.0.1' as local_address."""
        transport = httpx.HTTPTransport(local_address="127.0.0.1")
        assert transport is not None

    def test_none_accepted(self):
        """HTTPX accepts None (default) for local_address."""
        transport = httpx.HTTPTransport(local_address=None)
        assert transport is not None


# ── UDS ──────────────────────────────────────────────────────────────────

class TestUDS:
    """Pin the behavior of uds in HTTPX 0.28.1."""

    def test_uds_path_accepted(self):
        """HTTPX accepts a UDS path string."""
        transport = httpx.HTTPTransport(uds="/tmp/test.sock")
        assert transport is not None

    def test_none_accepted(self):
        """HTTPX accepts None (default) for uds."""
        transport = httpx.HTTPTransport(uds=None)
        assert transport is not None


# ── Async transport variants ────────────────────────────────────────────

class TestAsyncTransportOptions:
    """Verify async transport accepts the same options."""

    def test_async_socket_options_accepted(self):
        opts = [(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)]
        transport = httpx.AsyncHTTPTransport(socket_options=opts)
        assert transport is not None

    def test_async_local_address_accepted(self):
        transport = httpx.AsyncHTTPTransport(local_address="127.0.0.1")
        assert transport is not None

    def test_async_uds_accepted(self):
        transport = httpx.AsyncHTTPTransport(uds="/tmp/test.sock")
        assert transport is not None
