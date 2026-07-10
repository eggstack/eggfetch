"""Tests for the eggfetch Python redirect engine (Milestone J)."""

import http.server
import threading
import urllib.parse

import pytest

import eggfetch


# ---------------------------------------------------------------------------
# Test server with redirect endpoints
# ---------------------------------------------------------------------------


class _RedirectHandler(http.server.BaseHTTPRequestHandler):
    """Test server that supports redirect endpoints."""

    def _send_final(self):
        body = b"final destination"
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        base = f"http://127.0.0.1:{self.server.server_address[1]}/final"

        redirect_map = {
            "/redirect-301": 301,
            "/redirect-302": 302,
            "/redirect-303": 303,
            "/redirect-307": 307,
            "/redirect-308": 308,
        }

        if path in redirect_map:
            self.send_response(redirect_map[path])
            self.send_header("Location", base)
            self.end_headers()

        elif path == "/redirect-loop":
            self.send_response(302)
            self.send_header("Location", base.replace("/final", "/redirect-loop"))
            self.end_headers()

        elif path == "/redirect-no-location":
            self.send_response(302)
            self.end_headers()

        elif path == "/redirect-chain-a":
            self.send_response(302)
            self.send_header(
                "Location",
                f"http://127.0.0.1:{self.server.server_address[1]}/redirect-chain-b",
            )
            self.end_headers()

        elif path == "/redirect-chain-b":
            self.send_response(302)
            self.send_header("Location", base)
            self.end_headers()

        elif path == "/final":
            self._send_final()

        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        base = f"http://127.0.0.1:{self.server.server_address[1]}/final"

        post_redirect_map = {
            "/redirect-post-301": 301,
            "/redirect-post-303": 303,
            "/redirect-post-307": 307,
        }

        if path in post_redirect_map:
            self.send_response(post_redirect_map[path])
            self.send_header("Location", base)
            self.end_headers()

        elif path == "/final":
            self._send_final()

        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        pass  # Suppress request logging


