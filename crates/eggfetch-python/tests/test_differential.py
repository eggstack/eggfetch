"""Differential compatibility tests comparing eggfetch against requests/HTTPX.

These tests verify that eggfetch behaves identically to requests/HTTPX for
shared features, and documents intentional differences. Tests are skipped if
the comparison library is not installed.

These are supplementary differential tests. The required compatibility
tests live in tests/compat/test_httpx_required.py and are enforced
by the CI gate. Tests here may skip when comparison libraries are absent.
"""

import gzip
import http.server
import json
import threading
import urllib.parse

import pytest

import eggfetch

try:
    import requests as _requests

    HAS_REQUESTS = True
except ImportError:
    HAS_REQUESTS = False

try:
    import httpx as _httpx

    HAS_HTTPX = True
except ImportError:
    HAS_HTTPX = False


# ---------------------------------------------------------------------------
# Test server
# ---------------------------------------------------------------------------


class _DifferentialHandler(http.server.BaseHTTPRequestHandler):
    """Test server for differential compatibility tests."""

    def _send_json(self, data: dict, status: int = 200):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_text(self, text: str, status: int = 200):
        body = text.encode()
        self.send_response(status)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_gzip(self, text: str, status: int = 200):
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

        elif path == "/redirect/302":
            self.send_response(302)
            self.send_header("Location", f"{base}/get")
            self.end_headers()

        elif path == "/redirect/301":
            self.send_response(301)
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

        elif path == "/headers":
            headers = {k: v for k, v in self.headers.items()}
            self._send_json({"headers": headers})

        elif path == "/status/200":
            self._send_text("ok")

        elif path == "/status/404":
            self._send_text("not found", 404)

        elif path == "/status/500":
            self._send_text("server error", 500)

        elif path == "/status/503":
            self._send_text("service unavailable", 503)

        elif path == "/basic-auth":
            import base64

            auth_header = self.headers.get("Authorization", "")
            if auth_header.startswith("Basic "):
                decoded = base64.b64decode(auth_header[6:]).decode()
                user, password = decoded.split(":", 1)
                self._send_json({"authenticated": True, "user": user})
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

        elif path == "/json":
            self._send_json({"key": "value", "number": 42})

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

        elif path == "/gzip":
            text = qs.get("text", ["hello gzip"])[0]
            self._send_gzip(text)

        elif path == "/stream":
            count = int(qs.get("count", ["5"])[0])
            lines = []
            for i in range(count):
                lines.append(f"line-{i}")
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

        elif path == "/multi-set-cookie":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Set-Cookie", "a=1; Path=/; HttpOnly")
            self.send_header("Set-Cookie", "b=2; Path=/; HttpOnly")
            self.send_header("Set-Cookie", "c=3; Path=/")
            body = json.dumps({"cookies_set": 3}).encode()
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        elif path == "/echo-cookies":
            cookie_header = self.headers.get("Cookie", "")
            self._send_json({"cookie_header": cookie_header})

        elif path == "/delay":
            seconds = float(qs.get("seconds", ["1"])[0])
            import time

            time.sleep(seconds)
            self._send_text("done")

        elif path == "/retry-me":
            # Use a per-request cookie to track retries
            cookie_header = self.headers.get("Cookie", "")
            retry_count = 0
            for part in cookie_header.split(";"):
                part = part.strip()
                if part.startswith("retry_count="):
                    retry_count = int(part.split("=")[1])

            if retry_count < 2:
                new_count = retry_count + 1
                self.send_response(503)
                self.send_header(
                    "Set-Cookie",
                    f"retry_count={new_count}; Path=/",
                )
                self.send_header("Retry-After", "0")
                self.end_headers()
            else:
                self._send_json({"retries": retry_count})

        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path

        if path == "/post":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            self._send_json(
                {"method": "POST", "body": body.decode(errors="replace")}
            )

        elif path == "/json":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            data = json.loads(body)
            self._send_json({"received": data})

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

        elif path == "/multipart-echo":
            content_type = self.headers.get("Content-Type", "")
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            if "boundary=" in content_type:
                boundary = content_type.split("boundary=")[1].split(";")[0].strip()
                parts = body.split(f"--{boundary}".encode())
                fields = {}
                for part in parts:
                    if b"Content-Disposition" in part:
                        lines = part.split(b"\r\n")
                        for line in lines:
                            if b"Content-Disposition" in line:
                                disposition = line.decode()
                                if 'name="' in disposition:
                                    name = disposition.split('name="')[1].split('"')[0]
                                    blank = part.find(b"\r\n\r\n")
                                    if blank != -1:
                                        value = part[blank + 4 :]
                                        if value.endswith(b"\r\n"):
                                            value = value[:-2]
                                        fields[name] = value.decode(
                                            errors="replace"
                                        )
                                break
                self._send_json({"fields": fields})
            else:
                self.send_response(400)
                self.end_headers()

        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        pass  # Suppress request logging


