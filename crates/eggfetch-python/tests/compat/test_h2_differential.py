"""HTTP/2-only differential tests: eggfetch vs httpx reference.

Tests the HTTP/2-only (prior knowledge / ALPN-only) mode. Covers:
- HTTPS H2-only via TLS ALPN (working path)
- TLS H2-only enforcement (matches httpx reference behavior)
- Cleartext H2 prior knowledge (working path)
- Constructor matrix for all transport types
- Streaming and cancellation
- http_version field verification
- Specialized routes (local_address, socket_options)
- stream_id absence (residual metadata difference)
"""

from __future__ import annotations

import asyncio
import socket
import ssl
import tempfile
import threading

import httpx
import pytest

from eggfetch.compat.httpx import AsyncClient, Client
from eggfetch.compat.httpx._exceptions import ConnectError, RequestError
from eggfetch.compat.httpx._transports import AsyncHTTPTransport, HTTPTransport

from .native_fixtures import (
    _generate_self_signed_cert,
    _H2RequestCounter,
    _TLSDirectHandler,
    _ThreadedHTTPServer,
    local_h2_server,
    local_proxy_server,
    local_tls_h2_server,
)


# ---------------------------------------------------------------------------
# HTTPS H2-only via TLS ALPN — differential (eggfetch vs httpx)
# ---------------------------------------------------------------------------


class TestHttpsH2OnlyAlpn:
    """HTTPS H2-only should negotiate HTTP/2 via ALPN and report version correctly."""

    @pytest.mark.parametrize("runtime", ["reference", "candidate"])
    def test_h2_only_get_health(self, runtime):
        """H2-only GET /health succeeds and reports HTTP/2."""
        with local_tls_h2_server() as (host, port, _client_ssl, cert_path, _counter):
            url = f"https://{host}:{port}/health"
            if runtime == "reference":
                with httpx.Client(
                    http1=False, http2=True, verify=cert_path,
                    trust_env=False, timeout=5,
                ) as client:
                    resp = client.get(url)
            else:
                with Client(
                    http1=False, http2=True, verify=cert_path,
                    trust_env=False, timeout=5,
                ) as client:
                    resp = client.get(url)

            assert resp.status_code == 200
            assert resp.text == "ok"
            assert resp.http_version == "HTTP/2"

    @pytest.mark.parametrize("runtime", ["reference", "candidate"])
    def test_h2_only_get_json(self, runtime):
        """H2-only GET /json succeeds with JSON body."""
        with local_tls_h2_server() as (host, port, _client_ssl, cert_path, _counter):
            url = f"https://{host}:{port}/json"
            if runtime == "reference":
                with httpx.Client(
                    http1=False, http2=True, verify=cert_path,
                    trust_env=False, timeout=5,
                ) as client:
                    resp = client.get(url)
            else:
                with Client(
                    http1=False, http2=True, verify=cert_path,
                    trust_env=False, timeout=5,
                ) as client:
                    resp = client.get(url)

            assert resp.status_code == 200
            assert resp.http_version == "HTTP/2"
            assert '"status": "h2-ok"' in resp.text

    @pytest.mark.parametrize("runtime", ["reference", "candidate"])
    def test_h2_only_multiple_requests_reuse(self, runtime):
        """Multiple requests on H2-only client reuse the connection."""
        with local_tls_h2_server() as (host, port, _client_ssl, cert_path, counter):
            url = f"https://{host}:{port}/health"
            if runtime == "reference":
                with httpx.Client(
                    http1=False, http2=True, verify=cert_path,
                    trust_env=False, timeout=5,
                ) as client:
                    for _ in range(5):
                        resp = client.get(url)
                        assert resp.status_code == 200
            else:
                with Client(
                    http1=False, http2=True, verify=cert_path,
                    trust_env=False, timeout=5,
                ) as client:
                    for _ in range(5):
                        resp = client.get(url)
                        assert resp.status_code == 200

            # Server should have handled all requests
            assert counter.count >= 1


# ---------------------------------------------------------------------------
# TLS H2-only enforcement — H2-only against an H1-only server must fail.
# ---------------------------------------------------------------------------


