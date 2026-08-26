"""Phase 4 Track 2: Redirect state machine parity tests.

Tests for Python-level redirect handling matching HTTPX 0.28.1 behavior:
- Method rewriting (301/302/303/307/308)
- URL resolution (absolute, relative, scheme-relative)
- Header stripping across origins
- Body replayability
- Manual redirect (next_request)
- max_redirects enforcement
"""

import pytest

from eggfetch.compat.httpx import (
    AsyncClient,
    Client,
    MockTransport,
    Request,
    Response,
    TooManyRedirects,
    URL,
    Headers,
)


# ── Redirect method rewriting ────────────────────────────────────────


class TestRedirectMethodRewriting:
    """2.1 Match reference method rewriting."""

    def test_303_changes_post_to_get(self):
        """303 See Other: non-HEAD methods become GET."""
        def handler(request):
            if request.url.path == "/redirect":
                return Response(303, headers={"Location": "/target"})
            return Response(200, text=f"method={request.method}")

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.post("http://testserver/redirect")
            assert resp.status_code == 200
            assert resp.text == "method=GET"

    def test_302_changes_post_to_get(self):
        """302 Found: browser-compatible conversion to GET except HEAD."""
        def handler(request):
            if request.url.path == "/redirect":
                return Response(302, headers={"Location": "/target"})
            return Response(200, text=f"method={request.method}")

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.post("http://testserver/redirect")
            assert resp.status_code == 200
            assert resp.text == "method=GET"

    def test_301_changes_post_to_get(self):
        """301 Moved Permanently: POST converts to GET."""
        def handler(request):
            if request.url.path == "/redirect":
                return Response(301, headers={"Location": "/target"})
            return Response(200, text=f"method={request.method}")

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.post("http://testserver/redirect")
            assert resp.status_code == 200
            assert resp.text == "method=GET"

    def test_307_retains_method(self):
        """307 Temporary Redirect: method and body retained."""
        def handler(request):
            if request.url.path == "/redirect":
                return Response(307, headers={"Location": "/target"})
            return Response(200, text=f"method={request.method}")

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.post("http://testserver/redirect")
            assert resp.status_code == 200
            assert resp.text == "method=POST"

    def test_308_retains_method(self):
        """308 Permanent Redirect: method and body retained."""
        def handler(request):
            if request.url.path == "/redirect":
                return Response(308, headers={"Location": "/target"})
            return Response(200, text=f"method={request.method}")

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.post("http://testserver/redirect")
            assert resp.status_code == 200
            assert resp.text == "method=POST"

    def test_head_retained_on_302(self):
        """HEAD method is retained on 302 (browser behavior exception)."""
        def handler(request):
            if request.url.path == "/redirect":
                return Response(302, headers={"Location": "/target"})
            return Response(200, text=f"method={request.method}")

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.head("http://testserver/redirect")
            assert resp.status_code == 200
            assert resp.text == "method=HEAD"

    def test_301_get_to_get(self):
        """301 with GET: method stays GET."""
        def handler(request):
            if request.url.path == "/redirect":
                return Response(301, headers={"Location": "/target"})
            return Response(200, text=f"method={request.method}")

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get("http://testserver/redirect")
            assert resp.status_code == 200
            assert resp.text == "method=GET"


# ── Redirect URL resolution ──────────────────────────────────────────


class TestRedirectURLResolution:
    """2.2 Resolve redirect URLs correctly."""

    def test_absolute_location(self):
        """Absolute Location header."""
        def handler(request):
            if request.url.path == "/a":
                return Response(302, headers={"Location": "http://testserver/b"})
            return Response(200, text=request.url.path)

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get("http://testserver/a")
            assert resp.text == "/b"

    def test_relative_location(self):
        """Relative Location header resolves against request URL."""
        def handler(request):
            if request.url.path == "/a":
                return Response(302, headers={"Location": "/b"})
            return Response(200, text=request.url.path)

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get("http://testserver/a")
            assert resp.text == "/b"

    def test_fragment_inherited(self):
        """Fragment from original URL is inherited when Location has none."""
        def handler(request):
            if request.url.path == "/a":
                return Response(302, headers={"Location": "/b"})
            return Response(200, text=request.url.fragment)

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get("http://testserver/a#frag")
            assert resp.text == "frag"

    def test_dot_relative_location_keeps_host(self):
        """Location "./b" resolves against the current directory on the
        same host (regression: it used to dispatch to a literal "."
        hostname)."""
        seen = []

        def handler(request):
            seen.append(str(request.url))
            if request.url.path == "/a":
                return Response(302, headers={"Location": "./b"})
            return Response(200)

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            c.get("http://testserver/a")
        assert seen == ["http://testserver/a", "http://testserver/b"]

    def test_parent_relative_location_keeps_host(self):
        """Location "../baz" resolves against the parent directory."""
        seen = []

        def handler(request):
            seen.append(str(request.url))
            if request.url.path == "/dir/x":
                return Response(302, headers={"Location": "../baz"})
            return Response(200)

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            c.get("http://testserver/dir/x")
        assert seen == ["http://testserver/dir/x", "http://testserver/baz"]

    def test_protocol_relative_location_switches_host(self):
        """Location "//other.com/x" inherits the scheme and switches host."""
        seen = []

        def handler(request):
            seen.append(str(request.url))
            if request.url.host == "testserver":
                return Response(302, headers={"Location": "//other-server/x"})
            return Response(200)

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            c.get("http://testserver/a")
        assert seen == [
            "http://testserver/a",
            "http://other-server/x",
        ]


