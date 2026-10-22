"""HTTPX2 origin reachability (milestone 004).

Every URL a user can observe through the httpx2 facade must be an httpx2
``URL``, so the httpx2 ``origin`` delta is reachable from requests,
responses, clients and redirects — not only from a directly constructed
``httpx2.URL``.  The 0.28.1 facade must stay untouched: it has no
``origin`` and no ``Origin``.
"""

import json
import subprocess
import sys

import pytest

import eggfetch.compat.httpx as httpx1
import eggfetch.compat.httpx2 as httpx2

from .fixtures import create_server


@pytest.fixture(scope="module")
def server():
    """Local fixture server (same pattern as the other compat modules)."""
    srv, url = create_server()
    yield url
    srv.shutdown()


# ── Observable httpx2 paths carry httpx2 URLs ─────────────────────────


def test_request_url_is_httpx2_url_with_origin():
    request = httpx2.Request("GET", "https://example.com/x")
    assert isinstance(request.url, httpx2.URL)
    assert isinstance(request.url, httpx1.URL)
    assert request.url.origin == httpx2.Origin("https://example.com")
    assert request.url.origin.scheme == "https"
    assert request.url.origin.host == "example.com"
    assert request.url.origin.port == 443


def test_request_url_setter_keeps_httpx2_url():
    request = httpx2.Request("GET", "https://example.com/x")
    request.url = "https://other.example/y"
    assert isinstance(request.url, httpx2.URL)
    assert request.url.origin == httpx2.Origin("https://other.example")


def test_response_url_is_httpx2_url_with_origin():
    request = httpx2.Request("GET", "https://example.com/x")
    response = httpx2.Response(200, request=request)
    assert isinstance(response.url, httpx2.URL)
    assert response.url.origin == httpx2.Origin("https://example.com")


def test_response_accepts_base_facade_request_and_upgrades_url():
    base_request = httpx1.Request("GET", "https://example.com/x")
    response = httpx2.Response(200, request=base_request)
    assert isinstance(response.url, httpx2.URL)
    assert response.url.origin == httpx2.Origin("https://example.com")
    # The foreign request is upgraded too, so its URL carries ``origin``.
    assert isinstance(response.request, httpx2.Request)
    assert isinstance(response.request.url, httpx2.URL)
    assert response.request.url.origin == httpx2.Origin("https://example.com")


def test_sync_client_response_exposes_origin(server):
    with httpx2.Client() as client:
        response = client.get(f"{server}/get")
    assert isinstance(response, httpx2.Response)
    assert isinstance(response.url, httpx2.URL)
    assert response.url.origin == httpx2.Origin(server)
    assert response.request.url.origin == httpx2.Origin(server)


@pytest.mark.asyncio
async def test_async_client_response_exposes_origin(server):
    async with httpx2.AsyncClient() as client:
        response = await client.get(f"{server}/get")
    assert isinstance(response, httpx2.Response)
    assert isinstance(response.url, httpx2.URL)
    assert response.url.origin == httpx2.Origin(server)
    assert response.request.url.origin == httpx2.Origin(server)


def test_top_level_helper_response_exposes_origin(server):
    response = httpx2.get(f"{server}/get")
    assert isinstance(response, httpx2.Response)
    assert response.url.origin == httpx2.Origin(server)


def test_redirect_response_exposes_final_origin(server):
    with httpx2.Client(follow_redirects=True) as client:
        response = client.get(f"{server}/redirect/302")
    assert response.status_code == 200
    assert str(response.url) == f"{server}/get"
    assert isinstance(response.url, httpx2.URL)
    assert response.url.origin == httpx2.Origin(server)
    # The redirect hop itself is an httpx2 response with an httpx2 URL.
    assert response.history
    for hop in response.history:
        assert isinstance(hop, httpx2.Response)
        assert isinstance(hop.url, httpx2.URL)
        assert hop.url.origin == httpx2.Origin(server)
    assert isinstance(response.request, httpx2.Request)
    assert response.request.url.origin == httpx2.Origin(server)


@pytest.mark.asyncio
async def test_async_redirect_response_exposes_final_origin(server):
    async with httpx2.AsyncClient(follow_redirects=True) as client:
        response = await client.get(f"{server}/redirect/302")
    assert str(response.url) == f"{server}/get"
    assert isinstance(response.url, httpx2.URL)
    assert response.url.origin == httpx2.Origin(server)


def test_manual_redirect_next_request_exposes_origin(server):
    with httpx2.Client() as client:
        response = client.get(f"{server}/redirect/302")
    assert response.status_code == 302
    assert isinstance(response.next_request, httpx2.Request)
    assert isinstance(response.next_request.url, httpx2.URL)
    assert response.next_request.url.origin == httpx2.Origin(server)


def test_client_base_url_merge_keeps_httpx2_url(server):
    with httpx2.Client(base_url=server) as client:
        request = client.build_request("GET", "/get")
        assert isinstance(request, httpx2.Request)
        assert isinstance(request.url, httpx2.URL)
        assert request.url.origin == httpx2.Origin(server)


# ── Origin normalization is retained ───────────────────────────────────