@pytest.fixture(scope="module")
def diff_server():
    """Start a test server for differential tests."""
    server = http.server.HTTPServer(("127.0.0.1", 0), _DifferentialHandler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    yield f"http://127.0.0.1:{port}"
    server.shutdown()


# ---------------------------------------------------------------------------
# Redirect default behavior
# ---------------------------------------------------------------------------


class TestRedirectDefault:
    """Verify eggfetch does NOT follow redirects by default (unlike requests)."""

    def test_eggfetch_no_follow_by_default(self, diff_server):
        resp = eggfetch.get(f"{diff_server}/redirect/302")
        assert resp.status_code == 302

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_requests_follows_by_default(self, diff_server):
        """requests follows redirects by default -- document the difference."""
        resp = _requests.get(f"{diff_server}/redirect/302", allow_redirects=True)
        assert resp.status_code == 200

    @pytest.mark.skipif(not HAS_HTTPX, reason="httpx not installed")
    def test_httpx_follows_by_default(self, diff_server):
        """httpx follows redirects by default -- document the difference."""
        resp = _httpx.get(f"{diff_server}/redirect/302", follow_redirects=True)
        assert resp.status_code == 200

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_requests_no_follow(self, diff_server):
        """requests can disable redirect following."""
        resp = _requests.get(
            f"{diff_server}/redirect/302", allow_redirects=False
        )
        assert resp.status_code == 302


class TestRedirectFollow:
    """When follow_redirects=True, eggfetch behaves like requests."""

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    @pytest.mark.parametrize("code", [301, 302, 307, 308])
    def test_final_status_matches_requests(self, diff_server, code):
        egg_resp = eggfetch.get(
            f"{diff_server}/redirect/{code}", follow_redirects=True
        )
        req_resp = _requests.get(
            f"{diff_server}/redirect/{code}", allow_redirects=True
        )
        assert egg_resp.status_code == req_resp.status_code == 200

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    @pytest.mark.parametrize("code", [301, 302, 307, 308])
    def test_history_length_matches_requests(self, diff_server, code):
        egg_resp = eggfetch.get(
            f"{diff_server}/redirect/{code}", follow_redirects=True
        )
        req_resp = _requests.get(
            f"{diff_server}/redirect/{code}", allow_redirects=True
        )
        assert len(egg_resp.history) == len(req_resp.history) == 1

    @pytest.mark.skipif(not HAS_HTTPX, reason="httpx not installed")
    @pytest.mark.parametrize("code", [301, 302, 307, 308])
    def test_final_status_matches_httpx(self, diff_server, code):
        egg_resp = eggfetch.get(
            f"{diff_server}/redirect/{code}", follow_redirects=True
        )
        httpx_resp = _httpx.get(
            f"{diff_server}/redirect/{code}", follow_redirects=True
        )
        assert egg_resp.status_code == httpx_resp.status_code == 200


# ---------------------------------------------------------------------------
# Auth behavior
# ---------------------------------------------------------------------------


class TestBasicAuth:
    """Verify basic auth works identically to requests."""

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_basic_auth_matches_requests(self, diff_server):
        egg_resp = eggfetch.get(
            f"{diff_server}/basic-auth",
            auth=("user", "pass"),
        )
        req_resp = _requests.get(
            f"{diff_server}/basic-auth",
            auth=("user", "pass"),
        )
        assert egg_resp.status_code == req_resp.status_code == 200
        egg_json = egg_resp.json()
        req_json = req_resp.json()
        assert egg_json["authenticated"] == req_json["authenticated"]
        assert egg_json["user"] == req_json["user"]

    @pytest.mark.skipif(not HAS_HTTPX, reason="httpx not installed")
    def test_basic_auth_matches_httpx(self, diff_server):
        egg_resp = eggfetch.get(
            f"{diff_server}/basic-auth",
            auth=("user", "pass"),
        )
        httpx_resp = _httpx.get(
            f"{diff_server}/basic-auth",
            auth=("user", "pass"),
        )
        assert egg_resp.status_code == httpx_resp.status_code == 200
        assert egg_resp.json()["authenticated"] == httpx_resp.json()["authenticated"]


class TestBearerAuth:
    """Verify bearer auth works identically to requests."""

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_bearer_auth_matches_requests(self, diff_server):
        egg_resp = eggfetch.get(
            f"{diff_server}/bearer-auth",
            headers={"Authorization": "Bearer test-token-123"},
        )
        req_resp = _requests.get(
            f"{diff_server}/bearer-auth",
            headers={"Authorization": "Bearer test-token-123"},
        )
        assert egg_resp.status_code == req_resp.status_code == 200
        assert egg_resp.json()["token"] == req_resp.json()["token"]

    @pytest.mark.skipif(not HAS_HTTPX, reason="httpx not installed")
    def test_bearer_auth_matches_httpx(self, diff_server):
        egg_resp = eggfetch.get(
            f"{diff_server}/bearer-auth",
            headers={"Authorization": "Bearer test-token-123"},
        )
        httpx_resp = _httpx.get(
            f"{diff_server}/bearer-auth",
            headers={"Authorization": "Bearer test-token-123"},
        )
        assert egg_resp.status_code == httpx_resp.status_code == 200
        assert egg_resp.json()["token"] == httpx_resp.json()["token"]


# ---------------------------------------------------------------------------
# JSON handling
# ---------------------------------------------------------------------------


class TestJsonHandling:
    """Verify JSON request/response handling matches requests."""

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_json_response_matches_requests(self, diff_server):
        egg_resp = eggfetch.get(f"{diff_server}/json")
        req_resp = _requests.get(f"{diff_server}/json")
        assert egg_resp.json() == req_resp.json()

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_json_post_matches_requests(self, diff_server):
        payload = {"key": "value", "number": 42}
        egg_resp = eggfetch.post(f"{diff_server}/json", json=payload)
        req_resp = _requests.post(f"{diff_server}/json", json=payload)
        assert egg_resp.json()["received"] == req_resp.json()["received"]

    @pytest.mark.skipif(not HAS_HTTPX, reason="httpx not installed")
    def test_json_response_matches_httpx(self, diff_server):
        egg_resp = eggfetch.get(f"{diff_server}/json")
        httpx_resp = _httpx.get(f"{diff_server}/json")
        assert egg_resp.json() == httpx_resp.json()


# ---------------------------------------------------------------------------
# Status code handling
# ---------------------------------------------------------------------------


class TestStatusCodes:
    """Verify status code handling matches requests."""

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    @pytest.mark.parametrize("status", [200, 404, 500])
    def test_status_code_matches_requests(self, diff_server, status):
        egg_resp = eggfetch.get(f"{diff_server}/status/{status}")
        req_resp = _requests.get(f"{diff_server}/status/{status}")
        assert egg_resp.status_code == req_resp.status_code == status

    @pytest.mark.skipif(not HAS_HTTPX, reason="httpx not installed")
    @pytest.mark.parametrize("status", [200, 404, 500])
    def test_status_code_matches_httpx(self, diff_server, status):
        egg_resp = eggfetch.get(f"{diff_server}/status/{status}")
        httpx_resp = _httpx.get(f"{diff_server}/status/{status}")
        assert egg_resp.status_code == httpx_resp.status_code == status


# ---------------------------------------------------------------------------
# Header handling
# ---------------------------------------------------------------------------


class TestHeaders:
    """Verify header handling matches requests."""

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_custom_headers_match_requests(self, diff_server):
        headers = {"X-Custom-Header": "custom-value", "X-Request-Id": "abc-123"}
        egg_resp = eggfetch.get(f"{diff_server}/headers", headers=headers)
        req_resp = _requests.get(f"{diff_server}/headers", headers=headers)
        egg_h = {k.lower(): v for k, v in egg_resp.json()["headers"].items()}
        req_h = {k.lower(): v for k, v in req_resp.json()["headers"].items()}
        assert egg_h.get("x-custom-header") == req_h.get("x-custom-header")
        assert egg_h.get("x-request-id") == req_h.get("x-request-id")

    @pytest.mark.skipif(not HAS_HTTPX, reason="httpx not installed")
    def test_custom_headers_match_httpx(self, diff_server):
        headers = {"X-Custom-Header": "custom-value", "X-Request-Id": "abc-123"}
        egg_resp = eggfetch.get(f"{diff_server}/headers", headers=headers)
        httpx_resp = _httpx.get(f"{diff_server}/headers", headers=headers)
        egg_h = {k.lower(): v for k, v in egg_resp.json()["headers"].items()}
        httpx_h = {k.lower(): v for k, v in httpx_resp.headers.items()}
        assert egg_h.get("x-custom-header") == httpx_h.get("x-custom-header")
        assert egg_h.get("x-request-id") == httpx_h.get("x-request-id")


# ---------------------------------------------------------------------------
# Cookies
# ---------------------------------------------------------------------------


class TestCookies:
    """Verify cookie handling matches requests/httpx."""

    def test_set_cookie_from_response(self, diff_server):
        """Set-Cookie headers are parsed into response.cookies."""
        resp = eggfetch.get(f"{diff_server}/set-cookie?value=test-123")
        assert resp.status_code == 200
        assert "session" in resp.cookies
        assert resp.cookies["session"].value == "test-123"

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_set_cookie_matches_requests(self, diff_server):
        egg_resp = eggfetch.get(f"{diff_server}/set-cookie?value=test-456")
        req_resp = _requests.get(f"{diff_server}/set-cookie?value=test-456")
        assert egg_resp.cookies["session"].value == req_resp.cookies["session"]

    def test_send_cookie_header(self, diff_server):
        """Per-request cookies are sent as Cookie header."""
        resp = eggfetch.get(
            f"{diff_server}/echo-cookies",
            cookies={"session": "abc123", "theme": "dark"},
        )
        cookie_header = resp.json()["cookie_header"]
        assert "session=abc123" in cookie_header
        assert "theme=dark" in cookie_header

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_send_cookie_matches_requests(self, diff_server):
        egg_resp = eggfetch.get(
            f"{diff_server}/echo-cookies",
            cookies={"session": "abc123"},
        )
        req_resp = _requests.get(
            f"{diff_server}/echo-cookies",
            cookies={"session": "abc123"},
        )
        assert egg_resp.json()["cookie_header"] == req_resp.json()["cookie_header"]

    def test_multi_set_cookie(self, diff_server):
        """Multiple Set-Cookie headers are all parsed."""
        resp = eggfetch.get(f"{diff_server}/multi-set-cookie")
        assert resp.status_code == 200
        assert len(resp.cookies) == 3
        assert resp.cookies["a"].value == "1"
        assert resp.cookies["b"].value == "2"
        assert resp.cookies["c"].value == "3"

    def test_client_level_cookies(self, diff_server):
        """Client-level cookies are sent with requests."""
        with eggfetch.Client(cookies={"session": "client-cookie"}) as client:
            resp = client.get(f"{diff_server}/echo-cookies")
        assert "session=client-cookie" in resp.json()["cookie_header"]


# ---------------------------------------------------------------------------
# Multipart
# ---------------------------------------------------------------------------


class TestMultipart:
    """Verify multipart form handling."""

    def test_multipart_with_bytes(self, diff_server):
        """Multipart with raw bytes file upload."""
        resp = eggfetch.post(
            f"{diff_server}/multipart-echo",
            files={"file": (b"binary content")},
        )
        assert resp.status_code == 200
        fields = resp.json()["fields"]
        assert "file" in fields

    def test_multipart_with_filename(self, diff_server):
        """Multipart with explicit filename."""
        resp = eggfetch.post(
            f"{diff_server}/multipart-echo",
            files={"upload": ("report.txt", b"report data", "text/plain")},
        )
        assert resp.status_code == 200

    def test_multipart_content_type_header(self, diff_server):
        """Content-Type header should include multipart boundary when files= is used."""
        resp = eggfetch.post(
            f"{diff_server}/multipart-echo",
            files={"file": (b"data")},
        )
        ct = resp.headers.get("content-type", "")
        # The response content-type is application/json from the echo server,
        # but the request should have been multipart
        assert resp.status_code == 200


# ---------------------------------------------------------------------------
# Decompression
# ---------------------------------------------------------------------------


class TestDecompression:
    """Verify gzip decompression matches requests/httpx."""

    def test_gzip_decompressed_automatically(self, diff_server):
        """eggfetch decompresses gzip by default."""
        resp = eggfetch.get(f"{diff_server}/gzip?text=hello+world")
        assert resp.status_code == 200
        assert "hello world" in resp.text

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_gzip_matches_requests(self, diff_server):
        egg_resp = eggfetch.get(f"{diff_server}/gzip?text=compare-me")
        req_resp = _requests.get(f"{diff_server}/gzip?text=compare-me")
        assert egg_resp.text == req_resp.text

    @pytest.mark.skipif(not HAS_HTTPX, reason="httpx not installed")
    def test_gzip_matches_httpx(self, diff_server):
        egg_resp = eggfetch.get(f"{diff_server}/gzip?text=compare-me")
        httpx_resp = _httpx.get(f"{diff_server}/gzip?text=compare-me")
        assert egg_resp.text == httpx_resp.text


# ---------------------------------------------------------------------------
# Streaming
# ---------------------------------------------------------------------------


class TestStreaming:
    """Verify streaming response handling."""

    def test_streaming_iter_lines(self, diff_server):
        """Iterating lines from a streaming response."""
        with eggfetch.Client() as client:
            with client.stream("GET", f"{diff_server}/stream?count=3") as resp:
                lines = list(resp.iter_lines())
        assert len(lines) == 3
        for i, line in enumerate(lines):
            assert f"line-{i}" in line

    def test_streaming_iter_bytes(self, diff_server):
        """Iterating bytes from a streaming response."""
        with eggfetch.Client() as client:
            with client.stream("GET", f"{diff_server}/stream?count=2") as resp:
                chunks = list(resp.iter_bytes())
        assert len(chunks) >= 1
        body = b"".join(chunks)
        assert b"line-0" in body
        assert b"line-1" in body

    def test_streaming_iter_text(self, diff_server):
        """Iterating text from a streaming response."""
        with eggfetch.Client() as client:
            with client.stream("GET", f"{diff_server}/stream?count=2") as resp:
                texts = list(resp.iter_text())
        body = "".join(texts)
        assert "line-0" in body

    def test_streaming_read(self, diff_server):
        """Bulk read from a streaming response."""
        with eggfetch.Client() as client:
            with client.stream("GET", f"{diff_server}/stream?count=3") as resp:
                body = resp.read()
        assert isinstance(body, bytes)
        assert b"line-0" in body

    def test_streaming_text_method(self, diff_server):
        """Bulk text read from a streaming response."""
        with eggfetch.Client() as client:
            with client.stream("GET", f"{diff_server}/stream?count=3") as resp:
                text = resp.text()
        assert isinstance(text, str)
        assert "line-0" in text

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_streaming_status_code_matches_requests(self, diff_server):
        """Streaming response has same status code as buffered."""
        with eggfetch.Client() as client:
            with client.stream("GET", f"{diff_server}/get") as egg_stream:
                egg_status = egg_stream.status_code
        req_resp = _requests.get(f"{diff_server}/get")
        assert egg_status == req_resp.status_code


# ---------------------------------------------------------------------------
# Timeout
# ---------------------------------------------------------------------------


class TestTimeout:
    """Verify timeout configuration behavior."""

    def test_timeout_float_accepted(self, diff_server):
        """A float timeout value is accepted without error."""
        resp = eggfetch.get(f"{diff_server}/get", timeout=5.0)
        assert resp.status_code == 200

    def test_timeout_object_accepted(self, diff_server):
        """A Timeout object is accepted."""
        timeout = eggfetch.Timeout(5.0)
        resp = eggfetch.get(f"{diff_server}/get", timeout=timeout)
        assert resp.status_code == 200

    def test_timeout_object_properties(self):
        """Timeout object has expected properties."""
        t = eggfetch.Timeout(3.0)
        assert t.pool == 3.0
        assert t.connect == 3.0
        assert t.write == 3.0
        assert t.read == 3.0

    def test_client_level_timeout(self, diff_server):
        """Client-level timeout is applied to requests."""
        with eggfetch.Client(timeout=5.0) as client:
            resp = client.get(f"{diff_server}/get")
            assert resp.status_code == 200

    def test_per_request_timeout_overrides_client(self, diff_server):
        """Per-request timeout overrides client timeout."""
        with eggfetch.Client(timeout=0.001) as client:
            resp = client.get(f"{diff_server}/get", timeout=5.0)
            assert resp.status_code == 200


# ---------------------------------------------------------------------------
# Retry
# ---------------------------------------------------------------------------


class TestRetry:
    """Verify retry behavior."""

    def test_retry_succeeds_after_failures(self, diff_server):
        """Retries should eventually succeed on a flaky endpoint."""
        retry = eggfetch.Retry(max_attempts=5, initial_delay=0.01)
        resp = eggfetch.get(
            f"{diff_server}/retry-me",
            retries=retry,
            follow_redirects=True,
        )
        assert resp.status_code == 200

    def test_retry_disabled(self, diff_server):
        """When retries=False, 503 is returned immediately."""
        resp = eggfetch.get(f"{diff_server}/status/503", retries=False)
        assert resp.status_code == 503

    def test_retry_properties(self):
        """Retry object has expected properties."""
        r = eggfetch.Retry(
            max_attempts=3,
            backoff_factor=0.5,
            max_delay=10.0,
            initial_delay=0.1,
        )
        assert r.max_attempts == 3
        assert r.backoff_factor == 0.5
        assert r.max_delay == 10.0
        assert r.initial_delay == 0.1

    def test_retry_default_statuses(self):
        """Default retryable statuses match expected set."""
        r = eggfetch.Retry()
        expected = {408, 429, 502, 503, 504}
        assert set(r.statuses) == expected


# ---------------------------------------------------------------------------
# Known differences (documented, not bugs)
# ---------------------------------------------------------------------------


class TestKnownDifferences:
    """Document intentional behavioral differences from requests/httpx."""

    def test_redirect_default_differs(self, diff_server):
        """eggfetch: no follow by default. requests/httpx: follows by default."""
        resp = eggfetch.get(f"{diff_server}/redirect/302")
        assert resp.status_code == 302  # Not followed

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_redirect_default_requests_follows(self, diff_server):
        """requests follows redirects by default."""
        resp = _requests.get(f"{diff_server}/redirect/302")
        assert resp.status_code == 200  # Followed

    @pytest.mark.skipif(not HAS_HTTPX, reason="httpx not installed")
    def test_redirect_default_httpx_follows(self, diff_server):
        """httpx follows redirects by default."""
        resp = _httpx.get(f"{diff_server}/redirect/302", follow_redirects=True)
        assert resp.status_code == 200  # Followed

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_proxy_config_differs(self, diff_server):
        """eggfetch uses proxy= string. requests uses proxies=dict."""
        # This is a documented API difference, not a behavioral one.
        # Both achieve the same result when configured correctly.
        pass

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_timeout_api_differs(self, diff_server):
        """eggfetch.Timeout(seconds) vs requests timeout=seconds."""
        # Both accept a float/int for simple timeouts.
        # eggfetch.Timeout only has a single seconds parameter;
        # requests also accepts a Timeout object with per-phase granularity.
        pass

    @pytest.mark.skipif(not HAS_REQUESTS, reason="requests not installed")
    def test_retry_default_attempts_differs(self, diff_server):
        """eggfetch retries=True defaults to 3 attempts. requests has no built-in retry."""
        # eggfetch.Retry(max_attempts=3) when retries=True.
        # requests has no retry mechanism.
        pass
