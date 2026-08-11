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


class _ProxyHandler(http.server.BaseHTTPRequestHandler):
    """Minimal HTTP proxy that forwards requests and handles CONNECT.

    Records all observed methods and targets for verification by tests.
    """

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

    def _record_request(self, method: str, target: str = "") -> None:
        """Record method and target for test verification."""
        self.__class__.recorded_requests.append({
            "method": method,
            "target": target or self.path,
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
    """TLS-wrapped HTTP proxy for HTTPS-proxy endpoint qualification."""
    with tempfile.TemporaryDirectory() as tmpdir:
        cert_path, key_path = certificate or _generate_self_signed_cert(tmpdir)
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
            yield "127.0.0.1", port, handler_class, cert_path
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)


class _TLSDirectHandler(http.server.BaseHTTPRequestHandler):
    """Simple handler served over TLS."""

    def do_GET(self):
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
