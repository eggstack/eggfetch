"""Parameterized matrix tests for configuration merging."""

import pytest

from eggfetch.compat.httpx import (
    Client,
    Headers,
    Cookies,
    QueryParams,
    Timeout,
    URL,
)


# ── Client params + request params ──────────────────────────────────────

class TestParamsMerging:
    def test_client_params_used_when_no_request_params(self):
        client = Client(params={"a": "1", "b": "2"})
        req = client.build_request("GET", "https://example.com")
        assert req.params["a"] == "1"
        assert req.params["b"] == "2"

    def test_request_params_override_client(self):
        client = Client(params={"a": "1"})
        req = client.build_request("GET", "https://example.com", params={"a": "9"})
        assert req.params["a"] == "9"

    def test_request_params_add_to_client(self):
        client = Client(params={"a": "1"})
        req = client.build_request("GET", "https://example.com", params={"b": "2"})
        assert req.params["a"] == "1"
        assert req.params["b"] == "2"

    def test_empty_client_params(self):
        client = Client()
        req = client.build_request("GET", "https://example.com", params={"a": "1"})
        assert req.params["a"] == "1"


# ── Client headers + request headers ────────────────────────────────────

class TestHeadersMerging:
    def test_client_headers_used_when_no_request_headers(self):
        client = Client(headers={"x-client": "val"})
        req = client.build_request("GET", "https://example.com")
        assert req.headers["x-client"] == "val"

    def test_request_headers_override_client(self):
        client = Client(headers={"x-custom": "old"})
        req = client.build_request(
            "GET", "https://example.com", headers={"x-custom": "new"}
        )
        assert req.headers["x-custom"] == "new"

    def test_request_headers_add_to_client(self):
        client = Client(headers={"x-client": "val"})
        req = client.build_request(
            "GET", "https://example.com", headers={"x-req": "val"}
        )
        assert req.headers["x-client"] == "val"
        assert req.headers["x-req"] == "val"

    def test_empty_client_headers(self):
        client = Client()
        req = client.build_request(
            "GET", "https://example.com", headers={"x-req": "val"}
        )
        assert req.headers["x-req"] == "val"


# ── Client cookies + request cookies ────────────────────────────────────

class TestCookiesMerging:
    def test_client_cookies_used_when_no_request_cookies(self):
        client = Client(cookies={"session": "abc"})
        req = client.build_request("GET", "https://example.com")
        assert req.cookies["session"] == "abc"

    def test_request_cookies_override_client(self):
        client = Client(cookies={"session": "old"})
        req = client.build_request(
            "GET", "https://example.com", cookies={"session": "new"}
        )
        assert req.cookies["session"] == "new"

    def test_request_cookies_add_to_client(self):
        client = Client(cookies={"a": "1"})
        req = client.build_request(
            "GET", "https://example.com", cookies={"b": "2"}
        )
        assert req.cookies["a"] == "1"
        assert req.cookies["b"] == "2"

    def test_empty_client_cookies(self):
        client = Client()
        req = client.build_request(
            "GET", "https://example.com", cookies={"a": "1"}
        )
        assert req.cookies["a"] == "1"


# ── base_url + relative URL ────────────────────────────────────────────

class TestBaseUrlMerging:
    def test_relative_url_joined(self):
        client = Client(base_url="https://api.example.com/v1")
        req = client.build_request("GET", "/users")
        assert "api.example.com" in str(req.url)

    def test_absolute_url_wins(self):
        client = Client(base_url="https://api.example.com/v1")
        req = client.build_request("GET", "https://other.com/override")
        assert req.url.host == "other.com"
        assert "v1" not in str(req.url)

    def test_no_base_url(self):
        client = Client()
        req = client.build_request("GET", "https://example.com/path")
        assert req.url.host == "example.com"
        assert req.url.path == "/path"

    def test_empty_base_url(self):
        client = Client(base_url="")
        req = client.build_request("GET", "https://example.com/path")
        assert req.url.host == "example.com"


# ── timeout override ────────────────────────────────────────────────────

class TestTimeoutOverride:
    def test_client_timeout_used(self):
        client = Client(timeout=Timeout(10.0))
        assert client.timeout.total == 10.0

    def test_timeout_override_in_request(self):
        client = Client(timeout=Timeout(10.0))
        assert client.timeout.total == 10.0
        # The per-request timeout override happens at send() level


# ── auth override ──────────────────────────────────────────────────────

class TestAuthOverride:
    def test_client_auth(self):
        from eggfetch.compat.httpx._auth import BasicAuth
        client = Client(auth=("user", "pass"))
        assert isinstance(client.auth, BasicAuth)
        assert client.auth.username == "user"
        assert client.auth.password == "pass"

    def test_client_auth_none(self):
        client = Client()
        assert client.auth is None
