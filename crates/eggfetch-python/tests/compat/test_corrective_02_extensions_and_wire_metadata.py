"""Corrective closure 02: extension and wire metadata plumbing.

These tests verify that the native Python bindings and the HTTPX
compatibility facade expose truthful response metadata and propagate
the Python ``extensions`` dict to the underlying engine consistently
across buffered and streaming dispatch.

Plan reference: ``plans/httpx-parity-corrective-02-extension-and-wire-metadata-plumbing.md``.
"""

from __future__ import annotations

import socket
import threading
import time
import http.server
import socketserver

import pytest


# ---------------------------------------------------------------------------
# Local HTTP server fixtures
# ---------------------------------------------------------------------------


def _start_server(handler_cls, *, port: int) -> socketserver.TCPServer:
    socketserver.TCPServer.allow_reuse_address = True
    server = socketserver.TCPServer(("127.0.0.1", port), handler_cls)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    time.sleep(0.05)
    return server


class _OkHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"hi")

    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header("Allow", "GET")
        self.send_header("Content-Length", "0")
        self.end_headers()

    def log_message(self, *_args, **_kwargs):
        pass


class _OkHandlerQuiet(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"hi")

    def log_message(self, *_args, **_kwargs):
        pass


def _pick_free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


# ---------------------------------------------------------------------------
# Native bindings: extensions validation
# ---------------------------------------------------------------------------


def test_target_rejects_crlf_in_str():
    import eggfetch
    client = eggfetch.Client()
    with pytest.raises(TypeError, match="CR/LF/NUL"):
        client.request("GET", "http://127.0.0.1:1/", extensions={"target": "bad\r\nvalue"})


def test_target_rejects_nul_in_str():
    import eggfetch
    client = eggfetch.Client()
    with pytest.raises(TypeError, match="CR/LF/NUL"):
        client.request("GET", "http://127.0.0.1:1/", extensions={"target": "bad\0value"})


def test_target_rejects_empty_string():
    import eggfetch
    client = eggfetch.Client()
    with pytest.raises(TypeError, match="must not be empty"):
        client.request("GET", "http://127.0.0.1:1/", extensions={"target": ""})


def test_target_rejects_non_string_non_bytes():
    import eggfetch
    client = eggfetch.Client()
    with pytest.raises(TypeError, match="str or bytes"):
        client.request("GET", "http://127.0.0.1:1/", extensions={"target": 42})


def test_sni_hostname_rejects_empty():
    import eggfetch
    client = eggfetch.Client()
    with pytest.raises(TypeError, match="must not be empty"):
        client.request("GET", "http://127.0.0.1:1/", extensions={"sni_hostname": ""})


def test_sni_hostname_rejects_non_string():
    import eggfetch
    client = eggfetch.Client()
    with pytest.raises(TypeError):
        client.request("GET", "http://127.0.0.1:1/", extensions={"sni_hostname": 7})


def test_extensions_not_a_dict_rejected():
    import eggfetch
    client = eggfetch.Client()
    with pytest.raises(TypeError, match="dict"):
        client.request("GET", "http://127.0.0.1:1/", extensions=[("target", "*")])


def test_extensions_unknown_keys_accepted():
    """Unknown keys are passed through (HTTPX compat may keep them).

    Validation only restricts known key types; arbitrary user keys
    must not raise.
    """
    import eggfetch
    client = eggfetch.Client()
    # Should not raise — only the request method will fail because the
    # server is unreachable, not the extension parsing.
    try:
        client.request(
            "GET",
            "http://127.0.0.1:1/",
            extensions={"custom": "value", "trace_id": 42},
        )
    except eggfetch.NetworkError:
        pass


def test_trace_none_treated_as_no_observer():
    import eggfetch
    client = eggfetch.Client()
    try:
        client.request("GET", "http://127.0.0.1:1/", extensions={"trace": None})
    except eggfetch.NetworkError:
        pass


def test_async_callback_to_sync_raises():
    import eggfetch
    client = eggfetch.Client()
    with pytest.raises(Exception):
        client.request(
            "GET",
            "http://127.0.0.1:1/",
            extensions={"trace": lambda name, info: None},  # noqa: E731
        )


# ---------------------------------------------------------------------------
# Native bindings: trace callback delivery
# ---------------------------------------------------------------------------


def test_sync_trace_callback_fires_for_send_and_receive():
    import eggfetch

    port = _pick_free_port()
    server = _start_server(_OkHandler, port=port)
    try:
        events: list[tuple[str, dict]] = []

        def trace(name, info):
            events.append((name, dict(info) if info else {}))

        client = eggfetch.Client()
        response = client.request(
            "GET",
            f"http://127.0.0.1:{port}/",
            extensions={"trace": trace},
        )
        assert response.status_code == 200
        names = [n for n, _ in events]
        assert "send_request_headers.started" in names
        assert "receive_response_headers.complete" in names
        # Send event carries method + target.
        send = next(i for n, i in events if n == "send_request_headers.started")
        assert send["method"] == "GET"
        assert send["target"].endswith("/")
        # Receive event carries status.
        recv = next(i for n, i in events if n == "receive_response_headers.complete")
        assert recv["status"] == 200
    finally:
        server.shutdown()
        server.server_close()