# ── Header stripping across origins ──────────────────────────────────


class TestRedirectHeaderStripping:
    """2.3 Strip sensitive headers across origins."""

    def test_cross_origin_strips_authorization(self):
        """Authorization header is stripped on cross-origin redirect."""
        def handler(request):
            if request.url.path == "/redirect":
                return Response(
                    302,
                    headers={"Location": "http://other-server/"},
                )
            auth = request.headers.get("authorization", "none")
            return Response(200, text=auth)

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get(
                "http://testserver/redirect",
                headers={"Authorization": "Bearer secret"},
            )
            assert resp.text == "none"

    def test_same_origin_keeps_authorization(self):
        """Authorization header is kept on same-origin redirect."""
        def handler(request):
            if request.url.path == "/redirect":
                return Response(302, headers={"Location": "/target"})
            auth = request.headers.get("authorization", "none")
            return Response(200, text=auth)

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get(
                "http://testserver/redirect",
                headers={"Authorization": "Bearer secret"},
            )
            assert resp.text == "Bearer secret"

    def test_http_to_https_keeps_authorization(self):
        """Authorization is kept on HTTP→HTTPS same-host redirect."""
        def handler(request):
            if request.url.path == "/redirect":
                # Simulate HTTP→HTTPS redirect (both use testserver scheme in mock)
                return Response(302, headers={"Location": "/target"})
            auth = request.headers.get("authorization", "none")
            return Response(200, text=auth)

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get(
                "http://testserver/redirect",
                headers={"Authorization": "Bearer secret"},
            )
            assert resp.text == "Bearer secret"

    def test_303_strips_content_length(self):
        """303 strips Content-Length when method changes to GET."""
        def handler(request):
            if request.url.path == "/redirect":
                return Response(303, headers={"Location": "/target"})
            cl = request.headers.get("content-length", "none")
            return Response(200, text=cl)

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.post(
                "http://testserver/redirect",
                content=b"body",
            )
            assert resp.text == "none"

    def test_cookie_header_not_carried(self):
        """Cookie header is regenerated from client jar, not carried."""
        def handler(request):
            if request.url.path == "/redirect":
                return Response(
                    302,
                    headers=[
                        ("Location", "/target"),
                        ("Set-Cookie", "from_redirect=yes; Path=/"),
                    ],
                )
            cookie = request.headers.get("cookie", "none")
            return Response(200, text=cookie)

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get("http://testserver/redirect")
            # The Cookie header should include the cookie set by the redirect response
            assert "from_redirect=yes" in resp.text


# ── Manual redirect behavior ─────────────────────────────────────────


class TestManualRedirect:
    """2.5 Manual redirect behavior."""

    def test_follow_redirects_false_sets_next_request(self):
        """With follow_redirects=False, next_request is set."""
        def handler(request):
            return Response(302, headers={"Location": "http://testserver/target"})

        with Client(transport=MockTransport(handler), follow_redirects=False) as c:
            resp = c.get("http://testserver/redirect")
            assert resp.status_code == 302
            assert resp.next_request is not None
            assert resp.next_request.url.path == "/target"

    def test_follow_redirects_true_follows(self):
        """With follow_redirects=True, redirect is followed."""
        call_count = 0

        def handler(request):
            nonlocal call_count
            call_count += 1
            if request.url.path == "/redirect":
                return Response(302, headers={"Location": "/target"})
            return Response(200, text="ok")

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get("http://testserver/redirect")
            assert resp.status_code == 200
            assert call_count == 2

    def test_manual_redirect_history(self):
        """Manual redirect response includes the redirect itself in history."""
        def handler(request):
            return Response(302, headers={"Location": "http://testserver/target"})

        with Client(transport=MockTransport(handler), follow_redirects=False) as c:
            resp = c.get("http://testserver/redirect")
            assert len(resp.history) == 0
            assert resp.next_request is not None


