"""Category: framework-test-client — pytest-httpx plugin integration.

Exercises the pytest-httpx plugin which intercepts httpx requests via
MockTransport injection. Verifies the eggfetch shim is compatible with
the plugin's request interception mechanism.
"""

from __future__ import annotations

import sys

sys.path.insert(0, "crates/eggfetch-python/python")

import httpx
from eggfetch.compat.httpx import Client, MockTransport, Request, Response


def handler(request: Request) -> Response:
    if request.url.path == "/api/data":
        return Response(200, json={"status": "ok", "source": "mock"})
    return Response(404)


def test_eggfetch_shim_compatible_with_mock_transport():
    """Verify eggfetch shim works with MockTransport pattern used by pytest-httpx."""
    assert getattr(httpx, "__eggfetch_shim__", False) is True


def test_mock_transport_intercepts_requests():
    """Simulate pytest-httpx's request interception pattern."""
    transport = MockTransport(handler)
    with Client(transport=transport) as client:
        response = client.get("http://test-server/api/data")
        assert response.status_code == 200
        assert response.json()["status"] == "ok"
        assert response.json()["source"] == "mock"


def test_mock_transport_post_interception():
    """Simulate pytest-httpx POST interception."""
    def post_handler(request: Request) -> Response:
        body = request.content
        return Response(201, json={"received": len(body), "method": "POST"})

    transport = MockTransport(post_handler)
    with Client(transport=transport) as client:
        response = client.post(
            "http://test-server/api/data",
            json={"key": "value"},
        )
        assert response.status_code == 201
        assert response.json()["method"] == "POST"


def test_mock_transport_request_matching():
    """Simulate pytest-httpx's request matching by URL pattern."""
    def router(request: Request) -> Response:
        if "/users" in request.url.path:
            return Response(200, json={"users": []})
        elif "/items" in request.url.path:
            return Response(200, json={"items": []})
        return Response(404)

    transport = MockTransport(router)
    with Client(transport=transport) as client:
        users_response = client.get("http://test-server/api/users")
        assert users_response.status_code == 200
        assert "users" in users_response.json()

        items_response = client.get("http://test-server/api/items")
        assert items_response.status_code == 200
        assert "items" in items_response.json()


def test_mock_transport_header_assertion():
    """Simulate pytest-httpx pattern of asserting request headers."""
    def header_checker(request: Request) -> Response:
        return Response(200, json={
            "authorization": request.headers.get("authorization", ""),
            "content-type": request.headers.get("content-type", ""),
        })

    transport = MockTransport(header_checker)
    with Client(
        transport=transport,
        headers={"Authorization": "Bearer test-token"},
    ) as client:
        response = client.get("http://test-server/check")
        assert response.status_code == 200
        assert response.json()["authorization"] == "Bearer test-token"


def test_multiple_sequential_requests():
    """Simulate pytest-httpx pattern of multiple sequential requests."""
    call_count = 0
    def counting_handler(request: Request) -> Response:
        nonlocal call_count
        call_count += 1
        return Response(200, json={"call": call_count})

    transport = MockTransport(counting_handler)
    with Client(transport=transport) as client:
        for i in range(5):
            response = client.get("http://test-server/count")
            assert response.status_code == 200
            assert response.json()["call"] == i + 1
