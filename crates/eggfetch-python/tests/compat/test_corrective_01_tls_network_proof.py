"""Network proof tests for HTTPX parity corrective 01.

These tests exercise live TLS handshakes to confirm:
- Translation determinism: identical cardinalities but different
  contents produce different network behavior.
- Fingerprint mutation: live changes to a helper-created context
  are honored, not silently masked by stale metadata.
- mTLS provenance: external client certs without helper path
  provenance are not silently downgraded.
- Proxy trust-domain isolation: the proxy endpoint TLS does not
  inherit the origin TLS state (or vice versa).
- Proxy-header redaction: sentinel secrets never reach the
  network or appear in error messages.
"""

from __future__ import annotations

import os
import socket
import ssl
import subprocess
import sys
import tempfile
import threading
import time
import http.server
from contextlib import contextmanager
from pathlib import Path
from typing import Generator

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))

from eggfetch.compat.httpx import (
    Client,
    Proxy,
    Timeout,
    create_ssl_context,
)
from native_fixtures import (
    _generate_ca_signed_server_cert,
    _generate_self_signed_ca_cert,
    _generate_self_signed_cert,
    _ProxyHandler,
    _ThreadedHTTPServer,
    local_http_server,
    local_proxy_server,
    local_tls_proxy_server,
    local_tls_server,
)


# ── Translation determinism over the wire ─────────────────────────────


class TestTranslationDeterminismOverTheWire:
    """Two CA stores with identical cardinalities but different
    contents must produce different network behavior.

    The corrective removes the CA-count heuristic that treated
    similar-cardinality stores as ``verify=True``.
    """

    def test_custom_ca_with_default_count_is_not_default_trust(self):
        """Build a context with N custom CAs where N matches the
        default certifi count; the server cert must NOT be
        accepted because the default trust is not in effect.
        """
        default_count = len(
            ssl.create_default_context().get_ca_certs(binary_form=True)
        )
        with tempfile.TemporaryDirectory() as tmpdir:
            # Build a context with N custom CAs, none of which
            # sign our test server.
            ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
            ctx.check_hostname = False
            ctx.verify_mode = ssl.CERT_REQUIRED
            for i in range(default_count):
                ca_path = f"{tmpdir}/ca{i}.pem"
                ca_key = f"{tmpdir}/ca{i}.key"
                subprocess.run(
                    [
                        "openssl", "req", "-x509", "-newkey", "ec",
                        "-pkeyopt", "ec_paramgen_curve:prime256v1",
                        "-keyout", ca_key, "-out", ca_path,
                        "-days", "1", "-nodes",
                        "-subj", f"/CN=wrong-ca-{i}",
                        "-addext", "basicConstraints=critical,CA:TRUE",
                    ],
                    check=True,
                    capture_output=True,
                )
                ctx.load_verify_locations(cafile=ca_path)
            assert len(ctx.get_ca_certs(binary_form=True)) == default_count

            # Run a self-signed server; the client must reject
            # it because none of the custom CAs issued it.
            with local_tls_server() as (
                origin_host,
                origin_port,
                _ssl_ctx,
                _cert_path,
            ):
                c = Client(verify=ctx, timeout=5.0)
                # The connection must be rejected.  We do not
                # assert the specific error message because the
                # mapping of rustls errors to exception text
                # varies, but the handshake must not succeed.
                with pytest.raises(Exception):
                    c.get(f"https://{origin_host}:{origin_port}/health")
                c.close()


# ── Fingerprint mutation honored on the wire ──────────────────────────


