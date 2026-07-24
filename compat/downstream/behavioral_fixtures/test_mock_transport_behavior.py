"""Category: mock transport and request matching."""

import sys

sys.path.insert(0, "crates/eggfetch-python/python")

from eggfetch.compat.httpx import Client, MockTransport, Request, Response


def handler(request: Request) -> Response:
    if request.url.path == "/mock":
        return Response(200, json={"mock": True})
    return Response(404)


def test_mock_transport_dispatch():
    transport = MockTransport(handler)
    with Client(transport=transport) as c:
        r = c.get("http://test-server/mock")
        assert r.status_code == 200
        assert r.json()["mock"] is True


def test_mock_transport_404():
    transport = MockTransport(handler)
    with Client(transport=transport) as c:
        r = c.get("http://test-server/missing")
        assert r.status_code == 404


def test_mock_transport_post():
    def post_handler(request: Request) -> Response:
        return Response(201, json={"created": True})

    transport = MockTransport(post_handler)
    with Client(transport=transport) as c:
        r = c.post("http://test-server/resource", json={"name": "test"})
        assert r.status_code == 201
        assert r.json()["created"] is True


def test_mock_transport_headers():
    def header_handler(request: Request) -> Response:
        auth = request.headers.get("authorization", "")
        return Response(200, json={"auth": auth})

    transport = MockTransport(header_handler)
    with Client(transport=transport, headers={"Authorization": "Bearer tok"}) as c:
        r = c.get("http://test-server/check")
        assert r.status_code == 200
        assert r.json()["auth"] == "Bearer tok"


def test_mock_transport_request_url_parsing():
    def url_handler(request: Request) -> Response:
        return Response(200, json={
            "host": request.url.host,
            "path": request.url.path,
            "query": str(request.url.query),
        })

    transport = MockTransport(url_handler)
    with Client(transport=transport) as c:
        r = c.get("http://example.com/api/v1/items?page=2")
        assert r.status_code == 200
        data = r.json()
        assert data["host"] == "example.com"
        assert data["path"] == "/api/v1/items"
        assert "page=2" in data["query"]