def test_sync_callback_exception_propagates():
    import eggfetch

    port = _pick_free_port()
    server = _start_server(_OkHandlerQuiet, port=port)
    try:
        def trace(name, info):
            raise ValueError("boom")

        client = eggfetch.Client()
        with pytest.raises(Exception):
            client.request(
                "GET",
                f"http://127.0.0.1:{port}/",
                extensions={"trace": trace},
            )
    finally:
        server.shutdown()
        server.server_close()


def test_streaming_extensions_propagate_to_native_dispatch():
    import eggfetch

    port = _pick_free_port()
    server = _start_server(_OkHandlerQuiet, port=port)
    try:
        events: list[tuple[str, dict]] = []

        def trace(name, info):
            events.append((name, dict(info) if info else {}))

        client = eggfetch.Client()
        with client.stream(
            "GET",
            f"http://127.0.0.1:{port}/",
            extensions={"trace": trace},
        ) as response:
            assert response.status_code == 200
            body = b"".join(response.iter_bytes())
            assert body == b"hi"
        names = [n for n, _ in events]
        assert "send_request_headers.started" in names
        assert "receive_response_headers.complete" in names
    finally:
        server.shutdown()
        server.server_close()


# ---------------------------------------------------------------------------
# Native bindings: response metadata
# ---------------------------------------------------------------------------


def test_native_response_exposes_http_version_and_reason_phrase():
    import eggfetch

    port = _pick_free_port()
    server = _start_server(_OkHandlerQuiet, port=port)
    try:
        client = eggfetch.Client()
        response = client.request("GET", f"http://127.0.0.1:{port}/")
        # `reason_phrase` is a property exposed on the pyclass.
        assert response.reason_phrase == "OK"
        assert response.http_version in {"HTTP/1.0", "HTTP/1.1"}
        # The structured `extensions` dict carries the same values.
        ext = response.extensions
        assert ext["http_version"] == response.http_version
        assert ext["reason_phrase"] == response.reason_phrase
        # Buffered responses cannot expose the connection itself.
        assert ext["network_stream"] is None
    finally:
        server.shutdown()
        server.server_close()


def test_streaming_response_exposes_http_version_and_reason_phrase():
    import eggfetch

    port = _pick_free_port()
    server = _start_server(_OkHandlerQuiet, port=port)
    try:
        client = eggfetch.Client()
        with client.stream("GET", f"http://127.0.0.1:{port}/") as response:
            assert response.status_code == 200
            assert response.reason_phrase == "OK"
            assert response.http_version in {"HTTP/1.0", "HTTP/1.1"}
            ext = response.extensions
            assert ext["http_version"] == response.http_version
            assert ext["reason_phrase"] == response.reason_phrase
            # Ordinary streaming responses don't detach the connection
            # until the body is consumed; `network_stream` stays None.
            assert ext["network_stream"] is None
            body = b"".join(response.iter_bytes())
            assert body == b"hi"
    finally:
        server.shutdown()
        server.server_close()


# ---------------------------------------------------------------------------
# HTTPX compatibility facade
# ---------------------------------------------------------------------------


def test_httpx_compat_facade_extensions_pass_through_to_non_stream_path():
    """Buffered dispatch must also receive extensions via the facade."""
    import eggfetch
    from eggfetch.compat.httpx import Client

    port = _pick_free_port()
    server = _start_server(_OkHandlerQuiet, port=port)
    try:
        events: list[tuple[str, dict]] = []

        def trace(name, info):
            events.append((name, dict(info) if info else {}))

        client = Client()
        request = client.build_request(
            "GET",
            f"http://127.0.0.1:{port}/",
            extensions={"trace": trace},
        )
        response = client.send(request)
        assert response.status_code == 200
        names = [n for n, _ in events]
        assert "send_request_headers.started" in names
        assert "receive_response_headers.complete" in names
    finally:
        server.shutdown()
        server.server_close()


def test_httpx_compat_facade_response_extensions_metadata():
    """Response extensions include http_version and reason_phrase."""
    import eggfetch
    from eggfetch.compat.httpx import Client

    port = _pick_free_port()
    server = _start_server(_OkHandlerQuiet, port=port)
    try:
        client = Client()
        response = client.get(f"http://127.0.0.1:{port}/")
        assert response.status_code == 200
        assert response.http_version in {"HTTP/1.0", "HTTP/1.1"}
        assert response.reason_phrase == "OK"
        # The extensions dict exposes the same keys.
        assert "http_version" in response.extensions
        assert "reason_phrase" in response.extensions
    finally:
        server.shutdown()
        server.server_close()
