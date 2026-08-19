"""Deterministic local network fixtures for native lifecycle testing.

All fixtures use real local TCP sockets. No external internet access required.
Fixtures expose synchronization barriers rather than relying on sleeps.
"""
import http.server
import gzip
import json
import os
import socket
import socketserver
import ssl
import tempfile
import threading
import time
from contextlib import contextmanager
from typing import Generator


class _ThreadedHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    block_on_close = False
    allow_reuse_address = True
    request_queue_size = 32


class _ThreadedHTTPServerV6(_ThreadedHTTPServer):
    """Threaded HTTP server bound to the IPv6 loopback address."""

    address_family = socket.AF_INET6


class DelayedResponseHandler(http.server.BaseHTTPRequestHandler):
    """HTTP handler that can delay response headers or body chunks."""

    protocol_version = "HTTP/1.0"

    def do_GET(self):
        if self.path == "/health":
            body = b"ok"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
        elif self.path == "/json":
            body = json.dumps({"status": "ok"}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
        elif self.path == "/slow":
            time.sleep(3)
            body = b"slow"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
        else:
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.send_header("Connection", "close")
            self.end_headers()

    def do_POST(self):
        content_length = int(self.headers.get("Content-Length", 0))
        received_length = len(self.rfile.read(content_length)) if content_length else 0
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        body = json.dumps({"received": received_length}).encode()
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()

    def log_message(self, format, *args):
        pass


class HeadersStallHandler(http.server.BaseHTTPRequestHandler):
    """Sends headers immediately then stalls briefly before closing."""

    def do_GET(self):
        if self.path == "/headers-then-stall":
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Transfer-Encoding", "chunked")
            self.end_headers()
            time.sleep(3)
        else:
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"ok")
            self.wfile.flush()

    def log_message(self, format, *args):
        pass


def blocking_gzip_handler():
    """Return a handler that blocks after sending the first gzip byte."""

    class BlockingGzipHandler(http.server.BaseHTTPRequestHandler):
        first_body_sent = threading.Event()
        body_blocked = threading.Event()
        release_body = threading.Event()

        def do_GET(self):
            if self.path == "/gzip-blocked":
                original = b"native cancellation body " * 2048
                body = gzip.compress(original, mtime=0)
                self.send_response(200)
                self.send_header("Content-Encoding", "gzip")
                self.send_header("Content-Length", str(len(body)))
                self.send_header("Connection", "close")
                self.end_headers()
                self.wfile.write(body[:1])
                self.wfile.flush()
                self.__class__.first_body_sent.set()
                self.__class__.body_blocked.set()
                self.__class__.release_body.wait()
                try:
                    self.wfile.write(body[1:])
                    self.wfile.flush()
                except OSError:
                    pass
            elif self.path == "/follow-up":
                body = b"follow-up ok"
                self.send_response(200)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                self.wfile.flush()
            else:
                self.send_response(404)
                self.end_headers()

        def log_message(self, format, *args):
            pass

    return BlockingGzipHandler


@contextmanager
def local_http_server(
    handler_class=DelayedResponseHandler,
) -> Generator[tuple[str, int], None, None]:
    """Start a local HTTP server and yield (host, port)."""
    httpd = _ThreadedHTTPServer(("127.0.0.1", 0), handler_class)
    port = httpd.server_address[1]

    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()

    try:
        yield "127.0.0.1", port
    finally:
        httpd.shutdown()
        httpd.server_close()
        thread.join(timeout=2)


@contextmanager
def local_ipv6_http_server(
    handler_class=DelayedResponseHandler,
) -> Generator[tuple[str, int], None, None]:
    """Start a deterministic HTTP server on IPv6 loopback."""
    httpd = _ThreadedHTTPServerV6(("::1", 0), handler_class)
    port = httpd.server_address[1]
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    try:
        yield "::1", port
    finally:
        httpd.shutdown()
        httpd.server_close()
        thread.join(timeout=2)


@contextmanager
def local_stall_server() -> Generator[tuple[str, int, threading.Event], None, None]:
    """TCP server that accepts connections but never sends data (for timeout testing)."""
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
                conn, _addr = server.accept()
                conn.settimeout(1)
                while not stop.is_set():
                    try:
                        data = conn.recv(1024)
                        if not data:
                            break
                    except (socket.timeout, ConnectionResetError):
                        break
                conn.close()
            except (socket.timeout, OSError):
                break

    thread = threading.Thread(target=accept_loop, daemon=True)
    thread.start()
    ready.set()

    try:
        yield "127.0.0.1", port, ready
    finally:
        stop.set()
        server.close()
        thread.join(timeout=2)


