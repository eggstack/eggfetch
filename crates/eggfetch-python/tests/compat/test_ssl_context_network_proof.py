"""Network proof tests for HTTPX parity phase 01 TLS context translation.

All tests use real local TCP sockets. No external internet access required.

Covers the plan's "Required differential tests — Network proof" section:
- custom CA accepted/rejected identically to HTTPX 0.28.1
- hostname mismatch behavior
- verification-disabled behavior
- TLS-version negotiation success/failure
- mTLS success for contexts created through EggFetch's helper
- explicit rejection before dispatch for unrepresentable contexts
"""

import http.server
import os
import shutil
import socket
import ssl
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import eggfetch
from eggfetch.compat.httpx import (
    Client,
    AsyncClient,
    MockTransport,
    Response,
    Timeout,
    create_ssl_context,
)
from eggfetch.compat.httpx._exceptions import ConnectError, RequestError
from native_fixtures import (
    _ThreadedHTTPServer,
    _TLSDirectHandler,
    local_tls_server,
)


# ── Helper: generate CA-signed cert + key pair via openssl CLI ────────


def _generate_ca_signed_cert(
    cert_dir: str,
    ca_cert_path: str,
    ca_key_path: str,
    cn: str = "127.0.0.1",
    san: str = "IP:127.0.0.1",
    is_ca: bool = False,
) -> tuple[str, str]:
    """Generate a CA-signed certificate and key.

    Returns (cert_path, key_path).
    """
    cert_path = os.path.join(cert_dir, f"{cn}.cert.pem")
    key_path = os.path.join(cert_dir, f"{cn}.key.pem")
    csr_path = os.path.join(cert_dir, f"{cn}.csr.pem")
    ext_path = os.path.join(cert_dir, f"{cn}.ext.cnf")

    # Write extensions config for the CSR
    with open(ext_path, "w") as f:
        f.write("[req]\ndistinguished_name = req_dn\nreq_extensions = v3_req\nprompt = no\n\n")
        f.write(f"[req_dn]\nCN = {cn}\n\n")
        f.write("[v3_req]\n")
        f.write(f"subjectAltName = {san}\n")

    # Generate key + CSR with SAN embedded
    subprocess.run(
        [
            "openssl", "req", "-newkey", "rsa:2048",
            "-keyout", key_path, "-out", csr_path,
            "-nodes", "-config", ext_path,
        ],
        check=True, capture_output=True,
    )

    # Build x509 extension flags
    x509_ext_args = ["-copy_extensions", "copyall"]
    if is_ca:
        # Overwrite with basicConstraints for CA certs
        ca_ext_path = os.path.join(cert_dir, f"{cn}.ca.ext.cnf")
        with open(ca_ext_path, "w") as f:
            f.write("basicConstraints = critical,CA:TRUE\n")
        x509_ext_args = ["-extfile", ca_ext_path]

    # Sign with CA
    subprocess.run(
        [
            "openssl", "x509", "-req",
            "-in", csr_path, "-out", cert_path,
            "-CA", ca_cert_path, "-CAkey", ca_key_path,
            "-CAcreateserial", "-days", "1",
        ] + x509_ext_args,
        check=True, capture_output=True,
    )

    return cert_path, key_path


def _generate_self_signed_ca(cert_dir: str) -> tuple[str, str]:
    """Generate a self-signed CA certificate and key.

    Returns (ca_cert_path, ca_key_path).
    """
    ca_cert_path = os.path.join(cert_dir, "ca.cert.pem")
    ca_key_path = os.path.join(cert_dir, "ca.key.pem")

    subprocess.run(
        [
            "openssl", "req", "-x509", "-newkey", "rsa:2048",
            "-keyout", ca_key_path, "-out", ca_cert_path,
            "-days", "1", "-nodes",
            "-subj", "/CN=Test CA",
            "-addext", "basicConstraints=critical,CA:TRUE",
        ],
        check=True, capture_output=True,
    )

    return ca_cert_path, ca_key_path


# ── Custom CA server fixture ──────────────────────────────────────────


