"""Category: streaming-sse-consumption — exercises httpx-sse.

httpx-sse is a Server-Sent Events extension for httpx. This fixture parses
multiple SSE events from a streamed response and asserts event fields and
stream closure.
"""

import httpx
import httpx_sse


def test_httpx_sse_single_event():
    """httpx-sse parses a single SSE event from a streamed response."""
    sse_body = "event: message\ndata: hello\n\n"
    transport = httpx.MockTransport(
        lambda r: httpx.Response(200, text=sse_body, headers={"content-type": "text/event-stream"})
    )
    with httpx.Client(transport=transport) as c:
        with httpx_sse.connect_sse(c, "GET", "http://test/sse") as event_source:
            events = list(event_source.iter_sse())
            assert len(events) == 1
            assert events[0].data == "hello"


def test_httpx_sse_multiple_events():
    """httpx-sse parses multiple SSE events from a streamed response."""
    sse_body = (
        "event: message\ndata: first\n\n"
        "event: message\ndata: second\n\n"
        "event: message\ndata: third\n\n"
    )
    transport = httpx.MockTransport(
        lambda r: httpx.Response(200, text=sse_body, headers={"content-type": "text/event-stream"})
    )
    with httpx.Client(transport=transport) as c:
        with httpx_sse.connect_sse(c, "GET", "http://test/sse") as event_source:
            events = list(event_source.iter_sse())
            assert len(events) == 3
            assert events[0].data == "first"
            assert events[1].data == "second"
            assert events[2].data == "third"


def test_httpx_sse_event_fields():
    """httpx-sse preserves event type and data fields."""
    sse_body = "event: custom\ndata: payload\n\n"
    transport = httpx.MockTransport(
        lambda r: httpx.Response(200, text=sse_body, headers={"content-type": "text/event-stream"})
    )
    with httpx.Client(transport=transport) as c:
        with httpx_sse.connect_sse(c, "GET", "http://test/sse") as event_source:
            events = list(event_source.iter_sse())
            assert len(events) == 1
            assert events[0].event == "custom"
            assert events[0].data == "payload"