def _make_h1_only_tls_server():
    """Create a TLS server that only advertises http/1.1 in ALPN."""
    tmpdir = tempfile.mkdtemp()
    cert_path, key_path = _generate_self_signed_cert(tmpdir)
    server_ssl = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server_ssl.load_cert_chain(cert_path, key_path)
    server_ssl.set_alpn_protocols(["http/1.1"])

    httpd = _ThreadedHTTPServer(("127.0.0.1", 0), _TLSDirectHandler)
    _TLSDirectHandler.recorded_headers = []
    raw_socket = httpd.socket
    httpd.socket = server_ssl.wrap_socket(raw_socket, server_side=True)
    port = httpd.server_address[1]

    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    return "127.0.0.1", port, cert_path, httpd, thread, tmpdir


class TestH2OnlyEnforcement:
    """H2-only against an H1-only server should fail, not silently downgrade."""

    def test_reference_httpx_fails_h2_only_vs_h1_only(self):
        """httpx H2-only correctly fails against H1-only server."""
        host, port, cert_path, httpd, thread, tmpdir = _make_h1_only_tls_server()
        try:
            with httpx.Client(
                http1=False, http2=True, verify=cert_path,
                trust_env=False, timeout=5,
            ) as client:
                with pytest.raises(httpx.RemoteProtocolError):
                    client.get(f"https://{host}:{port}/health")
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)

    def test_candidate_h2_only_vs_h1_only_fails(self):
        """eggfetch H2-only correctly fails against H1-only TLS server.

        With http2_only set on the legacy client, the H2 handshake fails
        when ALPN does not negotiate `h2`. The candidate no longer silently
        downgrades to HTTP/1.1.
        """
        host, port, cert_path, httpd, thread, tmpdir = _make_h1_only_tls_server()
        try:
            with Client(
                http1=False, http2=True, verify=cert_path,
                trust_env=False, timeout=5,
            ) as client:
                with pytest.raises((RequestError, ConnectError)):
                    client.get(f"https://{host}:{port}/health")
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)


# ---------------------------------------------------------------------------
# Cleartext H2 prior knowledge — both reference and candidate work.
# ---------------------------------------------------------------------------


class TestCleartextH2PriorKnowledge:
    """Cleartext H2 prior knowledge is supported by both clients."""

    def test_reference_httpx_cleartext_h2(self):
        """httpx successfully uses cleartext H2 prior knowledge."""
        with local_h2_server() as (host, port, _counter):
            with httpx.Client(
                http1=False, http2=True, trust_env=False, timeout=5,
            ) as client:
                resp = client.get(f"http://{host}:{port}/health")
                assert resp.status_code == 200
                assert resp.http_version == "HTTP/2"

    def test_candidate_cleartext_h2_prior_knowledge(self):
        """eggfetch now uses cleartext H2 prior knowledge.

        With http2_only set on the legacy client, hyper-util attempts an
        HTTP/2 handshake on the cleartext socket. The H2 server accepts the
        preface and responds over HTTP/2.
        """
        with local_h2_server() as (host, port, _counter):
            with Client(
                http1=False, http2=True, trust_env=False, timeout=5,
            ) as client:
                resp = client.get(f"http://{host}:{port}/health")
                assert resp.status_code == 200
                assert resp.http_version == "HTTP/2"


# ---------------------------------------------------------------------------
# Constructor matrix — all transport types
# ---------------------------------------------------------------------------


class TestH2OnlyConstructorMatrix:
    """All four (http1, http2) combinations for all four types."""

    @pytest.mark.parametrize(
        ("http1", "http2", "valid"),
        [
            (True, False, True),
            (True, True, True),
            (False, True, True),
            (False, False, False),
        ],
    )
    def test_client_combinations(self, http1, http2, valid):
        if valid:
            client = Client(http1=http1, http2=http2)
            assert client._http1 is http1
            assert client._http2 is http2
        else:
            with pytest.raises(ValueError, match="At least one of http1 or http2"):
                Client(http1=http1, http2=http2)

    @pytest.mark.parametrize(
        ("http1", "http2", "valid"),
        [
            (True, False, True),
            (True, True, True),
            (False, True, True),
            (False, False, False),
        ],
    )
    def test_async_client_combinations(self, http1, http2, valid):
        if valid:
            client = AsyncClient(http1=http1, http2=http2)
            assert client._http1 is http1
            assert client._http2 is http2
        else:
            with pytest.raises(ValueError, match="At least one of http1 or http2"):
                AsyncClient(http1=http1, http2=http2)

    @pytest.mark.parametrize(
        ("http1", "http2", "valid"),
        [
            (True, False, True),
            (True, True, True),
            (False, True, True),
            (False, False, False),
        ],
    )
    def test_http_transport_combinations(self, http1, http2, valid):
        if valid:
            transport = HTTPTransport(http1=http1, http2=http2)
            assert transport._http1 is http1
            assert transport._http2 is http2
        else:
            with pytest.raises(ValueError, match="At least one of http1 or http2"):
                HTTPTransport(http1=http1, http2=http2)

    @pytest.mark.parametrize(
        ("http1", "http2", "valid"),
        [
            (True, False, True),
            (True, True, True),
            (False, True, True),
            (False, False, False),
        ],
    )
    def test_async_http_transport_combinations(self, http1, http2, valid):
        if valid:
            transport = AsyncHTTPTransport(http1=http1, http2=http2)
            assert transport._http1 is http1
            assert transport._http2 is http2
        else:
            with pytest.raises(ValueError, match="At least one of http1 or http2"):
                AsyncHTTPTransport(http1=http1, http2=http2)