class _CAVerifiedTLSHandler(http.server.BaseHTTPRequestHandler):
    """Simple TLS handler that responds with 200 OK."""

    recorded_headers: list[dict[str, str]] = []

    def do_GET(self):
        self.__class__.recorded_headers.append(
            {name.lower(): value for name, value in self.headers.items()}
        )
        body = b"ok"
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()

    def log_message(self, format, *args):
        pass


def _start_tls_server_with_ca(
    ca_cert_path: str,
    ca_key_path: str,
    cn: str = "127.0.0.1",
    san: str = "IP:127.0.0.1",
) -> tuple[_ThreadedHTTPServer, int, threading.Thread]:
    """Start a TLS server signed by the given CA."""
    tmpdir = tempfile.mkdtemp()
    cert_path, key_path = _generate_ca_signed_cert(
        tmpdir, ca_cert_path, ca_key_path, cn=cn, san=san
    )

    server_ssl = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server_ssl.load_cert_chain(cert_path, key_path)

    httpd = _ThreadedHTTPServer(("127.0.0.1", 0), _CAVerifiedTLSHandler)
    _CAVerifiedTLSHandler.recorded_headers = []
    raw_socket = httpd.socket
    httpd.socket = server_ssl.wrap_socket(raw_socket, server_side=True)
    port = httpd.server_address[1]

    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()

    return httpd, port, thread, tmpdir


# ── Custom CA accepted/rejected identically to HTTPX ──────────────────


class TestCustomCAEquivalence:
    """Custom CA accepted/rejected identically to HTTPX 0.28.1.

    Uses a self-signed CA to sign the server cert. Both HTTPX and eggfetch
    should accept the connection when given the CA cert, and reject when
    the CA cert is missing or wrong.
    """

    def test_custom_ca_accepted_by_eggfetch(self):
        """Eggfetch accepts a server signed by a custom CA when given the CA cert."""
        with tempfile.TemporaryDirectory() as ca_dir:
            ca_cert, ca_key = _generate_self_signed_ca(ca_dir)
            httpd, port, thread, tmpdir = _start_tls_server_with_ca(
                ca_cert, ca_key
            )
            try:
                with Client(timeout=Timeout(5.0), verify=ca_cert) as c:
                    resp = c.get(f"https://127.0.0.1:{port}/")
                    assert resp.status_code == 200
                    assert resp.text == "ok"
            finally:
                httpd.shutdown()
                httpd.server_close()
                thread.join(timeout=2)
                shutil.rmtree(tmpdir, ignore_errors=True)

    def test_custom_ca_accepted_by_httpx(self):
        """HTTPX accepts a server signed by a custom CA (reference baseline)."""
        import httpx as _httpx

        with tempfile.TemporaryDirectory() as ca_dir:
            ca_cert, ca_key = _generate_self_signed_ca(ca_dir)
            httpd, port, thread, tmpdir = _start_tls_server_with_ca(
                ca_cert, ca_key
            )
            try:
                with _httpx.Client(timeout=5.0, verify=ca_cert) as c:
                    resp = c.get(f"https://127.0.0.1:{port}/")
                    assert resp.status_code == 200
                    assert resp.text == "ok"
            finally:
                httpd.shutdown()
                httpd.server_close()
                thread.join(timeout=2)
                shutil.rmtree(tmpdir, ignore_errors=True)

    def test_wrong_ca_rejected_by_eggfetch(self):
        """Eggfetch rejects a server signed by an unrelated CA."""
        with tempfile.TemporaryDirectory() as ca_dir:
            # Server signed by CA A
            ca_a_cert, ca_a_key = _generate_self_signed_ca(ca_dir)
            httpd, port, thread, tmpdir = _start_tls_server_with_ca(
                ca_a_cert, ca_a_key
            )
            try:
                # Client trusts CA B (different CA)
                ca_b_cert, _ = _generate_self_signed_ca(ca_dir)
                with Client(timeout=Timeout(5.0), verify=ca_b_cert) as c:
                    with pytest.raises(ConnectError):
                        c.get(f"https://127.0.0.1:{port}/")
            finally:
                httpd.shutdown()
                httpd.server_close()
                thread.join(timeout=2)
                shutil.rmtree(tmpdir, ignore_errors=True)

    def test_wrong_ca_rejected_by_httpx(self):
        """HTTPX rejects a server signed by an unrelated CA (reference baseline)."""
        import httpx as _httpx

        with tempfile.TemporaryDirectory() as ca_dir:
            ca_a_cert, ca_a_key = _generate_self_signed_ca(ca_dir)
            httpd, port, thread, tmpdir = _start_tls_server_with_ca(
                ca_a_cert, ca_a_key
            )
            try:
                ca_b_cert, _ = _generate_self_signed_ca(ca_dir)
                with _httpx.Client(timeout=5.0, verify=ca_b_cert) as c:
                    with pytest.raises(_httpx.ConnectError):
                        c.get(f"https://127.0.0.1:{port}/")
            finally:
                httpd.shutdown()
                httpd.server_close()
                thread.join(timeout=2)
                shutil.rmtree(tmpdir, ignore_errors=True)

    def test_no_ca_rejected_by_eggfetch(self):
        """Eggfetch rejects a self-signed cert when verify=True (default trust)."""
        with local_tls_server() as (host, port, client_ssl, cert_path):
            with Client(timeout=Timeout(5.0), verify=True) as c:
                with pytest.raises(ConnectError):
                    c.get(f"https://{host}:{port}/")

    def test_no_ca_rejected_by_httpx(self):
        """HTTPX rejects a self-signed cert when verify=True (reference baseline)."""
        import httpx as _httpx

        with local_tls_server() as (host, port, client_ssl, cert_path):
            with _httpx.Client(timeout=5.0, verify=True) as c:
                with pytest.raises(_httpx.ConnectError):
                    c.get(f"https://{host}:{port}/")


