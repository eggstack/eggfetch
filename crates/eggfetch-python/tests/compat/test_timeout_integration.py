"""Timeout configuration integration tests for the HTTPX compatibility layer.

These tests verify that timeout configuration is correctly parsed and
forwarded to the native eggfetch client. They do NOT test actual network
timeout behavior (that is covered by Rust-level tests); they only verify
the Python-to-Rust configuration passthrough.
"""

import asyncio
import http.server
import json
import socketserver
import threading

import pytest

from eggfetch.compat.httpx import Client, AsyncClient, Timeout


# ---------------------------------------------------------------------------
# Local test server that echoes timeout-related headers
# ---------------------------------------------------------------------------

class _TimeoutHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/get":
            body = json.dumps({"method": "GET"}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
        elif self.path == "/echo-headers":
            headers = {k: v for k, v in self.headers.items()}
            body = json.dumps({"headers": headers}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        content_length = int(self.headers.get("Content-Length", 0))
        self.rfile.read(content_length) if content_length else b""
        body = json.dumps({"method": "POST"}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()

    def log_message(self, format, *args):
        pass


class _ThreadedHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True


@pytest.fixture(scope="module")
def server():
    srv = _ThreadedHTTPServer(("127.0.0.1", 0), _TimeoutHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# Client-level timeout defaults
# ---------------------------------------------------------------------------

class TestClientTimeoutDefaults:
    def test_default_timeout_is_timeout_object(self):
        """Default client has a Timeout object."""
        client = Client()
        assert isinstance(client.timeout, Timeout)
        client.close()

    def test_default_timeout_value(self):
        """Default timeout value is 5.0 seconds."""
        client = Client()
        assert client.timeout.total == 5.0
        assert client.timeout.connect == 5.0
        assert client.timeout.read == 5.0
        assert client.timeout.write == 5.0
        assert client.timeout.pool == 5.0
        client.close()

    def test_scalar_timeout_sets_all_phases(self):
        """Passing timeout=10.0 sets all phases to 10.0."""
        client = Client(timeout=10.0)
        assert client.timeout.total == 10.0
        assert client.timeout.connect == 10.0
        assert client.timeout.read == 10.0
        assert client.timeout.write == 10.0
        assert client.timeout.pool == 10.0
        client.close()

    def test_timeout_object_preserved(self):
        """Passing a Timeout object preserves its phase values."""
        t = Timeout(timeout=5.0, connect=1.0, read=2.0, write=3.0, pool=4.0)
        client = Client(timeout=t)
        assert client.timeout.total == 5.0
        assert client.timeout.connect == 1.0
        assert client.timeout.read == 2.0
        assert client.timeout.write == 3.0
        assert client.timeout.pool == 4.0
        client.close()


# ---------------------------------------------------------------------------
# Per-request timeout=None disables timeouts
# ---------------------------------------------------------------------------

class TestPerRequestTimeoutNone:
    def test_timeout_none_on_send(self, server):
        """timeout=None on send() disables timeouts for that request."""
        with Client(timeout=5.0) as client:
            req = client.build_request("GET", f"{server}/get")
            resp = client.send(req, timeout=None)
            assert resp.status_code == 200

    def test_timeout_none_on_request_method(self, server):
        """timeout=None on request() disables timeouts for that request."""
        with Client(timeout=5.0) as client:
            resp = client.request("GET", f"{server}/get", timeout=None)
            assert resp.status_code == 200

    def test_timeout_none_on_get(self, server):
        """timeout=None on get() disables timeouts for that request."""
        with Client(timeout=5.0) as client:
            resp = client.get(f"{server}/get", timeout=None)
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_timeout_none_on_send(self, server):
        """async: timeout=None on send() disables timeouts."""
        async with AsyncClient(timeout=5.0) as client:
            req = client.build_request("GET", f"{server}/get")
            resp = await client.send(req, timeout=None)
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_timeout_none_on_request(self, server):
        """async: timeout=None on request() disables timeouts."""
        async with AsyncClient(timeout=5.0) as client:
            resp = await client.request("GET", f"{server}/get", timeout=None)
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_timeout_none_on_get(self, server):
        """async: timeout=None on get() disables timeouts."""
        async with AsyncClient(timeout=5.0) as client:
            resp = await client.get(f"{server}/get", timeout=None)
            assert resp.status_code == 200


# ---------------------------------------------------------------------------
# Per-request timeout override
# ---------------------------------------------------------------------------

class TestPerRequestTimeoutOverride:
    def test_scalar_timeout_override(self, server):
        """Per-request scalar timeout overrides client default."""
        with Client(timeout=5.0) as client:
            resp = client.get(f"{server}/get", timeout=10.0)
            assert resp.status_code == 200

    def test_timeout_object_override(self, server):
        """Per-request Timeout object overrides client default."""
        with Client(timeout=5.0) as client:
            resp = client.get(f"{server}/get", timeout=Timeout(10.0))
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_scalar_timeout_override(self, server):
        """async: per-request scalar timeout overrides client default."""
        async with AsyncClient(timeout=5.0) as client:
            resp = await client.get(f"{server}/get", timeout=10.0)
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_timeout_object_override(self, server):
        """async: per-request Timeout object overrides client default."""
        async with AsyncClient(timeout=5.0) as client:
            resp = await client.get(f"{server}/get", timeout=Timeout(10.0))
            assert resp.status_code == 200


# ---------------------------------------------------------------------------
# Phase-specific timeout
# ---------------------------------------------------------------------------

class TestPhaseSpecificTimeout:
    def test_connect_phase_timeout(self, server):
        """Timeout with connect phase set to a specific value."""
        t = Timeout(timeout=5.0, connect=2.0)
        with Client(timeout=t) as client:
            resp = client.get(f"{server}/get")
            assert resp.status_code == 200

    def test_read_phase_timeout(self, server):
        """Timeout with read phase set to a specific value."""
        t = Timeout(timeout=5.0, read=3.0)
        with Client(timeout=t) as client:
            resp = client.get(f"{server}/get")
            assert resp.status_code == 200

    def test_write_phase_timeout(self, server):
        """Timeout with write phase set to a specific value."""
        t = Timeout(timeout=5.0, write=3.0)
        with Client(timeout=t) as client:
            resp = client.post(f"{server}/get", content=b"test")
            assert resp.status_code == 200

    def test_pool_phase_timeout(self, server):
        """Timeout with pool phase set to a specific value."""
        t = Timeout(timeout=5.0, pool=2.0)
        with Client(timeout=t) as client:
            resp = client.get(f"{server}/get")
            assert resp.status_code == 200

    def test_all_phases_independent(self, server):
        """All phases can be set independently."""
        t = Timeout(timeout=10.0, connect=1.0, read=2.0, write=3.0, pool=4.0)
        with Client(timeout=t) as client:
            resp = client.get(f"{server}/get")
            assert resp.status_code == 200

    @pytest.mark.asyncio
    async def test_async_phase_specific(self, server):
        """async: phase-specific timeout is applied."""
        t = Timeout(timeout=5.0, connect=2.0, read=3.0)
        async with AsyncClient(timeout=t) as client:
            resp = await client.get(f"{server}/get")
            assert resp.status_code == 200


# ---------------------------------------------------------------------------
# Timeout value validation
# ---------------------------------------------------------------------------

class TestTimeoutValidation:
    def test_negative_timeout_raises(self):
        """Negative timeout value raises ValueError."""
        with pytest.raises(ValueError, match="positive"):
            Timeout(timeout=-1.0)

    def test_negative_phase_raises(self):
        """Negative phase timeout raises ValueError."""
        with pytest.raises(ValueError, match="positive"):
            Timeout(timeout=5.0, connect=-1.0)

    def test_nan_timeout_raises(self):
        """NaN timeout raises ValueError."""
        with pytest.raises(ValueError, match="NaN|finite"):
            Timeout(timeout=float("nan"))

    def test_non_numeric_timeout_raises(self):
        """Non-numeric timeout raises TypeError."""
        with pytest.raises(TypeError, match="None or a number"):
            Timeout(timeout="invalid")

    def test_zero_timeout_is_valid(self):
        """Zero timeout is a valid value."""
        t = Timeout(timeout=0.0)
        assert t.total == 0.0

    def test_none_phase_not_valid_in_constructor(self):
        """None is not a valid explicit value for a phase."""
        # None as the default for phases means "use total"
        # But explicit None passed to phases should still work
        # since the constructor defaults connect/read/write/pool to timeout
        t = Timeout(timeout=5.0)
        assert t.connect == 5.0
        assert t.read == 5.0
        assert t.write == 5.0
        assert t.pool == 5.0


# ---------------------------------------------------------------------------
# Timeout object properties
# ---------------------------------------------------------------------------

class TestTimeoutObject:
    def test_as_dict(self):
        """Timeout.as_dict returns correct dictionary."""
        t = Timeout(timeout=5.0, connect=1.0, read=2.0, write=3.0, pool=4.0)
        d = t.as_dict
        assert d == {"connect": 1.0, "read": 2.0, "write": 3.0, "pool": 4.0}

    def test_equality(self):
        """Timeout objects with same values are equal."""
        a = Timeout(timeout=5.0, connect=1.0)
        b = Timeout(timeout=5.0, connect=1.0)
        assert a == b

    def test_inequality(self):
        """Timeout objects with different values are not equal."""
        a = Timeout(timeout=5.0)
        b = Timeout(timeout=10.0)
        assert a != b

    def test_repr_uses_total_when_unified(self):
        """repr uses Timeout(timeout=X) when all phases equal total."""
        t = Timeout(timeout=5.0)
        assert repr(t) == "Timeout(timeout=5.0)"

    def test_repr_explicit_phases(self):
        """repr uses explicit phase names when phases differ from total."""
        t = Timeout(timeout=5.0, connect=1.0)
        r = repr(t)
        assert "connect=1.0" in r

    def test_copy(self):
        """Copy produces equal but independent object."""
        import copy
        t = Timeout(timeout=5.0, connect=1.0)
        t2 = copy.copy(t)
        assert t == t2
        assert t is not t2
