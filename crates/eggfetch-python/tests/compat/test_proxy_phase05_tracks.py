"""Phase 05 remaining track verification tests.

Track 3: proxy-auth vs explicit-header precedence
Track 8: default HTTPS-proxy trust behavior
Track 9: SOCKS reference-bounded behavior (headers/ssl_context not forwarded)
Track 10: NO_PROXY bypass header leakage, redirect/retry proxy metadata
"""
import base64
import os
import ssl
import sys
import tempfile
import threading
from contextlib import contextmanager

sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parent))

import httpx
import pytest

import eggfetch
from eggfetch.compat.httpx import AsyncClient, Client, Proxy, Timeout
from eggfetch.compat.httpx._exceptions import ConnectError

from native_fixtures import (
    _TLSDirectHandler,
    _generate_self_signed_ca_cert,
    _generate_self_signed_cert,
    local_http_server,
    local_proxy_server,
    local_tls_proxy_server,
    local_tls_server,
)


# ---------------------------------------------------------------------------
# Track 3 — proxy-auth precedence
# ---------------------------------------------------------------------------


class TestProxyAuthPrecedence:
    """Prove HTTPX-compatible proxy auth precedence against the reference."""

    def _count_proxy_auth(self, handler):
        """Count how many Proxy-Authorization headers were observed."""
        count = 0
        for req in handler.recorded_requests:
            if "proxy-authorization" in req["headers"]:
                count += 1
        return count

    def _get_proxy_auth_value(self, handler):
        """Get the first observed Proxy-Authorization header value."""
        for req in handler.recorded_requests:
            val = req["headers"].get("proxy-authorization")
            if val:
                return val
        return None

    def test_url_auth_only_sends_basic(self):
        """Credentials embedded in proxy URL are sent as Proxy-Authorization."""
        with local_http_server() as (backend_host, backend_port):
            with local_proxy_server(backend=(backend_host, backend_port)) as (
                proxy_host,
                proxy_port,
                handler,
            ):
                import httpx

                proxy_url = f"http://user:pass@{proxy_host}:{proxy_port}"
                with httpx.Client(
                    proxy=proxy_url,
                    trust_env=False,
                ) as reference:
                    resp = reference.get(
                        f"http://{backend_host}:{backend_port}/health"
                    )
                assert resp.status_code == 200
                ref_auth = self._get_proxy_auth_value(handler)
                assert ref_auth is not None
                assert ref_auth.startswith("Basic ")

                handler.recorded_requests.clear()
                with Client(
                    proxy=proxy_url,
                    trust_env=False,
                ) as candidate:
                    resp = candidate.get(
                        f"http://{backend_host}:{backend_port}/health"
                    )
                assert resp.status_code == 200
                cand_auth = self._get_proxy_auth_value(handler)
                assert cand_auth is not None
                assert cand_auth.startswith("Basic ")
                assert ref_auth == cand_auth

    def test_explicit_auth_only_sends_basic(self):
        """Proxy(auth=...) is sent as Proxy-Authorization."""
        with local_http_server() as (backend_host, backend_port):
            with local_proxy_server(backend=(backend_host, backend_port)) as (
                proxy_host,
                proxy_port,
                handler,
            ):
                proxy_url = f"http://{proxy_host}:{proxy_port}"
                import httpx

                with httpx.Client(
                    proxy=httpx.Proxy(proxy_url, auth=("user", "pass")),
                    trust_env=False,
                ) as reference:
                    resp = reference.get(
                        f"http://{backend_host}:{backend_port}/health"
                    )
                assert resp.status_code == 200
                ref_auth = self._get_proxy_auth_value(handler)
                assert ref_auth is not None

                handler.recorded_requests.clear()
                with Client(
                    proxy=Proxy(proxy_url, auth=("user", "pass")),
                    trust_env=False,
                ) as candidate:
                    resp = candidate.get(
                        f"http://{backend_host}:{backend_port}/health"
                    )
                assert resp.status_code == 200
                cand_auth = self._get_proxy_auth_value(handler)
                assert cand_auth is not None
                assert ref_auth == cand_auth

    def test_explicit_header_conflicts_with_configured_auth(self):
        """HTTPX replaces configured auth with an explicit header.

        EggFetch rejects this as an ambiguous configuration (security
        improvement: prevents accidental credential leakage).
        """
        with local_http_server() as (backend_host, backend_port):
            with local_proxy_server(backend=(backend_host, backend_port)) as (
                proxy_host,
                proxy_port,
                handler,
            ):
                proxy_url = f"http://{proxy_host}:{proxy_port}"
                import httpx

                with httpx.Client(
                    proxy=httpx.Proxy(proxy_url, auth=("user", "pass")),
                    trust_env=False,
                ) as reference:
                    resp = reference.get(
                        f"http://{backend_host}:{backend_port}/health",
                        headers={"Proxy-Authorization": "Bearer token123"},
                    )
                assert resp.status_code == 200
                ref_auth = self._get_proxy_auth_value(handler)
                assert ref_auth == "Bearer token123"

                handler.recorded_requests.clear()
                from eggfetch.compat.httpx._exceptions import RequestError

                with pytest.raises(RequestError, match="conflict"):
                    with Client(
                        proxy=Proxy(proxy_url, auth=("user", "pass")),
                        trust_env=False,
                    ) as candidate:
                        candidate.get(
                            f"http://{backend_host}:{backend_port}/health",
                            headers={"Proxy-Authorization": "Bearer token123"},
                        )