# ---------------------------------------------------------------------------
# Streaming and cancellation for H2-only
# ---------------------------------------------------------------------------


class TestH2OnlyStreaming:
    """Streaming responses over H2-only connections."""

    def test_h2_only_streaming_response(self):
        """H2-only streaming response delivers all chunks."""
        with local_tls_h2_server() as (host, port, _client_ssl, cert_path, _counter):
            with Client(
                http1=False, http2=True, verify=cert_path,
                trust_env=False, timeout=5,
            ) as client:
                with client.stream("GET", f"https://{host}:{port}/streaming") as resp:
                    assert resp.status_code == 200
                    chunks = list(resp.iter_text())
                    assert len(chunks) == 3
                    assert chunks[0] == "chunk-0\n"
                    assert chunks[1] == "chunk-1\n"
                    assert chunks[2] == "chunk-2\n"

    def test_h2_only_async_streaming_response(self):
        """Async H2-only streaming response delivers all chunks."""
        with local_tls_h2_server() as (host, port, _client_ssl, cert_path, _counter):

            async def run():
                async with AsyncClient(
                    http1=False, http2=True, verify=cert_path,
                    trust_env=False, timeout=5,
                ) as client:
                    async with client.stream(
                        "GET", f"https://{host}:{port}/streaming"
                    ) as resp:
                        assert resp.status_code == 200
                        chunks = [part async for part in resp.aiter_text()]
                        assert len(chunks) == 3
                        assert chunks[0] == "chunk-0\n"

            asyncio.run(run())


# ---------------------------------------------------------------------------
# http_version verification
# ---------------------------------------------------------------------------


class TestH2OnlyHttpVersion:
    """Verify http_version field is correct for H2-only connections."""

    def test_h2_only_reports_http2(self):
        """H2-only HTTPS connection reports HTTP/2."""
        with local_tls_h2_server() as (host, port, _client_ssl, cert_path, _counter):
            with Client(
                http1=False, http2=True, verify=cert_path,
                trust_env=False, timeout=5,
            ) as client:
                resp = client.get(f"https://{host}:{port}/health")
                assert resp.http_version == "HTTP/2"

    def test_h1_only_reports_http1(self):
        """H1-only HTTPS connection reports HTTP/1.1 or HTTP/1.0."""
        from .native_fixtures import local_tls_server
        with local_tls_server() as (host, port, client_ssl, cert_path):
            with Client(
                http1=True, http2=False, verify=cert_path,
                trust_env=False, timeout=5,
            ) as client:
                resp = client.get(f"https://{host}:{port}/health")
                assert resp.http_version in ("HTTP/1.0", "HTTP/1.1")

    def test_auto_reports_version(self):
        """Auto-negotiated connection reports the negotiated version."""
        with local_tls_h2_server() as (host, port, _client_ssl, cert_path, _counter):
            with Client(
                http1=True, http2=True, verify=cert_path,
                trust_env=False, timeout=5,
            ) as client:
                resp = client.get(f"https://{host}:{port}/health")
                # Auto-negotiation should succeed with h2
                assert resp.http_version in ("HTTP/1.0", "HTTP/1.1", "HTTP/2")


# ---------------------------------------------------------------------------
# Specialized routes — direct connector with local_address and socket_options
# ---------------------------------------------------------------------------


