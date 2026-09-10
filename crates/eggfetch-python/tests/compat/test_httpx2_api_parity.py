"""HTTPX2 2.12.0 API parity tests (H2X-API-001..005, H2X-META-001).

Covers FunctionAuth, Origin/URL.origin, QUERY helpers, Headers merge
operators, status aliases/warnings/metadata, and import isolation
(httpx 0.28.1 facade unchanged).
"""

import warnings

import pytest

import eggfetch.compat.httpx as httpx1
import eggfetch.compat.httpx2 as httpx2


def test_function_auth_export_and_flow():
    assert hasattr(httpx2, "FunctionAuth")
    assert not hasattr(httpx1, "FunctionAuth")

    def add_auth(request):
        request.headers["Authorization"] = "Bearer token123"
        return request

    auth = httpx2.FunctionAuth(add_auth)
    req = httpx2.Request("GET", "http://example.com")
    flow = auth.auth_flow(req)
    out = next(flow)
    assert out.headers["Authorization"] == "Bearer token123"
    assert repr(auth).startswith("FunctionAuth(")


def test_function_auth_sync_async_client_integration():
    seen = []

    def func(request):
        seen.append(request.url.path if hasattr(request.url, "path") else str(request.url))
        request.headers["X-Func"] = "1"
        return request

    def handler(request):
        assert request.headers.get("X-Func") == "1"
        return httpx2.Response(200, text="ok")

    client = httpx2.Client(transport=httpx2.MockTransport(handler), auth=func)
    r = client.get("http://testserver/")
    assert r.status_code == 200
    assert seen, "auth function must be called"


@pytest.mark.asyncio
async def test_function_auth_async():
    def func(request):
        request.headers["X-Func"] = "async-1"
        return request

    def handler(request):
        assert request.headers.get("X-Func") == "async-1"
        return httpx2.Response(200, text="ok")

    async with httpx2.AsyncClient(
        transport=httpx2.MockTransport(handler), auth=func
    ) as client:
        r = await client.get("http://testserver/")
        assert r.status_code == 200


def test_origin_normalization_and_hash():
    o1 = httpx2.Origin("http://example.com")
    o2 = httpx2.Origin("http://example.com:80")
    assert o1 == o2
    assert hash(o1) == hash(o2)
    assert o1.port == 80
    assert o1.scheme == "http"
    assert o1.host == "example.com"

    o3 = httpx2.Origin("https://example.com")
    assert o3.port == 443
    assert o1 != o3

    # URL.origin return type and stability
    url = httpx2.URL("https://example.com:443/path?q=1#frag")
    origin = url.origin
    assert isinstance(origin, httpx2.Origin)
    assert origin == httpx2.Origin("https://example.com")
    assert url.origin == url.origin

    # IPv6 canonical form
    a = httpx2.Origin("http://[::1]/")
    b = httpx2.Origin("http://[0:0:0:0:0:0:0:1]/")
    assert a == b

    # Relative URL has no origin
    with pytest.raises(ValueError):
        httpx2.Origin("/relative/path")

    # 0.28.1 facade unchanged (no Origin)
    assert not hasattr(httpx1, "Origin")


def test_query_method_helpers():
    assert hasattr(httpx2, "query")
    assert hasattr(httpx2.Client, "query")
    assert hasattr(httpx2.AsyncClient, "query")
    assert not hasattr(httpx1, "query")

    def handler(request):
        assert request.method == "QUERY"
        return httpx2.Response(200, text="query-ok")

    client = httpx2.Client(transport=httpx2.MockTransport(handler))
    r = client.query("http://testserver/", content=b"payload")
    assert r.status_code == 200
    assert r.text == "query-ok"


@pytest.mark.asyncio
async def test_query_async():
    def handler(request):
        assert request.method == "QUERY"
        return httpx2.Response(200, text="q")

    async with httpx2.AsyncClient(transport=httpx2.MockTransport(handler)) as c:
        r = await c.query("http://testserver/")
        assert r.status_code == 200


def test_headers_merge_operators():
    h1 = httpx2.Headers({"a": "1", "b": "2"})
    h2 = h1 | {"b": "3", "c": "4"}
    assert isinstance(h2, httpx2.Headers)
    assert h2["b"] == "3"
    assert h2["c"] == "4"
    # Original unchanged
    assert h1["b"] == "2"

    # Case-insensitive replacement
    h3 = httpx2.Headers({"X-Custom": "a"}) | {"x-custom": "b"}
    assert h3["x-custom"] == "b"
    assert len([k for k in h3.keys() if k == "x-custom"]) == 1

    # |= mutates
    h4 = httpx2.Headers({"a": "1"})
    hid = id(h4)
    h4 |= {"b": "2"}
    assert id(h4) == hid
    assert h4["b"] == "2"

    # Invalid operand raises TypeError
    with pytest.raises(TypeError):
        h1 | 123  # type: ignore[operator]

    # Redaction preserved through merge
    h5 = httpx2.Headers({"authorization": "secret"}) | {"x": "1"}
    assert "<redacted>" in repr(h5)
    assert "secret" not in repr(h5)


def test_status_aliases_and_warnings():
    # Canonical RFC 9110 names
    assert httpx2.codes.CONTENT_TOO_LARGE == 413
    assert httpx2.codes.URI_TOO_LONG == 414
    assert httpx2.codes.RANGE_NOT_SATISFIABLE == 416
    assert httpx2.codes.UNPROCESSABLE_CONTENT == 422
    # Deprecated aliases warn with HTTPXDeprecationWarning (visible by default)
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        assert httpx2.codes.REQUEST_ENTITY_TOO_LARGE == 413
        assert any(
            issubclass(x.category, httpx2.HTTPXDeprecationWarning) for x in w
        ), "deprecated alias must warn"


def test_version_metadata_and_import_isolation():
    assert httpx2.__version__ == "2.12.0"
    assert httpx1.__version__ == "0.28.1"
    # Importing httpx2 never mutates httpx1
    import sys

    assert "httpx" not in sys.modules or sys.modules["httpx"].__name__ != (
        "eggfetch.compat.httpx2"
    )
    # Both facades coexist
    assert httpx1.Client is not httpx2.Client
    # 0.28.1 API unchanged after httpx2 import
    assert not hasattr(httpx1, "FunctionAuth")
    assert not hasattr(httpx1, "query")
