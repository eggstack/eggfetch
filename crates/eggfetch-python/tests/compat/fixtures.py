"""Deterministic behavior fixtures for HTTPX compatibility testing.

Provides reusable test server endpoints and structured test cases
with stable IDs for systematic compatibility verification.
"""

import base64
import gzip
import http.server
import json
import threading
import urllib.parse
from dataclasses import dataclass, field
from typing import Any


@dataclass
class BehaviorCase:
    """A structured test case for compatibility verification."""
    case_id: str
    description: str
    method: str
    path: str
    expected_status: int
    expected_fields: dict[str, Any] = field(default_factory=dict)
    headers: dict[str, str] = field(default_factory=dict)
    body: bytes | None = None
    content_type: str | None = None
    follow_redirects: bool = True
    timeout: float | None = None


class CompatTestHandler(http.server.BaseHTTPRequestHandler):
    """Deterministic test server for behavior fixtures."""

    def _send_json(self, data, status=200):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_text(self, text, status=200):
        body = text.encode()
        self.send_response(status)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_gzip(self, text, status=200):
        body = gzip.compress(text.encode())
        self.send_response(status)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Encoding", "gzip")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        qs = urllib.parse.parse_qs(parsed.query)
        port = self.server.server_address[1]
        base = f"http://127.0.0.1:{port}"

        if path == "/get":
            self._send_json({"method": "GET", "path": path})
        elif path == "/json":
            self._send_json({"key": "value", "number": 42})
        elif path == "/headers":
            headers = {k: v for k, v in self.headers.items()}
            self._send_json({"headers": headers})
        elif path == "/redirect/301":
            self.send_response(301)
            self.send_header("Location", f"{base}/get")
            self.end_headers()
        elif path == "/redirect/302":
            self.send_response(302)
            self.send_header("Location", f"{base}/get")
            self.end_headers()
        elif path == "/redirect/307":
            self.send_response(307)
            self.send_header("Location", f"{base}/get")
            self.end_headers()
        elif path == "/redirect/308":
            self.send_response(308)
            self.send_header("Location", f"{base}/get")
            self.end_headers()
        elif path == "/redirect/chain":
            depth = int(qs.get("depth", ["1"])[0])
            if depth > 0:
                self.send_response(302)
                self.send_header("Location", f"{base}/redirect/chain?depth={depth - 1}")
                self.end_headers()
            else:
                self._send_json({"method": "GET", "path": path})
        elif path == "/basic-auth":
            auth_header = self.headers.get("Authorization", "")
            if auth_header.startswith("Basic "):
                decoded = base64.b64decode(auth_header[6:]).decode()
                user, password = decoded.split(":", 1)
                self._send_json({"authenticated": True, "user": user, "password": password})
            else:
                self.send_response(401)
                self.send_header("WWW-Authenticate", 'Basic realm="test"')
                self.end_headers()
        elif path == "/bearer-auth":
            auth_header = self.headers.get("Authorization", "")
            if auth_header.startswith("Bearer "):
                token = auth_header[7:]
                self._send_json({"authenticated": True, "token": token})
            else:
                self.send_response(401)
                self.end_headers()
        elif path == "/gzip":
            text = qs.get("text", ["hello gzip"])[0]
            self._send_gzip(text)
        elif path == "/stream":
            count = int(qs.get("count", ["5"])[0])
            lines = [f"line-{i}" for i in range(count)]
            text = "\n".join(lines) + "\n"
            self._send_text(text)
        elif path == "/set-cookie":
            value = qs.get("value", ["test-cookie"])[0]
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Set-Cookie", f"session={value}; Path=/; HttpOnly")
            body = json.dumps({"cookie_set": True}).encode()
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path == "/echo-cookies":
            cookie_header = self.headers.get("Cookie", "")
            self._send_json({"cookie_header": cookie_header})
        elif path == "/delay":
            import time
            seconds = float(qs.get("seconds", ["0.1"])[0])
            time.sleep(seconds)
            self._send_text("done")
        elif path == "/status/200":
            self._send_text("ok")
        elif path == "/status/301":
            self.send_response(301)
            self.send_header("Location", f"{base}/get")
            self.end_headers()
        elif path == "/status/404":
            self._send_text("not found", 404)
        elif path == "/status/500":
            self._send_text("server error", 500)
        elif path == "/status/503":
            self._send_text("service unavailable", 503)
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        if path == "/json":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            data = json.loads(body)
            self._send_json({"received": data})
        elif path == "/echo":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            self.send_response(200)
            self.send_header(
                "Content-Type",
                self.headers.get("Content-Type", "application/octet-stream"),
            )
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path == "/form":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            data = urllib.parse.parse_qs(body.decode())
            self._send_json(
                {
                    "received": {
                        k: v[0] if len(v) == 1 else v
                        for k, v in data.items()
                    }
                }
            )
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        pass


def create_server():
    """Create and start a test server, returning the base URL."""
    server = http.server.HTTPServer(("127.0.0.1", 0), CompatTestHandler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, f"http://127.0.0.1:{port}"


# Pre-defined behavior cases
BEHAVIOR_CASES = [
    BehaviorCase(
        case_id="GET-001",
        description="Simple GET returns JSON",
        method="GET",
        path="/get",
        expected_status=200,
        expected_fields={"method": "GET"},
    ),
    BehaviorCase(
        case_id="REDIRECT-001",
        description="302 redirect not followed by default",
        method="GET",
        path="/redirect/302",
        expected_status=302,
        follow_redirects=False,
    ),
    BehaviorCase(
        case_id="REDIRECT-002",
        description="302 redirect followed with flag",
        method="GET",
        path="/redirect/302",
        expected_status=200,
        follow_redirects=True,
    ),
    BehaviorCase(
        case_id="BASIC-AUTH-001",
        description="Basic auth sends credentials",
        method="GET",
        path="/basic-auth",
        expected_status=200,
        headers={"Authorization": "Basic dXNlcjpwYXNz"},
        expected_fields={"authenticated": True, "user": "user"},
    ),
    BehaviorCase(
        case_id="BEARER-AUTH-001",
        description="Bearer auth sends token",
        method="GET",
        path="/bearer-auth",
        expected_status=200,
        headers={"Authorization": "Bearer test-token-123"},
        expected_fields={"authenticated": True, "token": "test-token-123"},
    ),
    BehaviorCase(
        case_id="JSON-001",
        description="GET /json returns expected payload",
        method="GET",
        path="/json",
        expected_status=200,
        expected_fields={"key": "value", "number": 42},
    ),
    BehaviorCase(
        case_id="JSON-POST-001",
        description="POST JSON echoes received data",
        method="POST",
        path="/json",
        expected_status=200,
        body=json.dumps({"data": "test"}).encode(),
        content_type="application/json",
        expected_fields={"received": {"data": "test"}},
    ),
    BehaviorCase(
        case_id="STATUS-404-001",
        description="404 status code",
        method="GET",
        path="/status/404",
        expected_status=404,
    ),
    BehaviorCase(
        case_id="STATUS-500-001",
        description="500 status code",
        method="GET",
        path="/status/500",
        expected_status=500,
    ),
]