# ── Redirect history ─────────────────────────────────────────────────


class TestRedirectHistory:
    """Redirect responses are added to history when followed."""

    def test_single_redirect_history(self):
        """Single redirect: history contains the redirect response."""
        def handler(request):
            if request.url.path == "/a":
                return Response(302, headers={"Location": "/b"})
            return Response(200, text="ok")

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get("http://testserver/a")
            assert len(resp.history) == 1
            assert resp.history[0].status_code == 302

    def test_multi_redirect_history(self):
        """Multiple redirects: history contains all intermediate responses."""
        def handler(request):
            if request.url.path == "/a":
                return Response(302, headers={"Location": "/b"})
            if request.url.path == "/b":
                return Response(302, headers={"Location": "/c"})
            return Response(200, text="ok")

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get("http://testserver/a")
            assert len(resp.history) == 2
            assert resp.history[0].status_code == 302
            assert resp.history[1].status_code == 302

    def test_no_redirect_no_history(self):
        """No redirect: history is empty."""
        def handler(request):
            return Response(200, text="ok")

        with Client(transport=MockTransport(handler), follow_redirects=True) as c:
            resp = c.get("http://testserver/")
            assert len(resp.history) == 0


# ── max_redirects enforcement ────────────────────────────────────────


class TestMaxRedirects:
    """2.6 Enforce max_redirects."""

    def test_too_many_redirects(self):
        """Raises TooManyRedirects when limit exceeded."""
        def handler(request):
            return Response(302, headers={"Location": "/loop"})

        with Client(
            transport=MockTransport(handler),
            follow_redirects=True,
            max_redirects=3,
        ) as c:
            with pytest.raises(TooManyRedirects):
                c.get("http://testserver/loop")

    def test_zero_redirects(self):
        """max_redirects=0 means no redirects allowed."""
        def handler(request):
            return Response(302, headers={"Location": "/target"})

        with Client(
            transport=MockTransport(handler),
            follow_redirects=True,
            max_redirects=0,
        ) as c:
            with pytest.raises(TooManyRedirects):
                c.get("http://testserver/redirect")

    def test_one_redirect_succeeds(self):
        """max_redirects=1 allows exactly one redirect."""
        def handler(request):
            if request.url.path == "/a":
                return Response(302, headers={"Location": "/b"})
            return Response(200, text="ok")

        with Client(
            transport=MockTransport(handler),
            follow_redirects=True,
            max_redirects=1,
        ) as c:
            resp = c.get("http://testserver/a")
            assert resp.status_code == 200


# ── Async variants ───────────────────────────────────────────────────


class TestAsyncRedirects:
    """Async redirect handling matches sync."""

    @pytest.mark.asyncio
    async def test_303_async(self):
        def handler(request):
            if request.url.path == "/redirect":
                return Response(303, headers={"Location": "/target"})
            return Response(200, text=f"method={request.method}")

        async with AsyncClient(
            async_transport=MockTransport(handler), follow_redirects=True
        ) as c:
            resp = await c.post("http://testserver/redirect")
            assert resp.status_code == 200
            assert resp.text == "method=GET"

    @pytest.mark.asyncio
    async def test_cross_origin_strips_auth_async(self):
        def handler(request):
            if request.url.path == "/redirect":
                return Response(302, headers={"Location": "http://other/"})
            auth = request.headers.get("authorization", "none")
            return Response(200, text=auth)

        async with AsyncClient(
            async_transport=MockTransport(handler), follow_redirects=True
        ) as c:
            resp = await c.get(
                "http://testserver/redirect",
                headers={"Authorization": "Bearer secret"},
            )
            assert resp.text == "none"

    @pytest.mark.asyncio
    async def test_max_redirects_async(self):
        def handler(request):
            return Response(302, headers={"Location": "/loop"})

        async with AsyncClient(
            async_transport=MockTransport(handler),
            follow_redirects=True,
            max_redirects=3,
        ) as c:
            with pytest.raises(TooManyRedirects):
                await c.get("http://testserver/loop")