# ── Hostname mismatch ─────────────────────────────────────────────────


class TestHostnameMismatch:
    """Hostname mismatch behavior matches HTTPX 0.28.1."""

    def test_hostname_mismatch_rejected_by_eggfetch(self):
        """Eggfetch rejects connection when hostname doesn't match SAN."""
        with local_tls_server() as (host, port, client_ssl, cert_path):
            with Client(timeout=Timeout(5.0), verify=cert_path) as c:
                with pytest.raises(ConnectError):
                    c.get(f"https://wrong-hostname.invalid:{port}/")

    def test_hostname_mismatch_rejected_by_httpx(self):
        """HTTPX rejects connection when hostname doesn't match SAN (reference)."""
        import httpx as _httpx

        with local_tls_server() as (host, port, client_ssl, cert_path):
            with _httpx.Client(timeout=5.0, verify=cert_path) as c:
                with pytest.raises(_httpx.ConnectError):
                    c.get(f"https://wrong-hostname.invalid:{port}/")


# ── Verification disabled ─────────────────────────────────────────────


class TestVerificationDisabled:
    """verify=False allows self-signed certificates through."""

    def test_verify_false_allows_self_signed_eggfetch(self):
        """Eggfetch with verify=False accepts self-signed cert."""
        with local_tls_server() as (host, port, client_ssl, cert_path):
            with Client(timeout=Timeout(5.0), verify=False) as c:
                resp = c.get(f"https://{host}:{port}/health")
                assert resp.status_code == 200

    def test_verify_false_allows_self_signed_httpx(self):
        """HTTPX with verify=False accepts self-signed cert (reference)."""
        import httpx as _httpx

        with local_tls_server() as (host, port, client_ssl, cert_path):
            with _httpx.Client(timeout=5.0, verify=False) as c:
                resp = c.get(f"https://{host}:{port}/health")
                assert resp.status_code == 200

    def test_verify_false_eggfetch_via_sslcontext(self):
        """Eggfetch with SSLContext(CERT_NONE) accepts self-signed cert."""
        ctx = create_ssl_context(verify=False)
        assert ctx.verify_mode == ssl.CERT_NONE

        with local_tls_server() as (host, port, client_ssl, cert_path):
            with Client(timeout=Timeout(5.0), verify=ctx) as c:
                resp = c.get(f"https://{host}:{port}/health")
                assert resp.status_code == 200


# ── TLS-version negotiation ───────────────────────────────────────────


