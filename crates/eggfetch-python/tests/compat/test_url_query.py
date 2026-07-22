"""Comprehensive tests for URL and QueryParams."""

import pytest

from eggfetch.compat.httpx import URL, QueryParams


# ── URL construction ────────────────────────────────────────────────────

class TestURLConstruction:
    def test_from_string(self):
        url = URL("https://example.com/path")
        assert str(url) == "https://example.com/path"

    def test_from_bytes(self):
        url = URL(b"https://example.com/path")
        assert url.host == "example.com"

    def test_from_existing_url(self):
        a = URL("https://example.com/path")
        b = URL(a)
        assert a is b  # identity return for URL input

    def test_none_yields_http_prefix(self):
        url = URL(None)
        assert url._raw == ""

    def test_relative_string(self):
        url = URL("/path/to/resource")
        assert url.path == "/path/to/resource"

    def test_empty_string(self):
        url = URL("")
        assert url._raw == ""


# ── URL properties ──────────────────────────────────────────────────────

class TestURLProperties:
    def test_scheme(self):
        assert URL("https://example.com").scheme == "https"
        assert URL("http://example.com").scheme == "http"

    def test_host(self):
        assert URL("https://example.com").host == "example.com"
        assert URL("https://Example.COM").host == "example.com"

    def test_port_explicit(self):
        assert URL("https://example.com:8443").port == 8443

    def test_port_default_http(self):
        assert URL("http://example.com:80").port == 80

    def test_port_default_https(self):
        assert URL("https://example.com:443").port == 443

    def test_port_none_when_omitted(self):
        assert URL("https://example.com").port is None

    def test_path(self):
        assert URL("https://example.com/a/b/c").path == "/a/b/c"

    def test_path_empty(self):
        assert URL("https://example.com").path == ""

    def test_query(self):
        url = URL("https://example.com?q=1&r=2")
        assert url.query == b"q=1&r=2"

    def test_query_empty(self):
        assert URL("https://example.com").query == b""

    def test_fragment(self):
        assert URL("https://example.com/path#section").fragment == "section"

    def test_fragment_empty(self):
        assert URL("https://example.com").fragment == ""

    def test_username(self):
        assert URL("https://user@example.com").username == "user"

    def test_password(self):
        assert URL("https://user:pass@example.com").password == "pass"

    def test_netloc(self):
        url = URL("https://example.com:8443/path")
        assert url.netloc == b"example.com:8443"

    def test_userinfo_with_password(self):
        url = URL("https://user:pass@example.com")
        assert url.userinfo == b"user:pass"

    def test_userinfo_without_password(self):
        url = URL("https://user@example.com")
        assert url.userinfo == b"user"

    def test_userinfo_empty(self):
        url = URL("https://example.com")
        assert url.userinfo == b""

    def test_raw_host(self):
        assert URL("https://example.com").raw_host == b"example.com"

    def test_raw_host_none(self):
        assert URL("").raw_host is None

    def test_raw_path(self):
        assert URL("https://example.com/a/b").raw_path == b"/a/b"

    def test_raw_path_empty_defaults_slash(self):
        assert URL("https://example.com").raw_path == b"/"

    def test_raw_scheme(self):
        assert URL("https://example.com").raw_scheme == b"https"


# ── Absolute / relative ────────────────────────────────────────────────

class TestURLAbsoluteRelative:
    def test_is_absolute_url(self):
        assert URL("https://example.com").is_absolute_url is True
        assert URL("http://example.com/path").is_absolute_url is True

    def test_is_relative_url(self):
        assert URL("/path").is_relative_url is True

    def test_absolute_not_relative(self):
        url = URL("https://example.com")
        assert url.is_absolute_url is True
        assert url.is_relative_url is False

    def test_relative_not_absolute(self):
        url = URL("/path")
        assert url.is_relative_url is True
        assert url.is_absolute_url is False


# ── copy_with / param helpers ──────────────────────────────────────────

