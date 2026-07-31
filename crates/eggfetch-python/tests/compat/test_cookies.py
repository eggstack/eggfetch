"""Tests for Cookies compatibility."""

import pytest

from eggfetch.compat.httpx import Cookies, Headers, Request, Response


class TestCookiesConstruction:
    def test_empty(self):
        c = Cookies()
        assert len(c) == 0

    def test_from_dict(self):
        c = Cookies({"a": "1", "b": "2"})
        assert c["a"] == "1"
        assert c["b"] == "2"

    def test_from_list_of_tuples(self):
        c = Cookies([("a", "1"), ("b", "2")])
        assert c["a"] == "1"

    def test_from_cookies(self):
        original = Cookies({"a": "1"})
        copy = Cookies(original)
        assert copy["a"] == "1"

    def test_invalid_type_raises(self):
        with pytest.raises(TypeError):
            Cookies(123)


class TestCookiesMutation:
    def test_set(self):
        c = Cookies()
        c.set("a", "1")
        assert c["a"] == "1"

    def test_set_with_domain_path(self):
        c = Cookies()
        c.set("a", "1", domain="example.com", path="/")
        assert c["a"] == "1"

    def test_get(self):
        c = Cookies({"a": "1"})
        assert c.get("a") == "1"
        assert c.get("missing") is None
        assert c.get("missing", "fallback") == "fallback"

    def test_delete(self):
        c = Cookies({"a": "1"})
        c.delete("a")
        assert "a" not in c

    def test_delete_with_domain_path(self):
        c = Cookies()
        c.set("a", "1", domain="example.com", path="/")
        c.delete("a", domain="example.com", path="/")
        assert "a" not in c

    def test_delete_with_domain_path_no_match(self):
        c = Cookies()
        c.set("a", "1", domain="example.com", path="/")
        c.delete("a", domain="other.com", path="/")
        assert "a" in c

    def test_clear(self):
        c = Cookies({"a": "1", "b": "2"})
        c.clear()
        assert len(c) == 0

    def test_update_dict(self):
        c = Cookies({"a": "1"})
        c.update({"a": "9", "b": "2"})
        assert c["a"] == "9"
        assert c["b"] == "2"

    def test_update_cookies(self):
        c = Cookies({"a": "1"})
        c.update(Cookies({"a": "9"}))
        assert c["a"] == "9"

    def test_update_list(self):
        c = Cookies()
        c.update([("a", "1")])
        assert c["a"] == "1"


class TestCookiesDunder:
    def test_setitem(self):
        c = Cookies()
        c["a"] = "1"
        assert c["a"] == "1"

    def test_getitem(self):
        c = Cookies({"a": "1"})
        assert c["a"] == "1"

    def test_getitem_missing(self):
        with pytest.raises(KeyError):
            Cookies()["missing"]

    def test_delitem(self):
        c = Cookies({"a": "1"})
        del c["a"]
        assert "a" not in c

    def test_contains(self):
        c = Cookies({"a": "1"})
        assert "a" in c
        assert "b" not in c

    def test_len(self):
        c = Cookies({"a": "1", "b": "2"})
        assert len(c) == 2

    def test_iter(self):
        c = Cookies({"a": "1", "b": "2"})
        keys = list(c)
        assert "a" in keys
        assert "b" in keys

    def test_bool_empty(self):
        assert not Cookies()

    def test_bool_nonempty(self):
        assert Cookies({"a": "1"})

    def test_items(self):
        c = Cookies({"a": "1"})
        assert list(c.items()) == [("a", "1")]

    def test_keys(self):
        c = Cookies({"a": "1", "b": "2"})
        assert set(c.keys()) == {"a", "b"}

    def test_values(self):
        c = Cookies({"a": "1", "b": "2"})
        assert set(c.values()) == {"1", "2"}

    def test_repr(self):
        c = Cookies({"a": "1"})
        assert "Cookies" in repr(c)


class TestCookiesSetDefault:
    def test_setdefault_existing(self):
        c = Cookies({"a": "1"})
        val = c.setdefault("a", "default")
        assert val == "1"

    def test_setdefault_missing(self):
        c = Cookies()
        val = c.setdefault("a", "default")
        assert val == "default"
        assert c["a"] == "default"

    def test_setdefault_none(self):
        c = Cookies()
        val = c.setdefault("a")
        assert val == ""


class TestCookiesFromRequest:
    def test_set_cookie_header(self):
        """set_cookie_header sets Cookie header on a request from the jar."""
        c = Cookies()
        c.set("session", "abc123", domain="example.com", path="/")
        c.set("theme", "dark", domain="example.com", path="/")

        h = Headers()
        req = Request("GET", "https://example.com/path", headers=h)
        c.set_cookie_header(req)

        cookie_header = req.headers.get("cookie") or req.headers.get("Cookie")
        assert cookie_header is not None
        assert "session=abc123" in cookie_header
        assert "theme=dark" in cookie_header

    def test_extract_cookies_from_response(self):
        """extract_cookies loads Set-Cookie headers from a response."""
        c = Cookies()
        h = Headers({"Set-Cookie": "session=abc123; Path=/"})
        resp = Response(200, headers=h, request=Request("GET", "https://example.com"))
        c.extract_cookies(resp)
        assert c.get("session") == "abc123"

    def test_extract_cookies_multiple_set_cookie(self):
        """extract_cookies handles multiple Set-Cookie headers."""
        c = Cookies()
        h = Headers([
            ("Set-Cookie", "a=1; Path=/"),
            ("Set-Cookie", "b=2; Path=/"),
        ])
        resp = Response(200, headers=h, request=Request("GET", "https://example.com"))
        c.extract_cookies(resp)
        assert c.get("a") == "1"
        assert c.get("b") == "2"
