"""Category: asgi-test-client — Starlette TestClient integration.

Exercises Starlette's TestClient which internally uses httpx.ASGITransport.
Verifies the eggfetch shim provides a compatible ASGI transport layer.
"""

from __future__ import annotations

import sys

sys.path.insert(0, "crates/eggfetch-python/python")

import httpx
from starlette.applications import Starlette
from starlette.requests import Request
from starlette.responses import JSONResponse, PlainTextResponse
from starlette.routing import Route
from starlette.testclient import TestClient


def homepage(request: Request) -> PlainTextResponse:
    return PlainTextResponse("ok")


async def echo_body(request: Request) -> JSONResponse:
    body = await request.json()
    return JSONResponse({"echo": body})


def get_headers(request: Request) -> JSONResponse:
    return JSONResponse(dict(request.headers))


def get_path(request: Request) -> PlainTextResponse:
    return PlainTextResponse(request.url.path)


def post_with_path(request: Request) -> PlainTextResponse:
    return PlainTextResponse(request.url.path)


app = Starlette(
    routes=[
        Route("/", homepage),
        Route("/echo", echo_body, methods=["POST"]),
        Route("/headers", get_headers),
        Route("/path/{name}", get_path),
        Route("/path/{name}", post_with_path, methods=["POST"]),
    ]
)

client = TestClient(app)


def test_starlette_uses_eggfetch_shim():
    assert getattr(httpx, "__eggfetch_shim__", False) is True


def test_starlette_get_homepage():
    response = client.get("/")
    assert response.status_code == 200
    assert response.text == "ok"


def test_starlette_post_echo():
    response = client.post("/echo", json={"key": "value"})
    assert response.status_code == 200
    assert response.json()["echo"] == {"key": "value"}


def test_starlette_request_headers():
    response = client.get("/headers", headers={"X-Custom": "test"})
    assert response.status_code == 200
    headers = response.json()
    assert "x-custom" in headers
    assert headers["x-custom"] == "test"


def test_starlette_path_params():
    response = client.get("/path/items")
    assert response.status_code == 200
    assert response.text == "/path/items"


def test_starlette_post_path_params():
    response = client.post("/path/items")
    assert response.status_code == 200
    assert response.text == "/path/items"


def test_starlette_multiple_requests():
    for _ in range(3):
        response = client.get("/")
        assert response.status_code == 200
        assert response.text == "ok"


def test_starlette_json_response_format():
    response = client.post("/echo", json={"nested": {"a": 1}})
    assert response.status_code == 200
    data = response.json()
    assert isinstance(data, dict)
    assert "echo" in data
    assert data["echo"]["nested"]["a"] == 1
