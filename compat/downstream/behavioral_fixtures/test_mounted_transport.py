"""Category: custom or mounted transport behavior."""

import sys

sys.path.insert(0, "crates/eggfetch-python/python")

from eggfetch.compat.httpx import Client, MockTransport, Request, Response


def custom_handler(request: Request) -> Response:
    return Response(200, json={"custom": True, "scheme": request.url.scheme})


def test_mount_transport():
    custom_transport = MockTransport(custom_handler)
    with Client(mounts={"custom://": custom_transport}) as c:
        r = c.get("custom://anything/path")
        assert r.status_code == 200
        assert r.json()["custom"] is True
        assert r.json()["scheme"] == "custom"


def test_mount_transport_with_default():
    default_handler = MockTransport(lambda r: Response(200, json={"default": True}))
    custom_handler_transport = MockTransport(custom_handler)
    with Client(
        mounts={"custom://": custom_handler_transport},
        transport=default_handler,
    ) as c:
        r1 = c.get("custom://anything/path")
        assert r1.status_code == 200
        assert r1.json()["custom"] is True

        r2 = c.get("http://example.com/normal")
        assert r2.status_code == 200
        assert r2.json()["default"] is True


def test_mount_transport_multiple_schemes():
    def api_handler(request: Request) -> Response:
        return Response(200, json={"transport": "api"})

    def ws_handler(request: Request) -> Response:
        return Response(200, json={"transport": "ws"})

    with Client(
        mounts={
            "api://": MockTransport(api_handler),
            "ws://": MockTransport(ws_handler),
        },
    ) as c:
        r1 = c.get("api://service/endpoint")
        assert r1.json()["transport"] == "api"

        r2 = c.get("ws://socket/connect")
        assert r2.json()["transport"] == "ws"


def test_mount_transport_with_path():
    def handler(request: Request) -> Response:
        return Response(200, json={"path": request.url.path})

    with Client(mounts={"https://api.example.com/v1": MockTransport(handler)}) as c:
        r = c.get("https://api.example.com/v1/users")
        assert r.status_code == 200
        # The mount strips the pattern prefix, but the full URL path is preserved
        assert r.json()["path"] == "/v1/users"
