"""Redirect-policy conformance fixture: engine vs facade vs upstream httpx 0.28.1.

The fixture pins two independent redirect policies that deliberately differ:

* the **engine** (``eggfetch-core``'s redirect pipeline, reached through the
  native ``eggfetch`` client), and
* the **facade** (``eggfetch.compat.httpx``, which reproduces HTTPX 0.28.1).

HTTPX 0.28.1 is the facade's specification, so the facade keeps HTTPX
behaviour and the engine keeps its stricter, credential-preserving rules
(ADR-0009 rule 4).  The divergence is recorded, not "fixed".

Two observation layers are used so that neither table can drift silently:

``policy layer``
    The facade's ``_redirect_headers`` / ``_redirect_method`` helpers are
    compared against upstream ``httpx.Client._redirect_headers`` /
    ``._redirect_method`` for every case below.  This is the only layer that
    can express the canonical ``http:80 -> https:443`` upgrade, because the
    canonical ports cannot be bound by an unprivileged test process.

``wire layer``
    The same redirect chain is driven through the native client, the facade
    and upstream httpx against local servers.  The outgoing hop's method and
    header set are recorded from the wire, which is what actually reaches the
    next origin.

Both layers assert against explicit expected tables (not merely
"facade == upstream"), so a change to *either* table fails loudly.
"""

from __future__ import annotations

import http.server
import ssl
import tempfile
import threading
import urllib.parse
from contextlib import contextmanager
from typing import Iterator

import httpx
import pytest

import eggfetch
from eggfetch.compat.httpx import Client as CompatClient
from eggfetch.compat.httpx import Request as CompatRequest
from eggfetch.compat.httpx import Response as CompatResponse
from eggfetch.compat.httpx import URL as CompatURL
from eggfetch.compat.httpx._client import _redirect_headers as facade_redirect_headers
from eggfetch.compat.httpx._client import _redirect_method as facade_redirect_method

from .native_fixtures import _generate_self_signed_cert

# ---------------------------------------------------------------------------
# Expected policy tables
# ---------------------------------------------------------------------------

#: HTTPX 0.28.1's method table, used by the facade and by upstream httpx.
#: 303 -> GET (unless HEAD); 302 -> GET (unless HEAD); 301 -> GET only for
#: POST; 307/308 keep the method.
HTTPX_METHOD_TABLE: dict[int, dict[str, str]] = {
    301: {"GET": "GET", "HEAD": "HEAD", "POST": "GET", "PUT": "PUT", "DELETE": "DELETE"},
    302: {"GET": "GET", "HEAD": "HEAD", "POST": "GET", "PUT": "GET", "DELETE": "GET"},
    303: {"GET": "GET", "HEAD": "HEAD", "POST": "GET", "PUT": "GET", "DELETE": "GET"},
    307: {"GET": "GET", "HEAD": "HEAD", "POST": "POST", "PUT": "PUT", "DELETE": "DELETE"},
    308: {"GET": "GET", "HEAD": "HEAD", "POST": "POST", "PUT": "PUT", "DELETE": "DELETE"},
}

#: eggfetch-core's method table (``redirect::redirect_method``).
#: 303 -> GET (unless HEAD); 301/302 -> GET only for POST; 307/308 keep the
#: method.
ENGINE_METHOD_TABLE: dict[int, dict[str, str]] = {
    301: {"GET": "GET", "HEAD": "HEAD", "POST": "GET", "PUT": "PUT", "DELETE": "DELETE"},
    302: {"GET": "GET", "HEAD": "HEAD", "POST": "GET", "PUT": "PUT", "DELETE": "DELETE"},
    303: {"GET": "GET", "HEAD": "HEAD", "POST": "GET", "PUT": "GET", "DELETE": "GET"},
    307: {"GET": "GET", "HEAD": "HEAD", "POST": "POST", "PUT": "PUT", "DELETE": "DELETE"},
    308: {"GET": "GET", "HEAD": "HEAD", "POST": "POST", "PUT": "PUT", "DELETE": "DELETE"},
}

#: The (status, method) pairs where the engine and HTTPX disagree.
#: HTTPX rewrites every non-HEAD 302 to GET; the engine only rewrites POST.
KNOWN_METHOD_DIVERGENCES: set[tuple[int, str]] = {(302, "PUT"), (302, "DELETE")}

