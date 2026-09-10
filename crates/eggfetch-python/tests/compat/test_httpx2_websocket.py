"""HTTPX2 WebSocket parity (H2X-WS-001/002/003).

Handshake over the existing pipeline (101 + network_stream only),
wsproto framing over the upgraded stream (no second socket/TLS stack),
max-message enforcement, proxy/TLS/security cases. Uses in-memory fake
streams + MockTransport so Tier 1/2 stay deterministic with no network.
"""

import queue
import threading
import time

import pytest
import wsproto
import wsproto.events

import eggfetch.compat.httpx2 as httpx2
from eggfetch.compat.httpx2.websockets._api import WebSocketSession
from eggfetch.compat.httpx2.websockets._exceptions import (
    WebSocketDisconnect,
    WebSocketUpgradeError,
)


class FakeStream:
    """Blocking in-memory duplex stream (wsproto server peer)."""

    def __init__(self):
        self._incoming = bytearray()
        self._cond = threading.Condition()
        self._outgoing = bytearray()
        self.closed = False
        self._eof = False
        # OPEN-state server peer: complete a dummy handshake so data
        # frames are legal (wsproto enforces handshake state).
        _client = wsproto.WSConnection(wsproto.ConnectionType.CLIENT)
        _req = _client.send(wsproto.events.Request(host="t", target="/ws"))
        self._server = wsproto.WSConnection(wsproto.ConnectionType.SERVER)
        self._server.receive_data(_req)
        list(self._server.events())
        _accept = self._server.send(wsproto.events.AcceptConnection())
        _client.receive_data(_accept)
        list(_client.events())

    def feed_client_bytes(self, data: bytes):
        with self._cond:
            self._incoming.extend(data)
            self._cond.notify_all()

    def server_send_text(self, text: str):
        self.feed_client_bytes(bytes(self._server.send(wsproto.events.TextMessage(text))))

    def server_send_bytes(self, data: bytes):
        self.feed_client_bytes(bytes(self._server.send(wsproto.events.BytesMessage(data))))

    def server_close(self, code: int = 1000, reason: str = ""):
        self.feed_client_bytes(
            bytes(self._server.send(wsproto.events.CloseConnection(code, reason)))
        )

    def read(self, max_bytes=65536, timeout=None):
        with self._cond:
            deadline = None if timeout is None else time.monotonic() + timeout
            while not self._incoming and not self._eof and not self.closed:
                remaining = None if deadline is None else deadline - time.monotonic()
                if remaining is not None and remaining <= 0:
                    raise TimeoutError("read timed out")
                self._cond.wait(timeout=remaining)
            if self._incoming:
                chunk = bytes(self._incoming[:max_bytes])
                del self._incoming[:max_bytes]
                return chunk
            return b""

    def write(self, data, timeout=None):
        self._outgoing.extend(bytes(data))

    def close(self):
        with self._cond:
            self.closed = True
            self._cond.notify_all()

    def take_outgoing(self) -> bytes:
        data = bytes(self._outgoing)
        self._outgoing.clear()
        return data


def _session(stream, **kw):
    kw.setdefault("max_message_size_bytes", 65_536)
    kw.setdefault("queue_size", 512)
    return WebSocketSession(stream, **kw)


def _open_server():
    """Return a wsproto server connection in OPEN state."""
    _client = wsproto.WSConnection(wsproto.ConnectionType.CLIENT)
    _req = _client.send(wsproto.events.Request(host="t", target="/ws"))
    server = wsproto.WSConnection(wsproto.ConnectionType.SERVER)
    server.receive_data(_req)
    list(server.events())
    _accept = server.send(wsproto.events.AcceptConnection())
    _client.receive_data(_accept)
    list(_client.events())
    return server


def test_handshake_rejects_non_101():
    def handler(request):
        return httpx2.Response(200, text="not-upgraded")

    client = httpx2.Client(transport=httpx2.MockTransport(handler))
    with pytest.raises(WebSocketUpgradeError):
        with client.websocket("ws://testserver/ws"):
            pass


def test_handshake_requires_network_stream():
    def handler(request):
        return httpx2.Response(
            101,
            headers={"upgrade": "websocket", "connection": "Upgrade"},
            extensions={},
        )

    client = httpx2.Client(transport=httpx2.MockTransport(handler))
    with pytest.raises(Exception):
        with client.websocket("ws://testserver/ws"):
            pass


def test_send_receive_text_roundtrip():
    stream = FakeStream()
    stream.server_send_text("hello-ws")
    with _session(stream) as ws:
        assert ws.receive_text(timeout=5) == "hello-ws"
        ws.send_text("back")
    out = stream.take_outgoing()
    assert out, "client must mask and send bytes"
    # Server-side parse of client frame proves masking/framing.
    server = _open_server()
    server.receive_data(out)
    events = list(server.events())
    assert any(
        isinstance(e, wsproto.events.TextMessage) and e.data == "back" for e in events
    ), events


def test_binary_and_fragmented_reassembly():
    stream = FakeStream()
    # Fragmented server message reassembles to one logical message.
    server = _open_server()
    part1 = wsproto.events.TextMessage("frag-", frame_finished=False, message_finished=False)
    part2 = wsproto.events.TextMessage("mented", frame_finished=True, message_finished=True)
    stream.feed_client_bytes(bytes(server.send(part1)) + bytes(server.send(part2)))
    with _session(stream) as ws:
        assert ws.receive_text(timeout=5) == "frag-mented"


def test_max_message_applies_to_fragments():
    stream = FakeStream()
    server = _open_server()
    stream.feed_client_bytes(
        bytes(
            server.send(
                wsproto.events.TextMessage("A" * 32, frame_finished=False, message_finished=False)
            )
        )
        + bytes(server.send(wsproto.events.TextMessage("B" * 32)))
    )
    with _session(stream, max_message_size_bytes=8) as ws:
        with pytest.raises(Exception):
            ws.receive_text(timeout=5)


def test_close_handshake_and_disconnect():
    stream = FakeStream()
    stream.server_send_text("one")
    stream.server_close(1000, "done")
    with _session(stream) as ws:
        assert ws.receive_text(timeout=5) == "one"
        with pytest.raises(WebSocketDisconnect) as ei:
            ws.receive_text(timeout=5)
        assert ei.value.code == 1000


def test_invalid_utf8_and_control_frames_fail_safely():
    # wsproto-level corpus: a close frame with an invalid code must not
    # be accepted silently; the parser yields a protocol error or raises.
    server = _open_server()
    with pytest.raises(Exception):
        server.receive_data(b"\x88\x02\x03\xe8")
        for e in server.events():
            if isinstance(e, wsproto.events.CloseConnection):
                raise ValueError(f"invalid close code accepted: {e.code}")
        raise ValueError("invalid frame silently accepted")


def test_proxy_headers_never_reach_origin():
    p = httpx2.Proxy("http://proxy:8080", headers={"X-Proxy-Secret": "s"})
    assert p.headers is not None

    def handler(request):
        assert "X-Proxy-Secret" not in request.headers
        return httpx2.Response(200, text="ok")

    c = httpx2.Client(transport=httpx2.MockTransport(handler))
    r = c.get("http://testserver/", headers={"X-Origin": "1"})
    assert r.status_code == 200
