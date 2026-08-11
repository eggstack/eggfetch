"""Tests for Timeout, Limits, Proxy, and status codes."""

import pytest

from eggfetch.compat.httpx import Client, Timeout, Limits, Proxy, codes, URL
from eggfetch.compat.httpx._status_codes import _StatusCodeGroup


# ── Timeout ─────────────────────────────────────────────────────────────

class TestTimeoutConstruction:
    def test_scalar_constructor(self):
        t = Timeout(5.0)
        assert t.connect == 5.0
        assert t.read == 5.0
        assert t.write == 5.0
        assert t.pool == 5.0
        assert t.total == 5.0

    def test_per_phase_constructor(self):
        t = Timeout(10.0, connect=1.0, read=2.0, write=3.0, pool=4.0)
        assert t.connect == 1.0
        assert t.read == 2.0
        assert t.write == 3.0
        assert t.pool == 4.0
        assert t.total == 10.0

    def test_default_timeout(self):
        t = Timeout()
        assert t.total == 5.0

    def test_integer_value(self):
        t = Timeout(3)
        assert t.connect == 3
        assert t.read == 3


class TestTimeoutProperties:
    def test_as_dict(self):
        t = Timeout(1.0, connect=2.0)
        d = t.as_dict
        assert d["connect"] == 2.0
        assert d["read"] == 1.0
        assert d["write"] == 1.0
        assert d["pool"] == 1.0


class TestTimeoutValidation:
    def test_negative_raises(self):
        with pytest.raises(ValueError, match="positive"):
            Timeout(-1.0)

    def test_non_number_raises(self):
        with pytest.raises(TypeError):
            Timeout("five")


class TestTimeoutEq:
    def test_equal(self):
        assert Timeout(5.0) == Timeout(5.0)

    def test_not_equal(self):
        assert Timeout(5.0) != Timeout(10.0)

    def test_not_equal_to_non_timeout(self):
        assert Timeout(5.0) != "not timeout"


class TestTimeoutRepr:
    def test_repr(self):
        t = Timeout(5.0)
        r = repr(t)
        assert "Timeout" in r
        assert "5.0" in r

    def test_repr_per_phase(self):
        t = Timeout(10.0, connect=1.0)
        r = repr(t)
        assert "connect=1.0" in r


class TestTimeoutCopy:
    def test_copy(self):
        import copy
        t = Timeout(1.0, connect=2.0)
        c = copy.copy(t)
        assert c == t
        assert c is not t

    def test_deepcopy(self):
        import copy
        t = Timeout(1.0, connect=2.0)
        c = copy.deepcopy(t)
        assert c == t


# ── Limits ──────────────────────────────────────────────────────────────

class TestLimitsConstruction:
    def test_defaults(self):
        lim = Limits()
        assert lim.max_connections is None
        assert lim.max_keepalive_connections is None
        assert lim.keepalive_expiry == 5.0

    def test_custom(self):
        lim = Limits(max_connections=100, max_keepalive_connections=20, keepalive_expiry=10.0)
        assert lim.max_connections == 100
        assert lim.max_keepalive_connections == 20
        assert lim.keepalive_expiry == 10.0


class TestLimitsEq:
    def test_equal(self):
        assert Limits(100, 20, 5.0) == Limits(100, 20, 5.0)

    def test_not_equal(self):
        assert Limits(100) != Limits(200)

    def test_not_equal_to_non_limits(self):
        assert Limits() != "not limits"


class TestLimitsRepr:
    def test_repr(self):
        lim = Limits(max_connections=100)
        r = repr(lim)
        assert "Limits" in r
        assert "100" in r


# ── Proxy ───────────────────────────────────────────────────────────────

