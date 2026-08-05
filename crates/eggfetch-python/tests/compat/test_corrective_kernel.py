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


def test_raw_iteration_marks_consumed_and_counts_source_bytes_before_adaptation():
    response = Response(200, stream=iter([b"abcd"]))
    iterator = response.iter_raw(chunk_size=1)
    assert next(iterator) == b"a"
    assert response.is_stream_consumed
    assert response.num_bytes_downloaded == 4


def test_buffered_raw_iteration_is_not_a_repeatable_decoded_read():
    response = Response(200, content=b"body")
    with pytest.raises(StreamConsumed):
        list(response.iter_raw())
    assert response.content == b"body"


@pytest.mark.asyncio
async def test_async_raw_iteration_rejects_sync_stream_modality():
    response = Response(200, stream=iter([b"body"]))
    with pytest.raises(RuntimeError, match="async iterator on an sync stream"):
        await anext(response.aiter_raw())
    assert not response.is_stream_consumed


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


# ── Redirect cookie security regressions (final closure 01) ───────────


@pytest.mark.parametrize("redirect_status", [302, 301, 303])
def test_cross_origin_redirect_does_not_carry_explicit_cookie(redirect_status):
    """Explicit Cookie header must not leak across origins on redirect."""
    seen = []

    def handler(request):
        seen.append((str(request.url), request.headers.get("cookie")))
        if "/redirect" in str(request.url):
            return Response(redirect_status, headers={"Location": "http://other-server/target"})
        return Response(200)

    with Client(transport=MockTransport(handler), follow_redirects=True) as client:
        client.get("http://testserver/redirect", headers={"Cookie": "secret=value"})

    # First hop has the explicit cookie
    assert seen[0][1] == "secret=value"
    # Second hop must NOT have the cookie
    assert seen[1][1] is None


@pytest.mark.parametrize("redirect_status", [302, 301, 303])
def test_same_origin_redirect_does_not_carry_explicit_cookie(redirect_status):
    """Explicit Cookie header is not carried on same-origin redirects either."""
    seen = []

    def handler(request):
        seen.append((str(request.url), request.headers.get("cookie")))
        if "/redirect" in str(request.url):
            return Response(redirect_status, headers={"Location": "/target"})
        return Response(200)

    with Client(transport=MockTransport(handler), follow_redirects=True) as client:
        client.get("http://testserver/redirect", headers={"Cookie": "explicit=val"})

    assert seen[0][1] == "explicit=val"
    assert seen[1][1] is None


def test_intermediate_set_cookie_visible_on_next_hop():
    """Cookie set by a redirect response must be available on the next hop."""
    seen = []

    def handler(request):
        seen.append((str(request.url), request.headers.get("cookie")))
        if "/redirect" in str(request.url):
            return Response(
                302,
                headers=[
                    ("Location", "/target"),
                    ("Set-Cookie", "hop_cookie=yes; Path=/"),
                ],
            )
        return Response(200)

    with Client(transport=MockTransport(handler), follow_redirects=True) as client:
        client.get("http://testserver/redirect")

    assert seen[1][1] is not None and "hop_cookie=yes" in seen[1][1]


# ── Body replay regressions (final closure 01) ─────────────────────────


def test_multipart_retained_body_not_lost_on_307():
    """Multipart data+files must survive a 307 redirect."""
    seen = []

    def handler(request):
        seen.append({
            "files": request._files,
            "multipart_data": request._multipart_data,
        })
        if "/redirect" in str(request.url):
            return Response(307, headers={"Location": "/target"})
        return Response(200)

    with Client(transport=MockTransport(handler), follow_redirects=True) as client:
        client.post(
            "http://testserver/redirect",
            files={"file": ("test.txt", b"content", "text/plain")},
            data={"field": "value"},
        )

    assert seen[0]["files"] is not None
    assert seen[1]["files"] is not None
    assert seen[0]["multipart_data"] == seen[1]["multipart_data"]


def test_multipart_retained_body_not_lost_on_308():
    """Multipart data+files must survive a 308 redirect."""
    seen = []

    def handler(request):
        seen.append({
            "files": request._files,
            "multipart_data": request._multipart_data,
        })
        if "/redirect" in str(request.url):
            return Response(308, headers={"Location": "/target"})
        return Response(200)

    with Client(transport=MockTransport(handler), follow_redirects=True) as client:
        client.post(
            "http://testserver/redirect",
            files={"file": ("test.txt", b"content", "text/plain")},
            data={"field": "value"},
        )

    assert seen[0]["files"] is not None
    assert seen[1]["files"] is not None
    assert seen[0]["multipart_data"] == seen[1]["multipart_data"]


def test_unreplayable_body_fails_before_second_dispatch():
    """A generator body through 307 must fail before a second dispatch."""
    dispatch_count = 0

    def handler(request):
        nonlocal dispatch_count
        dispatch_count += 1
        if dispatch_count == 1:
            return Response(307, headers={"Location": "/target"})
        return Response(200)

    with Client(transport=MockTransport(handler), follow_redirects=True) as client:
        with pytest.raises(StreamConsumed):
            client.post(
                "http://testserver/redirect",
                content=iter([b"chunk1", b"chunk2"]),
            )

    assert dispatch_count == 1


def test_method_rewrite_to_get_drops_body():
    """303 redirect from POST to GET drops the body and related headers."""
    seen = []

    def handler(request):
        seen.append((
            request.method,
            request.headers.get("content-length", "none"),
            request.headers.get("transfer-encoding", "none"),
        ))
        if "/redirect" in str(request.url):
            return Response(303, headers={"Location": "/target"})
        return Response(200)

    with Client(transport=MockTransport(handler), follow_redirects=True) as client:
        client.post("http://testserver/redirect", content=b"body")

    # First hop: POST with body
    assert seen[0][0] == "POST"
    assert seen[0][1] == "4"
    # Second hop: GET without body headers
    assert seen[1][0] == "GET"
    assert seen[1][1] == "none"
    assert seen[1][2] == "none"


# ── Raw stream lifecycle regressions (final closure 02) ────────────────


def test_raw_iteration_marks_consumed_and_closes():
    resp = Response(200, stream=iter([b"abc", b"def"]))
    chunks = list(resp.iter_raw())
    assert b"".join(chunks) == b"abcdef"
    assert resp.is_stream_consumed
    assert resp.is_closed


def test_raw_iteration_increments_byte_accounting():
    resp = Response(200, stream=iter([b"abc", b"def"]))
    list(resp.iter_raw())
    assert resp.num_bytes_downloaded == 6


def test_raw_partial_iteration_then_close():
    resp = Response(200, stream=iter([b"a", b"b", b"c"]))
    gen = resp.iter_raw(chunk_size=1)
    next(gen)
    gen.close()
    assert resp.is_stream_consumed
    assert not resp.is_closed
    resp.close()
    assert resp.is_closed


def test_raw_and_decoded_paths_are_distinct():
    resp = Response(200, stream=iter([b"abc"]))
    list(resp.iter_raw())
    with pytest.raises(StreamConsumed):
        list(resp.iter_bytes())