def test_url_origin_normalization_retained():
    # Default port for the scheme.
    assert httpx2.URL("https://example.com/x").origin == httpx2.Origin(
        "https://example.com"
    )
    assert httpx2.URL("http://example.com/x").origin == httpx2.Origin(
        "http://example.com"
    )
    # Lowercased host.
    assert httpx2.URL("https://Example.COM/x").origin.host == "example.com"
    # Explicit port is kept.
    assert httpx2.URL("https://example.com:8443/x").origin.port == 8443
    # Bracketed IPv6 host is unwrapped and re-normalized.
    ipv6 = httpx2.URL("http://[::1]/x").origin
    assert ipv6.host == "::1"
    assert ipv6 == httpx2.Origin("http://[0:0:0:0:0:0:0:1]/")
    # IDNA-decoded punycode host.
    assert httpx2.URL("https://xn--bcher-kva.example/x").origin.host == (
        "bücher.example"
    )
    # Relative URLs have no origin.
    with pytest.raises(ValueError):
        httpx2.Origin("/relative/path")


def test_origin_accepts_base_and_httpx2_url():
    base_url = httpx1.URL("https://example.com/x")
    httpx2_url = httpx2.URL("https://example.com/x")
    assert httpx2.Origin(base_url) == httpx2.Origin(httpx2_url)
    assert httpx2.Origin(base_url).host == "example.com"
    assert httpx2.Origin(httpx2_url).host == "example.com"


# ── httpx2 URL stays a drop-in replacement for the base URL ───────────


def test_httpx2_url_is_drop_in_for_base_url():
    url = httpx2.URL("https://example.com/path?a=1")
    assert isinstance(url, httpx1.URL)
    # Every base capability still works.
    assert url.scheme == "https"
    assert url.host == "example.com"
    assert url.path == "/path"
    assert url.params["a"] == "1"
    assert str(url) == "https://example.com/path?a=1"
    # Copies and joins stay httpx2 URLs (they carry ``origin``).
    for derived in (
        url.copy_with(params={"b": "2"}),
        url.copy_set_param("b", "2"),
        url.copy_remove_param("a"),
        url.copy_merge_params({"b": "2"}),
        url.copy_add_param("b", "2"),
        url.join("/other"),
    ):
        assert isinstance(derived, httpx2.URL)
        assert derived.origin == httpx2.Origin("https://example.com")


def test_httpx2_url_constructed_from_a_base_url_stays_an_httpx2_url():
    """A cross-facade URL argument must not leak a 0.28.1 URL back.

    The 0.28.1 constructor returns its argument unchanged when the argument
    is already a ``URL``.  Inherited unchanged, ``httpx2.URL(httpx.URL(...))``
    would hand back a 0.28.1 URL, which has no ``origin``.
    """
    base = httpx1.URL("https://example.com/path?a=1")
    upgraded = httpx2.URL(base)
    assert isinstance(upgraded, httpx2.URL)
    assert upgraded.origin == httpx2.Origin("https://example.com")
    # The adopted URL keeps the parsed parts of the 0.28.1 URL verbatim.
    assert str(upgraded) == str(base)
    assert upgraded._parts == base._parts
    # An httpx2 URL keeps its identity when passed back in.
    own = httpx2.URL("https://example.com/path?a=1")
    assert httpx2.URL(own) is own
    # The 0.28.1 facade keeps its own short-circuit untouched.
    assert httpx1.URL(base) is base
    assert not hasattr(httpx1.URL, "origin")


# ── Identity change is intentional, subclassing is preserved ──────────


def test_httpx2_request_response_are_subclasses_with_distinct_identity():
    assert issubclass(httpx2.Request, httpx1.Request)
    assert issubclass(httpx2.Response, httpx1.Response)
    # Intended user-visible change: identity with the 0.28.1 classes is gone.
    assert httpx2.Request is not httpx1.Request
    assert httpx2.Response is not httpx1.Response
    # isinstance relationships keep working.
    request = httpx2.Request("GET", "https://example.com/x")
    response = httpx2.Response(200, request=request)
    assert isinstance(request, httpx1.Request)
    assert isinstance(response, httpx1.Response)


# ── 0.28.1 isolation ──────────────────────────────────────────────────


def test_zero_281_facade_has_no_origin_delta():
    assert not hasattr(httpx1.URL, "origin")
    assert not hasattr(httpx1.URL("https://example.com"), "origin")
    assert not hasattr(httpx1, "Origin")
    assert "Origin" not in httpx1.__all__
    # 0.28.1 request/response behavior is unchanged.
    request = httpx1.Request("GET", "https://example.com/x")
    assert not isinstance(request.url, httpx2.URL)
    assert type(request.url) is httpx1.URL
    response = httpx1.Response(200, request=request)
    assert type(response.url) is httpx1.URL
    assert not hasattr(response.url, "origin")


def test_importing_httpx2_leaves_zero_281_facade_unmodified():
    """A clean interpreter proves the httpx2 import mutates nothing."""
    code = (
        "import json\n"
        "import eggfetch.compat.httpx as h1\n"
        "before = (list(h1.__all__), sorted(dir(h1)))\n"
        "import eggfetch.compat.httpx2 as h2\n"
        "after = (list(h1.__all__), sorted(dir(h1)))\n"
        "print(json.dumps({'before': before, 'after': after,\n"
        "                  'origin': hasattr(h1, 'Origin'),\n"
        "                  'sys_httpx': __import__('sys').modules.get('httpx') is not None}))\n"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True,
        text=True,
        check=True,
    )
    payload = json.loads(result.stdout.strip().splitlines()[-1])
    assert payload["before"] == payload["after"]
    assert payload["origin"] is False
    # The httpx2 facade never imports the real PyPI httpx.
    assert payload["sys_httpx"] is False
