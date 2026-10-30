"""Downstream checks for the optional real-HTTPX transport bridge."""

from __future__ import annotations

import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import httpx
import pytest

from eggfetch.compat.httpx import ReadTimeout as EggReadTimeout
from eggfetch.integrations.httpx import EggFetchTransport


class _Handler(BaseHTTPRequestHandler):
    response_delay = 0.0

    def do_POST(self) -> None:  # noqa: N802 - stdlib protocol callback
        content_length = int(self.headers.get("Content-Length", "0"))
        self.rfile.read(content_length)
        if self.response_delay:
            time.sleep(self.response_delay)
        body = b'{"ok":true}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args: object) -> None:
        pass


@pytest.fixture
def server():
    httpd = ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{httpd.server_port}/completion"
    finally:
        httpd.shutdown()
        thread.join(timeout=2)
        httpd.server_close()


def test_real_httpx_client_uses_eggfetch_and_returns_streamed_body(server: str) -> None:
    transport = EggFetchTransport(trust_env=False)
    client = httpx.Client(transport=transport, timeout=1.0)
    try:
        response = client.post(server, content=b"prompt")
        assert isinstance(client, httpx.Client)
        assert response.json() == {"ok": True}
        assert response.request.content == b"prompt"
    finally:
        client.close()


def test_delayed_response_headers_obey_httpx_read_timeout(server: str) -> None:
    _Handler.response_delay = 0.3
    try:
        with httpx.Client(
            transport=EggFetchTransport(trust_env=False),
            timeout=httpx.Timeout(connect=1.0, read=0.05, write=1.0, pool=1.0),
        ) as client:
            with pytest.raises(httpx.ReadTimeout):
                client.post(server, content=b"prompt")
    finally:
        _Handler.response_delay = 0.0


def test_eggfetch_timeout_maps_to_httpx_with_original_request(monkeypatch) -> None:
    transport = EggFetchTransport(trust_env=False)

    def fail_request(*args, **kwargs):
        raise EggReadTimeout("response headers timed out")

    monkeypatch.setattr(transport._transport, "handle_request", fail_request)
    request = httpx.Request("GET", "http://example.test/")
    with pytest.raises(httpx.ReadTimeout) as caught:
        transport.handle_request(request)
    assert caught.value.request is request

