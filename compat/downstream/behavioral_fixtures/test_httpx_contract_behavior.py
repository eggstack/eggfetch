"""Category: contract-tests — exercises the HTTPX 0.28.1 public API contract.

This fixture runs a curated public-behavior subset that does not depend on
HTTPX private internals unavailable by design. It covers request construction,
response construction, headers and repeated values, query parameters and
repeated values, URL behavior, cookies, timeout configuration, sync client
request flow, async client request flow, auth flow, event hooks, mock
transport, and exception request context.
"""

import httpx


def test_contract_request_construction():
    """Request construction preserves method, url, headers, content."""
    req = httpx.Request("POST", "http://test/api", json={"key": "value"})
    assert req.method == "POST"
    assert req.url.path == "/api"
    assert req.headers.get("content-type") == "application/json"


def test_contract_response_construction():
    """Response construction preserves status, headers, body."""
    resp = httpx.Response(201, json={"created": True}, headers={"X-Custom": "yes"})
    assert resp.status_code == 201
    assert resp.json()["created"] is True
    assert resp.headers.get("x-custom") == "yes"


def test_contract_headers_repeated_values():
    """Headers preserve repeated values."""
    h = httpx.Headers([("set-cookie", "a=1"), ("set-cookie", "b=2")])
    assert h.get_list("set-cookie") == ["a=1", "b=2"]


def test_contract_query_repeated_values():
    """Query parameters preserve repeated values."""
    url = httpx.URL("http://test/api?a=1&a=2&b=3")
    assert url.params.get_list("a") == ["1", "2"]
    assert url.params.get("b") == "3"


def test_contract_url_behavior():
    """URL parsing and reconstruction works correctly."""
    url = httpx.URL("http://user:pass@example.com:8080/path?query=1#frag")
    assert url.scheme == "http"
    assert url.host == "example.com"
    assert url.port == 8080
    assert url.path == "/path"
    assert url.query == b"query=1"


def test_contract_cookies():
    """Cookies can be set and read from a response."""
    req = httpx.Request("GET", "http://test/cookies")
    resp = httpx.Response(200, headers={"set-cookie": "session=abc123"}, request=req)
    assert resp.cookies.get("session") == "abc123"


def test_contract_timeout_config():
    """Timeout configuration is accepted and stored."""
    with httpx.Client(timeout=30.0) as c:
        assert c.timeout.read == 30.0


def test_contract_sync_client_flow():
    """Sync client request flow works end-to-end."""
    transport = httpx.MockTransport(lambda r: httpx.Response(200, json={"ok": True}))
    with httpx.Client(transport=transport) as c:
        resp = c.get("http://test/sync")
        assert resp.status_code == 200
        assert resp.json()["ok"] is True


def test_contract_async_client_flow():
    """Async client request flow works end-to-end."""
    import pytest

    transport = httpx.MockTransport(lambda r: httpx.Response(200, json={"ok": True}))

    @pytest.mark.asyncio
    async def _run():
        async with httpx.AsyncClient(transport=transport) as c:
            resp = await c.get("http://test/async")
            assert resp.status_code == 200
            assert resp.json()["ok"] is True

    import asyncio
    asyncio.run(_run())


def test_contract_auth_flow():
    """Auth flow with single yield works."""

    class TokenAuth(httpx.Auth):
        def auth_flow(self, request):
            request.headers["Authorization"] = "Bearer token"
            yield request

    transport = httpx.MockTransport(
        lambda r: httpx.Response(200, json={"auth": r.headers.get("authorization", "")})
    )
    with httpx.Client(auth=TokenAuth(), transport=transport) as c:
        resp = c.get("http://test/auth")
        assert resp.status_code == 200
        assert resp.json()["auth"] == "Bearer token"


def test_contract_event_hooks():
    """Event hooks are called during request/response."""
    hook_calls = []

    def on_request(request):
        hook_calls.append("request")

    def on_response(response):
        hook_calls.append("response")

    transport = httpx.MockTransport(lambda r: httpx.Response(200))
    with httpx.Client(
        transport=transport,
        event_hooks={"request": [on_request], "response": [on_response]},
    ) as c:
        c.get("http://test/hooks")
        assert "request" in hook_calls
        assert "response" in hook_calls


def test_contract_mock_transport():
    """MockTransport dispatches correctly."""
    transport = httpx.MockTransport(lambda r: httpx.Response(200, json={"mock": True}))
    with httpx.Client(transport=transport) as c:
        resp = c.get("http://test/mock")
        assert resp.status_code == 200
        assert resp.json()["mock"] is True


def test_contract_exception_request_context():
    """Exceptions carry the original request context."""

    def handler(r):
        raise httpx.ConnectError("refused", request=r)

    transport = httpx.MockTransport(handler)
    with httpx.Client(transport=transport) as c:
        try:
            c.get("http://test/error")
            assert False, "Expected ConnectError"
        except httpx.ConnectError as exc:
            assert exc.request is not None
            assert exc.request.url.path == "/error"
