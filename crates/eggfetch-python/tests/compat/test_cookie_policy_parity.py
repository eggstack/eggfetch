"""Facade-side cookie policy measurement (ADR-0009 rule 3; milestone 005 §6.1).

This is the facade half of the shared cookie policy matrix.  Every case is
measured against a **real local HTTP server over real sockets**: the fixture
server emits the case's ``Set-Cookie`` header and echoes back the ``Cookie``
header it received, so the facade's ``http.cookiejar`` jar is always fed a
genuine response instead of a hand-rolled fake.

The engine half of the same matrix lives in
``crates/eggfetch-core/tests/cookie_policy_parity_tests.rs`` and uses the same
case ids, hosts, paths and ``Set-Cookie`` headers, so the two matrices can be
compared row by row.

Two transports are used:

* ``_RealFixtureTransport`` performs a genuine HTTP/1.1 exchange with the
  fixture server over a real socket while preserving the *logical* request
  URL.  The logical host is what the cookie jar scopes against, and several
  matrix rows use hosts (``example.test``) that do not resolve, so the TCP
  destination is rewritten to the loopback fixture while the URL keeps its
  host.
* the facade's default native transport is exercised end to end against the
  same fixture server for the rows whose host is the loopback address, so the
  wire behaviour is observed without any transport substitution.
"""

from __future__ import annotations

import dataclasses
import http.client
import http.server
import time
from typing import Iterator

import httpx
import pytest

from eggfetch.compat.httpx import BaseTransport, Client, Headers, Request, Response

from .native_fixtures import local_http_server

# ── The shared cookie policy matrix ──────────────────────────────────────


@dataclasses.dataclass(frozen=True)
class _Case:
    """One row of the shared cookie policy matrix."""

    id: str
    host: str
    set_path: str
    request_path: str
    set_cookie: str


@dataclasses.dataclass(frozen=True)
class _Expectation:
    """The facade's agreed verdict for a case."""

    stored: bool
    #: The domain exactly as ``http.cookiejar`` stores it (leading dot kept).
    domain: str | None
    #: The host-only flag, i.e. the inverse of ``domain_specified``.
    host_only: bool | None
    path: str | None
    secure: bool | None
    persistent: bool | None
    #: ``"none"``, ``"future"`` or ``"exact"``.
    expires: str
    #: The exact expiry epoch when ``expires == "exact"``.
    expires_exact: int | None
    #: The exact ``Cookie`` header the facade contributes for ``request_path``.
    cookie_header: str | None


def _stored(
    domain: str,
    host_only: bool,
    path: str,
    *,
    secure: bool = False,
    persistent: bool = False,
    expires: str = "none",
    expires_exact: int | None = None,
    cookie_header: str | None = "a=1",
) -> _Expectation:
    return _Expectation(
        stored=True,
        domain=domain,
        host_only=host_only,
        path=path,
        secure=secure,
        persistent=persistent,
        expires=expires,
        expires_exact=expires_exact,
        cookie_header=cookie_header,
    )


def _rejected() -> _Expectation:
    return _Expectation(
        stored=False,
        domain=None,
        host_only=None,
        path=None,
        secure=None,
        persistent=None,
        expires="none",
        expires_exact=None,
        cookie_header=None,
    )


#: 2035-10-21T07:28:00Z, the ``Expires`` value used by the fixture.
_EXPIRES_FUTURE = 2076564480

