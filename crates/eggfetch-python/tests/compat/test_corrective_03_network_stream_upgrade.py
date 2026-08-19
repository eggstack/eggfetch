"""Tests for HTTPX parity corrective 03 — network stream and upgrade exposure.

Covers the plan's acceptance criteria for 101 upgraded streams,
CONNECT classification, runtime lifecycle, and ``start_tls`` policy.

References:
    - plans/httpx-parity-corrective-03-network-stream-upgrade-exposure.md
    - docs/architecture/core-engine.md (Network Stream and Upgrade Support)
    - docs/architecture/python-bindings.md (Wire metadata on response)
"""

from __future__ import annotations

import http.server
import shutil
import socket
import socketserver
import ssl
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path
from typing import Optional

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))


# ---------------------------------------------------------------------------
# Local fixtures
# ---------------------------------------------------------------------------


def _pick_free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def _start_server(handler_cls, *, port: Optional[int] = None) -> socketserver.TCPServer:
    socketserver.TCPServer.allow_reuse_address = True
    server = socketserver.TCPServer(("127.0.0.1", port or 0), handler_cls)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    time.sleep(0.05)
    return server


class _OkHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):  # noqa: N802 — http.server contract
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"hi")

    def log_message(self, *_args, **_kwargs):
        pass


class _UpgradeEchoHandler(http.server.BaseHTTPRequestHandler):
    """A simple HTTP/1.1 server that responds to 101 Switching Protocols.

    After the 101 response is sent, the connection stays open and the
    server echoes any bytes back. Leading bytes are sent before any
    client traffic to exercise the rewind buffer.
    """

    LEADING = b"LEADING"
    SUPPORTED_PATHS = {"/echo", "/plain", "/no-leading", "/close", "/slow"}

    def do_GET(self):  # noqa: N802 — http.server contract
        if self.path not in self.SUPPORTED_PATHS:
            self.send_response(404)
            self.end_headers()
            return

        # Flush the 101 response, then enter a bounded echo loop.
        try:
            self.wfile.write(
                b"HTTP/1.1 101 Switching Protocols\r\n"
                b"Upgrade: echo\r\n"
                b"Connection: Upgrade\r\n"
                b"\r\n"
            )
            if self.path in {"/", "/echo", "/plain"}:
                self.wfile.write(self.LEADING)
                self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            return

        self.connection.settimeout(2.0)
        buf = bytearray(4096)
        try:
            while True:
                n = self.connection.recv_into(buf)
                if not n:
                    break
                if self.path == "/close":
                    self.connection.sendall(bytes(buf[:n]))
                    try:
                        self.connection.shutdown(socket.SHUT_RDWR)
                    except OSError:
                        pass
                    return
                if self.path == "/slow":
                    time.sleep(0.05)
                self.connection.sendall(bytes(buf[:n]))
        except (OSError, socket.timeout):
            return

    def log_message(self, *_args, **_kwargs):
        pass


# ---------------------------------------------------------------------------
# 101 native sync
# ---------------------------------------------------------------------------


def test_sync_101_extensions_network_stream_is_upgraded():
    """A 101 response exposes the upgraded stream in extensions."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        client = eggfetch.Client()
        response = client.request(
            "GET",
            f"http://127.0.0.1:{server.server_address[1]}/echo",
            headers={"upgrade": "echo", "connection": "Upgrade"},
        )
        assert response.status_code == 101
        ext = response.extensions
        assert "network_stream" in ext
        ns = ext["network_stream"]
        assert ns is not None
        assert ns.is_upgraded
    finally:
        server.shutdown()
        server.server_close()


def test_sync_101_first_read_returns_leading_bytes():
    """Leading bytes sent before the upgrade is acknowledged come first."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        client = eggfetch.Client()
        response = client.request(
            "GET",
            f"http://127.0.0.1:{server.server_address[1]}/echo",
            headers={"upgrade": "echo", "connection": "Upgrade"},
        )
        assert response.status_code == 101
        ns = response.extensions["network_stream"]
        assert ns is not None
        first = ns.read(max_bytes=1024)
        assert first == b"LEADING"
    finally:
        server.shutdown()
        server.server_close()


