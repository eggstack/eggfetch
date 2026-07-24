"""Lossless header and query merge verification."""

import pytest
from eggfetch.compat.httpx import URL, Headers, QueryParams, Client


class TestQueryParamsMerge:
    def test_preserves_repeated_values(self):
        """QueryParams.merge() is additive: appends new entries."""
        base = QueryParams("a=1&a=2&b=3")
        extra = QueryParams("a=10&c=4")
        base.merge(extra)
        items = base.multi_items()
        assert ("a", "1") in items
        assert ("a", "2") in items
        assert ("b", "3") in items
        assert ("a", "10") in items
        assert ("c", "4") in items
        assert len(items) == 5

    def test_preserves_blank_values(self):
        """Blank query values survive merge."""
        params = QueryParams("a=&b=2")
        items = params.multi_items()
        assert ("a", "") in items

    def test_preserves_unicode(self):
        """Unicode query values survive merge."""
        params = QueryParams("name=hello%20world&lang=%C3%A9")
        assert params["name"] == "hello world"
        assert params["lang"] == "\u00e9"

    def test_merge_ordering(self):
        """Non-overlapping keys maintain insertion order."""
        base = QueryParams("b=1&d=2")
        extra = QueryParams("a=3&c=4")
        base.merge(extra)
        items = base.multi_items()
        keys = [k for k, v in items]
        assert keys == ["b", "d", "a", "c"]


class TestHeadersMerge:
    def test_duplicate_headers_preserved(self):
        """Duplicate header values survive update."""
        h = Headers()
        h.append("x-tag", "one")
        h.append("x-tag", "two")
        items = h.multi_items()
        tag_values = [v for k, v in items if k.lower() == "x-tag"]
        assert tag_values == ["one", "two"]

    def test_update_replaces_by_key(self):
        """Update removes old values for same key, appends new."""
        h = Headers([("x-tag", "old1"), ("x-tag", "old2"), ("x-other", "keep")])
        h.update({"x-tag": "new1"})
        items = h.multi_items()
        tag_values = [v for k, v in items if k.lower() == "x-tag"]
        assert tag_values == ["new1"]
        assert ("x-other", "keep") in items

    def test_case_insensitive_replacement(self):
        """Header replacement is case-insensitive."""
        h = Headers([("X-Tag", "old")])
        h.update({"x-tag": "new"})
        items = h.multi_items()
        tag_values = [v for k, v in items if k.lower() == "x-tag"]
        assert tag_values == ["new"]

    def test_update_preserves_incoming_duplicates(self):
        """Update with duplicate incoming headers preserves all incoming values."""
        h = Headers([("x-tag", "old1"), ("x-tag", "old2")])
        h.update(Headers([("x-tag", "new1"), ("x-tag", "new2")]))
        items = h.multi_items()
        tag_values = [v for k, v in items if k.lower() == "x-tag"]
        assert tag_values == ["new1", "new2"]


class TestClientMerge:
    def test_client_query_merge_replaces_duplicates(self):
        """Client default query params replace by key when request overrides."""
        with Client(base_url="http://example.com", params="a=1&a=2&b=3") as client:
            req = client.build_request("GET", "/path", params="a=10&c=4")
            url = req.url
            query_items = url.params.multi_items()
            # Client's a=1,a=2 replaced by request's a=10; b=3 survives; c=4 added
            assert ("b", "3") in query_items
            assert ("a", "10") in query_items
            assert ("c", "4") in query_items
            assert len(query_items) == 3

    def test_client_header_merge_replaces_by_key(self):
        """Client default headers replace by key when request overrides."""
        with Client(
            base_url="http://example.com",
            headers={"X-Custom": "client-val"}
        ) as client:
            req = client.build_request("GET", "/path", headers={"X-Custom": "req-val"})
            assert req.headers["x-custom"] == "req-val"

    def test_client_query_merge_preserves_non_overlapping(self):
        """Non-overlapping client params survive merge."""
        with Client(base_url="http://example.com", params="x=1&y=2") as client:
            req = client.build_request("GET", "/path", params="z=3")
            query_items = req.url.params.multi_items()
            assert ("x", "1") in query_items
            assert ("y", "2") in query_items
            assert ("z", "3") in query_items
            assert len(query_items) == 3

    def test_client_header_merge_preserves_non_overlapping(self):
        """Non-overlapping client headers survive merge."""
        with Client(
            base_url="http://example.com",
            headers={"X-Client": "val", "X-Other": "keep"}
        ) as client:
            req = client.build_request("GET", "/path", headers={"X-Client": "new"})
            assert req.headers["x-client"] == "new"
            assert req.headers["x-other"] == "keep"
