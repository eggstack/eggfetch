"""Tests for the eggfetch Python retry subsystem (Milestone U)."""

import http.server
import threading

import pytest

import eggfetch

from conftest import _ThreadingHTTPServer


# ---------------------------------------------------------------------------
# Test server with retry endpoints
# ---------------------------------------------------------------------------

_call_count = 0


class _RetryHandler(http.server.BaseHTTPRequestHandler):
    """Test server that supports retry-related endpoints."""

    def _send_json(self, code, body):
        import json

        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        global _call_count
        if self.path == "/fail-twice":
            _call_count += 1
            if _call_count <= 2:
                self._send_json(503, {"error": "unavailable"})
            else:
                self._send_json(200, {"ok": True})
        elif self.path == "/always-fail":
            _call_count += 1
            self._send_json(503, {"error": "unavailable"})
        elif self.path == "/ok":
            self._send_json(200, {"ok": True})
        elif self.path == "/retry-after":
            _call_count += 1
            if _call_count <= 2:
                self.send_response(503)
                self.send_header("Retry-After", "0")
                self.send_header("Content-Length", "0")
                self.end_headers()
            else:
                self._send_json(200, {"ok": True})
        else:
            self._send_json(404, {"error": "not found"})

    def do_POST(self):
        global _call_count
        if self.path == "/fail-once-post":
            _call_count += 1
            if _call_count == 1:
                self._send_json(503, {"error": "unavailable"})
            else:
                length = int(self.headers.get("Content-Length", 0))
                body = self.rfile.read(length)
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
        else:
            self._send_json(404, {"error": "not found"})

    def do_PUT(self):
        global _call_count
        if self.path == "/fail-once-put":
            _call_count += 1
            if _call_count == 1:
                self._send_json(503, {"error": "unavailable"})
            else:
                self._send_json(200, {"ok": True})
        else:
            self._send_json(404, {"error": "not found"})

    def log_message(self, format, *args):
        pass  # suppress noisy logs