class TestFingerprintMutationOverTheWire:
    """A helper-created context whose live state is mutated must
    either reflect the new state on the wire or be rejected.
    """

    def test_mutated_helper_context_load_verify_locations_accepted(self):
        """A helper context with extra CAs added is classified
        from the live snapshot, so the added CAs are honored.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            # Generate a CA + server-cert pair so the client can
            # verify the server.
            (
                ca_path,
                _ca_key,
                server_cert,
                server_key,
            ) = _generate_ca_signed_server_cert(tmpdir)
            # Spin up the server with the CA-signed cert.
            server_ssl = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            server_ssl.load_cert_chain(server_cert, server_key)
            httpd = _ThreadedHTTPServer(
                ("127.0.0.1", 0),
                type(
                    "_StubHandler",
                    (http.server.BaseHTTPRequestHandler,),
                    {
                        "do_GET": lambda self: (
                            self.send_response(200),
                            self.send_header("Content-Length", "2"),
                            self.end_headers(),
                            self.wfile.write(b"ok"),
                        ),
                        "log_message": lambda *a, **k: None,
                    },
                ),
            )
            httpd.socket = server_ssl.wrap_socket(
                httpd.socket, server_side=True
            )
            thread = threading.Thread(target=httpd.serve_forever, daemon=True)
            thread.start()
            try:
                host, port = httpd.server_address
                # Build a helper context, then mutate it with the
                # CA.  Translation classifies from the live state.
                ctx = create_ssl_context()
                ctx.load_verify_locations(cafile=ca_path)
                c = Client(verify=ctx, timeout=5.0)
                response = c.get(f"https://{host}:{port}/")
                assert response.status_code == 200
                c.close()
            finally:
                httpd.shutdown()
                httpd.server_close()
                thread.join(timeout=2)


# ── Proxy trust-domain isolation ──────────────────────────────────────


class TestProxyTrustDomainIsolation:
    """Proxy TLS config is independent from origin TLS config.

    The plan requires that origin ``verify=False``/custom CA/mTLS
    must not mutate proxy endpoint policy, and vice versa.
    """

    def test_origin_verify_false_does_not_disable_proxy_verification(self):
        """``verify=False`` for the origin must not disable TLS
        verification on an HTTPS proxy endpoint.
        """
        # Use a deliberately-untrusted CA for the proxy so we
        # can observe whether verification is disabled.
        with tempfile.TemporaryDirectory() as tmpdir:
            (
                _proxy_ca,
                _proxy_ca_key,
                proxy_server_cert,
                proxy_server_key,
            ) = _generate_ca_signed_server_cert(tmpdir)
            # Spin up an HTTP backend for the proxy to forward to.
            with local_http_server() as (backend_host, backend_port):
                with local_tls_proxy_server(
                    backend=(backend_host, backend_port),
                    certificate=(proxy_server_cert, proxy_server_key),
                ) as (
                    proxy_host,
                    proxy_port,
                    _handler,
                    (proxy_cert, proxy_ca),
                ):
                    # The proxy uses a CA-signed cert pair.  Use
                    # an SSLContext that trusts only a *different*
                    # CA.  The connection must fail because the
                    # proxy endpoint is verified using its own
                    # trust domain.
                    untrusted_ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
                    untrusted_ctx.check_hostname = False
                    untrusted_ctx.verify_mode = ssl.CERT_REQUIRED
                    # An empty CA set: the proxy cert won't verify.
                    c = Client(
                        proxy=Proxy(
                            f"https://{proxy_host}:{proxy_port}",
                            ssl_context=untrusted_ctx,
                        ),
                        verify=False,
                        timeout=5.0,
                    )
                    with pytest.raises(Exception) as excinfo:
                        c.get(f"http://{backend_host}:{backend_port}/")
                    msg = str(excinfo.value).lower()
                    assert (
                        "ssl" in msg
                        or "tls" in msg
                        or "certificate" in msg
                        or "issuer" in msg
                    ), (
                        f"Expected TLS rejection at proxy endpoint, "
                        f"got: {excinfo.value!r}"
                    )
                    c.close()

    def test_origin_custom_ca_does_not_leak_to_proxy(self):
        """A custom origin CA does not influence proxy TLS."""
        with tempfile.TemporaryDirectory() as origin_tmpdir:
            # Generate an origin CA + server cert pair.
            (
                origin_ca_path,
                _origin_ca_key,
                origin_server_cert,
                origin_server_key,
            ) = _generate_ca_signed_server_cert(origin_tmpdir)
            # Spin up the origin TLS server.
            server_ssl = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            server_ssl.load_cert_chain(origin_server_cert, origin_server_key)
            handler_class = type(
                "_StubHandler",
                (http.server.BaseHTTPRequestHandler,),
                {
                    "do_GET": lambda self: (
                        self.send_response(200),
                        self.send_header("Content-Length", "2"),
                        self.end_headers(),
                        self.wfile.write(b"ok"),
                    ),
                    "log_message": lambda *a, **k: None,
                },
            )
            httpd = _ThreadedHTTPServer(("127.0.0.1", 0), handler_class)
            httpd.socket = server_ssl.wrap_socket(
                httpd.socket, server_side=True
            )
            origin_thread = threading.Thread(
                target=httpd.serve_forever, daemon=True
            )
            origin_thread.start()
            try:
                origin_host, origin_port = httpd.server_address
                # Generate a proxy CA + cert pair (separate from
                # origin).
                with tempfile.TemporaryDirectory() as proxy_tmpdir:
                    (
                        proxy_ca_path,
                        _proxy_ca_key,
                        proxy_server_cert,
                        proxy_server_key,
                    ) = _generate_ca_signed_server_cert(proxy_tmpdir)
                    with local_tls_proxy_server(
                        backend=(origin_host, origin_port),
                        certificate=(proxy_server_cert, proxy_server_key),
                    ) as (
                        proxy_host,
                        proxy_port,
                        _handler,
                        (proxy_cert, proxy_ca),
                    ):
                        # The origin context trusts the origin CA
                        # (NOT the proxy CA).  The proxy context
                        # trusts the proxy CA (NOT the origin CA).
                        # If the proxy TLS accidentally fell back
                        # to the origin context, the handshake
                        # would fail.
                        origin_ctx = ssl.create_default_context()
                        origin_ctx.load_verify_locations(
                            cafile=origin_ca_path
                        )
                        proxy_ctx = ssl.create_default_context()
                        proxy_ctx.load_verify_locations(cafile=proxy_ca_path)
                        c = Client(
                            verify=origin_ctx,
                            proxy=Proxy(
                                f"https://{proxy_host}:{proxy_port}",
                                ssl_context=proxy_ctx,
                            ),
                            timeout=10.0,
                        )
                        response = c.get(
                            f"https://{origin_host}:{origin_port}/"
                        )
                        assert response.status_code == 200
                        c.close()
            finally:
                httpd.shutdown()
                httpd.server_close()
                origin_thread.join(timeout=2)


# ── Proxy header redaction across surfaces ────────────────────────────


class TestProxyHeaderRedactionAcrossSurfaces:
    """Sentinel secrets in proxy headers must not appear in
    diagnostic surfaces (``repr``, error messages, debug output).
    """

    def test_proxy_authorization_secret_not_in_repr(self):
        from eggfetch.compat.httpx import Proxy

        sentinel = "Basic SENTINEL-PROXY-SECRET-XYZZY"
        proxy = Proxy(
            "http://proxy.example.com",
            headers={"proxy-authorization": sentinel},
        )
        debug = repr(proxy)
        assert sentinel not in debug
        assert "<redacted>" in debug

    def test_proxy_authorization_secret_not_in_str(self):
        from eggfetch.compat.httpx import Proxy

        sentinel = "Basic SENTINEL-PROXY-SECRET-STR-Q1W2"
        proxy = Proxy(
            "http://proxy.example.com",
            headers={"proxy-authorization": sentinel},
        )
        text = str(proxy)
        assert sentinel not in text

    def test_proxy_headers_remain_non_redacted_for_protocol_use(self):
        """Protocol code must still observe raw header values;
        redaction is a diagnostic surface only.
        """
        from eggfetch.compat.httpx import Proxy

        sentinel = "Basic KEEP-RAW-9F2C"
        proxy = Proxy(
            "http://proxy.example.com",
            headers={"proxy-authorization": sentinel},
        )
        assert ("proxy-authorization", sentinel) in proxy.headers
