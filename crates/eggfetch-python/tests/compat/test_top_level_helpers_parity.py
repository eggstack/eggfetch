"""Track 1: Top-level convenience function parity with HTTPX 0.28.1.

Tests that the top-level helper signatures match HTTPX 0.28.1, that
client-only options configure the temporary client rather than reaching
the request method, and that stream() is a real context manager.
"""

import asyncio
import http.server
import inspect
import json
import socketserver
import threading

import pytest

from eggfetch.compat.httpx import (
    Client,
    get,
    post,
    put,
    patch,
    delete,
    head,
    options,
    request,
    stream,
    Timeout,
)


# ---------------------------------------------------------------------------
# Local test server
# ---------------------------------------------------------------------------

class _TopLevelHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        from urllib.parse import urlparse
        path = urlparse(self.path).path
        if path == "/get":
            body = json.dumps({"method": "GET"}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path == "/echo-headers":
            headers = {k: v for k, v in self.headers.items()}
            body = json.dumps({"headers": headers}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path == "/stream":
            body = b"line1\nline2\nline3\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        from urllib.parse import urlparse
        path = urlparse(self.path).path
        if path == "/echo":
            cl = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(cl)
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path == "/json":
            cl = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(cl)
            data = json.loads(body)
            resp_body = json.dumps({"received": data}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(resp_body)))
            self.end_headers()
            self.wfile.write(resp_body)
        else:
            self.send_response(404)
            self.end_headers()

    def do_PUT(self):
        from urllib.parse import urlparse
        path = urlparse(self.path).path
        if path == "/put":
            cl = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(cl)
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.end_headers()

    def do_PATCH(self):
        from urllib.parse import urlparse
        path = urlparse(self.path).path
        if path == "/patch":
            cl = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(cl)
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.end_headers()

    def do_DELETE(self):
        from urllib.parse import urlparse
        path = urlparse(self.path).path
        if path == "/delete":
            body = b"deleted"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.end_headers()

    def do_HEAD(self):
        from urllib.parse import urlparse
        path = urlparse(self.path).path
        if path == "/head":
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", "0")
            self.end_headers()
        else:
            self.send_response(404)
            self.end_headers()

    def do_OPTIONS(self):
        from urllib.parse import urlparse
        path = urlparse(self.path).path
        if path == "/options":
            self.send_response(200)
            self.send_header("Allow", "GET, POST, OPTIONS")
            self.end_headers()
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        pass


class _ThreadedHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    allow_reuse_address = True


@pytest.fixture
def server():
    srv = _ThreadedHTTPServer(("127.0.0.1", 0), _TopLevelHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# Signature parity tests
# ---------------------------------------------------------------------------

class TestTopLevelSignatures:
    """Verify top-level function signatures match HTTPX 0.28.1."""

    def test_request_signature(self):
        sig = inspect.signature(request)
        params = list(sig.parameters.keys())
        assert params[0] == "method"
        assert params[1] == "url"
        for name in ["params", "content", "data", "files", "json",
                      "headers", "cookies", "auth", "proxy", "timeout",
                      "follow_redirects", "verify", "trust_env"]:
            assert name in params, f"Missing parameter: {name}"
        # All after method/url must be keyword-only
        for name in params[2:]:
            param = sig.parameters[name]
            assert param.kind == inspect.Parameter.KEYWORD_ONLY, (
                f"Parameter {name} should be KEYWORD_ONLY, got {param.kind}"
            )

    def test_get_signature(self):
        sig = inspect.signature(get)
        params = list(sig.parameters.keys())
        assert params[0] == "url"
        for name in ["params", "headers", "cookies", "auth", "proxy",
                      "follow_redirects", "verify", "timeout", "trust_env"]:
            assert name in params, f"Missing parameter: {name}"
        # No content/data/files/json on GET
        for name in ["content", "data", "files", "json"]:
            assert name not in params, f"GET should not have parameter: {name}"

    def test_post_signature(self):
        sig = inspect.signature(post)
        params = list(sig.parameters.keys())
        assert params[0] == "url"
        for name in ["content", "data", "files", "json", "params",
                      "headers", "cookies", "auth", "proxy",
                      "follow_redirects", "verify", "timeout", "trust_env"]:
            assert name in params, f"Missing parameter: {name}"

    def test_put_signature(self):
        sig = inspect.signature(put)
        params = list(sig.parameters.keys())
        assert params[0] == "url"
        for name in ["content", "data", "files", "json", "params",
                      "headers", "cookies", "auth", "proxy",
                      "follow_redirects", "verify", "timeout", "trust_env"]:
            assert name in params, f"Missing parameter: {name}"

    def test_patch_signature(self):
        sig = inspect.signature(patch)
        params = list(sig.parameters.keys())
        assert params[0] == "url"
        for name in ["content", "data", "files", "json", "params",
                      "headers", "cookies", "auth", "proxy",
                      "follow_redirects", "verify", "timeout", "trust_env"]:
            assert name in params, f"Missing parameter: {name}"

    def test_delete_signature(self):
        sig = inspect.signature(delete)
        params = list(sig.parameters.keys())
        assert params[0] == "url"
        for name in ["params", "headers", "cookies", "auth", "proxy",
                      "follow_redirects", "verify", "timeout", "trust_env"]:
            assert name in params, f"Missing parameter: {name}"
        # No content/data/files/json on DELETE
        for name in ["content", "data", "files", "json"]:
            assert name not in params, f"DELETE should not have parameter: {name}"

    def test_head_signature(self):
        sig = inspect.signature(head)
        params = list(sig.parameters.keys())
        assert params[0] == "url"
        for name in ["params", "headers", "cookies", "auth", "proxy",
                      "follow_redirects", "verify", "timeout", "trust_env"]:
            assert name in params, f"Missing parameter: {name}"

    def test_options_signature(self):
        sig = inspect.signature(options)
        params = list(sig.parameters.keys())
        assert params[0] == "url"
        for name in ["params", "headers", "cookies", "auth", "proxy",
                      "follow_redirects", "verify", "timeout", "trust_env"]:
            assert name in params, f"Missing parameter: {name}"

    def test_stream_signature(self):
        sig = inspect.signature(stream)
        params = list(sig.parameters.keys())
        assert params[0] == "method"
        assert params[1] == "url"
        for name in ["params", "content", "data", "files", "json",
                      "headers", "cookies", "auth", "proxy", "timeout",
                      "follow_redirects", "verify", "trust_env"]:
            assert name in params, f"Missing parameter: {name}"


# ---------------------------------------------------------------------------
# Client-only options configure the temporary client
# ---------------------------------------------------------------------------

class TestTopLevelClientOptions:
    """Verify proxy, verify, timeout, trust_env configure the temp client."""

    def test_get_with_timeout(self, server):
        resp = get(f"{server}/get", timeout=10.0)
        assert resp.status_code == 200

    def test_get_with_verify(self, server):
        resp = get(f"{server}/get", verify=True)
        assert resp.status_code == 200

    def test_post_with_timeout(self, server):
        resp = post(f"{server}/echo", content=b"test", timeout=10.0)
        assert resp.status_code == 200

    def test_request_with_timeout(self, server):
        resp = request("GET", f"{server}/get", timeout=10.0)
        assert resp.status_code == 200


# ---------------------------------------------------------------------------
# Request parameters reach the request call unchanged
# ---------------------------------------------------------------------------

class TestTopLevelRequestParams:
    def test_get_with_params(self, server):
        resp = get(f"{server}/get", params={"q": "test"})
        assert resp.status_code == 200

    def test_post_with_json(self, server):
        resp = post(f"{server}/json", json={"key": "val"})
        assert resp.status_code == 200

    def test_post_with_content(self, server):
        resp = post(f"{server}/echo", content=b"hello")
        assert resp.content == b"hello"

    def test_put_with_content(self, server):
        resp = put(f"{server}/put", content=b"data")
        assert resp.content == b"data"

    def test_patch_with_content(self, server):
        resp = patch(f"{server}/patch", content=b"data")
        assert resp.content == b"data"

    def test_delete(self, server):
        resp = delete(f"{server}/delete")
        assert resp.status_code == 200

    def test_head(self, server):
        resp = head(f"{server}/head")
        assert resp.status_code == 200

    def test_options(self, server):
        resp = options(f"{server}/options")
        assert resp.status_code == 200


# ---------------------------------------------------------------------------
# Top-level stream() context manager behavior
# ---------------------------------------------------------------------------

class TestTopLevelStream:
    def test_stream_yields_open_response(self, server):
        with stream("GET", f"{server}/stream") as response:
            assert not response.is_closed
            body = response.read()
            assert len(body) > 0

    def test_stream_closes_response_after_exit(self, server):
        with stream("GET", f"{server}/stream") as response:
            body = response.read()
        assert response.is_closed

    def test_stream_closes_on_exception(self, server):
        try:
            with stream("GET", f"{server}/stream") as response:
                raise RuntimeError("intentional")
        except RuntimeError:
            pass
        assert response.is_closed

    def test_stream_is_context_manager(self, server):
        assert hasattr(stream, "__call__")
        cm = stream("GET", f"{server}/stream")
        assert hasattr(cm, "__enter__")
        assert hasattr(cm, "__exit__")
