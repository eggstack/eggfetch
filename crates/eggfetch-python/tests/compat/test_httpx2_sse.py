"""HTTPX2 SSE parity (H2X-SSE-001/002).

Byte-by-byte/chunk-split differential vs reference framing, oversized
never-terminated bound, close/cancel releases resources, sync/async
equivalence. No second network stack: parser over streamed responses.
"""

import pytest

import eggfetch.compat.httpx2 as httpx2
from eggfetch.compat.httpx2._sse import (
    _SSEEventDecoder,
    _SSELineDecoder,
    _SSEParser,
)


def _sse_response(chunks, content_type="text/event-stream"):
    def handler(request):
        return httpx2.Response(
            200, headers={"content-type": content_type}, stream=iter(chunks)
        )

    client = httpx2.Client(transport=httpx2.MockTransport(handler))
    return client


def test_sse_basic_fields_and_multiline():
    payload = b"event: greeting\ndata: line1\ndata: line2\nid: 42\n\n"
    client = _sse_response([payload])
    with client.stream("GET", "http://testserver/events") as r:
        src = httpx2.EventSource(r)
        events = list(src)
    assert len(events) == 1
    e = events[0]
    assert e.event == "greeting"
    assert e.data == "line1\nline2"
    assert e.id == "42"


def test_sse_chunk_split_boundaries():
    # CR, LF, CRLF split across arbitrary chunk boundaries.
    full = b"data: a\r\ndata: b\revent: x\ndata: c\n\n"
    for split in (1, 2, 3, 5, 7):
        chunks = [full[i : i + split] for i in range(0, len(full), split)]
        client = _sse_response(chunks)
        with client.stream("GET", "http://testserver/e") as r:
            events = list(httpx2.EventSource(r))
        assert len(events) == 1, f"split={split}"
        assert events[0].data == "a\nb\nc"
        assert events[0].event == "x"


def test_sse_comments_and_empty_ignored():
    payload = b": comment\ndata: hi\n\n\n: another\ndata: yo\n\n"
    client = _sse_response([payload])
    with client.stream("GET", "http://testserver/e") as r:
        events = list(httpx2.EventSource(r))
    assert [e.data for e in events] == ["hi", "yo"]


def test_sse_wrong_content_type_rejected():
    client = _sse_response([b"data: x\n\n"], content_type="text/plain")
    with client.stream("GET", "http://testserver/e") as r:
        with pytest.raises(httpx2.SSEError):
            list(httpx2.EventSource(r))


def test_sse_max_event_size_bounded():
    payload = b"data: " + b"A" * 100 + b"\n\n"
    client = _sse_response([payload])
    with client.stream("GET", "http://testserver/e") as r:
        with pytest.raises(httpx2.SSEError):
            list(httpx2.EventSource(r, max_event_size=10))


def test_sse_never_terminated_event_bounded():
    # A never-terminated event accumulating beyond max must raise on
    # subsequent decode, not buffer unboundedly.
    parser = _SSEParser(max_event_size=16)
    with pytest.raises(httpx2.SSEError):
        for _ in range(10):
            list(parser.decode("data: " + "A" * 8 + "\n"))


def test_sse_retry_and_json():
    payload = b'retry: 3000\ndata: {"a": 1}\n\n'
    client = _sse_response([payload])
    with client.stream("GET", "http://testserver/e") as r:
        events = list(httpx2.EventSource(r))
    assert events[0].retry == 3000
    assert events[0].json() == {"a": 1}


@pytest.mark.asyncio
async def test_sse_async_equivalence():
    payload = b"data: hello\n\n"

    async def handler(request):
        return httpx2.Response(
            200, headers={"content-type": "text/event-stream"}, content=payload
        )

    async with httpx2.AsyncClient(transport=httpx2.MockTransport(handler)) as c:
        async with c.stream("GET", "http://testserver/e") as r:
            events = [e async for e in httpx2.EventSource(r)]
    assert len(events) == 1
    assert events[0].data == "hello"


def test_sse_close_releases_response():
    payload = b"data: x\n\n"
    client = _sse_response([payload])
    with client.stream("GET", "http://testserver/e") as r:
        src = httpx2.EventSource(r)
        events = list(src)
        assert len(events) == 1
        # After iteration the response is closed (lease released).
        assert r.is_closed
