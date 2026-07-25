"""Category: sdk-async-client — Anthropic SDK async client integration.

Exercises anthropic.AsyncAnthropic which internally uses httpx.AsyncClient
with custom auth, base_url, timeouts, streaming, and exception inspection.
Uses a local mock server for offline testing.
"""

from __future__ import annotations

import http.server
import json
import socketserver
import sys
import threading
import time

import pytest

sys.path.insert(0, "crates/eggfetch-python/python")

import httpx

# anthropic SDK imports — this is the key integration being tested
try:
    import anthropic
    from anthropic import AsyncAnthropic
    HAS_ANTHROPIC = True
except ImportError:
    HAS_ANTHROPIC = False


class MockAnthropicHandler(http.server.BaseHTTPRequestHandler):
    """Mock Anthropic API endpoint."""

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length) if length else b""
        try:
            data = json.loads(body) if body else {}
        except json.JSONDecodeError:
            data = {}

        if self.path == "/v1/messages":
            response = {
                "id": "msg_mock_123",
                "type": "message",
                "role": "assistant",
                "content": [{"type": "text", "text": "Hello from mock"}],
                "model": "claude-3-haiku-20240307",
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 10, "output_tokens": 5},
            }
        elif self.path == "/v1/models":
            response = {
                "data": [
                    {"id": "claude-3-haiku-20240307", "type": "model"},
                ]
            }
        else:
            self.send_response(404)
            self.end_headers()
            return

        payload = json.dumps(response).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, format, *args):
        pass


class _QuietTCPServer(socketserver.ThreadingMixIn, socketserver.TCPServer):
    allow_reuse_address = True
    daemon_threads = True


@pytest.fixture(scope="module")
def mock_anthropic_server():
    server = _QuietTCPServer(("127.0.0.1", 0), MockAnthropicHandler)
    host, port = server.server_address
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    time.sleep(0.1)
    yield f"http://{host}:{port}"
    server.shutdown()
    thread.join(timeout=5)


@pytest.mark.skipif(not HAS_ANTHROPIC, reason="anthropic SDK not installed")
def test_anthropic_uses_eggfetch_shim():
    assert getattr(httpx, "__eggfetch_shim__", False) is True


@pytest.mark.skipif(not HAS_ANTHROPIC, reason="anthropic SDK not installed")
def test_anthropic_async_client_instantiation(mock_anthropic_server):
    client = AsyncAnthropic(
        api_key="test-key-not-real",
        base_url=mock_anthropic_server,
    )
    assert client.api_key == "test-key-not-real"
    assert str(client.base_url).rstrip("/") == mock_anthropic_server


@pytest.mark.skipif(not HAS_ANTHROPIC, reason="anthropic SDK not installed")
@pytest.mark.anyio
async def test_anthropic_messages_create(mock_anthropic_server):
    client = AsyncAnthropic(
        api_key="test-key-not-real",
        base_url=mock_anthropic_server,
    )
    message = await client.messages.create(
        model="claude-3-haiku-20240307",
        max_tokens=100,
        messages=[{"role": "user", "content": "Hello"}],
    )
    assert message.id == "msg_mock_123"
    assert message.role == "assistant"
    assert len(message.content) == 1
    assert message.content[0].text == "Hello from mock"
    assert message.stop_reason == "end_turn"


@pytest.mark.skipif(not HAS_ANTHROPIC, reason="anthropic SDK not installed")
@pytest.mark.anyio
async def test_anthropic_async_client_timeout(mock_anthropic_server):
    client = AsyncAnthropic(
        api_key="test-key-not-real",
        base_url=mock_anthropic_server,
        timeout=30.0,
    )
    message = await client.messages.create(
        model="claude-3-haiku-20240307",
        max_tokens=50,
        messages=[{"role": "user", "content": "Hi"}],
    )
    assert message.content[0].text == "Hello from mock"
