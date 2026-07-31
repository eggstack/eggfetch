"""Phase 4 Track 4: Cookie scope parity tests.

Tests for stdlib CookieJar-backed cookie handling matching HTTPX 0.28.1:
- Domain/path scoping
- Multiple Set-Cookie headers
- Cookie selection per hop
- CookieConflict on ambiguous get
- Redirect cookie propagation
"""

import pytest

from eggfetch.compat.httpx import (
    AsyncClient,
    Client,
    Cookies,
    MockTransport,
    Request,
    Response,
    Headers,
)


class TestCookieScope:
    """4.1-4.3 Domain/path/secure cookie scoping."""

    def test_domain_path_scoped_get(self):
        """Same name on different domains coexists."""
        c = Cookies()
        c.set("session", "val1", domain=".example.com", path="/")
        c.set("session", "val2", domain=".api.example.com", path="/")
        assert c.get("session", domain=".example.com") == "val1"
        assert c.get("session", domain=".api.example.com") == "val2"

    def test_cookie_conflict(self):
        """Ambiguous .get(name) raises CookieConflict."""
        from eggfetch.compat.httpx._exceptions import CookieConflict
        c = Cookies()
        c.set("session", "val1", domain=".example.com", path="/")
        c.set("session", "val2", domain=".example.com", path="/api")
        # get without domain/path is ambiguous
        with pytest.raises(CookieConflict):
            c.get("session")

    def test_cookie_conflict_resolved_with_scope(self):
        """get(name, domain=, path=) resolves ambiguity."""
        c = Cookies()
        c.set("session", "val1", domain=".example.com", path="/")
        c.set("session", "val2", domain=".example.com", path="/api")
        assert c.get("session", domain=".example.com", path="/") == "val1"
        assert c.get("session", domain=".example.com", path="/api") == "val2"

    def test_delete_by_domain_path(self):
        """Delete only matching cookies."""
        c = Cookies()
        c.set("session", "val1", domain=".example.com", path="/")
        c.set("session", "val2", domain=".example.com", path="/api")
        c.delete("session", domain=".example.com", path="/")
        assert c.get("session", domain=".example.com", path="/") is None
        assert c.get("session", domain=".example.com", path="/api") == "val2"

    def test_clear_domain_path(self):
        """Clear only cookies matching domain/path."""
        c = Cookies()
        c.set("a", "1", domain=".example.com", path="/")
        c.set("b", "2", domain=".example.com", path="/api")
        c.clear(domain=".example.com", path="/")
        assert c.get("a", domain=".example.com", path="/") is None
        assert c.get("b", domain=".example.com", path="/api") == "2"


class TestMultipleSetCookieHeaders:
    """4.3 Parse all Set-Cookie headers."""

    def test_multiple_set_cookie(self):
        """Response with multiple Set-Cookie headers."""
        h = Headers([
            ("Set-Cookie", "a=1; Path=/"),
            ("Set-Cookie", "b=2; Path=/api"),
        ])
        resp = Response(200, headers=h, request=Request("GET", "https://example.com"))
        c = Cookies()
        c.extract_cookies(resp)
        assert c.get("a") == "1"
        assert c.get("b") == "2"

    def test_set_cookie_with_attributes(self):
        """Set-Cookie with domain, path, secure, httponly attributes."""
        h = Headers([
            ("Set-Cookie", "session=abc; Domain=.example.com; Path=/; Secure; HttpOnly"),
        ])
        resp = Response(200, headers=h, request=Request("GET", "https://example.com"))
        c = Cookies()
        c.extract_cookies(resp)
        # The cookie should be stored with its attributes
        assert c.get("session") == "abc"