@contextmanager
def local_tls_handshake_stall_server() -> Generator[tuple[str, int], None, None]:
    """Accept TCP but never complete a TLS handshake."""
    stop = threading.Event()
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(("127.0.0.1", 0))
    server.listen(1)
    port = server.getsockname()[1]
    server.settimeout(1)

    def accept_loop():
        while not stop.is_set():
            try:
                conn, _addr = server.accept()
                # Poll the stop flag without closing the connection on a
                # socket timeout. Closing at the one-second connect budget
                # races the reference client's TLS timeout and can turn the
                # expected ConnectTimeout into an EOF ConnectError.
                conn.settimeout(0.1)
                while not stop.is_set():
                    try:
                        if not conn.recv(1024):
                            break
                    except socket.timeout:
                        continue
                    except (ConnectionResetError, OSError):
                        break
                conn.close()
            except (socket.timeout, OSError):
                break

    thread = threading.Thread(target=accept_loop, daemon=True)
    thread.start()
    try:
        yield "127.0.0.1", port
    finally:
        stop.set()
        server.close()
        thread.join(timeout=2)


class _ProxyHandler(http.server.BaseHTTPRequestHandler):
    """Minimal HTTP proxy that forwards requests and handles CONNECT.

    Records all observed methods and targets for verification by tests.
    """

    # Close each proxy-side request after forwarding one response. Keeping a
    # proxy connection alive across fixture instances can leave handler
    # threads holding loopback sockets after the context manager exits.
    protocol_version = "HTTP/1.0"

    backend: tuple[str, int] | None = None
    recorded_requests: list[dict] = []

    def do_GET(self):
        self._record_request("GET")
        self._forward_request()

    def do_POST(self):
        self._record_request("POST")
        self._forward_request()

    def do_CONNECT(self):
        """Handle CONNECT tunnel (used for TLS through proxy)."""
        self._record_request("CONNECT", target=self.path)
        target_host, _, target_port = self.path.partition(":")
        target_port = int(target_port) if target_port else 443
        try:
            upstream = socket.create_connection((target_host, target_port), timeout=5)
        except OSError as exc:
            self.send_error(502, f"Upstream connect failed: {exc}")
            return
        self.send_response(200, "Connection Established")
        self.end_headers()
        self.connection.setblocking(False)
        upstream.setblocking(False)
        self._tunnel(self.connection, upstream)
        upstream.close()
        self.close_connection = True

    def _record_request(self, method: str, target: str = "") -> None:
        """Record method and target for test verification."""
        self.__class__.recorded_requests.append({
            "method": method,
            "target": target or self.path,
            "headers": {name.lower(): value for name, value in self.headers.items()},
        })

    def _tunnel(self, client: socket.socket, upstream: socket.socket):
        """Bidirectional data relay between client and upstream."""
        import select
        client.setblocking(True)
        upstream.setblocking(True)
        while True:
            try:
                readable, _, _ = select.select([client, upstream], [], [], 5)
                if not readable:
                    break
                for sock in readable:
                    data = sock.recv(8192)
                    if not data:
                        return
                    target = upstream if sock is client else client
                    target.sendall(data)
            except (OSError, ConnectionResetError):
                break

    def _forward_request(self):
        """Forward an HTTP request to the target server."""
        content_length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_length) if content_length else b""

        target = getattr(self.__class__, "backend", None)
        if target is None:
            self.send_error(502, "No backend configured")
            return

        # Strip scheme and host from path (proxy clients send full URL)
        path = self.path
        if path.startswith("http://"):
            path = path[7:]
            path = path[path.index("/"):] if "/" in path else "/"
        elif path.startswith("https://"):
            path = path[8:]
            path = path[path.index("/"):] if "/" in path else "/"

        try:
            upstream = socket.create_connection(target, timeout=5)
        except OSError:
            self.send_error(502, "Upstream unavailable")
            return
        try:
            req_line = (
                f"{self.command} {path} HTTP/1.1\r\n"
                f"Host: {target[0]}:{target[1]}\r\nConnection: close\r\n"
            )
            if body:
                req_line += f"Content-Length: {len(body)}\r\n"
            req_line += "\r\n"
            upstream.sendall(req_line.encode() + body)
            response = b""
            while True:
                chunk = upstream.recv(8192)
                if not chunk:
                    break
                response += chunk
        except OSError:
            self.send_error(502, "Upstream error")
            return
        finally:
            upstream.close()
        try:
            self.wfile.write(response)
            self.wfile.flush()
        except BrokenPipeError:
            pass
        finally:
            self.close_connection = True

    def log_message(self, format, *args):
        pass