class TestURLCopyWith:
    def test_copy_with_params(self):
        url = URL("https://example.com/path")
        new = url.copy_with(params={"q": "1"})
        assert str(new) == "https://example.com/path?q=1"

    def test_copy_set_param(self):
        url = URL("https://example.com?a=1&b=2")
        new = url.copy_set_param("a", "9")
        assert "a=9" in str(new)
        assert "b=2" in str(new)

    def test_copy_remove_param(self):
        url = URL("https://example.com?a=1&b=2")
        new = url.copy_remove_param("a")
        assert "a=" not in str(new)
        assert "b=2" in str(new)

    def test_copy_merge_params(self):
        url = URL("https://example.com?a=1")
        new = url.copy_merge_params({"b": "2"})
        assert "a=1" in str(new)
        assert "b=2" in str(new)

    def test_copy_add_param(self):
        url = URL("https://example.com?a=1")
        new = url.copy_add_param("b", "2")
        assert "a=1" in str(new)
        assert "b=2" in str(new)


# ── URL join ────────────────────────────────────────────────────────────

class TestURLJoin:
    def test_join_relative(self):
        base = URL("https://example.com/a/b")
        joined = base.join(URL("c/d"))
        assert "c/d" in str(joined)

    def test_join_queryparams_obj(self):
        from eggfetch.compat.httpx import QueryParams
        url = URL("https://example.com/path")
        qp = QueryParams({"q": "test"})
        new = url.copy_with(params=qp)
        assert "q=test" in str(new)


# ── Default port stripping ─────────────────────────────────────────────

class TestURLDefaultPort:
    def test_http_80_stripped(self):
        url = URL("http://example.com:80/path")
        assert ":80" not in str(url)
        assert str(url) == "http://example.com/path"

    def test_https_443_stripped(self):
        url = URL("https://example.com:443/path")
        assert ":443" not in str(url)
        assert str(url) == "https://example.com/path"

    def test_non_default_port_kept(self):
        url = URL("http://example.com:8080/path")
        assert ":8080" in str(url)


# ── Credential redaction in repr ───────────────────────────────────────

class TestURLRepr:
    def test_password_redacted(self):
        url = URL("https://user:secret@example.com")
        r = repr(url)
        assert "secret" not in r
        assert "***" in r

    def test_no_password_no_redaction(self):
        url = URL("https://user@example.com")
        assert "user" in repr(url)


# ── IPv6 ────────────────────────────────────────────────────────────────

class TestURLIPv6:
    def test_ipv6_host(self):
        url = URL("https://[::1]:8443/path")
        assert url.host == "::1"
        assert url.port == 8443


# ── Unicode hosts ──────────────────────────────────────────────────────

class TestURLUnicode:
    def test_unicode_host(self):
        url = URL("https://münchen.de/path")
        assert "münchen" in (url.host or "")


# ── QueryParams construction ──────────────────────────────────────────

class TestQueryParamsConstruction:
    def test_from_dict(self):
        qp = QueryParams({"a": "1", "b": "2"})
        assert qp["a"] == "1"
        assert qp["b"] == "2"

    def test_from_list_of_tuples(self):
        qp = QueryParams([("a", "1"), ("b", "2")])
        assert qp["a"] == "1"
        assert qp["b"] == "2"

    def test_from_string(self):
        qp = QueryParams("a=1&b=2")
        assert qp["a"] == "1"
        assert qp["b"] == "2"

    def test_from_queryparams(self):
        original = QueryParams({"a": "1"})
        copy = QueryParams(original)
        assert copy["a"] == "1"
        assert copy is not original

    def test_from_none(self):
        qp = QueryParams(None)
        assert len(qp) == 0

    def test_invalid_type_raises(self):
        with pytest.raises(TypeError):
            QueryParams(123)

    def test_empty_string(self):
        qp = QueryParams("")
        assert len(qp) == 0


