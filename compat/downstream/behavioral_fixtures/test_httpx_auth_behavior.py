"""Category: custom-auth-flow — exercises httpx-auth.

httpx-auth is an authentication library implementing OAuth, Digest, and API
key flows on top of httpx.Auth. This fixture applies a real package auth
object, sends a request, and asserts the generated auth header.
"""

import httpx
import httpx_auth


def test_httpx_auth_basic():
    """httpx-auth Basic auth generates the correct Authorization header."""
    auth = httpx_auth.Basic("user", "pass")
    transport = httpx.MockTransport(
        lambda r: httpx.Response(200, json={"auth": r.headers.get("authorization", "")})
    )
    with httpx.Client(auth=auth, transport=transport) as c:
        r = c.get("http://test/auth")
        assert r.status_code == 200
        auth_header = r.json()["auth"]
        assert auth_header.startswith("Basic ")


def test_httpx_auth_api_key():
    """httpx-auth API key auth generates the correct header."""
    auth = httpx_auth.HeaderApiKey(
        api_key="secret-key",
        header_name="X-API-Key",
    )
    transport = httpx.MockTransport(
        lambda r: httpx.Response(200, json={"api_key": r.headers.get("x-api-key", "")})
    )
    with httpx.Client(auth=auth, transport=transport) as c:
        r = c.get("http://test/api")
        assert r.status_code == 200
        assert r.json()["api_key"] == "secret-key"


def test_httpx_auth_query():
    """httpx-auth query parameter auth appends the key to the URL."""
    auth = httpx_auth.QueryApiKey(
        api_key="query-secret",
        query_parameter_name="api_key",
    )
    transport = httpx.MockTransport(
        lambda r: httpx.Response(200, json={"path": str(r.url)})
    )
    with httpx.Client(auth=auth, transport=transport) as c:
        r = c.get("http://test/query")
        assert r.status_code == 200
        assert "api_key=query-secret" in r.json()["path"]