@contextmanager
def local_proxy_server(
    backend: tuple[str, int] | None = None,
) -> Generator[tuple[str, int, type], None, None]:
    """Deterministic loopback HTTP proxy that forwards requests and handles CONNECT.

    If *backend* is given, all requests are forwarded to that (host, port) pair.

    Yields (host, port, handler_class) where handler_class.recorded_requests
    contains the list of observed method/target dicts for test verification.
    """
    handler_class = _ProxyHandler
    if backend is not None:
        handler_class = type("_ConfiguredProxy", (_ProxyHandler,), {"backend": backend})
    # Clear recorded requests for this proxy instance
    handler_class.recorded_requests = []
    httpd = _ThreadedHTTPServer(("127.0.0.1", 0), handler_class)
    port = httpd.server_address[1]
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    try:
        yield "127.0.0.1", port, handler_class
    finally:
        httpd.shutdown()
        httpd.server_close()
        thread.join(timeout=2)


@contextmanager
def local_tls_proxy_server(
    backend: tuple[str, int] | None = None,
    certificate: tuple[str, str] | None = None,
) -> Generator[tuple[str, int, type, str], None, None]:
    """TLS-wrapped HTTP proxy for HTTPS-proxy endpoint qualification.

    The default cert is a CA-signed server cert (the CA is a
    separate ``CA:TRUE`` cert) so the client trust anchor
    (``ca_cert_path``) is enumerable through
    ``ssl.SSLContext.get_ca_certs()``.  See
    ``_generate_ca_signed_server_cert``.

    The yielded ``cert_path`` is the server cert; use
    ``local_tls_proxy_server_with_ca`` if you also need the CA
    cert as a separate trust anchor.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        if certificate is not None:
            cert_path, key_path = certificate
            ca_cert_path = None
        else:
            (
                ca_cert_path,
                _ca_key_path,
                cert_path,
                key_path,
            ) = _generate_ca_signed_server_cert(tmpdir)
        server_ssl = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        server_ssl.load_cert_chain(cert_path, key_path)
        handler_class = _ProxyHandler
        if backend is not None:
            handler_class = type("_ConfiguredTlsProxy", (_ProxyHandler,), {"backend": backend})
        handler_class.recorded_requests = []
        httpd = _ThreadedHTTPServer(("127.0.0.1", 0), handler_class)
        raw_socket = httpd.socket
        httpd.socket = server_ssl.wrap_socket(raw_socket, server_side=True)
        port = httpd.server_address[1]
        thread = threading.Thread(target=httpd.serve_forever, daemon=True)
        thread.start()
        try:
            yield "127.0.0.1", port, handler_class, (cert_path, ca_cert_path)
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)


class _TLSDirectHandler(http.server.BaseHTTPRequestHandler):
    """Simple handler served over TLS."""

    recorded_headers: list[dict[str, str]] = []

    def do_GET(self):
        self.__class__.recorded_headers.append(
            {name.lower(): value for name, value in self.headers.items()}
        )
        if self.path == "/health":
            body = b"ok"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == "/json":
            body = json.dumps({"status": "tls-ok"}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()

    def log_message(self, format, *args):
        pass


def _generate_self_signed_cert(cert_dir: str) -> tuple[str, str]:
    """Generate a self-signed certificate and key file."""
    cert_path = os.path.join(cert_dir, "cert.pem")
    key_path = os.path.join(cert_dir, "key.pem")
    import subprocess
    subprocess.run(
        [
            "openssl", "req", "-x509", "-newkey", "rsa:2048",
            "-keyout", key_path, "-out", cert_path,
            "-days", "1", "-nodes",
            "-subj", "/CN=127.0.0.1",
            "-addext", "basicConstraints=critical,CA:FALSE",
            "-addext", "subjectAltName=IP:127.0.0.1",
        ],
        check=True, capture_output=True,
    )
    return cert_path, key_path


def _generate_self_signed_ca_cert(cert_dir: str) -> tuple[str, str]:
    """Generate a self-signed CA certificate and key file.

    The cert is marked with ``CA:TRUE`` so it can act as a trust
    anchor that the Python ``ssl`` module exposes through
    ``SSLContext.get_ca_certs()``.  The same cert can be used as
    both a server cert (for a TLS server fixture) and a trust
    anchor (for the client).  Using a non-CA cert (CA:FALSE) as a
    trust anchor is accepted by ``load_verify_locations`` but
    hidden from ``get_ca_certs``, which prevents eggfetch's
    translation layer from extracting the actual DER anchors.
    """
    cert_path = os.path.join(cert_dir, "cert.pem")
    key_path = os.path.join(cert_dir, "key.pem")
    import subprocess
    subprocess.run(
        [
            "openssl", "req", "-x509", "-newkey", "rsa:2048",
            "-keyout", key_path, "-out", cert_path,
            "-days", "1", "-nodes",
            "-subj", "/CN=127.0.0.1",
            "-addext", "basicConstraints=critical,CA:TRUE",
            "-addext", "subjectAltName=IP:127.0.0.1",
        ],
        check=True, capture_output=True,
    )
    return cert_path, key_path


def _generate_ca_signed_server_cert(
    cert_dir: str,
) -> tuple[str, str, str, str]:
    """Generate a CA cert and a CA-signed server cert.

    Returns ``(ca_cert_path, ca_key_path, server_cert_path, server_key_path)``.

    The server cert is signed by the CA and has ``CA:FALSE`` so it
    is a valid server identity.  The CA cert has ``CA:TRUE`` so
    it is a valid trust anchor that the Python ``ssl`` module
    exposes through ``SSLContext.get_ca_certs()``.  The test
    loads the CA cert into the client trust store and the server
    uses the server cert as its identity.
    """
    import subprocess

    ca_cert_path = os.path.join(cert_dir, "ca.cert.pem")
    ca_key_path = os.path.join(cert_dir, "ca.key.pem")
    server_cert_path = os.path.join(cert_dir, "server.cert.pem")
    server_key_path = os.path.join(cert_dir, "server.key.pem")
    server_csr_path = os.path.join(cert_dir, "server.csr.pem")
    server_ext_path = os.path.join(cert_dir, "server.ext.cnf")

    # Generate the CA cert (CA:TRUE).
    subprocess.run(
        [
            "openssl", "req", "-x509", "-newkey", "rsa:2048",
            "-keyout", ca_key_path, "-out", ca_cert_path,
            "-days", "1", "-nodes",
            "-subj", "/CN=Test CA",
            "-addext", "basicConstraints=critical,CA:TRUE",
        ],
        check=True,
        capture_output=True,
    )

    # Generate the server cert CSR with SAN.
    with open(server_ext_path, "w") as f:
        f.write("[req]\ndistinguished_name = req_dn\n")
        f.write("req_extensions = v3_req\n")
        f.write("prompt = no\n\n")
        f.write("[req_dn]\nCN = 127.0.0.1\n\n")
        f.write("[v3_req]\nsubjectAltName = IP:127.0.0.1\n")

    subprocess.run(
        [
            "openssl", "req", "-newkey", "rsa:2048",
            "-keyout", server_key_path, "-out", server_csr_path,
            "-nodes", "-config", server_ext_path,
        ],
        check=True,
        capture_output=True,
    )

    # Sign the server cert with the CA.  basicConstraints=CA:FALSE
    # is the default; we only copy the SAN extension.
    subprocess.run(
        [
            "openssl", "x509", "-req",
            "-in", server_csr_path, "-out", server_cert_path,
            "-CA", ca_cert_path, "-CAkey", ca_key_path,
            "-CAcreateserial", "-days", "1",
            "-copy_extensions", "copyall",
        ],
        check=True,
        capture_output=True,
    )

    return ca_cert_path, ca_key_path, server_cert_path, server_key_path


def pem_to_der_bytes(pem_path: str) -> bytes:
    """Convert a PEM certificate to DER bytes.

    Used by tests that need to pass a trust anchor to eggfetch via
    a path that does not depend on the Python ``ssl`` module's
    ability (or inability) to enumerate the loaded CAs.
    """
    from cryptography import x509
    import cryptography.hazmat.primitives.serialization as ser

    with open(pem_path, "rb") as f:
        cert = x509.load_pem_x509_certificate(f.read())
    return cert.public_bytes(ser.Encoding.DER)


@contextmanager
def local_tls_server() -> Generator[tuple[str, int, ssl.SSLContext, str], None, None]:
    """TLS-capable test server with a self-signed certificate.

    Yields (host, port, client_ssl_context, cert_path) where client_ssl_context
    can be used to verify the server certificate, and cert_path is the
    CA certificate PEM file path that the native engine accepts.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        cert_path, key_path = _generate_self_signed_cert(tmpdir)

        server_ssl = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        server_ssl.load_cert_chain(cert_path, key_path)

        httpd = _ThreadedHTTPServer(("127.0.0.1", 0), _TLSDirectHandler)
        _TLSDirectHandler.recorded_headers = []
        raw_socket = httpd.socket
        httpd.socket = server_ssl.wrap_socket(raw_socket, server_side=True)
        port = httpd.server_address[1]

        client_ssl = ssl.create_default_context()
        client_ssl.load_verify_locations(cert_path)

        thread = threading.Thread(target=httpd.serve_forever, daemon=True)
        thread.start()
        try:
            yield "127.0.0.1", port, client_ssl, cert_path
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)