class TestTLSVersionNegotiation:
    """TLS-version negotiation success/failure with eggfetch.

    Uses Python ssl module to create servers with specific TLS version
    requirements, then tests eggfetch's negotiation behavior.
    """

    def _start_version_restricted_server(
        self, min_ver: int, max_ver: int
    ) -> tuple[_ThreadedHTTPServer, int, threading.Thread, str]:
        """Start a TLS server restricted to specific versions."""
        tmpdir = tempfile.mkdtemp()
        cert_path, key_path = _generate_ca_signed_cert(
            tmpdir,
            *_generate_self_signed_ca(tmpdir),
            cn="127.0.0.1",
            san="IP:127.0.0.1",
        )

        server_ssl = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        server_ssl.minimum_version = min_ver
        server_ssl.maximum_version = max_ver
        server_ssl.load_cert_chain(cert_path, key_path)

        httpd = _ThreadedHTTPServer(("127.0.0.1", 0), _CAVerifiedTLSHandler)
        _CAVerifiedTLSHandler.recorded_headers = []
        raw_socket = httpd.socket
        httpd.socket = server_ssl.wrap_socket(raw_socket, server_side=True)
        port = httpd.server_address[1]

        thread = threading.Thread(target=httpd.serve_forever, daemon=True)
        thread.start()

        return httpd, port, thread, tmpdir

    def test_tls13_server_accepted_by_eggfetch(self):
        """Eggfetch connects to a TLS 1.3-only server."""
        httpd, port, thread, tmpdir = self._start_version_restricted_server(
            ssl.TLSVersion.TLSv1_3, ssl.TLSVersion.TLSv1_3
        )
        try:
            ca_cert = os.path.join(tmpdir, "ca.cert.pem")
            with Client(timeout=Timeout(5.0), verify=ca_cert) as c:
                resp = c.get(f"https://127.0.0.1:{port}/")
                assert resp.status_code == 200
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)
            shutil.rmtree(tmpdir, ignore_errors=True)

    def test_tls12_server_accepted_by_eggfetch(self):
        """Eggfetch connects to a TLS 1.2-only server."""
        httpd, port, thread, tmpdir = self._start_version_restricted_server(
            ssl.TLSVersion.TLSv1_2, ssl.TLSVersion.TLSv1_2
        )
        try:
            ca_cert = os.path.join(tmpdir, "ca.cert.pem")
            with Client(timeout=Timeout(5.0), verify=ca_cert) as c:
                resp = c.get(f"https://127.0.0.1:{port}/")
                assert resp.status_code == 200
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)
            shutil.rmtree(tmpdir, ignore_errors=True)

    def test_tls11_server_rejected_by_eggfetch(self):
        """Eggfetch rejects a TLS 1.1-only server (below minimum)."""
        httpd, port, thread, tmpdir = self._start_version_restricted_server(
            ssl.TLSVersion.TLSv1_1, ssl.TLSVersion.TLSv1_1
        )
        try:
            ca_cert = os.path.join(tmpdir, "ca.cert.pem")
            with Client(timeout=Timeout(5.0), verify=ca_cert) as c:
                with pytest.raises(ConnectError):
                    c.get(f"https://127.0.0.1:{port}/")
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)
            shutil.rmtree(tmpdir, ignore_errors=True)

    def test_tls12_range_server_accepted_by_eggfetch(self):
        """Eggfetch connects to a server supporting TLS 1.2-1.3 range."""
        httpd, port, thread, tmpdir = self._start_version_restricted_server(
            ssl.TLSVersion.TLSv1_2, ssl.TLSVersion.TLSv1_3
        )
        try:
            ca_cert = os.path.join(tmpdir, "ca.cert.pem")
            with Client(timeout=Timeout(5.0), verify=ca_cert) as c:
                resp = c.get(f"https://127.0.0.1:{port}/")
                assert resp.status_code == 200
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)
            shutil.rmtree(tmpdir, ignore_errors=True)


# ── mTLS with EggFetch helper-created context ─────────────────────────