def test_sync_101_write_read_roundtrip():
    """After the leading bytes, write/read echo works."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        client = eggfetch.Client()
        response = client.request(
            "GET",
            f"http://127.0.0.1:{server.server_address[1]}/echo",
            headers={"upgrade": "echo", "connection": "Upgrade"},
        )
        ns = response.extensions["network_stream"]
        assert ns is not None
        # Drain leading bytes first.
        ns.read(max_bytes=1024)
        payload = b"hello from upgraded"
        ns.write(payload)
        echoed = ns.read(max_bytes=1024)
        assert echoed == payload
    finally:
        server.shutdown()
        server.server_close()


def test_sync_101_close_is_idempotent():
    """Repeated close() does not raise."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        client = eggfetch.Client()
        response = client.request(
            "GET",
            f"http://127.0.0.1:{server.server_address[1]}/echo",
            headers={"upgrade": "echo", "connection": "Upgrade"},
        )
        ns = response.extensions["network_stream"]
        assert ns is not None
        ns.close()
        # Should be a no-op the second time.
        ns.close()
    finally:
        server.shutdown()
        server.server_close()


def test_sync_101_get_extra_info_unavailable_for_hyper_adapter():
    """``get_extra_info`` returns None for keys whose underlying transport
    does not expose the value (Hyper's adapter does not surface
    socket addresses)."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        client = eggfetch.Client()
        response = client.request(
            "GET",
            f"http://127.0.0.1:{server.server_address[1]}/echo",
            headers={"upgrade": "echo", "connection": "Upgrade"},
        )
        ns = response.extensions["network_stream"]
        assert ns is not None
        for key in ("client_addr", "server_addr", "ssl_version", "ssl_cipher"):
            value = ns.get_extra_info(key)
            assert value is None or isinstance(value, str)
    finally:
        server.shutdown()
        server.server_close()


# ---------------------------------------------------------------------------
# Ordinary response: no network_stream
# ---------------------------------------------------------------------------


def test_ordinary_response_extensions_network_stream_is_none():
    """Non-101 responses must not expose a writable network_stream."""
    import eggfetch

    server = _start_server(_OkHandler)
    try:
        client = eggfetch.Client()
        response = client.request("GET", f"http://127.0.0.1:{server.server_address[1]}/")
        assert response.status_code == 200
        ext = response.extensions
        assert ext["network_stream"] is None
    finally:
        server.shutdown()
        server.server_close()


def test_streaming_response_exposes_network_stream_only_in_extensions():
    """Streaming responses also expose `network_stream` via the dedicated
    reader, but the extensions dict either stays None (the canonical
    access path is via the `network_stream` getter) or carries the same
    handle."""
    import eggfetch

    server = _start_server(_OkHandler)
    try:
        client = eggfetch.Client()
        with client.stream("GET", f"http://127.0.0.1:{server.server_address[1]}/") as response:
            assert response.status_code == 200
            ext = response.extensions
            # For ordinary (non-101) streaming, the connection is held in
            # the body iterator; extensions must not double-expose it.
            assert ext["network_stream"] is None
            body = b"".join(response.iter_bytes())
            assert body == b"hi"
    finally:
        server.shutdown()
        server.server_close()


# ---------------------------------------------------------------------------
# 101 native async (streaming)
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_async_101_streaming_network_stream_is_upgraded():
    """An async 101 streaming response exposes the upgraded stream via the
    ``network_stream`` accessor."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        async with eggfetch.AsyncClient() as client:
            async with await client.stream(
                "GET",
                f"http://127.0.0.1:{server.server_address[1]}/echo",
                headers={"upgrade": "echo", "connection": "Upgrade"},
            ) as response:
                assert response.status_code == 101
                ns = response.network_stream
                # The stream path uses the async wrapper.
                assert ns is not None
                assert ns.is_upgraded
    finally:
        server.shutdown()
        server.server_close()


@pytest.mark.asyncio
async def test_async_101_first_read_returns_leading_bytes():
    """The async read coroutine returns the leading bytes."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        async with eggfetch.AsyncClient() as client:
            async with await client.stream(
                "GET",
                f"http://127.0.0.1:{server.server_address[1]}/echo",
                headers={"upgrade": "echo", "connection": "Upgrade"},
            ) as response:
                ns = response.network_stream
                assert ns is not None
                first = await ns.read(max_bytes=1024)
                assert first == b"LEADING"
    finally:
        server.shutdown()
        server.server_close()


@pytest.mark.asyncio
async def test_async_101_write_read_roundtrip():
    """Async write/read echo works after the upgrade."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        async with eggfetch.AsyncClient() as client:
            async with await client.stream(
                "GET",
                f"http://127.0.0.1:{server.server_address[1]}/echo",
                headers={"upgrade": "echo", "connection": "Upgrade"},
            ) as response:
                ns = response.network_stream
                assert ns is not None
                await ns.read(max_bytes=1024)
                payload = b"async echo hello"
                await ns.write(payload)
                echoed = await ns.read(max_bytes=1024)
                assert echoed == payload
                await ns.aclose()
    finally:
        server.shutdown()
        server.server_close()