# ---------------------------------------------------------------------------
# HTTP/2 prior-knowledge server fixtures
# ---------------------------------------------------------------------------

import h2.config
import h2.connection
import h2.events


def _h2_handle_request(
    conn: h2.connection.H2Connection,
    event: h2.events.RequestReceived,
) -> None:
    """Handle a single HTTP/2 request and send a response."""
    headers = dict(event.headers)
    path = headers.get(b":path", b"/").decode("ascii", errors="replace")
    method = headers.get(b":method", b"GET").decode("ascii", errors="replace")

    if path == "/health":
        body = b"ok"
        resp_headers = [
            (b":status", b"200"),
            (b"content-type", b"text/plain"),
            (b"content-length", str(len(body)).encode()),
        ]
        conn.send_headers(event.stream_id, resp_headers, end_stream=False)
        conn.send_data(event.stream_id, body, end_stream=True)
    elif path == "/json":
        import json as _json
        body = _json.dumps({"status": "h2-ok"}).encode()
        resp_headers = [
            (b":status", b"200"),
            (b"content-type", b"application/json"),
            (b"content-length", str(len(body)).encode()),
        ]
        conn.send_headers(event.stream_id, resp_headers, end_stream=False)
        conn.send_data(event.stream_id, body, end_stream=True)
    elif path == "/echo-body":
        resp_headers = [
            (b":status", b"200"),
            (b"content-type", b"application/octet-stream"),
        ]
        conn.send_headers(event.stream_id, resp_headers, end_stream=False)
    elif path == "/streaming":
        resp_headers = [
            (b":status", b"200"),
            (b"content-type", b"text/plain"),
        ]
        conn.send_headers(event.stream_id, resp_headers, end_stream=False)
        for i in range(3):
            chunk = f"chunk-{i}\n".encode()
            conn.send_data(event.stream_id, chunk, end_stream=False)
        conn.send_data(event.stream_id, b"", end_stream=True)
    elif path == "/close":
        conn.close_connection()
    else:
        resp_headers = [
            (b":status", b"404"),
            (b"content-length", b"0"),
        ]
        conn.send_headers(event.stream_id, resp_headers, end_stream=True)