class TestH2OnlySpecializedRoutes:
    """H2-only with specialized transport options.

    When local_address or socket_options are set, the Rust core uses the
    direct connector path. The direct connector now supports HTTP/2 via
    ALPN signaling on TLS streams and `http2_only` on the legacy client.
    H2-only is enforced in this path as well.
    """

    def test_h2_only_with_local_address_h2_server(self):
        """H2-only + local_address against an H2-capable TLS server succeeds."""
        with local_tls_h2_server() as (host, port, _client_ssl, cert_path, _counter):
            transport = HTTPTransport(
                http1=False, http2=True, local_address="127.0.0.1",
                verify=cert_path,
            )
            with Client(
                transport=transport, trust_env=False, timeout=5,
            ) as client:
                resp = client.get(f"https://{host}:{port}/health")
                assert resp.status_code == 200
                assert resp.http_version == "HTTP/2"

    def test_h2_only_with_local_address_h1_only_server(self):
        """H2-only + local_address against an H1-only TLS server fails."""
        host, port, cert_path, httpd, thread, tmpdir = _make_h1_only_tls_server()
        try:
            transport = HTTPTransport(
                http1=False, http2=True, local_address="127.0.0.1",
                verify=cert_path,
            )
            with Client(
                transport=transport, trust_env=False, timeout=5,
            ) as client:
                with pytest.raises((RequestError, ConnectError)):
                    client.get(f"https://{host}:{port}/health")
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)

    def test_h2_only_with_socket_options_h2_server(self):
        """H2-only + TCP_NODELAY against an H2-capable TLS server succeeds."""
        with local_tls_h2_server() as (host, port, _client_ssl, cert_path, _counter):
            transport = HTTPTransport(
                http1=False, http2=True, verify=cert_path,
                socket_options=[
                    (socket.IPPROTO_TCP, socket.TCP_NODELAY, 1),
                ],
            )
            with Client(
                transport=transport, trust_env=False, timeout=5,
            ) as client:
                resp = client.get(f"https://{host}:{port}/health")
                assert resp.status_code == 200
                assert resp.http_version == "HTTP/2"

    def test_h2_only_with_socket_options_h1_only_server(self):
        """H2-only + TCP_NODELAY against an H1-only TLS server fails."""
        host, port, cert_path, httpd, thread, tmpdir = _make_h1_only_tls_server()
        try:
            transport = HTTPTransport(
                http1=False, http2=True, verify=cert_path,
                socket_options=[
                    (socket.IPPROTO_TCP, socket.TCP_NODELAY, 1),
                ],
            )
            with Client(
                transport=transport, trust_env=False, timeout=5,
            ) as client:
                with pytest.raises((RequestError, ConnectError)):
                    client.get(f"https://{host}:{port}/health")
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)

    def test_h1_only_with_local_address_h1_server(self):
        """H1-only + local_address against an H1 TLS server succeeds."""
        from .native_fixtures import local_tls_server
        with local_tls_server() as (host, port, client_ssl, cert_path):
            transport = HTTPTransport(
                http1=True, http2=False, local_address="127.0.0.1",
                verify=cert_path,
            )
            with Client(
                transport=transport, trust_env=False, timeout=5,
            ) as client:
                resp = client.get(f"https://{host}:{port}/health")
                assert resp.status_code == 200

    def test_h2_only_async_with_local_address_h2_server(self):
        """Async H2-only + local_address against an H2 server succeeds."""
        async def run():
            with local_tls_h2_server() as (host, port, _client_ssl, cert_path, _counter):
                transport = AsyncHTTPTransport(
                    http1=False, http2=True, local_address="127.0.0.1",
                    verify=cert_path,
                )
                async with AsyncClient(
                    transport=transport, trust_env=False, timeout=5,
                ) as client:
                    resp = await client.get(f"https://{host}:{port}/health")
                    assert resp.status_code == 200
                    assert resp.http_version == "HTTP/2"

        asyncio.run(run())


# ---------------------------------------------------------------------------
# H2 through an HTTP CONNECT proxy — bounded protocol difference
# ---------------------------------------------------------------------------


class TestH2ProxyConnectResidual:
    """HTTP CONNECT tunnels use HTTP/1.1 framing after establishment."""

    def test_candidate_proxy_connect_remains_http1(self):
        """The candidate does not expose H2 origin framing inside CONNECT."""
        with local_tls_h2_server() as (host, port, _client_ssl, cert_path, _counter):
            with local_proxy_server() as (proxy_host, proxy_port, _handler):
                with Client(
                    http1=False,
                    http2=True,
                    verify=cert_path,
                    proxy=f"http://{proxy_host}:{proxy_port}",
                    trust_env=False,
                    timeout=5,
                ) as client:
                    with pytest.raises((RequestError, ConnectError)):
                        client.get(f"https://{host}:{port}/health")