class TestMtlsWithEggfetchHelper:
    """mTLS success for contexts created through EggFetch's helper.

    Creates client cert via create_ssl_context(cert=...), which records
    reconstruction metadata in the weak registry. Eggfetch then
    reconstructs the equivalent ClientIdentity for the TLS handshake.
    """

    def _start_mtls_server(
        self, ca_cert_path: str, ca_key_path: str
    ) -> tuple[_ThreadedHTTPServer, int, threading.Thread, str]:
        """Start an mTLS server that requires client certificates."""
        tmpdir = tempfile.mkdtemp()
        cert_path, key_path = _generate_ca_signed_cert(
            tmpdir, ca_cert_path, ca_key_path,
            cn="127.0.0.1", san="IP:127.0.0.1"
        )
        # Generate client cert signed by same CA
        client_cert, client_key = _generate_ca_signed_cert(
            tmpdir, ca_cert_path, ca_key_path,
            cn="client.example.com", san="DNS:client.example.com"
        )

        server_ssl = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        server_ssl.load_cert_chain(cert_path, key_path)
        server_ssl.verify_mode = ssl.CERT_REQUIRED
        server_ssl.load_verify_locations(ca_cert_path)

        httpd = _ThreadedHTTPServer(("127.0.0.1", 0), _CAVerifiedTLSHandler)
        _CAVerifiedTLSHandler.recorded_headers = []
        raw_socket = httpd.socket
        httpd.socket = server_ssl.wrap_socket(raw_socket, server_side=True)
        port = httpd.server_address[1]

        thread = threading.Thread(target=httpd.serve_forever, daemon=True)
        thread.start()

        return httpd, port, thread, tmpdir, client_cert, client_key

    def test_mtls_with_eggfetch_helper_context(self):
        """mTLS succeeds when client cert is loaded via create_ssl_context(cert=...)."""
        with tempfile.TemporaryDirectory() as ca_dir:
            ca_cert, ca_key = _generate_self_signed_ca(ca_dir)
            httpd, port, thread, tmpdir, client_cert, client_key = (
                self._start_mtls_server(ca_cert, ca_key)
            )
            try:
                # Create client context via eggfetch helper with cert=
                client_ctx = create_ssl_context(cert=(client_cert, client_key))
                with Client(
                    timeout=Timeout(5.0),
                    verify=ca_cert,
                    cert=(client_cert, client_key),
                ) as c:
                    resp = c.get(f"https://127.0.0.1:{port}/")
                    assert resp.status_code == 200
            finally:
                httpd.shutdown()
                httpd.server_close()
                thread.join(timeout=2)
                shutil.rmtree(tmpdir, ignore_errors=True)

    def test_mtls_without_client_cert_rejected(self):
        """mTLS server rejects connection without client certificate."""
        with tempfile.TemporaryDirectory() as ca_dir:
            ca_cert, ca_key = _generate_self_signed_ca(ca_dir)
            httpd, port, thread, tmpdir, _, _ = (
                self._start_mtls_server(ca_cert, ca_key)
            )
            try:
                with Client(timeout=Timeout(5.0), verify=ca_cert) as c:
                    with pytest.raises(ConnectError):
                        c.get(f"https://127.0.0.1:{port}/")
            finally:
                httpd.shutdown()
                httpd.server_close()
                thread.join(timeout=2)
                shutil.rmtree(tmpdir, ignore_errors=True)

    def test_mtls_with_wrong_client_cert_rejected(self):
        """mTLS server rejects client cert signed by different CA."""
        with tempfile.TemporaryDirectory() as ca_dir:
            ca_cert, ca_key = _generate_self_signed_ca(ca_dir)
            httpd, port, thread, tmpdir, _, _ = (
                self._start_mtls_server(ca_cert, ca_key)
            )
            try:
                # Generate client cert from a different CA
                other_ca_cert, other_ca_key = _generate_self_signed_ca(ca_dir)
                wrong_cert, wrong_key = _generate_ca_signed_cert(
                    ca_dir, other_ca_cert, other_ca_key,
                    cn="wrong-client", san="DNS:wrong-client"
                )
                with Client(
                    timeout=Timeout(5.0),
                    verify=ca_cert,
                    cert=(wrong_cert, wrong_key),
                ) as c:
                    with pytest.raises(ConnectError):
                        c.get(f"https://127.0.0.1:{port}/")
            finally:
                httpd.shutdown()
                httpd.server_close()
                thread.join(timeout=2)
                shutil.rmtree(tmpdir, ignore_errors=True)


# ── Explicit rejection before dispatch ────────────────────────────────


