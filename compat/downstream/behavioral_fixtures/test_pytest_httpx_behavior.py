"""Category: framework-test-client — exercises pytest-httpx.

pytest-httpx is a pytest fixture plugin that intercepts httpx requests via
MockTransport. This fixture uses the httpx_mock fixture and asserts request
matching and response injection.
"""

import httpx


def test_pytest_httpx_mock_fixture(httpx_mock):
    """pytest-httpx httpx_mock fixture intercepts requests and injects responses."""
    httpx_mock.add_response(url="http://test/mock", json={"mocked": True})
    with httpx.Client() as c:
        resp = c.get("http://test/mock")
        assert resp.status_code == 200
        assert resp.json()["mocked"] is True


def test_pytest_httpx_mock_assert_called(httpx_mock):
    """pytest-httpx asserts the expected request was made."""
    httpx_mock.add_response(url="http://test/assert", status_code=201, text="created")
    with httpx.Client() as c:
        resp = c.post("http://test/assert", json={"data": 1})
        assert resp.status_code == 201
        assert resp.text == "created"
    # pytest-httpx asserts all registered responses were requested
    requests = httpx_mock.get_requests()
    assert len(requests) == 1


def test_pytest_httpx_mock_headers(httpx_mock):
    """pytest-httpx matches requests by headers."""
    httpx_mock.add_response(
        url="http://test/headers",
        match_headers={"x-custom": "abc"},
        json={"headers_matched": True},
    )
    with httpx.Client() as c:
        resp = c.get("http://test/headers", headers={"x-custom": "abc"})
        assert resp.status_code == 200
        assert resp.json()["headers_matched"] is True
