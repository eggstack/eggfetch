"""Deterministic local network fixtures for native lifecycle testing.

All fixtures use real local TCP sockets. No external internet access required.
Fixtures expose synchronization barriers rather than relying on sleeps.
"""
import http.server
import json
import socket
import socketserver
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
