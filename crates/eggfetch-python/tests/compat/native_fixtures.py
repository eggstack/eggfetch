"""Deterministic local network fixtures for native lifecycle testing.

All fixtures use real local TCP sockets. No external internet access required.
Fixtures expose synchronization barriers rather than relying on sleeps.
"""
import http.server
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
    allow_reuse_address = True
    request_queue_size = 32


class DelayedResponseHandler(http.server.BaseHTTPRequestHandler):
    """HTTP handler that can delay response headers or body chunks."""

    def do_GET(self):
        if self.path == "/health":
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"ok")
            self.wfile.flush()
        elif self.path == "/json":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"status": "ok"}).encode())
            self.wfile.flush()
        elif self.path == "/slow":
            time.sleep(3)
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"slow")
            self.wfile.flush()
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        content_length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_length) if content_length else b""
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps({"received": len(body)}).encode())
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
    """Minimal HTTP proxy that forwards requests and handles CONNECT."""

    def do_GET(self):
        self._forward_request()

    def do_POST(self):
        self._forward_request()

    def do_CONNECT(self):
        """Handle CONNECT tunnel (used for TLS through proxy)."""
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
        try:
            upstream = socket.create_connection(("127.0.0.1", 1), timeout=5)
        except OSError:
            self.send_error(502, "Upstream unavailable")
            return
        try:
            req_line = f"{self.command} / HTTP/1.1\r\nHost: 127.0.0.1:1\r\n"
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
def local_proxy_server() -> Generator[tuple[str, int], None, None]:
    """Deterministic loopback HTTP proxy that forwards requests and handles CONNECT."""
    httpd = _ThreadedHTTPServer(("127.0.0.1", 0), _ProxyHandler)
    port = httpd.server_address[1]
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    try:
        yield "127.0.0.1", port
    finally:
        httpd.shutdown()


class _TLSDirectHandler(http.server.BaseHTTPRequestHandler):
    """Simple handler served over TLS."""

    def do_GET(self):
        if self.path == "/health":
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"ok")
        elif self.path == "/json":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"status": "tls-ok"}).encode())
        else:
            self.send_response(404)
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
        ],
        check=True, capture_output=True,
    )
    return cert_path, key_path


@contextmanager
def local_tls_server() -> Generator[tuple[str, int, ssl.SSLContext], None, None]:
    """TLS-capable test server with a self-signed certificate.

    Yields (host, port, client_ssl_context) where client_ssl_context can be
    used to verify the server certificate.
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
            yield "127.0.0.1", port, client_ssl
        finally:
            httpd.shutdown()
