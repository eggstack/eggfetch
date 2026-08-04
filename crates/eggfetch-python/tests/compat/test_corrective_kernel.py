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


def test_buffered_redirect_replay_uses_one_body_source():
    seen = []
    def handler(request):
        seen.append((request.method, request.content))
        if len(seen) == 1:
            return Response(307, headers={"location": "https://example.com/next"})
        return Response(200, content=b"ok")
    with Client(transport=MockTransport(handler), follow_redirects=True) as client:
        response = client.post("https://example.com/start", content=b"body")
    assert response.status_code == 200
    assert seen == [("POST", b"body"), ("POST", b"body")]


def test_request_local_cookie_and_query_are_not_lost_or_duplicated():
    seen = []
    def handler(request):
        seen.append((str(request.url), request.headers.get("cookie")))
        return Response(200)
    with Client(transport=MockTransport(handler), cookies={"client": "one"}) as client:
        client.get("https://example.com/path", params={"a": "1"}, cookies={"request": "two"}, headers={"Cookie": "explicit=three"})
    assert seen == [("https://example.com/path?a=1", "explicit=three; client=one; request=two")]


def test_split_utf8_stream_is_decoded_incrementally():
    value = "€".encode("utf-8")
    response = Response(200, stream=iter([value[:1], value[1:]]))
    assert list(response.iter_text(chunk_size=1)) == ["€"]
    assert response.num_bytes_downloaded == len(value)


def test_timeout_none_native_conversion_remains_disabled():
    from eggfetch.compat.httpx._client import _convert_timeout, _request_timeout
    request = Request("GET", "https://example.com", extensions={"timeout": {"connect": None, "read": None, "write": None, "pool": None}})
    timeout = _request_timeout(request, 5)
    assert timeout.as_dict == {"connect": None, "read": None, "write": None, "pool": None}
    native = _convert_timeout(timeout)
    assert native.connect is None and native.read is None and native.write is None and native.pool is None
