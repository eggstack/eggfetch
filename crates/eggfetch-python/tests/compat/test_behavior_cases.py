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
    if "application/json" in httpx_resp.headers.get("content-type", ""):
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
