"""Tests for HTTPX compat layer request streaming."""

import http.server
import socketserver
import threading

import pytest

from eggfetch.compat.httpx import Client, AsyncClient, Request


class _EchoHandler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        content_length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_length)
        self.send_response(200)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()

    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.end_headers()
        self.wfile.write(b"ok")
        self.wfile.flush()

    def log_message(self, format, *args):
        pass


class _ThreadedHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True


@pytest.fixture(scope="module")
def server():
    srv = _ThreadedHTTPServer(("127.0.0.1", 0), _EchoHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


class TestRequestConstruction:
    def test_request_with_content(self):
        req = Request("POST", "http://example.com", content=b"hello")
        assert req.content == b"hello"
        assert req.method == "POST"

    def test_request_with_stream(self):
        def gen():
            yield b"hello"
            yield b" "
            yield b"world"
        req = Request("POST", "http://example.com", stream=gen())
        assert req.stream is not None
        assert req.content is None

    def test_request_read_consumes_stream(self):
        def gen():
            yield b"hello"
            yield b"world"
        req = Request("POST", "http://example.com", stream=gen())
        data = req.read()
        assert data == b"helloworld"
        assert req._is_stream_consumed

    def test_request_auto_content_type_json(self):
        req = Request("POST", "http://example.com", json={"key": "value"})
        assert "application/json" in req.headers["content-type"]

    def test_request_auto_content_type_form(self):
        req = Request("POST", "http://example.com", data={"key": "value"})
        assert "application/x-www-form-urlencoded" in req.headers["content-type"]

    def test_request_host_header(self):
        req = Request("GET", "http://example.com/path")
        assert req.headers["host"] == "example.com"

    def test_request_host_header_with_port(self):
        req = Request("GET", "http://example.com:8080/path")
        assert req.headers["host"] == "example.com:8080"

    def test_no_transfer_encoding_for_explicit_stream(self):
        def gen():
            yield b"data"
        req = Request("POST", "http://example.com", stream=gen())
        assert "transfer-encoding" not in req.headers

    def test_request_content_length_for_bytes(self):
        req = Request("POST", "http://example.com", content=b"hello")
        assert req.headers["content-length"] == "5"

    def test_request_files_stored(self):
        req = Request("POST", "http://example.com", files={"file": b"data"})
        assert req._files == {"file": b"data"}


class TestClientSendBytes:
    def test_post_bytes(self, server):
        with Client() as client:
            resp = client.post(f"{server}/echo", content=b"hello")
            assert resp.status_code == 200
            assert resp.content == b"hello"

    def test_post_json(self, server):
        with Client() as client:
            resp = client.post(f"{server}/echo", json={"key": "value"})
            assert resp.status_code == 200

    def test_get(self, server):
        with Client() as client:
            resp = client.get(f"{server}/")
            assert resp.status_code == 200
            assert resp.content == b"ok"
