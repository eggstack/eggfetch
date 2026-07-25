"""Category: mock-transport-request-matching — exercises respx.

respx is a widely-used mock transport library that patches httpx transports.
This fixture registers a route, sends through httpx.Client, and asserts
route match, call count, status, and body.
"""

import httpx
import respx


def test_respx_route_match_and_body():
    """respx registers a route and asserts match, call count, status, body."""
    with respx.mock:
        route = respx.get("http://test/mock").respond(200, json={"mocked": True})
        with httpx.Client() as c:
            resp = c.get("http://test/mock")
            assert resp.status_code == 200
            assert resp.json()["mocked"] is True
            assert route.called
            assert route.call_count == 1


def test_respx_post_route_match():
    """respx matches POST requests and asserts body echo."""
    with respx.mock:
        route = respx.post("http://test/echo").respond(201, json={"received": True})
        with httpx.Client() as c:
            resp = c.post("http://test/echo", json={"key": "value"})
            assert resp.status_code == 201
            assert resp.json()["received"] is True
            assert route.called
            assert route.call_count == 1


def test_respx_multiple_routes():
    """respx handles multiple routes on the same mock."""
    with respx.mock:
        route1 = respx.get("http://test/first").respond(200, text="first")
        route2 = respx.get("http://test/second").respond(200, text="second")
        with httpx.Client() as c:
            r1 = c.get("http://test/first")
            r2 = c.get("http://test/second")
            assert r1.text == "first"
            assert r2.text == "second"
            assert route1.called
            assert route2.called


def test_respx_call_details():
    """respx records request details for inspection."""
    with respx.mock:
        route = respx.get("http://test/inspect").respond(200)
        with httpx.Client() as c:
            c.get("http://test/inspect", headers={"X-Custom": "abc"})
            assert route.called
            assert route.call_count == 1
            call = route.calls[0]
            assert call.request.url.path == "/inspect"
            assert call.request.headers.get("x-custom") == "abc"