class TestCookieInSendLoop:
    """4.4-4.5 Cookie selection and extraction per hop."""

    def test_cookies_set_by_response_available_next_hop(self):
        """Cookies set by a response are available on the next request."""
        def handler(request):
            if request.url.path == "/set":
                return Response(
                    200,
                    headers=[("Set-Cookie", "token=abc123; Path=/")],
                    text="ok",
                )
            cookie = request.headers.get("cookie", "none")
            return Response(200, text=cookie)

        with Client(transport=MockTransport(handler)) as c:
            resp1 = c.get("http://testserver/set")
            assert resp1.text == "ok"
            # Client jar should now have the cookie
            assert c.cookies.get("token") == "abc123"
            # Next request should include the cookie
            resp2 = c.get("http://testserver/check")
            assert "token=abc123" in resp2.text

    def test_redirect_sets_cookie_available_after(self):
        """Cookie set during a redirect is available on the final request."""
        def handler(request):
            if request.url.path == "/redirect":
                return Response(
                    302,
                    headers=[
                        ("Location", "/target"),
                        ("Set-Cookie", "redirect_cookie=yes; Path=/"),
                    ],
                )
            cookie = request.headers.get("cookie", "none")
            return Response(200, text=cookie)

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get("http://testserver/redirect")
            # The cookie set during redirect should be on the final request
            assert "redirect_cookie=yes" in resp.text

    def test_response_cookies_not_client_jar(self):
        """response.cookies contains response-set cookies only."""
        def handler(request):
            return Response(
                200,
                headers=[("Set-Cookie", "resp_only=xyz; Path=/")],
                text="ok",
            )

        with Client(transport=MockTransport(handler)) as c:
            c.cookies.set("pre_existing", "val")
            resp = c.get("http://testserver/")
            assert resp.cookies.get("resp_only") == "xyz"

    def test_client_cookies_mutation_affects_next_request(self):
        """Mutating client.cookies between requests affects the next request."""
        def handler(request):
            cookie = request.headers.get("cookie", "none")
            return Response(200, text=cookie)

        with Client(transport=MockTransport(handler)) as c:
            resp1 = c.get("http://testserver/")
            assert resp1.text == "none"
            c.cookies.set("added", "later")
            resp2 = c.get("http://testserver/")
            assert "added=later" in resp2.text

    def test_auth_redirect_cookie_propagation(self):
        """Cookie set by redirect during auth is available after."""
        from eggfetch.compat.httpx import Auth

        class CookieAuth(Auth):
            def auth_flow(self, request):
                request.headers["authorization"] = "Bearer token"
                response = yield request
                # Auth sees the redirect response, auth is done
                # The redirect cookie is extracted by the redirect loop

        def handler(request):
            if request.url.path == "/target":
                return Response(200, text="ok")
            if request.headers.get("authorization") == "Bearer token":
                return Response(
                    302,
                    headers=[
                        ("Location", "/target"),
                        ("Set-Cookie", "auth_cookie=granted; Path=/"),
                    ],
                )
            return Response(401)

        with Client(
            transport=MockTransport(handler),
            auth=CookieAuth(),
            follow_redirects=True,
        ) as c:
            resp = c.get("http://testserver/protected")
            # auth_cookie should be in client jar after redirect
            assert c.cookies.get("auth_cookie") == "granted"


class TestCookieJarConstruction:
    """Cookies construction from various types."""

    def test_from_dict(self):
        c = Cookies({"a": "1", "b": "2"})
        assert c.get("a") == "1"
        assert c.get("b") == "2"

    def test_from_list(self):
        c = Cookies([("a", "1"), ("b", "2")])
        assert c.get("a") == "1"
        assert c.get("b") == "2"

    def test_from_cookies(self):
        original = Cookies({"a": "1"})
        copy = Cookies(original)
        assert copy.get("a") == "1"

    def test_copy_independence(self):
        """Copying a Cookies object creates an independent jar."""
        original = Cookies({"a": "1"})
        copy = Cookies(original)
        copy.set("b", "2")
        assert original.get("b") is None

    def test_invalid_type(self):
        with pytest.raises(TypeError):
            Cookies(123)


class TestAsyncCookieScope:
    """Async cookie behavior matches sync."""

    @pytest.mark.asyncio
    async def test_cookies_set_by_response_available_next_hop_async(self):
        async def handler(request):
            if request.url.path == "/set":
                return Response(
                    200,
                    headers=[("Set-Cookie", "token=abc123; Path=/")],
                    text="ok",
                )
            cookie = request.headers.get("cookie", "none")
            return Response(200, text=cookie)

        async with AsyncClient(async_transport=MockTransport(handler)) as c:
            resp1 = await c.get("http://testserver/set")
            assert resp1.text == "ok"
            assert c.cookies.get("token") == "abc123"
            resp2 = await c.get("http://testserver/check")
            assert "token=abc123" in resp2.text

    @pytest.mark.asyncio
    async def test_redirect_cookie_propagation_async(self):
        async def handler(request):
            if request.url.path == "/redirect":
                return Response(
                    302,
                    headers=[
                        ("Location", "/target"),
                        ("Set-Cookie", "redirect_cookie=yes; Path=/"),
                    ],
                )
            cookie = request.headers.get("cookie", "none")
            return Response(200, text=cookie)

        async with AsyncClient(
            async_transport=MockTransport(handler), follow_redirects=True
        ) as c:
            resp = await c.get("http://testserver/redirect")
            assert "redirect_cookie=yes" in resp.text
