"""Category: event-hooks-instrumentation — exercises opentelemetry-instrumentation-httpx.

opentelemetry-instrumentation-httpx instruments httpx clients to emit
tracing spans. This fixture instruments a client, executes a request,
asserts span callback execution, then uninstruments cleanly.
"""

import threading
import time
import http.server
import socketserver

import httpx
from opentelemetry import trace
from opentelemetry.instrumentation.httpx import HTTPXClientInstrumentor
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import SimpleSpanProcessor, ConsoleSpanExporter


class _Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = b'{"ok": true}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        pass


class _QuietServer(socketserver.ThreadingMixIn, socketserver.TCPServer):
    allow_reuse_address = True
    daemon_threads = True


def _start_server():
    server = _QuietServer(("127.0.0.1", 0), _Handler)
    host, port = server.server_address
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    time.sleep(0.1)
    return server, thread, f"http://{host}:{port}"


def _setup_tracer_provider():
    """Set up a tracer provider with an in-memory span exporter."""
    provider = TracerProvider()
    trace.set_tracer_provider(provider)
    return provider


def test_opentelemetry_instrumentation_spans():
    """opentelemetry-instrumentation-httpx emits spans for client requests."""
    provider = _setup_tracer_provider()
    span_names = []

    class _CapturingProcessor(SimpleSpanProcessor):
        def on_end(self, span):
            span_names.append(span.name)
            super().on_end(span)

    provider.add_span_processor(_CapturingProcessor(ConsoleSpanExporter()))

    server, thread, url = _start_server()
    try:
        HTTPXClientInstrumentor().instrument()
        try:
            with httpx.Client() as c:
                resp = c.get(f"{url}/instrumented")
                assert resp.status_code == 200
                assert resp.json()["ok"] is True
        finally:
            HTTPXClientInstrumentor().uninstrument()
    finally:
        server.shutdown()
        thread.join(timeout=5)

    assert len(span_names) > 0, "Expected at least one span to be emitted"


def test_opentelemetry_uninstrument():
    """opentelemetry-instrumentation-httpx uninstruments cleanly."""
    _setup_tracer_provider()
    HTTPXClientInstrumentor().instrument()
    HTTPXClientInstrumentor().uninstrument()
    # After uninstrument, requests should still work without spans
    server, thread, url = _start_server()
    try:
        with httpx.Client() as c:
            resp = c.get(f"{url}/uninstrumented")
            assert resp.status_code == 200
    finally:
        server.shutdown()
        thread.join(timeout=5)


def test_opentelemetry_span_attributes():
    """opentelemetry-instrumentation-httpx captures HTTP attributes on spans."""
    provider = TracerProvider()
    captured_attrs = {}

    class _AttrProcessor(SimpleSpanProcessor):
        def on_end(self, span):
            captured_attrs.update(span.attributes)
            super().on_end(span)

    provider.add_span_processor(_AttrProcessor(ConsoleSpanExporter()))

    server, thread, url = _start_server()
    try:
        HTTPXClientInstrumentor().instrument(tracer_provider=provider)
        try:
            with httpx.Client() as c:
                resp = c.get(f"{url}/attrs")
                assert resp.status_code == 200
        finally:
            HTTPXClientInstrumentor().uninstrument()
    finally:
        server.shutdown()
        thread.join(timeout=5)

    # The span should have HTTP-related attributes
    assert any("http" in str(k).lower() for k in captured_attrs.keys())
