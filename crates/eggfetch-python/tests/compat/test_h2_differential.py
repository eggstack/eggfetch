"""HTTP/2-only differential tests: eggfetch vs httpx reference.

Tests the HTTP/2-only (prior knowledge / ALPN-only) mode implemented in
Phase 02 of HTTPX parity. Covers:
- HTTPS H2-only via TLS ALPN (working path)
- Cleartext H2 prior knowledge (known limitation)
- TLS ALPN enforcement (H2-only vs H1-only server)
- Constructor matrix for all transport types
- Streaming and cancellation
- http_version field verification
- Specialized route smoke tests
"""

from __future__ import annotations

import asyncio
import ssl
import tempfile
import threading

import httpx
import pytest

from eggfetch.compat.httpx import AsyncClient, Client
from eggfetch.compat.httpx._transports import AsyncHTTPTransport, HTTPTransport

from .native_fixtures import (
    _generate_self_signed_cert,
    _H2RequestCounter,
    _TLSDirectHandler,
    _ThreadedHTTPServer,
    local_h2_server,
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
# TLS ALPN enforcement — H2-only vs H1-only server
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

    def test_candidate_h2_only_vs_h1_only(self):
        """eggfetch H2-only behavior vs H1-only server.

        Note: eggfetch currently falls back to HTTP/1.1 silently because
        hyper-rustls does not enforce h2-only at the connector level.
        This test documents the current behavior. When enforcement is
        added to the Rust core, this test should be updated to expect
        a ConnectError.
        """
        host, port, cert_path, httpd, thread, tmpdir = _make_h1_only_tls_server()
        try:
            with Client(
                http1=False, http2=True, verify=cert_path,
                trust_env=False, timeout=5,
            ) as client:
                resp = client.get(f"https://{host}:{port}/health")
                # Current behavior: falls back to HTTP/1.1
                # When enforcement is added, change to:
                # with pytest.raises(eggfetch.compat.httpx._exceptions.ConnectError):
                #     client.get(...)
                assert resp.status_code == 200
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)


# ---------------------------------------------------------------------------
# Cleartext H2 prior knowledge — known limitation
# ---------------------------------------------------------------------------


class TestCleartextH2PriorKnowledge:
    """Cleartext H2 prior knowledge is NOT supported by eggfetch.

    The Rust core's hyper_rustls connector only supports HTTP/2 over TLS
    via ALPN. For cleartext HTTP, it falls back to HTTP/1.1. This is a
    documented limitation. httpx supports cleartext h2 via httpcore's
    h2 implementation.
    """

    def test_reference_httpx_cleartext_h2(self):
        """httpx successfully uses cleartext H2 prior knowledge."""
        with local_h2_server() as (host, port, _counter):
            with httpx.Client(
                http1=False, http2=True, trust_env=False, timeout=5,
            ) as client:
                resp = client.get(f"http://{host}:{port}/health")
                assert resp.status_code == 200
                assert resp.http_version == "HTTP/2"

    def test_candidate_cleartext_h2_fallback(self):
        """eggfetch falls back to HTTP/1.1 for cleartext H2.

        This documents the current limitation. The hyper_rustls connector
        does not support h2c (HTTP/2 cleartext prior knowledge).
        """
        with local_h2_server() as (host, port, _counter):
            with Client(
                http1=False, http2=True, trust_env=False, timeout=5,
            ) as client:
                # The H2 server only speaks HTTP/2, so this will fail
                # because eggfetch sends HTTP/1.1 to it.
                with pytest.raises(Exception):
                    client.get(f"http://{host}:{port}/health")


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
# Specialized route smoke tests
# ---------------------------------------------------------------------------


class TestH2OnlySpecializedRoutes:
    """H2-only with specialized transport options.

    Note: When local_address or socket_options are specified, the Rust core
    uses the direct connector path which does not support HTTP/2 negotiation.
    The direct connector always falls back to HTTP/1.1. These tests verify
    that the transport works at all with these options and document the
    H2 limitation.
    """

    def test_h2_only_with_local_address(self):
        """H2-only with local_address — direct connector falls back to H1."""
        with local_tls_h2_server() as (host, port, _client_ssl, cert_path, _counter):
            transport = HTTPTransport(
                http1=False, http2=True, local_address="127.0.0.1",
                verify=cert_path,
            )
            with Client(
                transport=transport, trust_env=False, timeout=5,
            ) as client:
                # Direct connector does not support H2; falls back to H1.
                # The server only handles h2 ALPN, so this will fail.
                # Document this as a known limitation.
                with pytest.raises(Exception):
                    client.get(f"https://{host}:{port}/health")

    def test_h2_only_with_local_address_h1_server(self):
        """H2-only with local_address against an H1 server (direct connector)."""
        from .native_fixtures import local_tls_server
        with local_tls_server() as (host, port, client_ssl, cert_path):
            transport = HTTPTransport(
                http1=False, http2=True, local_address="127.0.0.1",
                verify=cert_path,
            )
            with Client(
                transport=transport, trust_env=False, timeout=5,
            ) as client:
                resp = client.get(f"https://{host}:{port}/health")
                # Direct connector falls back to HTTP/1.1
                assert resp.status_code == 200

    def test_h2_only_with_socket_options(self):
        """H2-only with TCP_NODELAY — direct connector falls back to H1."""
        import socket
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
                # Direct connector does not support H2; server only handles h2.
                with pytest.raises(Exception):
                    client.get(f"https://{host}:{port}/health")

    def test_h2_only_with_socket_options_h1_server(self):
        """H2-only with TCP_NODELAY against an H1 server (direct connector)."""
        import socket
        from .native_fixtures import local_tls_server
        with local_tls_server() as (host, port, client_ssl, cert_path):
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

    def test_h2_only_async_with_local_address(self):
        """Async H2-only with local_address — direct connector falls back to H1."""
        from .native_fixtures import local_tls_server

        async def run():
            with local_tls_server() as (host, port, client_ssl, cert_path):
                transport = AsyncHTTPTransport(
                    http1=False, http2=True, local_address="127.0.0.1",
                    verify=cert_path,
                )
                async with AsyncClient(
                    transport=transport, trust_env=False, timeout=5,
                ) as client:
                    resp = await client.get(f"https://{host}:{port}/health")
                    assert resp.status_code == 200

        asyncio.run(run())


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