@pytest.fixture(scope="module")
def redirect_server():
    """Start a test server with redirect endpoints."""
    server = http.server.HTTPServer(("127.0.0.1", 0), _RedirectHandler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    yield f"http://127.0.0.1:{port}"
    server.shutdown()


# ---------------------------------------------------------------------------
# Sync redirect tests
# ---------------------------------------------------------------------------


class TestSyncRedirectDefault:
    """By default, redirects are NOT followed."""

    def test_default_no_follow(self, redirect_server):
        resp = eggfetch.get(f"{redirect_server}/redirect-302")
        assert resp.status_code == 302
        assert resp.headers.get("location") is not None


class TestSyncFollowRedirects:
    """Test follow_redirects=True follows redirects."""

    def test_follow_301(self, redirect_server):
        resp = eggfetch.get(
            f"{redirect_server}/redirect-301", follow_redirects=True
        )
        assert resp.status_code == 200
        assert resp.text == "final destination"
        assert len(resp.history) == 1
        assert resp.history[0].status_code == 301

    def test_follow_302(self, redirect_server):
        resp = eggfetch.get(
            f"{redirect_server}/redirect-302", follow_redirects=True
        )
        assert resp.status_code == 200
        assert resp.text == "final destination"
        assert len(resp.history) == 1

    def test_follow_303(self, redirect_server):
        resp = eggfetch.get(
            f"{redirect_server}/redirect-303", follow_redirects=True
        )
        assert resp.status_code == 200
        assert resp.text == "final destination"
        assert len(resp.history) == 1

    def test_follow_307(self, redirect_server):
        resp = eggfetch.get(
            f"{redirect_server}/redirect-307", follow_redirects=True
        )
        assert resp.status_code == 200
        assert resp.text == "final destination"
        assert len(resp.history) == 1

    def test_follow_308(self, redirect_server):
        resp = eggfetch.get(
            f"{redirect_server}/redirect-308", follow_redirects=True
        )
        assert resp.status_code == 200
        assert resp.text == "final destination"
        assert len(resp.history) == 1


class TestSyncRedirectChain:
    """Test multi-hop redirect chains."""

    def test_two_hop_chain(self, redirect_server):
        resp = eggfetch.get(
            f"{redirect_server}/redirect-chain-a", follow_redirects=True
        )
        assert resp.status_code == 200
        assert resp.text == "final destination"
        assert len(resp.history) == 2
        assert resp.history[0].status_code == 302
        assert resp.history[1].status_code == 302

    def test_final_url_is_correct(self, redirect_server):
        resp = eggfetch.get(
            f"{redirect_server}/redirect-chain-a", follow_redirects=True
        )
        assert resp.url.endswith("/final")


class TestSyncMaxRedirects:
    """Test max_redirects limit."""

    def test_too_many_redirects(self, redirect_server):
        with pytest.raises(eggfetch.TooManyRedirects):
            eggfetch.get(
                f"{redirect_server}/redirect-loop",
                follow_redirects=True,
                max_redirects=3,
            )

    def test_max_redirects_respected(self, redirect_server):
        with pytest.raises(eggfetch.TooManyRedirects):
            eggfetch.get(
                f"{redirect_server}/redirect-loop",
                follow_redirects=True,
                max_redirects=1,
            )

    def test_enough_max_redirects(self, redirect_server):
        resp = eggfetch.get(
            f"{redirect_server}/redirect-chain-a",
            follow_redirects=True,
            max_redirects=5,
        )
        assert resp.status_code == 200


class TestSyncNoLocation:
    """Test redirect response without Location header."""

    def test_no_location_returns_response(self, redirect_server):
        resp = eggfetch.get(
            f"{redirect_server}/redirect-no-location", follow_redirects=True
        )
        assert resp.status_code == 302


class TestSyncClientRedirect:
    """Test redirect config on Client constructor."""

    def test_client_follow_redirects(self, redirect_server):
        with eggfetch.Client(follow_redirects=True) as client:
            resp = client.get(f"{redirect_server}/redirect-302")
            assert resp.status_code == 200
            assert resp.text == "final destination"
            assert len(resp.history) == 1

    def test_client_max_redirects(self, redirect_server):
        with eggfetch.Client(follow_redirects=True, max_redirects=1) as client:
            with pytest.raises(eggfetch.TooManyRedirects):
                client.get(f"{redirect_server}/redirect-chain-a")

    def test_client_per_request_override(self, redirect_server):
        with eggfetch.Client() as client:
            # Client default: no follow
            resp = client.get(f"{redirect_server}/redirect-302")
            assert resp.status_code == 302

            # Per-request override: follow
            resp = client.get(
                f"{redirect_server}/redirect-302", follow_redirects=True
            )
            assert resp.status_code == 200


class TestSyncRedirectHistory:
    """Test that redirect history is properly populated."""

    def test_history_entries_have_status(self, redirect_server):
        resp = eggfetch.get(
            f"{redirect_server}/redirect-chain-a", follow_redirects=True
        )
        assert len(resp.history) == 2
        for entry in resp.history:
            assert hasattr(entry, "status_code")
            assert entry.status_code in (301, 302, 303, 307, 308)

    def test_history_entries_have_headers(self, redirect_server):
        resp = eggfetch.get(
            f"{redirect_server}/redirect-chain-a", follow_redirects=True
        )
        for entry in resp.history:
            assert hasattr(entry, "headers")

    def test_history_entries_have_url(self, redirect_server):
        resp = eggfetch.get(
            f"{redirect_server}/redirect-chain-a", follow_redirects=True
        )
        for entry in resp.history:
            assert hasattr(entry, "url")


class TestSyncPostRedirect:
    """Test POST redirect behavior (method rewrite)."""

    def test_post_301_becomes_get(self, redirect_server):
        resp = eggfetch.post(
            f"{redirect_server}/redirect-post-301",
            follow_redirects=True,
        )
        assert resp.status_code == 200
        assert resp.text == "final destination"

    def test_post_303_becomes_get(self, redirect_server):
        resp = eggfetch.post(
            f"{redirect_server}/redirect-post-303",
            follow_redirects=True,
        )
        assert resp.status_code == 200
        assert resp.text == "final destination"

    def test_post_307_preserves_post(self, redirect_server):
        resp = eggfetch.post(
            f"{redirect_server}/redirect-post-307",
            content=b"hello",
            follow_redirects=True,
        )
        assert resp.status_code == 200
        assert resp.text == "final destination"