def _h2_server_loop(
    raw_sock: socket.socket,
    stop: threading.Event,
) -> None:
    """Run an HTTP/2 server loop on a single accepted connection."""
    config = h2.config.H2Configuration(client_side=False)
    conn = h2.connection.H2Connection(config=config)
    conn.initiate_connection()
    raw_sock.sendall(conn.data_to_send())

    body_buffers: dict[int, bytes] = {}

    while not stop.is_set():
        try:
            raw_sock.settimeout(0.5)
            data = raw_sock.recv(65535)
            if not data:
                break
        except (socket.timeout, OSError):
            continue

        try:
            events = conn.receive_data(data)
        except h2.exceptions.ProtocolError:
            # A negative H2-only test may deliberately send HTTP/1.1 after
            # the TLS handshake.  Treat the invalid preamble as a completed
            # fixture interaction instead of leaking a thread warning.
            break
        for event in events:
            if isinstance(event, h2.events.RequestReceived):
                try:
                    _h2_handle_request(conn, event)
                except Exception:
                    try:
                        conn.reset_stream(
                            event.stream_id,
                            error_code=h2.errors.ErrorCodes.INTERNAL_ERROR,
                        )
                    except Exception:
                        pass
            elif isinstance(event, h2.events.DataReceived):
                stream_id = event.stream_id
                body_buffers[stream_id] = body_buffers.get(stream_id, b"") + event.data
                conn.acknowledge_received_data(len(event.data), stream_id)
            elif isinstance(event, h2.events.StreamEnded):
                stream_id = event.stream_id
                if stream_id in body_buffers:
                    body = body_buffers.pop(stream_id)
                    resp_headers = [
                        (b":status", b"200"),
                        (b"content-type", b"application/octet-stream"),
                        (b"content-length", str(len(body)).encode()),
                    ]
                    conn.send_headers(stream_id, resp_headers, end_stream=True)
                    conn.send_data(stream_id, body, end_stream=True)

        try:
            raw_sock.sendall(conn.data_to_send())
        except OSError:
            break

    try:
        raw_sock.close()
    except OSError:
        pass


