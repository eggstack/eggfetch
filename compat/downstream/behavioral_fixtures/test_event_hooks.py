"""Category: event hooks or instrumentation."""

import sys

sys.path.insert(0, "crates/eggfetch-python/python")

from eggfetch.compat.httpx import Client, MockTransport, Request, Response


def test_request_hook():
    hook_calls = []

    def on_request(request: Request):
        hook_calls.append(("request", request.url.path))

    def on_response(response):
        hook_calls.append(("response", response.status_code))

    transport = MockTransport(lambda r: Response(200))
    with Client(
        transport=transport,
        event_hooks={"request": [on_request], "response": [on_response]},
    ) as c:
        r = c.get("http://test-server/hook")
        assert r.status_code == 200
        assert ("request", "/hook") in hook_calls
        assert ("response", 200) in hook_calls


def test_multiple_request_hooks():
    calls_a = []
    calls_b = []

    def hook_a(request: Request):
        calls_a.append(request.url.path)

    def hook_b(request: Request):
        calls_b.append(request.url.path)

    transport = MockTransport(lambda r: Response(200))
    with Client(
        transport=transport,
        event_hooks={"request": [hook_a, hook_b], "response": []},
    ) as c:
        r = c.get("http://test-server/multi")
        assert r.status_code == 200
        assert calls_a == ["/multi"]
        assert calls_b == ["/multi"]


def test_response_hook_receives_status():
    captured = {}

    def on_response(response):
        captured["status"] = response.status_code
        captured["headers"] = dict(response.headers)

    transport = MockTransport(lambda r: Response(201, headers={"X-Custom": "yes"}))
    with Client(
        transport=transport,
        event_hooks={"request": [], "response": [on_response]},
    ) as c:
        r = c.post("http://test-server/create")
        assert r.status_code == 201
        assert captured["status"] == 201
        assert captured["headers"].get("x-custom") == "yes"


def test_hook_sees_request_headers():
    captured_headers = {}

    def on_request(request: Request):
        captured_headers.update(dict(request.headers))

    transport = MockTransport(lambda r: Response(200))
    with Client(
        transport=transport,
        headers={"X-Trace-Id": "abc-123"},
        event_hooks={"request": [on_request], "response": []},
    ) as c:
        r = c.get("http://test-server/trace")
        assert r.status_code == 200
        assert "x-trace-id" in captured_headers
