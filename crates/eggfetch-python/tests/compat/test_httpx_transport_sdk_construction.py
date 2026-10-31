"""Construction checks for downstream SDKs that require real HTTPX clients.

Run with AInews's pinned SDK versions installed. Each test is independently
optional so EggFetch's ordinary compat environment does not acquire SDK deps.
"""

from __future__ import annotations

import httpx
import pytest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread

from eggfetch.integrations.httpx import EggFetchTransport


def _transport() -> EggFetchTransport:
    return EggFetchTransport(trust_env=False)


@pytest.fixture
def openai_stub_url():
    class Handler(BaseHTTPRequestHandler):
        def do_POST(self) -> None:  # noqa: N802 - stdlib protocol callback
            length = int(self.headers.get("Content-Length", "0"))
            self.rfile.read(length)
            body = (
                b'{"id":"chatcmpl-test","object":"chat.completion",'
                b'"created":1,"model":"stub-model","choices":[{"index":0,'
                b'"message":{"role":"assistant","content":"ok"},'
                b'"finish_reason":"stop"}],"usage":{"prompt_tokens":1,'
                b'"completion_tokens":1,"total_tokens":2}}'
            )
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, format: str, *args: object) -> None:
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}/v1"
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_openai_accepts_real_httpx_client_with_eggfetch_transport() -> None:
    openai = pytest.importorskip("openai")
    transport = _transport()
    client = openai.OpenAI(
        api_key="test-key",
        http_client=httpx.Client(transport=transport),
        max_retries=0,
    )
    try:
        assert isinstance(client._client, httpx.Client)
        assert client._client._transport is transport
    finally:
        client.close()


def test_openai_chat_completion_uses_eggfetch_transport(openai_stub_url: str) -> None:
    openai = pytest.importorskip("openai")
    transport = _transport()
    client = openai.OpenAI(
        api_key="test-key",
        base_url=openai_stub_url,
        http_client=httpx.Client(transport=transport),
        max_retries=0,
    )
    try:
        result = client.chat.completions.create(
            model="stub-model",
            messages=[{"role": "user", "content": "hello"}],
        )
        assert result.choices[0].message.content == "ok"
        assert result.usage.total_tokens == 2
    finally:
        client.close()


def test_anthropic_accepts_real_httpx_client_with_eggfetch_transport() -> None:
    anthropic = pytest.importorskip("anthropic")
    transport = _transport()
    client = anthropic.Anthropic(
        api_key="test-key",
        http_client=httpx.Client(transport=transport),
        max_retries=0,
    )
    try:
        assert isinstance(client._client, httpx.Client)
        assert client._client._transport is transport
    finally:
        client.close()


def test_ollama_passes_transport_to_its_sdk_created_httpx_client() -> None:
    ollama = pytest.importorskip("ollama")
    transport = _transport()
    client = ollama.Client(host="http://127.0.0.1:1", transport=transport)
    try:
        assert isinstance(client._client, httpx.Client)
        assert client._client._transport is transport
    finally:
        client._client.close()


def test_google_genai_sync_client_args_accept_eggfetch_transport() -> None:
    genai = pytest.importorskip("google.genai")
    types = pytest.importorskip("google.genai.types")
    transport = _transport()
    client = genai.Client(
        api_key="test-key",
        http_options=types.HttpOptions(client_args={"transport": transport}),
    )
    try:
        sdk_httpx = client._api_client._httpx_client
        assert isinstance(sdk_httpx, httpx.Client)
        assert sdk_httpx._transport is transport
    finally:
        client.close()