class _H2RequestCounter:
    """Thread-safe counter for tracking requests handled by H2 server."""

    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._count = 0

    def increment(self) -> int:
        with self._lock:
            self._count += 1
            return self._count

    @property
    def count(self) -> int:
        with self._lock:
            return self._count


@contextmanager
def local_h2_server() -> Generator[
    tuple[str, int, _H2RequestCounter], None, None
]:
    """Start a local HTTP/2 prior-knowledge (cleartext) server.

    The server speaks raw HTTP/2 without an HTTP/1.1 Upgrade handshake.
    Clients must send the HTTP/2 connection preface directly.

    Yields (host, port, request_counter).
    """
    stop = threading.Event()
    counter = _H2RequestCounter()
    server_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server_sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server_sock.bind(("127.0.0.1", 0))
    server_sock.listen(32)
    port = server_sock.getsockname()[1]
    server_sock.settimeout(1)

    def accept_loop() -> None:
        while not stop.is_set():
            try:
                conn, _addr = server_sock.accept()
                counter.increment()
                t = threading.Thread(
                    target=_h2_server_loop,
                    args=(conn, stop),
                    daemon=True,
                )
                t.start()
            except (socket.timeout, OSError):
                continue

    thread = threading.Thread(target=accept_loop, daemon=True)
    thread.start()
    try:
        yield "127.0.0.1", port, counter
    finally:
        stop.set()
        server_sock.close()
        thread.join(timeout=3)


def _tls_h2_server_loop(
    raw_sock: socket.socket,
    stop: threading.Event,
    server_ssl: ssl.SSLContext,
) -> None:
    """Run an HTTP/2 server loop over TLS."""
    try:
        tls_sock = server_ssl.wrap_socket(raw_sock, server_side=True)
    except (ssl.SSLError, OSError):
        raw_sock.close()
        return

    alpn = tls_sock.selected_alpn_protocol()
    if alpn == "h2":
        _h2_server_loop(tls_sock, stop)
    else:
        # Fallback: if client didn't negotiate h2, close
        tls_sock.close()


@contextmanager
def local_tls_h2_server() -> Generator[
    tuple[str, int, ssl.SSLContext, str, _H2RequestCounter], None, None
]:
    """TLS server that negotiates HTTP/2 via ALPN.

    The server advertises ``h2`` and ``http/1.1`` in ALPN. If the client
    selects ``h2``, the connection is served over HTTP/2. Otherwise the
    connection is closed (no HTTP/1.1 fallback in this fixture).

    Yields (host, port, client_ssl_context, cert_path, request_counter).
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        cert_path, key_path = _generate_self_signed_cert(tmpdir)

        server_ssl = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        server_ssl.load_cert_chain(cert_path, key_path)
        server_ssl.set_alpn_protocols(["h2", "http/1.1"])

        client_ssl = ssl.create_default_context()
        client_ssl.load_verify_locations(cert_path)

        stop = threading.Event()
        counter = _H2RequestCounter()
        server_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        server_sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server_sock.bind(("127.0.0.1", 0))
        server_sock.listen(32)
        port = server_sock.getsockname()[1]
        server_sock.settimeout(1)

        def accept_loop() -> None:
            while not stop.is_set():
                try:
                    conn, _addr = server_sock.accept()
                    counter.increment()
                    t = threading.Thread(
                        target=_tls_h2_server_loop,
                        args=(conn, stop, server_ssl),
                        daemon=True,
                    )
                    t.start()
                except (socket.timeout, OSError):
                    continue

        thread = threading.Thread(target=accept_loop, daemon=True)
        thread.start()
        try:
            yield "127.0.0.1", port, client_ssl, cert_path, counter
        finally:
            stop.set()
            server_sock.close()
            thread.join(timeout=3)
