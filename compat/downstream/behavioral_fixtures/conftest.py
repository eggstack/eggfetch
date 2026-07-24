"""Shared conftest for behavioral fixtures — starts a local HTTP test server."""

from __future__ import annotations

import http.server
import json
import socketserver
import threading
import time

import pytest


class _Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/get":
            self._respond(200, {"method": "GET", "path": self.path})
        elif self.path == "/headers":
            headers = dict(self.headers)
            self._respond(200, {"headers": headers})
        else:
            self._respond(404, {"error": "not found"})

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length) if length else b""
        try:
            data = json.loads(body) if body else {}
        except json.JSONDecodeError:
            data = {"raw": body.decode("utf-8", errors="replace")}
        self._respond(200, {"method": "POST", "path": self.path, "data": data})

    def do_PUT(self):
        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length) if length else b""
        try:
            data = json.loads(body) if body else {}
        except json.JSONDecodeError:
            data = {"raw": body.decode("utf-8", errors="replace")}
        self._respond(200, {"method": "PUT", "path": self.path, "data": data})

    def do_DELETE(self):
        self._respond(200, {"method": "DELETE", "path": self.path})

    def _respond(self, status: int, body: dict):
        payload = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, format, *args):
        pass  # suppress request logs


class _QuietTCPServer(socketserver.TCPServer):
    allow_reuse_address = True


@pytest.fixture(scope="session")
def http_server():
    """Start a local HTTP server on a random port for the test session."""
    server = _QuietTCPServer(("127.0.0.1", 0), _Handler)
    host, port = server.server_address
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    time.sleep(0.1)
    yield f"http://{host}:{port}"
    server.shutdown()
    thread.join(timeout=5)