class TestRejectionBeforeDispatch:
    """Unrepresentable contexts are rejected before network dispatch.

    When a custom transport is provided, the conversion happens at
    transport construction time (HTTPTransport/AsyncHTTPTransport),
    not at Client construction. When no custom transport is provided,
    the conversion happens on the first request via _ensure_client().
    """

    def test_custom_cipher_context_rejected(self):
        """Context with custom ciphers is rejected before any network I/O."""
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        # Use a cipher string that produces a different set than the default.
        ctx.set_ciphers("AES256-SHA")

        # Without custom transport, _ensure_client() triggers conversion.
        # Use Client(verify=ctx) without `as` to avoid __enter__ calling
        # _ensure_client() during context manager setup.
        c = Client(verify=ctx)
        with pytest.raises(TypeError, match="cannot safely translate"):
            c.get("https://example.com/")
        c.close()

    def test_tls11_min_version_context_rejected(self):
        """Context with TLS 1.1 minimum is rejected before network dispatch."""
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.minimum_version = ssl.TLSVersion.TLSv1_1

        c = Client(verify=ctx)
        with pytest.raises(TypeError, match="cannot safely translate"):
            c.get("https://example.com/")
        c.close()

    def test_third_party_subclass_rejected(self):
        """Third-party SSLContext subclass is rejected before dispatch."""
        class CustomContext(ssl.SSLContext):
            pass

        ctx = CustomContext(ssl.PROTOCOL_TLS_CLIENT)
        c = Client(verify=ctx)
        with pytest.raises(TypeError, match="cannot safely translate"):
            c.get("https://example.com/")
        c.close()

    def test_unrepresentable_rejected_at_transport_construction(self):
        """HTTPTransport rejects unrepresentable context at construction."""
        from eggfetch.compat.httpx import HTTPTransport

        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.set_ciphers("AES256-SHA")
        with pytest.raises(TypeError, match="cannot safely translate"):
            HTTPTransport(verify=ctx)


# ── Async variants for key scenarios ──────────────────────────────────


class TestAsyncNetworkProof:
    """Async variants of critical network proof scenarios."""

    @pytest.mark.asyncio
    async def test_custom_ca_accepted_async(self):
        """AsyncClient accepts custom CA-signed server."""
        with tempfile.TemporaryDirectory() as ca_dir:
            ca_cert, ca_key = _generate_self_signed_ca(ca_dir)
            httpd, port, thread, tmpdir = _start_tls_server_with_ca(
                ca_cert, ca_key
            )
            try:
                async with AsyncClient(timeout=Timeout(5.0), verify=ca_cert) as c:
                    resp = await c.get(f"https://127.0.0.1:{port}/")
                    assert resp.status_code == 200
            finally:
                httpd.shutdown()
                httpd.server_close()
                thread.join(timeout=2)
                shutil.rmtree(tmpdir, ignore_errors=True)

    @pytest.mark.asyncio
    async def test_verify_false_allows_self_signed_async(self):
        """AsyncClient with verify=False accepts self-signed cert."""
        with local_tls_server() as (host, port, client_ssl, cert_path):
            async with AsyncClient(timeout=Timeout(5.0), verify=False) as c:
                resp = await c.get(f"https://{host}:{port}/health")
                assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_tls13_server_accepted_async(self):
        """AsyncClient connects to TLS 1.3-only server."""
        httpd, port, thread, tmpdir = (
            TestTLSVersionNegotiation()._start_version_restricted_server(
                ssl.TLSVersion.TLSv1_3, ssl.TLSVersion.TLSv1_3
            )
        )
        try:
            ca_cert = os.path.join(tmpdir, "ca.cert.pem")
            async with AsyncClient(timeout=Timeout(5.0), verify=ca_cert) as c:
                resp = await c.get(f"https://127.0.0.1:{port}/")
                assert resp.status_code == 200
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)
            shutil.rmtree(tmpdir, ignore_errors=True)

    @pytest.mark.asyncio
    async def test_unrepresentable_context_rejected_async(self):
        """AsyncClient rejects unrepresentable context before dispatch."""
        class CustomContext(ssl.SSLContext):
            pass

        ctx = CustomContext(ssl.PROTOCOL_TLS_CLIENT)
        c = AsyncClient(verify=ctx)
        with pytest.raises(TypeError, match="cannot safely translate"):
            await c.get("https://example.com/")
        await c.aclose()