CASES: tuple[tuple[_Case, _Expectation], ...] = (
    # ── Domain attribute ────────────────────────────────────────────
    (
        _Case("no_domain", "example.test", "/s", "/", "a=1"),
        _stored("example.test", True, "/"),
    ),
    (
        _Case("domain_exact", "example.test", "/s", "/", "a=1; Domain=example.test"),
        _stored(".example.test", False, "/"),
    ),
    (
        _Case("domain_leading_dot", "example.test", "/s", "/", "a=1; Domain=.example.test"),
        _stored(".example.test", False, "/"),
    ),
    (
        _Case("domain_parent", "sub.example.test", "/s", "/", "a=1; Domain=example.test"),
        _stored(".example.test", False, "/"),
    ),
    (
        _Case("domain_mismatch", "example.test", "/s", "/", "a=1; Domain=other.test"),
        _rejected(),
    ),
    (
        _Case("domain_unrelated_suffix", "example.test", "/s", "/", "a=1; Domain=evil.test"),
        _rejected(),
    ),
    # ── IP literals ──────────────────────────────────────────────────
    # ADR-0009 rule 3: the facade's jar is ``http.cookiejar``, the jar HTTPX
    # 0.28.1 uses, so an exact-match IP-literal ``Domain`` is accepted here.
    (
        _Case("domain_ipv4_literal", "127.0.0.1", "/s", "/", "a=1; Domain=127.0.0.1"),
        _stored(".127.0.0.1", False, "/"),
    ),
    (
        _Case(
            "domain_ipv4_leading_dot",
            "127.0.0.1",
            "/s",
            "/",
            "a=1; Domain=.127.0.0.1",
        ),
        _stored(".127.0.0.1", False, "/"),
    ),
    (
        _Case("domain_ipv4_mismatch", "127.0.0.1", "/s", "/", "a=1; Domain=127.0.0.2"),
        _rejected(),
    ),
    # Recorded residual difference: ``http.cookiejar`` stores the cookie but
    # does not return it, because ``eff_request_host`` rewrites a dotless IPv6
    # host to ``[::1].local``.  See ``docs/residual-differences.md``.
    (
        _Case("domain_ipv6_literal", "[::1]", "/s", "/", "a=1; Domain=[::1]"),
        _stored(".[::1]", False, "/", cookie_header=None),
    ),
    # A host-only cookie for the very same IPv6 host is returned by both
    # jars: the residual difference above is specific to an explicit
    # IPv6-literal ``Domain`` attribute.  ``http.cookiejar`` stores the
    # host-only domain as the mangled ``[::1].local``.
    (
        _Case("ipv6_host_only", "[::1]", "/s", "/", "a=1"),
        _stored("[::1].local", True, "/"),
    ),
    (
        _Case("domain_ipv6_unbracketed", "[::1]", "/s", "/", "a=1; Domain=::1"),
        _rejected(),
    ),
    # ── Path scoping ────────────────────────────────────────────────
    (
        _Case("path_api", "example.test", "/api/set", "/api/v1", "a=1; Path=/api"),
        _stored("example.test", True, "/api"),
    ),
    (
        _Case("path_api_no_match", "example.test", "/api/set", "/other", "a=1; Path=/api"),
        _stored("example.test", True, "/api", cookie_header=None),
    ),
    (
        _Case("path_root", "example.test", "/s", "/api/v1", "a=1; Path=/"),
        _stored("example.test", True, "/"),
    ),
    # ── Secure / expiry ─────────────────────────────────────────────
    (
        _Case("secure_set", "example.test", "/s", "/", "a=1; Secure"),
        _stored("example.test", True, "/", secure=True, cookie_header=None),
    ),
    (
        _Case("max_age", "example.test", "/s", "/", "a=1; Max-Age=3600"),
        _stored("example.test", True, "/", persistent=True, expires="future"),
    ),
    (
        _Case(
            "expires_future",
            "example.test",
            "/s",
            "/",
            "a=1; Expires=Wed, 21 Oct 2035 07:28:00 GMT",
        ),
        _stored(
            "example.test",
            True,
            "/",
            persistent=True,
            expires="future",
            expires_exact=_EXPIRES_FUTURE,
        ),
    ),
    (
        _Case(
            "expires_past",
            "example.test",
            "/s",
            "/",
            "a=1; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
        ),
        _rejected(),
    ),
)

CASE_IDS = [case.id for case, _ in CASES]
_CASE_BY_ID = {case.id: case for case, _ in CASES}


# ── The real local HTTP fixture ─────────────────────────────────────────


class _CookiePolicyHandler(http.server.BaseHTTPRequestHandler):
    """Emit a case's ``Set-Cookie`` header, or echo the received ``Cookie``."""

    protocol_version = "HTTP/1.0"

    def do_GET(self) -> None:  # noqa: N802 - stdlib naming
        case_id = self.headers.get("X-Cookie-Policy-Case", "")
        if case_id:
            body = b"set-cookie"
            self.send_response(200)
            self.send_header("Set-Cookie", _CASE_BY_ID[case_id].set_cookie)
        else:
            body = (self.headers.get("Cookie") or "").encode("utf-8")
            self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()

    def log_message(self, format: str, *args: object) -> None:
        """Silence the default stderr access log."""


@pytest.fixture(scope="module")
def cookie_policy_server() -> Iterator[tuple[str, int]]:
    """A real local HTTP server serving the cookie policy matrix."""
    with local_http_server(_CookiePolicyHandler) as (host, port):
        yield host, port


# ── Transports ──────────────────────────────────────────────────────────


class _RealFixtureTransport(BaseTransport):
    """Exchange with the fixture server over a real socket.

    The logical request URL is preserved, so the facade's jar scopes the
    cookie against the case host; only the TCP destination is rewritten to
    the loopback fixture.
    """

    def __init__(self, host: str, port: int) -> None:
        self._host = host
        self._port = port

    def handle_request(self, request: Request) -> Response:
        connection = http.client.HTTPConnection(self._host, self._port, timeout=10)
        try:
            headers = dict(request.headers.multi_items())
            headers["Host"] = request.url.netloc.decode("ascii")
            connection.request(
                request.method,
                request.url.raw_path.decode("ascii"),
                body=request.content or None,
                headers=headers,
            )
            raw = connection.getresponse()
            body = raw.read()
            return Response(
                raw.status,
                headers=Headers(raw.getheaders()),
                content=body,
                request=request,
            )
        finally:
            connection.close()