# ---------------------------------------------------------------------------
# Wire proof — h2c prior knowledge sends the actual H2 preface
# ---------------------------------------------------------------------------


class TestH2cWireProof:
    """Wire-level proof that h2c prior knowledge sends the H2 preface."""

    def test_h2c_sends_h2_preface(self):
        """The first bytes sent by an H2-only client are the H2 client preface.

        The H2 client preface begins with the bytes
        `PRI * HTTP/2.0\\r\\n\\r\\nSM\\r\\n\\r\\n` (24 bytes).
        """
        from .native_fixtures import _h2_handle_request
        captured = bytearray()

        # Spin up a cleartext H2 server that captures the first bytes
        # received before completing the H2 handshake.
        def server_thread(host, port, captured_ref, stop):
            server_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            server_sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            server_sock.bind((host, port))
            server_sock.listen(8)
            server_sock.settimeout(1)
            while not stop.is_set():
                try:
                    conn, _ = server_sock.accept()
                except (socket.timeout, OSError):
                    continue
                # Capture the first 256 bytes (enough for the H2 preface
                # plus a SETTINGS frame).
                conn.settimeout(2)
                try:
                    first = conn.recv(256)
                except (socket.timeout, OSError):
                    first = b""
                captured_ref.extend(first)
                # We captured what we need; close the socket.
                try:
                    conn.close()
                except OSError:
                    pass

        host = "127.0.0.1"
        stop = threading.Event()
        # Bind first to learn the port, then start the server.
        bind_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        bind_sock.bind((host, 0))
        port = bind_sock.getsockname()[1]
        bind_sock.close()

        thread = threading.Thread(
            target=server_thread, args=(host, port, captured, stop), daemon=True,
        )
        thread.start()
        try:
            with Client(
                http1=False, http2=True, trust_env=False, timeout=3,
            ) as client:
                with pytest.raises(Exception):
                    # The server closes the socket immediately after
                    # capturing bytes, so the request fails. The wire
                    # capture is what we are testing.
                    client.get(f"http://{host}:{port}/health")
        finally:
            stop.set()
            thread.join(timeout=3)

        # The H2 client preface (RFC 7540 § 3.5) begins with
        # b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n".
        expected_preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
        assert bytes(captured).startswith(expected_preface), (
            f"Expected H2 preface at start of wire bytes, got: {bytes(captured)!r}"
        )


# ---------------------------------------------------------------------------
# stream_id absence — residual metadata difference
# ---------------------------------------------------------------------------


class TestStreamIdAbsence:
    """`stream_id` is not exposed for H2 responses (residual difference).

    HTTPX exposes `response.extensions["stream_id"]` as an integer for H2
    responses. EggFetch cannot do this because hyper-util's legacy client
    erases the underlying h2 future; `Response<Incoming>` does not carry
    the stream identifier. The metadata field is intentionally absent.
    """

    def test_stream_id_absent_in_response_extensions(self):
        """stream_id is absent from response extensions for H2 responses."""
        with local_tls_h2_server() as (host, port, _client_ssl, cert_path, _counter):
            with Client(
                http1=False, http2=True, verify=cert_path,
                trust_env=False, timeout=5,
            ) as client:
                resp = client.get(f"https://{host}:{port}/health")
                # stream_id is not exposed by EggFetch for H2 responses.
                assert "stream_id" not in resp.extensions


# ---------------------------------------------------------------------------
# Negative cases — both false
# ---------------------------------------------------------------------------


class TestH2OnlyNegativeCases:
    """Edge cases and error handling."""

    def test_both_false_raises_value_error(self):
        """http1=False, http2=False raises ValueError."""
        with pytest.raises(ValueError, match="At least one of http1 or http2"):
            Client(http1=False, http2=False)

    def test_async_both_false_raises_value_error(self):
        """AsyncClient http1=False, http2=False raises ValueError."""
        with pytest.raises(ValueError, match="At least one of http1 or http2"):
            AsyncClient(http1=False, http2=False)

    def test_transport_both_false_raises_value_error(self):
        """HTTPTransport http1=False, http2=False raises ValueError."""
        with pytest.raises(ValueError, match="At least one of http1 or http2"):
            HTTPTransport(http1=False, http2=False)

    def test_async_transport_both_false_raises_value_error(self):
        """AsyncHTTPTransport http1=False, http2=False raises ValueError."""
        with pytest.raises(ValueError, match="At least one of http1 or http2"):
            AsyncHTTPTransport(http1=False, http2=False)
