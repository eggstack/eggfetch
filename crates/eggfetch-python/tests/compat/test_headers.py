"""Tests for Headers compatibility."""

import pytest

from eggfetch.compat.httpx import Headers


class TestHeadersConstruction:
    def test_from_dict(self):
        h = Headers({"content-type": "text/plain", "x-custom": "val"})
        assert h["content-type"] == "text/plain"

    def test_from_list_of_tuples(self):
        h = Headers([("x-a", "1"), ("x-b", "2")])
        assert h["x-a"] == "1"
        assert h["x-b"] == "2"

    def test_from_headers_obj(self):
        original = Headers({"x-a": "1"})
        copy = Headers(original)
        assert copy["x-a"] == "1"

    def test_invalid_type_raises(self):
        with pytest.raises(TypeError):
            Headers("not valid")


class TestHeadersLookup:
    def test_case_insensitive_get(self):
        h = Headers({"Content-Type": "text/plain"})
        assert h["content-type"] == "text/plain"
        assert h["CONTENT-TYPE"] == "text/plain"

    def test_get_default(self):
        h = Headers()
        assert h.get("missing") is None
        assert h.get("missing", "fallback") == "fallback"

    def test_get_returns_last_value(self):
        h = Headers([("x-dup", "first"), ("x-dup", "second")])
        assert h["x-dup"] == "second"

    def test_contains_case_insensitive(self):
        h = Headers({"X-Custom": "val"})
        assert "x-custom" in h
        assert "X-CUSTOM" in h
        assert "missing" not in h


class TestHeadersMultiValue:
    def test_get_list(self):
        h = Headers([("set-cookie", "a=1"), ("set-cookie", "b=2")])
        assert h.get_list("set-cookie") == ["a=1", "b=2"]

    def test_get_list_empty(self):
        h = Headers()
        assert h.get_list("missing") == []

    def test_multi_items_preserves_all(self):
        h = Headers([("x-a", "1"), ("x-a", "2"), ("x-b", "3")])
        items = h.multi_items()
        assert len(items) == 3
        assert items[0] == ("x-a", "1")
        assert items[1] == ("x-a", "2")
        assert items[2] == ("x-b", "3")

    def test_keys_deduplicates(self):
        h = Headers([("x-a", "1"), ("x-a", "2")])
        assert h.keys() == ["x-a"]

    def test_values_first_per_key(self):
        h = Headers([("x-a", "1"), ("x-a", "2"), ("x-b", "3")])
        assert h.values() == ["1", "3"]

    def test_items_first_per_key(self):
        h = Headers([("x-a", "1"), ("x-a", "2")])
        assert h.items() == [("x-a", "1")]


class TestHeadersRaw:
    def test_raw_bytes(self):
        h = Headers({"content-type": "text/plain"})
        raw = h.raw
        assert len(raw) == 1
        assert raw[0] == (b"content-type", b"text/plain")


class TestHeadersMutation:
    def test_setitem(self):
        h = Headers()
        h["x-custom"] = "val"
        assert h["x-custom"] == "val"

    def test_setitem_replaces(self):
        h = Headers({"x-a": "old"})
        h["x-a"] = "new"
        assert h["x-a"] == "new"

    def test_update_dict(self):
        h = Headers({"x-a": "1"})
        h.update({"x-b": "2"})
        assert h["x-b"] == "2"

    def test_update_headers(self):
        h = Headers({"x-a": "1"})
        h.update(Headers({"x-b": "2"}))
        assert h["x-b"] == "2"

    def test_pop(self):
        h = Headers({"x-a": "1", "x-b": "2"})
        val = h.pop("x-a")
        assert val == "1"
        assert "x-a" not in h
        assert "x-b" in h

    def test_pop_default(self):
        h = Headers()
        val = h.pop("missing", "default")
        assert val == "default"

    def test_clear(self):
        h = Headers({"x-a": "1"})
        h.clear()
        assert len(h) == 0

    def test_copy(self):
        h = Headers({"x-a": "1"})
        c = h.copy()
        c["x-a"] = "changed"
        assert h["x-a"] == "1"  # original unchanged

    def test_delitem(self):
        h = Headers({"x-a": "1"})
        del h["x-a"]
        assert "x-a" not in h

    def test_delitem_missing(self):
        with pytest.raises(KeyError):
            del Headers()["missing"]