@pytest.mark.asyncio
async def test_async_101_aclose_is_idempotent():
    """Repeated aclose() does not raise."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        async with eggfetch.AsyncClient() as client:
            async with await client.stream(
                "GET",
                f"http://127.0.0.1:{server.server_address[1]}/echo",
                headers={"upgrade": "echo", "connection": "Upgrade"},
            ) as response:
                ns = response.network_stream
                assert ns is not None
                await ns.aclose()
                await ns.aclose()  # idempotent
    finally:
        server.shutdown()
        server.server_close()


@pytest.mark.asyncio
async def test_async_101_read_after_close_raises():
    """Async read after aclose raises a stable mapped error."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        async with eggfetch.AsyncClient() as client:
            async with await client.stream(
                "GET",
                f"http://127.0.0.1:{server.server_address[1]}/echo",
                headers={"upgrade": "echo", "connection": "Upgrade"},
            ) as response:
                ns = response.network_stream
                assert ns is not None
                await ns.aclose()
                with pytest.raises(ValueError):
                    await ns.read(max_bytes=1024)
    finally:
        server.shutdown()
        server.server_close()


# ---------------------------------------------------------------------------
# HTTPX compat facade
# ---------------------------------------------------------------------------


def test_httpx_compat_101_extensions_pass_through_network_stream():
    """The HTTPX compat response exposes the network_stream via extensions."""
    from eggfetch.compat.httpx import Client

    server = _start_server(_UpgradeEchoHandler)
    try:
        client = Client()
        request = client.build_request(
            "GET",
            f"http://127.0.0.1:{server.server_address[1]}/echo",
            headers={"upgrade": "echo", "connection": "Upgrade"},
        )
        response = client.send(request)
        assert response.status_code == 101
        ns = response.extensions.get("network_stream")
        assert ns is not None
        assert ns.is_upgraded
    finally:
        server.shutdown()
        server.server_close()


def test_httpx_compat_ordinary_response_network_stream_is_none():
    """The HTTPX compat response for ordinary responses has no network_stream."""
    from eggfetch.compat.httpx import Client

    server = _start_server(_OkHandler)
    try:
        client = Client()
        response = client.get(f"http://127.0.0.1:{server.server_address[1]}/")
        assert response.status_code == 200
        assert response.extensions.get("network_stream") is None
    finally:
        server.shutdown()
        server.server_close()


@pytest.mark.asyncio
async def test_httpx_compat_async_101_extensions_pass_through_network_stream():
    """The HTTPX compat async response exposes the network_stream."""
    from eggfetch.compat.httpx import AsyncClient

    server = _start_server(_UpgradeEchoHandler)
    try:
        async with AsyncClient() as client:
            async with client.stream(
                "GET",
                f"http://127.0.0.1:{server.server_address[1]}/echo",
                headers={"upgrade": "echo", "connection": "Upgrade"},
            ) as response:
                assert response.status_code == 101
                # The HTTPX compat facade propagates the network_stream
                # through the Python extensions dict.
                ns = response.extensions.get("network_stream")
                assert ns is not None
                assert ns.is_upgraded
    finally:
        server.shutdown()
        server.server_close()


# ---------------------------------------------------------------------------
# start_tls classification
# ---------------------------------------------------------------------------


def test_sync_101_start_tls_rejected_for_adapter_variant():
    """start_tls on a Hyper adapter-backed stream is rejected explicitly."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        client = eggfetch.Client()
        response = client.request(
            "GET",
            f"http://127.0.0.1:{server.server_address[1]}/echo",
            headers={"upgrade": "echo", "connection": "Upgrade"},
        )
        ns = response.extensions["network_stream"]
        assert ns is not None
        with pytest.raises(ValueError, match="adapter"):
            ns.start_tls(ssl_context=None, server_hostname="example.com")
    finally:
        server.shutdown()
        server.server_close()


def test_sync_101_start_tls_rejected_after_close():
    """start_tls on a closed stream returns a stable mapped error."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        client = eggfetch.Client()
        response = client.request(
            "GET",
            f"http://127.0.0.1:{server.server_address[1]}/echo",
            headers={"upgrade": "echo", "connection": "Upgrade"},
        )
        ns = response.extensions["network_stream"]
        assert ns is not None
        ns.close()
        with pytest.raises(ValueError):
            ns.start_tls(ssl_context=None, server_hostname="example.com")
    finally:
        server.shutdown()
        server.server_close()