#: Headers whose presence on the outgoing hop is part of the policy.
TRACKED_HEADERS = (
    "authorization",
    "proxy-authorization",
    "cookie",
    "referer",
    "origin",
    "host",
)

#: The sensitive headers sent on the first hop of every recorded chain.
FIRST_HOP_HEADERS = {
    "authorization": "Bearer secret-token",
    "proxy-authorization": "Basic cHJveHktY3JlZHM=",
    "cookie": "session=abc",
    "referer": "http://referer.test/start",
    "origin": "http://origin.test",
    "host": "sentinel.invalid",
}


# ---------------------------------------------------------------------------
# Local redirect servers
# ---------------------------------------------------------------------------


class _RecordingRedirectHandler(http.server.BaseHTTPRequestHandler):
    """Records every request and answers ``/redirect/<status>/<target>``."""

    protocol_version = "HTTP/1.1"

    def _respond(self) -> None:
        self.server.recorded.append(  # type: ignore[attr-defined]
            {
                "method": self.command,
                "path": self.path,
                "headers": {name.lower(): value for name, value in self.headers.items()},
            }
        )
        parts = self.path.strip("/").split("/")
        if parts[0] == "redirect" and len(parts) >= 3:
            status = int(parts[1])
            target = urllib.parse.unquote(parts[2])
            self.send_response(status)
            self.send_header("Location", target)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        body = b"ok"
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    do_GET = _respond
    do_HEAD = _respond
    do_POST = _respond
    do_PUT = _respond
    do_DELETE = _respond
    do_PATCH = _respond

    def log_message(self, *_args: object) -> None:
        pass


class _ThreadedServer(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


@contextmanager
def _redirect_server() -> Iterator[tuple[str, int, list[dict[str, object]]]]:
    """Plain HTTP server that records requests and can issue redirects."""
    httpd = _ThreadedServer(("127.0.0.1", 0), _RecordingRedirectHandler)
    httpd.recorded = []  # type: ignore[attr-defined]
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    try:
        yield "127.0.0.1", httpd.server_address[1], httpd.recorded  # type: ignore[attr-defined]
    finally:
        httpd.shutdown()
        httpd.server_close()
        thread.join(timeout=2)


@contextmanager
def _tls_redirect_server() -> Iterator[tuple[str, int, str, list[dict[str, object]]]]:
    """TLS redirect server; yields (host, port, ca_certificate_path, records)."""
    with tempfile.TemporaryDirectory() as tmpdir:
        cert_path, key_path = _generate_self_signed_cert(tmpdir)
        server_ssl = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        server_ssl.load_cert_chain(cert_path, key_path)
        httpd = _ThreadedServer(("127.0.0.1", 0), _RecordingRedirectHandler)
        httpd.recorded = []  # type: ignore[attr-defined]
        httpd.socket = server_ssl.wrap_socket(httpd.socket, server_side=True)
        thread = threading.Thread(target=httpd.serve_forever, daemon=True)
        thread.start()
        try:
            yield (
                "127.0.0.1",
                httpd.server_address[1],
                cert_path,
                httpd.recorded,  # type: ignore[attr-defined]
            )
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)


def _redirect_url(origin: str, status: int, target: str) -> str:
    return f"{origin}/redirect/{status}/{urllib.parse.quote(target, safe='')}"


def _tracked(headers: dict[str, str]) -> dict[str, str]:
    return {name: headers[name] for name in TRACKED_HEADERS if name in headers}


def _origin(url: str) -> tuple[str, str, int]:
    """Return (scheme, host, port) with the default port made explicit."""
    parsed = httpx.URL(url)
    scheme = parsed.scheme
    host = parsed.host
    port = parsed.port
    if port is None:
        port = 443 if scheme == "https" else 80
    return scheme, host, port


# ---------------------------------------------------------------------------
# Policy layer: facade vs upstream httpx 0.28.1
# ---------------------------------------------------------------------------

#: (case id, first-hop URL, redirect target URL)
POLICY_CASES = [
    ("same-origin", "http://origin.test/start", "http://origin.test/next"),
    (
        "same-origin-explicit-port",
        "http://origin.test:8080/start",
        "http://origin.test:8080/next",
    ),
    ("cross-origin-host", "http://origin.test/start", "http://other.test/next"),
    (
        "cross-origin-port",
        "http://origin.test:8080/start",
        "http://origin.test:9090/next",
    ),
    # Canonical upgrade: same host, http:80 -> https:443.
    ("https-upgrade-canonical", "http://origin.test/start", "https://origin.test/next"),
    # Non-canonical upgrade: same host, non-default ports.
    (
        "https-upgrade-noncanonical",
        "http://origin.test:8080/start",
        "https://origin.test:8443/next",
    ),
    # Downgrade: same host, https:443 -> http:80.
    ("https-downgrade", "https://origin.test/start", "http://origin.test/next"),
]


