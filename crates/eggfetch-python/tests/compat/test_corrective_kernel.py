"""Compact Tier 1 regression kernel for the HTTPX corrective closure."""

import pytest

from eggfetch.compat.httpx import Client, MockTransport, Request, Response
from eggfetch.compat.httpx._exceptions import RequestNotRead, StreamConsumed


def test_timeout_override_uses_httpx_mapping():
    seen = []

    def handler(request):
        seen.append(request.extensions["timeout"])
        return Response(200)

    with Client(transport=MockTransport(handler), timeout=5) as client:
        client.get("https://example.com", timeout=None)
        client.get("https://example.com", timeout=2)

    assert seen == [
        {"connect": None, "read": None, "write": None, "pool": None},
        {"connect": 2, "read": 2, "write": 2, "pool": 2},
    ]


@pytest.mark.parametrize("method", ["POST", "PUT", "PATCH"])
def test_empty_body_header(method):
    assert Request(method, "https://example.com").headers["content-length"] == "0"


def test_stream_request_is_unread_until_read():
    request = Request("POST", "https://example.com", stream=iter([b"body"]))
    with pytest.raises(RequestNotRead):
        _ = request.content
    assert request.read() == b"body"


def test_buffered_response_and_live_iteration_state():
    buffered = Response(200, content=b"body")
    assert buffered.is_closed and buffered.is_stream_consumed
    live = Response(200, stream=iter([b"body"]))
    assert list(live.iter_bytes()) == [b"body"]
    assert live.is_closed and live.is_stream_consumed
    with pytest.raises(StreamConsumed):
        list(live.iter_bytes())


def test_unattached_response_request_is_an_error():
    response = Response(200)
    with pytest.raises(RuntimeError):
        _ = response.request


def test_cookie_header_has_one_facade_source():
    captured = []

    def handler(request):
        captured.append(request.headers.get("cookie"))
        return Response(200)

    with Client(transport=MockTransport(handler), cookies={"session": "one"}) as client:
        client.get("https://example.com")
    assert captured == ["session=one"]


def test_transport_close_is_permanent():
    from eggfetch.compat.httpx import HTTPTransport

    transport = HTTPTransport()
    transport.close()
    with pytest.raises(RuntimeError):
        transport.handle_request(Request("GET", "https://example.com"))