class TestHeadersValidation:
    def test_cr_in_name_raises(self):
        with pytest.raises(ValueError, match="CR/LF"):
            Headers({"bad\rname": "val"})

    def test_lf_in_name_raises(self):
        with pytest.raises(ValueError, match="CR/LF"):
            Headers({"bad\nname": "val"})

    def test_cr_in_value_raises(self):
        with pytest.raises(ValueError, match="CR/LF"):
            Headers({"x-ok": "bad\rvalue"})

    def test_lf_in_value_raises(self):
        with pytest.raises(ValueError, match="CR/LF"):
            Headers({"x-ok": "bad\nvalue"})


class TestHeadersEq:
    def test_equal(self):
        assert Headers({"a": "1"}) == Headers({"a": "1"})

    def test_not_equal(self):
        assert Headers({"a": "1"}) != Headers({"a": "2"})

    def test_not_equal_to_non_headers(self):
        assert Headers({"a": "1"}) != "not headers"


class TestHeadersReprRedaction:
    """repr() must redact credential-bearing headers (project rule)."""

    @pytest.mark.parametrize(
        ("name", "value"),
        [
            ("authorization", "Bearer sk-live-secret"),
            ("proxy-authorization", "Basic dXNlcjpwYXNz"),
            ("cookie", "session=secret"),
            ("set-cookie", "session=secret; Path=/"),
        ],
    )
    def test_sensitive_values_redacted(self, name, value):
        h = Headers([(name, value)])
        r = repr(h)
        assert "<redacted>" in r
        assert value not in r

    def test_repr_is_case_insensitive(self):
        h = Headers([("Authorization", "Bearer token")])
        assert "<redacted>" in repr(h)

    def test_non_sensitive_values_visible(self):
        h = Headers({"content-type": "text/plain"})
        assert "'content-type': 'text/plain'" in repr(h)

    def test_repr_does_not_mutate(self):
        h = Headers([("authorization", "Bearer token"), ("x-ok", "visible")])
        repr(h)
        assert h["authorization"] == "Bearer token"
        assert h.get("authorization") == "Bearer token"


class TestHeadersMisc:
    def test_encoding(self):
        h = Headers(encoding="latin-1")
        assert h.encoding == "latin-1"

    def test_len(self):
        h = Headers([("a", "1"), ("b", "2")])
        assert len(h) == 2

    def test_iter(self):
        h = Headers([("a", "1"), ("b", "2")])
        assert list(h) == ["a", "b"]


class TestHeadersDuplicateConversion:
    """Test that duplicate headers survive conversion for native client."""

    def test_duplicate_headers_preserved_in_list(self):
        """Headers with duplicates should convert to list of tuples."""
        h = Headers([("set-cookie", "a=1"), ("set-cookie", "b=2"), ("x-other", "val")])
        items = h.multi_items()
        assert len(items) == 3
        assert ("set-cookie", "a=1") in items
        assert ("set-cookie", "b=2") in items
        assert ("x-other", "val") in items

    def test_single_header_converts_to_list(self):
        """Single header should still work as list of tuples."""
        h = Headers({"content-type": "text/plain"})
        items = h.multi_items()
        assert len(items) == 1
        assert items[0] == ("content-type", "text/plain")


# ── MutableMapping ABC ──────────────────────────────────────────────────

class TestHeadersMutableMappingABC:
    def test_isinstance_mutable_mapping(self):
        from collections.abc import MutableMapping
        h = Headers()
        assert isinstance(h, MutableMapping)

    def test_setdefault_existing_key(self):
        h = Headers({"x-a": "existing"})
        result = h.setdefault("x-a", "default")
        assert result == "existing"
        assert h["x-a"] == "existing"

    def test_setdefault_missing_key(self):
        h = Headers()
        result = h.setdefault("x-a", "default")
        assert result == "default"
        assert h["x-a"] == "default"

    def test_setdefault_empty_default(self):
        h = Headers()
        result = h.setdefault("x-a")
        assert result == ""
        assert h["x-a"] == ""

    def test_setdefault_case_insensitive(self):
        h = Headers({"X-A": "val"})
        result = h.setdefault("x-a", "default")
        assert result == "val"

    def test_popitem_nonempty(self):
        h = Headers([("x-a", "1"), ("x-b", "2")])
        name, value = h.popitem()
        assert name == "x-a"
        assert value == "1"
        assert len(h) == 1

    def test_popitem_empty_raises(self):
        with pytest.raises(KeyError):
            Headers().popitem()

    def test_popitem_removes_from_headers(self):
        h = Headers([("x-a", "1"), ("x-b", "2")])
        h.popitem()
        assert "x-a" not in h
        assert "x-b" in h

    def test_len_after_popitem(self):
        h = Headers([("x-a", "1"), ("x-b", "2")])
        h.popitem()
        assert len(h) == 1
