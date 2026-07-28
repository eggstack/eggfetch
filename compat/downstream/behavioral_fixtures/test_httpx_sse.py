"""Category: streaming-sse-consumption — httpx-sse integration.

Exercises httpx-sse's ServerSentEvent stream consumption using a local
HTTP server that emits proper SSE-formatted responses.
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
from httpx_sse import EventSource


class SSEHandler(http.server.BaseHTTPRequestHandler):
    """Handler that emits Server-Sent Events."""

    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.end_headers()

        events = [
            {"event": "message", "data": '{"msg": "hello"}'},
            {"event": "update", "data": '{"count": 1}'},
            {"event": "done", "data": '{"status": "complete"}'},
        ]
        for ev in events:
            line = f"event: {ev['event']}\ndata: {ev['data']}\n\n"
            self.wfile.write(line.encode())
            self.wfile.flush()
        self.wfile.write(b"event: end\ndata: {}\n\n")
        self.wfile.flush()

    def log_message(self, format, *args):
        pass


class _QuietTCPServer(socketserver.ThreadingMixIn, socketserver.TCPServer):
    allow_reuse_address = True
    daemon_threads = True


@pytest.fixture(scope="module")
def sse_server():
    server = _QuietTCPServer(("127.0.0.1", 0), SSEHandler)
    host, port = server.server_address
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    time.sleep(0.1)
    yield f"http://{host}:{port}"
    server.shutdown()
    thread.join(timeout=5)


def test_httpx_sse_uses_eggfetch_shim():
    assert getattr(httpx, "__eggfetch_shim__", False) is True


def test_httpx_sse_event_source_iteration(sse_server):
    collected_events = []
    with httpx.Client() as client:
        with client.stream("GET", f"{sse_server}/") as response:
            es = EventSource(response)
            for event in es:
                collected_events.append({
                    "event": event.event,
                    "data": event.data,
                })
    assert len(collected_events) >= 3
    assert collected_events[0]["event"] == "message"
    assert collected_events[0]["data"] == '{"msg": "hello"}'


def test_httpx_sse_event_data_parsing(sse_server):
    with httpx.Client() as client:
        with client.stream("GET", f"{sse_server}/") as response:
            es = EventSource(response)
            events = list(es)
    assert len(events) >= 3
    assert events[1]["event"] == "update"
    data = json.loads(events[1].data)
    assert data["count"] == 1


def test_httpx_sse_end_event(sse_server):
    with httpx.Client() as client:
        with client.stream("GET", f"{sse_server}/") as response:
            es = EventSource(response)
            events = list(es)
    assert events[-1]["event"] == "end"
