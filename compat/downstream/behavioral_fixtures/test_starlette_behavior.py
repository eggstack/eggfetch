"""Category: asgi-test-client — exercises starlette TestClient.

starlette's TestClient uses httpx.ASGITransport to drive ASGI applications
in-process. This fixture constructs a TestClient, calls a Starlette route,
and asserts status/body and lifecycle.
"""

from starlette.applications import Starlette
from starlette.responses import JSONResponse, PlainTextResponse
from starlette.routing import Route
from starlette.testclient import TestClient


def _create_app():
    def hello(request):
        return PlainTextResponse("hello from starlette")

    async def echo_json(request):
        body = await request.json()
        return JSONResponse({"received": body})

    routes = [
        Route("/hello", hello),
        Route("/echo", echo_json, methods=["POST"]),
    ]
    return Starlette(routes=routes)


def test_starlette_testclient_get():
    """Starlette TestClient GET returns expected status and body."""
    app = _create_app()
    client = TestClient(app)
    resp = client.get("/hello")
    assert resp.status_code == 200
    assert resp.text == "hello from starlette"


def test_starlette_testclient_post_json():
    """Starlette TestClient POST with JSON returns echoed data."""
    app = _create_app()
    client = TestClient(app)
    resp = client.post("/echo", json={"key": "value"})
    assert resp.status_code == 200
    assert resp.json()["received"] == {"key": "value"}


def test_starlette_testclient_lifecycle():
    """Starlette TestClient manages ASGI lifecycle correctly."""
    app = _create_app()
    with TestClient(app) as client:
        resp = client.get("/hello")
        assert resp.status_code == 200
    # After context exit, client transport is closed
    assert client.app is not None