def _facade_policy(
    method: str, status: int, origin_url: str, target_url: str, headers: dict[str, str]
) -> tuple[str, dict[str, str]]:
    request = CompatRequest(method, origin_url, headers=headers)
    response = CompatResponse(status, request=request)
    new_method = facade_redirect_method(request, response)
    new_headers = facade_redirect_headers(request, CompatURL(target_url), new_method)
    return new_method, _tracked({name.lower(): value for name, value in new_headers.items()})


def _upstream_policy(
    method: str, status: int, origin_url: str, target_url: str, headers: dict[str, str]
) -> tuple[str, dict[str, str]]:
    request = httpx.Request(method, origin_url, headers=headers)
    response = httpx.Response(status, request=request)
    with httpx.Client() as client:
        new_method = client._redirect_method(request, response)
        new_headers = client._redirect_headers(request, httpx.URL(target_url), new_method)
    return new_method, _tracked({name.lower(): value for name, value in new_headers.items()})


@pytest.mark.parametrize(("case", "origin_url", "target_url"), POLICY_CASES)
def test_facade_redirect_policy_matches_upstream_httpx(case, origin_url, target_url):
    """The facade's redirect policy must stay byte-equivalent to HTTPX 0.28.1."""
    facade = _facade_policy("GET", 302, origin_url, target_url, FIRST_HOP_HEADERS)
    upstream = _upstream_policy("GET", 302, origin_url, target_url, FIRST_HOP_HEADERS)
    assert facade == upstream, f"facade drifted from upstream httpx in case {case!r}"


@pytest.mark.parametrize(("case", "origin_url", "target_url"), POLICY_CASES)
def test_facade_redirect_policy_pins_httpx_header_rules(case, origin_url, target_url):
    """Pin the facade's per-case header rules against HTTPX 0.28.1's rules.

    ``_is_https_redirect`` is true only for a same-host ``http:80 ->
    https:443`` hop, so only that case keeps ``Authorization`` across the
    origin change.
    """
    _, headers = _facade_policy("GET", 302, origin_url, target_url, FIRST_HOP_HEADERS)

    same_origin = _origin(origin_url) == _origin(target_url)
    canonical_upgrade = (
        httpx.URL(origin_url).host == httpx.URL(target_url).host
        and httpx.URL(origin_url).scheme == "http"
        and httpx.URL(origin_url).port in (None, 80)
        and httpx.URL(target_url).scheme == "https"
        and httpx.URL(target_url).port in (None, 443)
    )

    assert ("authorization" in headers) is (same_origin or canonical_upgrade), case
    # HTTPX never strips Proxy-Authorization, Referer or Origin.
    assert "proxy-authorization" in headers, case
    assert "referer" in headers, case
    assert "origin" in headers, case
    # HTTPX always drops an explicit Cookie header on a redirect.
    assert "cookie" not in headers, case
    # Host is rewritten only when the origin changes.
    if same_origin:
        assert headers.get("host") == FIRST_HOP_HEADERS["host"], case
    else:
        assert headers.get("host") == httpx.URL(target_url).netloc.decode("ascii"), case


@pytest.mark.parametrize("status", sorted(HTTPX_METHOD_TABLE))
@pytest.mark.parametrize("method", ["GET", "HEAD", "POST", "PUT", "DELETE"])
def test_facade_redirect_method_matches_upstream_and_expected_table(status, method):
    """The facade's method table must equal upstream httpx's and the pinned table."""
    origin_url = "http://origin.test/start"
    facade_method, _ = _facade_policy(method, status, origin_url, "http://other.test/next", {})
    upstream_method, _ = _upstream_policy(method, status, origin_url, "http://other.test/next", {})
    assert facade_method == upstream_method
    assert facade_method == HTTPX_METHOD_TABLE[status][method]


# ---------------------------------------------------------------------------
# Wire layer: engine vs facade vs upstream httpx
# ---------------------------------------------------------------------------