# ── QueryParams accessors ─────────────────────────────────────────────

class TestQueryParamsAccess:
    def test_get_returns_last(self):
        qp = QueryParams([("a", "1"), ("a", "2")])
        assert qp.get("a") == "2"

    def test_get_default(self):
        qp = QueryParams()
        assert qp.get("missing") is None
        assert qp.get("missing", "fallback") == "fallback"

    def test_get_list(self):
        qp = QueryParams([("a", "1"), ("a", "2"), ("b", "3")])
        assert qp.get_list("a") == ["1", "2"]
        assert qp.get_list("b") == ["3"]
        assert qp.get_list("missing") == []

    def test_multi_items(self):
        qp = QueryParams([("a", "1"), ("a", "2")])
        assert qp.multi_items() == [("a", "1"), ("a", "2")]

    def test_keys(self):
        qp = QueryParams([("a", "1"), ("a", "2"), ("b", "3")])
        assert qp.keys() == ["a", "b"]

    def test_values(self):
        qp = QueryParams([("a", "1"), ("a", "2"), ("b", "3")])
        assert qp.values() == ["1", "3"]

    def test_items(self):
        qp = QueryParams([("a", "1"), ("a", "2"), ("b", "3")])
        assert qp.items() == [("a", "1"), ("b", "3")]


# ── QueryParams mutation ───────────────────────────────────────────────

class TestQueryParamsMutation:
    def test_add(self):
        qp = QueryParams({"a": "1"})
        qp.add("b", "2")
        assert qp.multi_items() == [("a", "1"), ("b", "2")]

    def test_set(self):
        qp = QueryParams({"a": "1", "b": "2"})
        qp.set("a", "9")
        assert qp["a"] == "9"

    def test_remove(self):
        qp = QueryParams({"a": "1", "b": "2"})
        qp.remove("a")
        assert "a" not in qp

    def test_update_dict(self):
        qp = QueryParams({"a": "1"})
        qp.update({"a": "9", "b": "2"})
        assert qp["a"] == "9"
        assert qp["b"] == "2"

    def test_update_queryparams(self):
        qp = QueryParams({"a": "1"})
        qp.update(QueryParams({"a": "9", "b": "2"}))
        assert qp["a"] == "9"
        assert qp["b"] == "2"

    def test_merge(self):
        qp = QueryParams({"a": "1"})
        qp.merge({"a": "9"})
        assert qp.get_list("a") == ["1", "9"]


# ── QueryParams dunder methods ────────────────────────────────────────

class TestQueryParamsDunder:
    def test_eq(self):
        assert QueryParams({"a": "1"}) == QueryParams({"a": "1"})
        assert QueryParams({"a": "1"}) != QueryParams({"a": "2"})

    def test_hash(self):
        a = QueryParams({"a": "1"})
        b = QueryParams({"a": "1"})
        assert hash(a) == hash(b)

    def test_str(self):
        qp = QueryParams({"a": "1"})
        assert "a=1" in str(qp)

    def test_repr(self):
        qp = QueryParams({"a": "1"})
        assert "QueryParams" in repr(qp)

    def test_bool_empty(self):
        assert not QueryParams()

    def test_bool_nonempty(self):
        assert QueryParams({"a": "1"})

    def test_len(self):
        assert len(QueryParams({"a": "1", "b": "2"})) == 2

    def test_contains(self):
        qp = QueryParams({"a": "1"})
        assert "a" in qp
        assert "b" not in qp

    def test_iter(self):
        qp = QueryParams([("a", "1"), ("b", "2")])
        assert list(qp) == ["a", "b"]

    def test_getitem_missing(self):
        with pytest.raises(KeyError):
            QueryParams()["missing"]

    def test_delitem(self):
        qp = QueryParams({"a": "1", "b": "2"})
        del qp["a"]
        assert "a" not in qp

    def test_delitem_missing(self):
        with pytest.raises(KeyError):
            del QueryParams()["missing"]
