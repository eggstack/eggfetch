"""Structured behavior case tests for HTTPX compatibility.

Each case has a stable ID and runs against both httpx and eggfetch
to verify identical behavior.
"""

import json

import httpx
import pytest

import eggfetch

from .fixtures import BEHAVIOR_CASES, BehaviorCase, create_server

assert httpx.__version__ == "0.28.1"


@pytest.fixture(scope="module")
def server():
    srv, url = create_server()
    yield url
    srv.shutdown()


def _run_case_httpx(case: BehaviorCase, base_url: str):
    """Run a behavior case against httpx."""
    url = f"{base_url}{case.path}"
    kwargs = {"follow_redirects": case.follow_redirects}
    if case.headers:
        kwargs["headers"] = case.headers
    if case.timeout:
        kwargs["timeout"] = case.timeout
    if case.body:
        kwargs["content"] = case.body
        if case.content_type:
            kwargs["headers"] = {**(case.headers or {}), "Content-Type": case.content_type}

    if case.method == "GET":
        return httpx.get(url, **kwargs)
    elif case.method == "POST":
        return httpx.post(url, **kwargs)
    else:
        return httpx.request(case.method, url, **kwargs)


def _run_case_eggfetch(case: BehaviorCase, base_url: str):
    """Run a behavior case against eggfetch."""
    url = f"{base_url}{case.path}"
    kwargs = {"follow_redirects": case.follow_redirects}
    if case.headers:
        kwargs["headers"] = case.headers
    if case.timeout:
        kwargs["timeout"] = case.timeout
    if case.body:
        kwargs["content"] = case.body
        if case.content_type:
            kwargs["headers"] = {**(case.headers or {}), "Content-Type": case.content_type}

    if case.method == "GET":
        return eggfetch.get(url, **kwargs)
    elif case.method == "POST":
        return eggfetch.post(url, **kwargs)
    else:
        return eggfetch.request(case.method, url, **kwargs)


@pytest.mark.parametrize(
    "case",
    BEHAVIOR_CASES,
    ids=[c.case_id for c in BEHAVIOR_CASES],
)
def test_eggfetch_matches_httpx_status(server, case: BehaviorCase):
    """Eggfetch status code should match httpx for each behavior case."""
    httpx_resp = _run_case_httpx(case, server)
    egg_resp = _run_case_eggfetch(case, server)
    assert egg_resp.status_code == httpx_resp.status_code, (
        f"Case {case.case_id}: eggfetch={egg_resp.status_code} httpx={httpx_resp.status_code}"
    )


@pytest.mark.parametrize(
    "case",
    BEHAVIOR_CASES,
    ids=[c.case_id for c in BEHAVIOR_CASES],
)
def test_eggfetch_matches_httpx_body(server, case: BehaviorCase):
    """Eggfetch response body should match httpx for JSON cases."""
    httpx_resp = _run_case_httpx(case, server)
    egg_resp = _run_case_eggfetch(case, server)
    ct = httpx_resp.headers.get("content-type", "")
    if "application/json" in ct and httpx_resp.content and case.method != "HEAD":
        if not case.case_id.startswith("HEADER-"):
            assert httpx_resp.json() == egg_resp.json(), (
                f"Case {case.case_id}: body mismatch"
            )


def test_all_cases_have_required_fields():
    """Every behavior case must have a stable ID and expected status."""
    for case in BEHAVIOR_CASES:
        assert case.case_id, f"Case missing case_id: {case}"
        assert isinstance(case.expected_status, int), (
            f"Case {case.case_id}: expected_status must be int"
        )


METHOD_CASES = [c for c in BEHAVIOR_CASES if c.case_id.startswith(
    ("PUT-", "DELETE-", "PATCH-", "HEAD-", "OPTIONS-", "PROPFIND-")
)]

HEADER_CASES = [c for c in BEHAVIOR_CASES if c.case_id.startswith("HEADER-")]

QUERY_CASES = [c for c in BEHAVIOR_CASES if c.case_id.startswith("QUERY-")]

LARGE_BODY_CASES = [c for c in BEHAVIOR_CASES if c.case_id.startswith("LARGE-")]


@pytest.mark.parametrize(
    "case",
    METHOD_CASES,
    ids=[c.case_id for c in METHOD_CASES],
)
def test_eggfetch_matches_httpx_method(server, case: BehaviorCase):
    """Eggfetch HTTP method dispatch matches httpx for all method types."""
    httpx_resp = _run_case_httpx(case, server)
    egg_resp = _run_case_eggfetch(case, server)
    assert egg_resp.status_code == httpx_resp.status_code, (
        f"Case {case.case_id}: method={case.method} "
        f"eggfetch={egg_resp.status_code} httpx={httpx_resp.status_code}"
    )
    ct = httpx_resp.headers.get("content-type", "")
    if "application/json" in ct and httpx_resp.content:
        assert httpx_resp.json() == egg_resp.json(), (
            f"Case {case.case_id}: JSON body mismatch"
        )