class TestProxyConstruction:
    def test_from_string(self):
        p = Proxy("http://proxy.example.com:8080")
        assert str(p.url) == "http://proxy.example.com:8080"

    def test_from_url(self):
        u = URL("http://proxy.example.com:8080")
        p = Proxy(u)
        assert str(p.url) == "http://proxy.example.com:8080"

    def test_invalid_type_raises(self):
        with pytest.raises(TypeError):
            Proxy(123)

    def test_with_headers(self):
        p = Proxy("http://proxy.example.com", headers={"X-Key": "val"})
        assert p.headers == {"X-Key": "val"}

    def test_invalid_headers_type_raises(self):
        with pytest.raises(TypeError):
            Proxy("http://proxy.example.com", headers="not dict")

    def test_headers_are_rejected_before_native_dispatch(self):
        proxy = Proxy("http://proxy.example.com", headers={"X-Key": "val"})
        with pytest.raises(NotImplementedError, match="not yet"):
            with Client(proxy=proxy, trust_env=False):
                pass

    def test_with_auth(self):
        p = Proxy("http://proxy.example.com", auth=("user", "pass"))
        assert p.auth == ("user", "pass")

    def test_raw_auth_none(self):
        p = Proxy("http://proxy.example.com")
        assert p.raw_auth is None

    def test_raw_auth_tuple(self):
        p = Proxy("http://proxy.example.com", auth=("user", "pass"))
        assert p.raw_auth == ("user", "pass")

    def test_ssl_context_none(self):
        p = Proxy("http://proxy.example.com")
        assert p.ssl_context is None

    def test_ssl_context_stored(self):
        import ssl
        ctx = ssl.create_default_context()
        p = Proxy("http://proxy.example.com", ssl_context=ctx)
        assert p.ssl_context is ctx


class TestProxyRepr:
    def test_repr_no_creds(self):
        p = Proxy("http://proxy.example.com")
        r = repr(p)
        assert "Proxy" in r

    def test_repr_redacts_password(self):
        p = Proxy("http://user:secret@proxy.example.com")
        r = repr(p)
        assert "secret" not in r
        assert "***" in r

    def test_repr_with_auth(self):
        p = Proxy("http://proxy.example.com", auth=("user", "pass"))
        r = repr(p)
        assert "auth=***" in r


# ── Status codes ────────────────────────────────────────────────────────

class TestStatusCodes:
    def test_ok(self):
        assert codes.OK == 200

    def test_not_found(self):
        assert codes.NOT_FOUND == 404

    def test_internal_server_error(self):
        assert codes.INTERNAL_SERVER_ERROR == 500

    def test_bad_request(self):
        assert codes.BAD_REQUEST == 400

    def test_unauthorized(self):
        assert codes.UNAUTHORIZED == 401

    def test_forbidden(self):
        assert codes.FORBIDDEN == 403

    def test_moved_permanently(self):
        assert codes.MOVED_PERMANENTLY == 301

    def test_found(self):
        assert codes.FOUND == 302

    def test_service_unavailable(self):
        assert codes.SERVICE_UNAVAILABLE == 503

    def test_too_many_requests(self):
        assert codes.TOO_MANY_REQUESTS == 429

    def test_bad_gateway(self):
        assert codes.BAD_GATEWAY == 502

    def test_gateway_timeout(self):
        assert codes.GATEWAY_TIMEOUT == 504

    def test_created(self):
        assert codes.CREATED == 201

    def test_no_content(self):
        assert codes.NO_CONTENT == 204

    def test_continue(self):
        assert codes.CONTINUE == 100

    def test_invalid_code_raises(self):
        with pytest.raises(AttributeError):
            _ = codes.THIS_DOES_NOT_EXIST


class TestStatusCodesIntEnum:
    def test_is_int_enum(self):
        from enum import IntEnum
        assert issubclass(_StatusCodeGroup, IntEnum)

    def test_ok_is_int(self):
        assert isinstance(codes.OK, int)
        assert codes.OK == 200

    def test_int_conversion(self):
        assert int(codes.OK) == 200

    def test_construct_from_int(self):
        assert codes(200) == codes.OK

    def test_member_name(self):
        assert codes.OK.name == "OK"

    def test_member_value(self):
        assert codes.OK.value == 200

    def test_isinstance_int(self):
        assert isinstance(codes.NOT_FOUND, int)

    def test_comparison_with_int(self):
        assert codes.OK == 200
        assert codes.NOT_FOUND != 200
