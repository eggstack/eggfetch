"""Category: custom auth flow — exercises multi-yield auth."""

import sys

sys.path.insert(0, "crates/eggfetch-python/python")

from eggfetch.compat.httpx import Auth, Client, MockTransport, Request, Response


class SingleYieldAuth(Auth):
    """Auth that adds a token on the first request."""

    def auth_flow(self, request: Request):
        request.headers["Authorization"] = "Bearer initial-token"
        yield request


class RetryAuth(Auth):
    """Auth that retries with escalating tokens on 401."""

    def auth_flow(self, request: Request):
        request.headers["Authorization"] = "Bearer attempt-1"
        response = yield request
        if response.status_code == 401:
            request.headers["Authorization"] = "Bearer attempt-2"
            response = yield request
        if response.status_code == 401:
            request.headers["Authorization"] = "Bearer attempt-3"
            yield request


class ThreeYieldAuth(Auth):
    """Auth that always yields three times regardless of response."""

    def auth_flow(self, request: Request):
        request.headers["Authorization"] = "Bearer token-v1"
        yield request
        request.headers["Authorization"] = "Bearer token-v2"
        yield request
        request.headers["Authorization"] = "Bearer token-v3"
        yield request


def test_single_yield_auth():
    def handler(request: Request) -> Response:
        auth = request.headers.get("authorization", "")
        return Response(200, json={"auth": auth})

    transport = MockTransport(handler)
    with Client(transport=transport, auth=SingleYieldAuth()) as c:
        r = c.get("http://test-server/auth")
        assert r.status_code == 200
        assert r.json()["auth"] == "Bearer initial-token"


def test_retry_auth_succeeds_first():
    def handler(request: Request) -> Response:
        auth = request.headers.get("authorization", "")
        return Response(200, json={"auth": auth})

    transport = MockTransport(handler)
    with Client(transport=transport, auth=RetryAuth()) as c:
        r = c.get("http://test-server/auth")
        assert r.status_code == 200
        assert r.json()["auth"] == "Bearer attempt-1"


def test_retry_auth_escalates_on_401():
    attempts = []

    def handler(request: Request) -> Response:
        auth = request.headers.get("authorization", "")
        attempts.append(auth)
        if len(attempts) < 3:
            return Response(401)
        return Response(200, json={"auth": auth})

    transport = MockTransport(handler)
    with Client(transport=transport, auth=RetryAuth()) as c:
        r = c.get("http://test-server/auth")
        assert r.status_code == 200
        assert len(attempts) == 3
        assert attempts[0] == "Bearer attempt-1"
        assert attempts[1] == "Bearer attempt-2"
        assert attempts[2] == "Bearer attempt-3"


def test_three_yield_auth():
    attempts = []

    def handler(request: Request) -> Response:
        auth = request.headers.get("authorization", "")
        attempts.append(auth)
        return Response(200)

    transport = MockTransport(handler)
    with Client(transport=transport, auth=ThreeYieldAuth()) as c:
        r = c.get("http://test-server/auth")
        assert r.status_code == 200
        assert len(attempts) == 3
        assert attempts[0] == "Bearer token-v1"
        assert attempts[1] == "Bearer token-v2"
        assert attempts[2] == "Bearer token-v3"


def test_no_auth_no_header():
    def handler(request: Request) -> Response:
        auth = request.headers.get("authorization", "")
        return Response(200, json={"auth": auth})

    transport = MockTransport(handler)
    with Client(transport=transport) as c:
        r = c.get("http://test-server/auth")
        assert r.status_code == 200
        assert r.json()["auth"] == ""