@pytest.mark.parametrize(
    "case",
    HEADER_CASES,
    ids=[c.case_id for c in HEADER_CASES],
)
def test_eggfetch_matches_httpx_headers(server, case: BehaviorCase):
    """Eggfetch header handling matches httpx for custom and duplicate headers."""
    httpx_resp = _run_case_httpx(case, server)
    egg_resp = _run_case_eggfetch(case, server)
    assert egg_resp.status_code == httpx_resp.status_code, (
        f"Case {case.case_id}: status mismatch"
    )
    if case.expected_fields and "application/json" in httpx_resp.headers.get("content-type", ""):
        httpx_json = httpx_resp.json()
        egg_json = egg_resp.json()
        for key in case.expected_fields:
            assert httpx_json.get(key) == egg_json.get(key), (
                f"Case {case.case_id}: field '{key}' mismatch "
                f"httpx={httpx_json.get(key)!r} eggfetch={egg_json.get(key)!r}"
            )


@pytest.mark.parametrize(
    "case",
    QUERY_CASES,
    ids=[c.case_id for c in QUERY_CASES],
)
def test_eggfetch_matches_httpx_query_params(server, case: BehaviorCase):
    """Eggfetch query parameter parsing matches httpx."""
    httpx_resp = _run_case_httpx(case, server)
    egg_resp = _run_case_eggfetch(case, server)
    assert egg_resp.status_code == httpx_resp.status_code, (
        f"Case {case.case_id}: status mismatch"
    )
    if "application/json" in httpx_resp.headers.get("content-type", ""):
        assert httpx_resp.json() == egg_resp.json(), (
            f"Case {case.case_id}: query params mismatch"
        )


@pytest.mark.parametrize(
    "case",
    LARGE_BODY_CASES,
    ids=[c.case_id for c in LARGE_BODY_CASES],
)
def test_eggfetch_matches_httpx_large_body(server, case: BehaviorCase):
    """Eggfetch handles large response bodies correctly."""
    httpx_resp = _run_case_httpx(case, server)
    egg_resp = _run_case_eggfetch(case, server)
    assert egg_resp.status_code == httpx_resp.status_code, (
        f"Case {case.case_id}: status mismatch"
    )
    assert len(egg_resp.content) == len(httpx_resp.content), (
        f"Case {case.case_id}: body length mismatch "
        f"eggfetch={len(egg_resp.content)} httpx={len(httpx_resp.content)}"
    )
    assert egg_resp.content == httpx_resp.content, (
        f"Case {case.case_id}: body content mismatch"
    )


def test_eggfetch_matches_httpx_connection_refused():
    """Eggfetch and httpx both raise errors on connection refused."""
    port = 19999
    url = f"http://127.0.0.1:{port}/anything"
    httpx_error = None
    egg_error = None
    try:
        httpx.get(url, timeout=2.0)
    except (httpx.ConnectError, httpx.ConnectTimeout, OSError):
        httpx_error = True
    try:
        eggfetch.get(url, timeout=2.0)
    except Exception:
        egg_error = True
    assert httpx_error is not None, "httpx should raise on connection refused"
    assert egg_error is not None, "eggfetch should raise on connection refused"


def test_eggfetch_matches_httpx_invalid_url():
    """Eggfetch and httpx both reject invalid URLs."""
    httpx_error = None
    egg_error = None
    try:
        httpx.get("not-a-valid-url")
    except (httpx.UnsupportedProtocol, ValueError):
        httpx_error = True
    try:
        eggfetch.get("not-a-valid-url")
    except Exception:
        egg_error = True
    assert httpx_error is not None, "httpx should raise on invalid URL"
    assert egg_error is not None, "eggfetch should raise on invalid URL"


def test_binary_response_roundtrip(server):
    """Binary content is preserved through both httpx and eggfetch."""
    httpx_resp = httpx.get(f"{server}/binary")
    egg_resp = eggfetch.get(f"{server}/binary")
    assert httpx_resp.status_code == 200
    assert egg_resp.status_code == 200
    assert httpx_resp.content == egg_resp.content
    assert len(httpx_resp.content) == 256


def test_empty_response_body_roundtrip(server):
    """Empty response body is handled identically."""
    httpx_resp = httpx.get(f"{server}/empty-body-response")
    egg_resp = eggfetch.get(f"{server}/empty-body-response")
    assert httpx_resp.status_code == egg_resp.status_code == 200
    assert httpx_resp.text == egg_resp.text == ""


def test_many_headers_roundtrip(server):
    """Response with many headers is handled identically."""
    httpx_resp = httpx.get(f"{server}/many-headers")
    egg_resp = eggfetch.get(f"{server}/many-headers")
    assert httpx_resp.status_code == egg_resp.status_code == 200
    for i in range(50):
        hdr = f"x-custom-{i:03d}"
        assert httpx_resp.headers.get(hdr) == f"value-{i}"
        assert egg_resp.headers.get(hdr) == f"value-{i}"
    assert httpx_resp.json() == egg_resp.json()