@pytest.fixture(scope="module")
def server():
    global _call_count
    srv = _ThreadingHTTPServer(("127.0.0.1", 0), _RetryHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


@pytest.fixture(autouse=True)
def reset_count():
    global _call_count
    _call_count = 0
    yield
    _call_count = 0


# ---------------------------------------------------------------------------
# Retry class construction
# ---------------------------------------------------------------------------


class TestRetryConstruction:
    def test_default_retry(self):
        r = eggfetch.Retry()
        assert r.max_attempts == 1
        assert r.backoff_factor == 0.5

    def test_custom_retry(self):
        r = eggfetch.Retry(max_attempts=5, backoff_factor=1.0)
        assert r.max_attempts == 5
        assert r.backoff_factor == 1.0

    def test_retry_properties(self):
        r = eggfetch.Retry(
            max_attempts=4,
            backoff_factor=2.0,
            initial_delay=1.0,
            max_delay=60.0,
            respect_retry_after=True,
            allow_post=True,
            allow_put=True,
            allow_delete=True,
            allow_patch=True,
            max_elapsed=120.0,
        )
        assert r.max_attempts == 4
        assert r.backoff_factor == 2.0
        assert r.initial_delay == 1.0
        assert r.max_delay == 60.0
        assert r.respect_retry_after is True
        assert r.allow_post is True
        assert r.allow_put is True
        assert r.allow_delete is True
        assert r.allow_patch is True
        assert r.max_elapsed == 120.0

    def test_retry_repr(self):
        r = eggfetch.Retry(max_attempts=3, backoff_factor=0.5)
        assert "Retry(" in repr(r)
        assert "3" in repr(r)

    def test_retry_statuses_default(self):
        r = eggfetch.Retry()
        assert sorted(r.statuses) == [408, 429, 502, 503, 504]

    def test_retry_statuses_custom(self):
        r = eggfetch.Retry(statuses={500, 502})
        assert sorted(r.statuses) == [500, 502]

    def test_invalid_floats_raise_value_error(self):
        # Negative, NaN, and infinite values must raise ValueError rather
        # than panicking inside Duration::from_secs_f64.
        with pytest.raises(ValueError):
            eggfetch.Retry(max_delay=-1.0)
        with pytest.raises(ValueError):
            eggfetch.Retry(initial_delay=float("nan"))
        with pytest.raises(ValueError):
            eggfetch.Retry(initial_delay=float("inf"))
        with pytest.raises(ValueError):
            eggfetch.Retry(max_elapsed=-0.5)
        with pytest.raises(ValueError):
            eggfetch.Retry(max_elapsed=float("inf"))


# ---------------------------------------------------------------------------
# Sync retry tests
# ---------------------------------------------------------------------------


class TestSyncRetry:
    def test_no_retry_by_default(self, server):
        global _call_count
        with eggfetch.Client() as client:
            r = client.get(f"{server}/fail-twice")
            assert r.status_code == 503
            assert _call_count == 1

    def test_retry_succeeds_after_retries(self, server):
        global _call_count
        with eggfetch.Client() as client:
            r = client.get(f"{server}/fail-twice", retries=True)
            assert r.status_code == 200
            assert _call_count == 3

    def test_retry_gives_up_after_max_attempts(self, server):
        global _call_count
        r_obj = eggfetch.Retry(max_attempts=2)
        with eggfetch.Client() as client:
            r = client.get(f"{server}/always-fail", retries=r_obj)
            assert r.status_code == 503
            assert _call_count == 2

    def test_retry_false_disables(self, server):
        global _call_count
        with eggfetch.Client() as client:
            r = client.get(f"{server}/fail-twice", retries=False)
            assert r.status_code == 503
            assert _call_count == 1

    def test_retry_none_inherits_client(self, server):
        global _call_count
        r_obj = eggfetch.Retry(max_attempts=3)
        with eggfetch.Client(retries=r_obj) as client:
            r = client.get(f"{server}/fail-twice", retries=None)
            assert r.status_code == 200
            assert _call_count == 3


# ---------------------------------------------------------------------------
# Async retry tests
# ---------------------------------------------------------------------------


class TestAsyncRetry:
    async def test_async_no_retry_by_default(self, server):
        global _call_count
        async with eggfetch.AsyncClient() as client:
            r = await client.get(f"{server}/fail-twice")
            assert r.status_code == 503
            assert _call_count == 1

    async def test_async_retry_succeeds(self, server):
        global _call_count
        async with eggfetch.AsyncClient() as client:
            r = await client.get(f"{server}/fail-twice", retries=True)
            assert r.status_code == 200
            assert _call_count == 3

    async def test_async_retry_client_level(self, server):
        global _call_count
        r_obj = eggfetch.Retry(max_attempts=3)
        async with eggfetch.AsyncClient(retries=r_obj) as client:
            r = await client.get(f"{server}/fail-twice")
            assert r.status_code == 200
            assert _call_count == 3


# ---------------------------------------------------------------------------
# POST/PUT retry tests
# ---------------------------------------------------------------------------


class TestUnsafeMethodRetry:
    def test_post_not_retried_by_default(self, server):
        global _call_count
        with eggfetch.Client() as client:
            r = client.post(f"{server}/fail-once-post", retries=True)
            assert r.status_code == 503
            assert _call_count == 1

    def test_post_retried_when_allowed(self, server):
        global _call_count
        r_obj = eggfetch.Retry(max_attempts=2, allow_post=True)
        with eggfetch.Client() as client:
            r = client.post(f"{server}/fail-once-post", retries=r_obj)
            assert r.status_code == 200
            assert _call_count == 2

    def test_put_not_retried_by_default(self, server):
        global _call_count
        with eggfetch.Client() as client:
            r = client.put(f"{server}/fail-once-put", retries=True)
            assert r.status_code == 503
            assert _call_count == 1

    def test_put_retried_when_allowed(self, server):
        global _call_count
        r_obj = eggfetch.Retry(max_attempts=2, allow_put=True)
        with eggfetch.Client() as client:
            r = client.put(f"{server}/fail-once-put", retries=r_obj)
            assert r.status_code == 200
            assert _call_count == 2


# ---------------------------------------------------------------------------
# Client-level retry configuration
# ---------------------------------------------------------------------------


class TestClientRetryConfig:
    def test_client_retries_kwarg(self, server):
        global _call_count
        r_obj = eggfetch.Retry(max_attempts=3)
        with eggfetch.Client(retries=r_obj) as client:
            r = client.get(f"{server}/fail-twice")
            assert r.status_code == 200
            assert _call_count == 3

    def test_request_retry_overrides_client(self, server):
        global _call_count
        with eggfetch.Client() as client:
            r = client.get(f"{server}/fail-twice", retries=True)
            assert r.status_code == 200
            assert _call_count == 3

    def test_request_without_retry_disables_client(self, server):
        global _call_count
        r_obj = eggfetch.Retry(max_attempts=3)
        with eggfetch.Client(retries=r_obj) as client:
            r = client.get(f"{server}/fail-twice", retries=False)
            assert r.status_code == 503
            assert _call_count == 1


# ---------------------------------------------------------------------------
# Exception types
# ---------------------------------------------------------------------------


class TestRetryExceptions:
    def test_exception_types_exist(self):
        assert issubclass(eggfetch.BodyNotReplayableForRetry, Exception)
        assert issubclass(eggfetch.RetryBudgetExhausted, Exception)
        assert issubclass(eggfetch.RetryNotConfigured, Exception)