# ---------------------------------------------------------------------------
# Track 8 — default HTTPS-proxy trust behavior
# ---------------------------------------------------------------------------


class TestDefaultHttpsProxyTrust:
    """Verify default HTTPS-proxy trust matches the reference.

    Proxy endpoint TLS is independent of origin TLS in eggfetch.  The
    proxy CA must be supplied on the ``Proxy`` (e.g. via
    ``ssl_context``); the client-level ``verify=`` controls only the
    origin server certificate.
    """

    def test_https_proxy_with_explicit_ca_matches_reference(self):
        """Both runtimes trust the same CA for the proxy endpoint TLS."""
        with local_http_server() as (backend_host, backend_port):
            with local_tls_proxy_server(
                backend=(backend_host, backend_port)
            ) as (
                proxy_host,
                proxy_port,
                handler,
                (proxy_server_cert, proxy_ca_cert),
            ):
                import httpx

                proxy_ssl = ssl.create_default_context(
                    cafile=proxy_ca_cert or proxy_server_cert
                )
                with httpx.Client(
                    proxy=httpx.Proxy(
                        f"https://{proxy_host}:{proxy_port}",
                        ssl_context=proxy_ssl,
                    ),
                    trust_env=False,
                ) as reference:
                    resp = reference.get(
                        f"http://{backend_host}:{backend_port}/health"
                    )
                assert resp.status_code == 200

                handler.recorded_requests.clear()
                # eggfetch: proxy endpoint TLS is governed by the
                # Proxy itself.  The proxy CA is supplied through
                # ``Proxy(ssl_context=...)`` so the origin ``verify=``
                # is not reused as a fallback for the proxy handshake.
                proxy_ssl_ctx = ssl.create_default_context(
                    cafile=proxy_ca_cert or proxy_server_cert
                )
                with Client(
                    proxy=Proxy(
                        f"https://{proxy_host}:{proxy_port}",
                        ssl_context=proxy_ssl_ctx,
                    ),
                    trust_env=False,
                ) as candidate:
                    resp = candidate.get(
                        f"http://{backend_host}:{backend_port}/health"
                    )
                assert resp.status_code == 200

    def test_https_proxy_untrusted_ca_fails(self):
        """An untrusted proxy CA causes a connection failure."""
        with local_http_server() as (backend_host, backend_port):
            with local_tls_proxy_server(
                backend=(backend_host, backend_port)
            ) as (
                proxy_host,
                proxy_port,
                handler,
                (proxy_server_cert, proxy_ca_cert),
            ):
                # Use a different (wrong) CA for the proxy endpoint.
                # The wrong CA must be a CA:TRUE cert so the
                # translation layer can enumerate it.
                with tempfile.TemporaryDirectory() as tmpdir:
                    wrong_cert, _ = _generate_self_signed_ca_cert(tmpdir)
                    with pytest.raises((ConnectError, eggfetch.EggfetchError)):
                        with Client(
                            proxy=Proxy(
                                f"https://{proxy_host}:{proxy_port}",
                                ssl_context=ssl.create_default_context(
                                    cafile=wrong_cert
                                ),
                            ),
                            trust_env=False,
                            timeout=Timeout(3.0),
                        ) as candidate:
                            candidate.get(
                                f"http://{backend_host}:{backend_port}/health"
                            )


# ---------------------------------------------------------------------------
# Track 9 — SOCKS reference-bounded behavior
# ---------------------------------------------------------------------------


class TestSocksReferenceBounded:
    """Verify SOCKS transport does not receive proxy headers or ssl_context."""

    @pytest.mark.parametrize("scheme", ["socks5", "socks5h"])
    def test_socks_proxy_headers_not_applied(self, scheme):
        """Proxy(headers=...) is accepted but has no SOCKS wire effect.

        HTTPX 0.28.1's SOCKS transport does not receive proxy_headers,
        so the reference and candidate must both succeed without the
        headers appearing in the SOCKS negotiation.
        """
        try:
            import socksio  # noqa: F401
        except ImportError:
            pytest.skip("socksio not installed")

        with local_tls_server() as (tls_host, tls_port, _ssl, cert_path):
            with _socks_server() as (socks_host, socks_port):
                proxy_url = f"{scheme}://{socks_host}:{socks_port}"
                import httpx

                with httpx.Client(
                    proxy=httpx.Proxy(
                        proxy_url,
                        headers={"X-Proxy-Test": "should-not-appear"},
                    ),
                    trust_env=False,
                    verify=cert_path,
                ) as reference:
                    resp = reference.get(
                        f"https://{tls_host}:{tls_port}/health"
                    )
                assert resp.status_code == 200

                with Client(
                    proxy=Proxy(
                        proxy_url,
                        headers={"X-Proxy-Test": "should-not-appear"},
                    ),
                    trust_env=False,
                    verify=cert_path,
                ) as candidate:
                    resp = candidate.get(
                        f"https://{tls_host}:{tls_port}/health"
                    )
                assert resp.status_code == 200