@pytest.mark.asyncio
async def test_async_101_start_tls_rejected_for_adapter_variant():
    """Async start_tls on a Hyper adapter-backed stream is rejected."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        async with eggfetch.AsyncClient() as client:
            response = await client.request(
                "GET",
                f"http://127.0.0.1:{server.server_address[1]}/echo",
                headers={"upgrade": "echo", "connection": "Upgrade"},
            )
            ns = response.extensions["network_stream"]
            assert ns is not None
            with pytest.raises(ValueError, match="adapter"):
                await ns.start_tls(ssl_context=None, server_hostname="example.com")
    finally:
        server.shutdown()
        server.server_close()


# ---------------------------------------------------------------------------
# Runtime lifecycle
# ---------------------------------------------------------------------------


def test_sync_101_stream_survives_client_close():
    """An upgraded stream extracted from a sync client can outlive the
    client when the wrapper was constructed with a runtime lease."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        client = eggfetch.Client()
        response = client.request(
            "GET",
            f"http://127.0.0.1:{server.server_address[1]}/echo",
            headers={"upgrade": "echo", "connection": "Upgrade"},
        )
        ns = response.extensions["network_stream"]
        assert ns is not None
        # Drop the client while the stream is still alive.
        client.close()
        # The stream still serves IO.
        first = ns.read(max_bytes=1024)
        assert first == b"LEADING"
        ns.write(b"survived-client-close")
        echoed = ns.read(max_bytes=1024)
        assert echoed == b"survived-client-close"
        ns.close()
    finally:
        server.shutdown()
        server.server_close()


# ---------------------------------------------------------------------------
# Internal CONNECT classification
# ---------------------------------------------------------------------------


def _start_https_origin_server() -> "tuple[str, str, tempfile.TemporaryDirectory]":
    """Start a self-signed TLS origin server that returns 200 OK.

    Returns the origin URL, the cert path, and a temporary directory
    that **must be kept alive** for the cert to remain on disk.
    """
    if shutil.which("openssl") is None:
        pytest.skip("openssl binary required for the TLS origin fixture")

    tmpdir = tempfile.TemporaryDirectory()
    cert_path = f"{tmpdir.name}/server.pem"
    key_path = f"{tmpdir.name}/server.key"
    subprocess.run(
        [
            "openssl", "req", "-x509", "-newkey", "rsa:2048",
            "-keyout", key_path, "-out", cert_path,
            "-days", "1", "-nodes",
            "-subj", "/CN=127.0.0.1",
            "-addext", "subjectAltName=IP:127.0.0.1",
        ],
        check=True,
        capture_output=True,
    )
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.bind(("127.0.0.1", 0))
    sock.listen(8)
    port = sock.getsockname()[1]
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(certfile=cert_path, keyfile=key_path)

    def serve():
        while True:
            try:
                raw, _ = sock.accept()
            except OSError:
                return
            try:
                tls = ctx.wrap_socket(raw, server_side=True)
            except ssl.SSLError:
                raw.close()
                continue
            t = threading.Thread(
                target=_handle_tls_connection, args=(tls,), daemon=True
            )
            t.start()

    thread = threading.Thread(target=serve, daemon=True)
    thread.start()
    time.sleep(0.05)
    return f"https://127.0.0.1:{port}", cert_path, tmpdir


def _handle_tls_connection(conn):
    try:
        data = b""
        while b"\r\n\r\n" not in data:
            chunk = conn.recv(4096)
            if not chunk:
                return
            data += chunk
        response = (
            b"HTTP/1.1 200 OK\r\n"
            b"Content-Length: 2\r\n"
            b"Connection: close\r\n"
            b"\r\n"
            b"ok"
        )
        conn.sendall(response)
    finally:
        try:
            conn.close()
        except OSError:
            pass