def _run_chain(
    runtime: str,
    url: str,
    *,
    method: str = "GET",
    body: bytes | None = None,
    verify: str | None = None,
) -> None:
    headers = dict(FIRST_HOP_HEADERS)
    if runtime == "engine":
        with eggfetch.Client(follow_redirects=True, timeout=5, verify=verify) as client:
            client.request(method, url, headers=headers, content=body)
        return
    if runtime == "facade":
        with CompatClient(follow_redirects=True, timeout=5, verify=verify) as client:
            client.request(method, url, headers=headers, content=body)
        return
    with httpx.Client(follow_redirects=True, timeout=5, verify=verify) as client:
        client.request(method, url, headers=headers, content=body)


def _wire_scenario(
    origin: str,
    target: str,
    target_records: list[dict[str, object]],
    *,
    status: int = 302,
    method: str = "GET",
    body: bytes | None = None,
    verify: str | None = None,
) -> dict[str, tuple[str, dict[str, str]]]:
    """Run one redirect chain through all three runtimes and record the hop."""
    results: dict[str, tuple[str, dict[str, str]]] = {}
    url = _redirect_url(origin, status, target)
    for runtime in ("engine", "facade", "upstream"):
        target_records.clear()
        _run_chain(runtime, url, method=method, body=body, verify=verify)
        assert target_records, f"{runtime} produced no recorded hop for {url}"
        hop = target_records[-1]
        headers = hop["headers"]
        assert isinstance(headers, dict)
        results[runtime] = (str(hop["method"]), _tracked(headers))
    return results


def test_wire_same_origin_redirect_header_policy():
    """Same-origin hop: the engine keeps the Cookie header, HTTPX drops it."""
    with _redirect_server() as (host, port, records):
        origin = f"http://{host}:{port}"
        results = _wire_scenario(origin, f"{origin}/next", records)

    engine_method, engine_headers = results["engine"]
    facade_method, facade_headers = results["facade"]
    upstream_method, upstream_headers = results["upstream"]

    assert engine_method == facade_method == upstream_method == "GET"
    # The facade reproduces HTTPX 0.28.1 exactly.
    assert facade_headers == upstream_headers
    # Engine: keeps Authorization and Cookie, always drops Proxy-Authorization.
    assert engine_headers.get("authorization") == FIRST_HOP_HEADERS["authorization"]
    assert engine_headers.get("cookie") == FIRST_HOP_HEADERS["cookie"]
    assert "proxy-authorization" not in engine_headers
    # HTTPX: keeps Authorization, drops Cookie, keeps Proxy-Authorization.
    assert facade_headers.get("authorization") == FIRST_HOP_HEADERS["authorization"]
    assert "cookie" not in facade_headers
    assert facade_headers.get("proxy-authorization") == FIRST_HOP_HEADERS["proxy-authorization"]
    # Both keep the same-origin Host, Referer and Origin headers.
    for headers in (engine_headers, facade_headers):
        assert headers.get("host") == FIRST_HOP_HEADERS["host"]
        assert headers.get("referer") == FIRST_HOP_HEADERS["referer"]
        assert headers.get("origin") == FIRST_HOP_HEADERS["origin"]


def test_wire_cross_origin_redirect_header_policy():
    """Cross-origin hop: the engine drops Referer/Origin, HTTPX keeps them."""
    with _redirect_server() as (host, port, _origin_records):
        origin = f"http://{host}:{port}"
        with _redirect_server() as (other_host, other_port, target_records):
            target = f"http://{other_host}:{other_port}/next"
            results = _wire_scenario(origin, target, target_records)
            expected_host = f"{other_host}:{other_port}"

    engine_method, engine_headers = results["engine"]
    facade_method, facade_headers = results["facade"]
    upstream_method, upstream_headers = results["upstream"]

    assert engine_method == facade_method == upstream_method == "GET"
    assert facade_headers == upstream_headers
    # Engine drops every credential-adjacent header across origins.
    assert "authorization" not in engine_headers
    assert "proxy-authorization" not in engine_headers
    assert "cookie" not in engine_headers
    assert "referer" not in engine_headers
    assert "origin" not in engine_headers
    # HTTPX drops Authorization and Cookie but keeps the rest.
    assert "authorization" not in facade_headers
    assert "cookie" not in facade_headers
    assert facade_headers.get("proxy-authorization") == FIRST_HOP_HEADERS["proxy-authorization"]
    assert facade_headers.get("referer") == FIRST_HOP_HEADERS["referer"]
    assert facade_headers.get("origin") == FIRST_HOP_HEADERS["origin"]
    # Both rewrite Host to the destination.
    assert engine_headers.get("host") == expected_host
    assert facade_headers.get("host") == expected_host