class _HttpxRealFixtureTransport(httpx.BaseTransport):
    """The same real-socket exchange, driven through HTTPX 0.28.1 itself."""

    def __init__(self, host: str, port: int) -> None:
        self._host = host
        self._port = port

    def handle_request(self, request: httpx.Request) -> httpx.Response:
        connection = http.client.HTTPConnection(self._host, self._port, timeout=10)
        try:
            headers = dict(request.headers.multi_items())
            headers["Host"] = request.url.netloc.decode("ascii")
            connection.request(
                request.method,
                request.url.raw_path.decode("ascii"),
                body=request.content or None,
                headers=headers,
            )
            raw = connection.getresponse()
            body = raw.read()
            return httpx.Response(
                raw.status,
                headers=httpx.Headers(raw.getheaders()),
                content=body,
                request=request,
            )
        finally:
            connection.close()


# ── Measurement helpers ─────────────────────────────────────────────────


def _measure_facade(case: _Case, host: str, port: int) -> _Expectation:
    """Measure the facade jar for ``case`` against the real fixture server."""
    with Client(transport=_RealFixtureTransport(host, port)) as client:
        client.get(
            f"http://{case.host}:{port}{case.set_path}",
            headers={"x-cookie-policy-case": case.id},
        )
        stored = list(client.cookies.jar)
        echo = client.get(f"http://{case.host}:{port}{case.request_path}")

    if not stored:
        return _rejected()

    cookie = stored[0]
    if cookie.expires is None:
        expires_kind = "none"
    elif cookie.expires > int(time.time()):
        expires_kind = "future"
    else:
        expires_kind = "exact"

    return _Expectation(
        stored=True,
        domain=cookie.domain,
        host_only=not cookie.domain_specified,
        path=cookie.path,
        secure=cookie.secure,
        persistent=cookie.expires is not None,
        expires=expires_kind,
        expires_exact=cookie.expires,
        cookie_header=echo.text or None,
    )


def _measure_httpx(case: _Case, host: str, port: int) -> _Expectation:
    """Measure the same case with the reference HTTPX 0.28.1 client."""
    with httpx.Client(transport=_HttpxRealFixtureTransport(host, port)) as client:
        client.get(
            f"http://{case.host}:{port}{case.set_path}",
            headers={"x-cookie-policy-case": case.id},
        )
        stored = list(client.cookies.jar)
        echo = client.get(f"http://{case.host}:{port}{case.request_path}")

    if not stored:
        return _rejected()

    cookie = stored[0]
    if cookie.expires is None:
        expires_kind = "none"
    elif cookie.expires > int(time.time()):
        expires_kind = "future"
    else:
        expires_kind = "exact"

    return _Expectation(
        stored=True,
        domain=cookie.domain,
        host_only=not cookie.domain_specified,
        path=cookie.path,
        secure=cookie.secure,
        persistent=cookie.expires is not None,
        expires=expires_kind,
        expires_exact=cookie.expires,
        cookie_header=echo.text or None,
    )


# ── Tests ───────────────────────────────────────────────────────────────


@pytest.mark.parametrize(("case", "expected"), CASES, ids=CASE_IDS)
def test_facade_cookie_policy_matches_agreed_matrix(
    case: _Case,
    expected: _Expectation,
    cookie_policy_server: tuple[str, int],
) -> None:
    """The facade's jar reaches the agreed verdict for every matrix row."""
    host, port = cookie_policy_server
    measured = _measure_facade(case, host, port)

    assert measured.stored is expected.stored, case.id
    assert measured.domain == expected.domain, case.id
    assert measured.host_only == expected.host_only, case.id
    assert measured.path == expected.path, case.id
    assert measured.secure == expected.secure, case.id
    assert measured.persistent == expected.persistent, case.id
    assert measured.expires == expected.expires, case.id
    if expected.expires_exact is not None:
        assert measured.expires_exact == expected.expires_exact, case.id
    assert measured.cookie_header == expected.cookie_header, case.id


@pytest.mark.parametrize(("case", "expected"), CASES, ids=CASE_IDS)
def test_facade_matches_httpx_0281_reference(
    case: _Case,
    expected: _Expectation,
    cookie_policy_server: tuple[str, int],
) -> None:
    """The facade's jar is HTTPX 0.28.1's jar, so the verdicts must match."""
    host, port = cookie_policy_server
    assert _measure_facade(case, host, port) == _measure_httpx(case, host, port), case.id


def test_default_native_transport_sends_ip_literal_domain_cookie(
    cookie_policy_server: tuple[str, int],
) -> None:
    """End to end over the default native transport: the cookie is sent.

    This is the row that used to diverge: the engine jar rejected
    ``Domain=127.0.0.1`` outright while the facade accepted it.
    """
    host, port = cookie_policy_server
    with Client() as client:
        client.get(
            f"http://{host}:{port}/s",
            headers={"x-cookie-policy-case": "domain_ipv4_literal"},
        )
        echo = client.get(f"http://{host}:{port}/")

    assert echo.text == "a=1"


def test_default_native_transport_drops_mismatched_domain_cookie(
    cookie_policy_server: tuple[str, int],
) -> None:
    """End to end over the default native transport: a foreign domain is dropped."""
    host, port = cookie_policy_server
    with Client() as client:
        client.get(
            f"http://{host}:{port}/s",
            headers={"x-cookie-policy-case": "domain_mismatch"},
        )
        echo = client.get(f"http://{host}:{port}/")

    assert echo.text == ""