def test_https_through_proxy_response_has_no_network_stream():
    """An HTTPS-through-proxy response must not expose the internal
    CONNECT tunnel as a writable network_stream.

    The plan requires that the internal CONNECT tunnel is classified as
    ``None`` (not exposed as a network_stream) — the canonical access path
    for the tunnel is the body iterator, not the upgraded stream.
    """
    import eggfetch
    import socket as _socket
    from native_fixtures import local_proxy_server

    # 1. Start a self-signed TLS origin server. The cert is generated
    # in a long-lived TemporaryDirectory so the cert file is available
    # to the client when verify= is read.
    server_sock = _socket.socket(_socket.AF_INET, _socket.SOCK_STREAM)
    server_sock.bind(("127.0.0.1", 0))
    server_sock.listen(8)
    origin_port = server_sock.getsockname()[1]
    if shutil.which("openssl") is None:
        pytest.skip("openssl binary required for the TLS origin fixture")
    tmpdir = tempfile.TemporaryDirectory()
    cert_path = f"{tmpdir.name}/server.pem"
    key_path = f"{tmpdir.name}/server.key"
    subprocess.run(
        [
            "openssl", "req", "-x509", "-newkey", "rsa:2048",
            "-keyout", key_path, "-out", cert_path,
            "-days", "1", "-nodes",
            "-subj", "/CN=127.0.0.1",
            "-addext", "subjectAltName=IP:127.0.0.1",
        ],
        check=True,
        capture_output=True,
    )
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(certfile=cert_path, keyfile=key_path)

    def serve_origin():
        while True:
            try:
                raw, _ = server_sock.accept()
            except OSError:
                return
            try:
                tls = ctx.wrap_socket(raw, server_side=True)
            except ssl.SSLError:
                raw.close()
                continue
            threading.Thread(
                target=_handle_tls_connection, args=(tls,), daemon=True
            ).start()

    threading.Thread(target=serve_origin, daemon=True).start()
    time.sleep(0.05)

    try:
        with local_proxy_server() as (proxy_host, proxy_port, _handler):
            client = eggfetch.Client(
                proxy=f"http://{proxy_host}:{proxy_port}",
                verify=False,
            )
            response = client.request(
                "GET", f"https://127.0.0.1:{origin_port}/"
            )
            assert response.status_code == 200
            # The internal CONNECT tunnel must not be exposed.
            assert response.extensions["network_stream"] is None
    finally:
        try:
            server_sock.close()
        except OSError:
            pass
        tmpdir.cleanup()


# ---------------------------------------------------------------------------
# Network stream API surface
# ---------------------------------------------------------------------------


def test_available_extra_info_keys_match_documented_set():
    """``get_extra_info`` accepts the documented keys and returns
    appropriate typed values (None or string)."""
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        client = eggfetch.Client()
        response = client.request(
            "GET",
            f"http://127.0.0.1:{server.server_address[1]}/echo",
            headers={"upgrade": "echo", "connection": "Upgrade"},
        )
        ns = response.extensions["network_stream"]
        assert ns is not None
        for key in (
            "client_addr",
            "server_addr",
            "ssl_version",
            "ssl_cipher",
            "ssl_alpn",
            "ssl_server_name",
        ):
            value = ns.get_extra_info(key)
            assert value is None or isinstance(value, str)
        # Unknown keys must return None rather than raise.
        assert ns.get_extra_info("__not_a_key__") is None
    finally:
        server.shutdown()
        server.server_close()


def test_read_with_timeout_error_round_trip():
    """``read(timeout=...)`` returns ``TimeoutError`` when the server does
    not send any data within the timeout window.

    The server is configured to receive-then-echo: when the client
    sends no data, the server blocks on ``recv`` and the read times
    out.
    """
    import eggfetch

    server = _start_server(_UpgradeEchoHandler)
    try:
        client = eggfetch.Client()
        response = client.request(
            "GET",
            # /plain does not echo; after the 101, the server waits
            # forever for the client to send data.
            f"http://127.0.0.1:{server.server_address[1]}/plain",
            headers={"upgrade": "echo", "connection": "Upgrade"},
        )
        ns = response.extensions["network_stream"]
        assert ns is not None
        # Drain leading bytes.
        ns.read(max_bytes=1024)
        # Server waits for client data; the read should time out.
        with pytest.raises(TimeoutError):
            ns.read(max_bytes=1024, timeout=0.5)
    finally:
        server.shutdown()
        server.server_close()