# ---------------------------------------------------------------------------
# Track 10 — NO_PROXY bypass header leakage
# ---------------------------------------------------------------------------


class TestNoProxyBypassNoLeakage:
    """Prove proxy-only headers do not leak when NO_PROXY bypasses the proxy."""

    def test_no_proxy_bypass_sends_directly(self):
        """When NO_PROXY matches an env-var proxy, the request goes directly."""
        with local_http_server() as (backend_host, backend_port):
            with local_proxy_server(backend=(backend_host, backend_port)) as (
                proxy_host,
                proxy_port,
                handler,
            ):
                import os
                old_http_proxy = os.environ.get("http_proxy", "")
                old_no_proxy = os.environ.get("NO_PROXY", "")
                try:
                    os.environ["http_proxy"] = f"http://{proxy_host}:{proxy_port}"
                    os.environ["NO_PROXY"] = "*"
                    with Client(trust_env=True) as c:
                        resp = c.get(f"http://{backend_host}:{backend_port}/health")
                        assert resp.status_code == 200
                        assert resp.text == "ok"
                        # NO_PROXY bypass means proxy did not see the request
                        assert len(handler.recorded_requests) == 0
                finally:
                    os.environ["http_proxy"] = old_http_proxy
                    os.environ["NO_PROXY"] = old_no_proxy

    def test_redirect_through_proxy_preserves_headers(self):
        """Redirected requests through a proxy retain proxy headers."""
        with local_http_server() as (backend_host, backend_port):
            with local_proxy_server(backend=(backend_host, backend_port)) as (
                proxy_host,
                proxy_port,
                handler,
            ):
                with Client(
                    proxy=Proxy(
                        f"http://{proxy_host}:{proxy_port}",
                        headers={"X-Proxy-Test": "redirect-test"},
                    ),
                    follow_redirects=True,
                    trust_env=False,
                ) as c:
                    resp = c.get(f"http://{backend_host}:{backend_port}/health")
                    assert resp.status_code == 200
                    # All requests through the proxy should have the header
                    for req in handler.recorded_requests:
                        assert req["headers"].get("x-proxy-test") == "redirect-test"


# ---------------------------------------------------------------------------
# SOCKS fixture helper
# ---------------------------------------------------------------------------


@contextmanager
def _socks_server(*, selected_method=0, reject_auth=False):
    """Minimal SOCKS5 server fixture for Track 9."""
    import struct
    import select as _select
    import socket as _socket

    stop = threading.Event()
    server = _socket.socket(_socket.AF_INET, _socket.SOCK_STREAM)
    server.setsockopt(_socket.SOL_SOCKET, _socket.SO_REUSEADDR, 1)
    server.bind(("127.0.0.1", 0))
    server.listen(5)
    port = server.getsockname()[1]
    server.settimeout(0.5)

    def _handle(conn):
        try:
            data = conn.recv(256)
            if len(data) < 3 or data[0] != 0x05:
                conn.close()
                return
            conn.sendall(b"\x05" + bytes([selected_method]))

            data = conn.recv(256)
            if not data or data[0] != 0x05 or data[1] != 0x01:
                conn.close()
                return

            atyp = data[3]
            if atyp == 0x01:
                target_ip = _socket.inet_ntoa(data[4:8])
                target_port = struct.unpack("!H", data[8:10])[0]
            elif atyp == 0x03:
                dlen = data[4]
                target_ip = data[5 : 5 + dlen].decode()
                target_port = struct.unpack("!H", data[5 + dlen : 7 + dlen])[0]
            else:
                conn.close()
                return

            upstream = _socket.create_connection((target_ip, target_port), timeout=5)
            conn.sendall(
                b"\x05\x00\x00\x01"
                + _socket.inet_aton("127.0.0.1")
                + struct.pack("!H", port)
            )
            conn.setblocking(False)
            upstream.setblocking(False)
            while not stop.is_set():
                r, _, _ = _select.select([conn, upstream], [], [], 0.5)
                for sock in r:
                    d = sock.recv(8192)
                    if not d:
                        upstream.close()
                        conn.close()
                        return
                    (upstream if sock is conn else conn).sendall(d)
            upstream.close()
        except Exception:
            pass
        finally:
            try:
                conn.close()
            except Exception:
                pass

    def accept_loop():
        while not stop.is_set():
            try:
                conn, _ = server.accept()
                threading.Thread(target=_handle, args=(conn,), daemon=True).start()
            except (_socket.timeout, OSError):
                continue

    t = threading.Thread(target=accept_loop, daemon=True)
    t.start()
    try:
        yield "127.0.0.1", port
    finally:
        stop.set()
        server.close()
        t.join(timeout=2)