def test_wire_https_upgrade_is_cross_origin_for_the_engine():
    """An http -> https hop changes the origin, so the engine drops credentials.

    The canonical ``http:80 -> https:443`` upgrade cannot be bound by an
    unprivileged test process; the policy layer proves the facade keeps
    ``Authorization`` there, and the engine's origin-based rule (proved here
    for a real scheme change) drops it.
    """
    with _redirect_server() as (host, port, _origin_records):
        origin = f"http://{host}:{port}"
        with _tls_redirect_server() as (tls_host, tls_port, cert_path, target_records):
            target = f"https://{tls_host}:{tls_port}/next"
            results = _wire_scenario(origin, target, target_records, verify=cert_path)

    engine_method, engine_headers = results["engine"]
    facade_method, facade_headers = results["facade"]
    upstream_method, upstream_headers = results["upstream"]

    assert engine_method == facade_method == upstream_method == "GET"
    assert facade_headers == upstream_headers
    assert "authorization" not in engine_headers
    assert "proxy-authorization" not in engine_headers
    assert "cookie" not in engine_headers
    assert "referer" not in engine_headers
    assert "origin" not in engine_headers
    # The local upgrade is not the canonical http:80 -> https:443 form, so
    # HTTPX drops Authorization as well.
    assert "authorization" not in facade_headers


def test_wire_https_downgrade_drops_credentials_for_both_runtimes():
    """An https -> http downgrade is dispatched and drops credentials."""
    with _tls_redirect_server() as (tls_host, tls_port, cert_path, _tls_records):
        with _redirect_server() as (host, port, target_records):
            origin = f"https://{tls_host}:{tls_port}"
            target = f"http://{host}:{port}/next"
            results = _wire_scenario(origin, target, target_records, verify=cert_path)

    engine_method, engine_headers = results["engine"]
    facade_method, facade_headers = results["facade"]
    upstream_method, upstream_headers = results["upstream"]

    assert engine_method == facade_method == upstream_method == "GET"
    assert facade_headers == upstream_headers
    assert "authorization" not in engine_headers
    assert "authorization" not in facade_headers
    assert "proxy-authorization" not in engine_headers
    assert "cookie" not in engine_headers
    assert "cookie" not in facade_headers


@pytest.mark.parametrize("status", sorted(ENGINE_METHOD_TABLE))
@pytest.mark.parametrize("method", ["GET", "HEAD", "POST", "PUT", "DELETE"])
def test_wire_redirect_method_matrix(status, method):
    """Pin the engine's and the facade's method tables on the wire."""
    body = b"payload" if method in ("POST", "PUT") else None
    with _redirect_server() as (host, port, _origin_records):
        origin = f"http://{host}:{port}"
        with _redirect_server() as (other_host, other_port, target_records):
            target = f"http://{other_host}:{other_port}/next"
            results = _wire_scenario(
                origin, target, target_records, status=status, method=method, body=body
            )

    engine_method = results["engine"][0]
    facade_method = results["facade"][0]
    upstream_method = results["upstream"][0]

    assert engine_method == ENGINE_METHOD_TABLE[status][method]
    assert facade_method == HTTPX_METHOD_TABLE[status][method]
    assert upstream_method == HTTPX_METHOD_TABLE[status][method]
    # The facade must stay byte-equivalent to upstream httpx.
    assert facade_method == upstream_method
    assert results["facade"][1] == results["upstream"][1]

    divergence = (status, method) in KNOWN_METHOD_DIVERGENCES
    assert (engine_method != facade_method) is divergence, (
        f"unexpected method divergence for {status}/{method}: "
        f"engine={engine_method} facade={facade_method}"
    )


def test_known_method_divergence_is_exactly_the_pinned_set():
    """Guard the recorded divergence set itself."""
    actual = {
        (status, method)
        for status in ENGINE_METHOD_TABLE
        for method in ENGINE_METHOD_TABLE[status]
        if ENGINE_METHOD_TABLE[status][method] != HTTPX_METHOD_TABLE[status][method]
    }
    assert actual == KNOWN_METHOD_DIVERGENCES
