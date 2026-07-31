"""Phase 2 Track 1: Request URL and body construction parity tests."""

import json

import pytest

from eggfetch.compat.httpx import Request, QueryParams, URL


class TestParamsAppliedToUrl:
    """1.1 Apply params to the Request URL."""

    def test_simple_params_in_url(self):
        req = Request("GET", "https://example.test/path", params=[("a", "1")])
        assert "a=1" in str(req.url)

    def test_repeated_query_values_preserved(self):
        req = Request(
            "GET",
            "https://example.test/path",
            params=[("a", "1"), ("a", "2"), ("a", "3")],
        )
        url_str = str(req.url)
        assert url_str.count("a=1") == 1
        assert url_str.count("a=2") == 1
        assert url_str.count("a=3") == 1

    def test_existing_query_replaced_by_params(self):
        req = Request(
            "GET",
            "https://example.test/path?existing=1",
            params=[("a", "1"), ("a", "2")],
        )
        url_str = str(req.url)
        assert "existing" not in url_str
        assert "a=1" in url_str
        assert "a=2" in url_str

    def test_params_empty_no_change(self):
        req = Request("GET", "https://example.test/path?existing=1")
        assert "existing=1" in str(req.url)

    def test_params_object_in_url(self):
        qp = QueryParams([("q", "test"), ("page", "2")])
        req = Request("GET", "https://example.test/search", params=qp)
        url_str = str(req.url)
        assert "q=test" in url_str
        assert "page=2" in url_str

    def test_params_query_params_matches_url(self):
        req = Request(
            "GET",
            "https://example.test/search",
            params=[("q", "egg"), ("lang", "python")],
        )
        url_params = req.url.params
        assert url_params.get("q") == "egg"
        assert url_params.get("lang") == "python"


class TestBodySourceExclusion:
    """1.2 Replace body-source mutual exclusion with HTTPX encoding rules."""

    def test_data_plus_files_valid(self):
        req = Request(
            "POST",
            "https://example.test/upload",
            data={"field": "value"},
            files={"file": b"content"},
        )
        assert req._files == {"file": b"content"}
        assert req._multipart_data == {"field": "value"}

    def test_content_exclusive_with_data(self):
        with pytest.raises(ValueError, match="mutually exclusive"):
            Request("POST", "https://example.test", content=b"x", data={"k": "v"})

    def test_content_exclusive_with_files(self):
        with pytest.raises(ValueError, match="mutually exclusive"):
            Request("POST", "https://example.test", content=b"x", files={"f": b"d"})

    def test_content_exclusive_with_json(self):
        with pytest.raises(ValueError, match="mutually exclusive"):
            Request("POST", "https://example.test", content=b"x", json={"k": "v"})

    def test_json_exclusive_with_data(self):
        with pytest.raises(ValueError, match="mutually exclusive"):
            Request("POST", "https://example.test", json={"k": "v"}, data={"a": "b"})

    def test_json_exclusive_with_files(self):
        with pytest.raises(ValueError, match="mutually exclusive"):
            Request("POST", "https://example.test", json={"k": "v"}, files={"f": b"d"})

    def test_stream_exclusive_with_content(self):
        with pytest.raises(ValueError, match="mutually exclusive"):
            Request("POST", "https://example.test", stream=iter([b"x"]), content=b"y")

    def test_stream_exclusive_with_json(self):
        with pytest.raises(ValueError, match="mutually exclusive"):
            Request("POST", "https://example.test", stream=iter([b"x"]), json={"k": "v"})

    def test_stream_exclusive_with_data(self):
        with pytest.raises(ValueError, match="mutually exclusive"):
            Request("POST", "https://example.test", stream=iter([b"x"]), data={"a": "b"})

    def test_stream_exclusive_with_files(self):
        with pytest.raises(ValueError, match="mutually exclusive"):
            Request("POST", "https://example.test", stream=iter([b"x"]), files={"f": b"d"})


class TestJsonSerialization:
    """1.3 Use HTTPX-compatible JSON serialization."""

    def test_compact_json_no_spaces(self):
        req = Request("POST", "https://example.test", json={"key": "value"})
        assert req.content == b'{"key":"value"}'

    def test_json_unicode(self):
        req = Request("POST", "https://example.test", json={"text": "héllo"})
        assert req.content == '{"text":"héllo"}'.encode("utf-8")

    def test_json_nested(self):
        data = {"a": {"b": [1, 2, 3]}, "c": True, "d": None}
        req = Request("POST", "https://example.test", json=data)
        expected = json.dumps(data, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
        assert req.content == expected

    def test_json_content_type(self):
        req = Request("POST", "https://example.test", json={"k": "v"})
        assert req.headers["content-type"] == "application/json"

    def test_json_content_length(self):
        req = Request("POST", "https://example.test", json={"k": "v"})
        assert req.headers["content-length"] == str(len(req.content))

    def test_json_custom_content_type_not_overwritten(self):
        req = Request(
            "POST",
            "https://example.test",
            json={"k": "v"},
            headers={"Content-Type": "application/custom"},
        )
        assert req.headers["content-type"] == "application/custom"


class TestMultipartForm:
    """1.4 Preserve multipart form and file metadata."""

    def test_files_only(self):
        req = Request(
            "POST",
            "https://example.test/upload",
            files={"file": b"content"},
        )
        assert req._files == {"file": b"content"}

    def test_files_tuple_two(self):
        req = Request(
            "POST",
            "https://example.test/upload",
            files=[("file", b"content")],
        )
        assert req._files == [("file", b"content")]

    def test_files_tuple_three(self):
        req = Request(
            "POST",
            "https://example.test/upload",
            files=[("file", b"content", "text/plain")],
        )
        assert req._files == [("file", b"content", "text/plain")]

    def test_files_tuple_four(self):
        req = Request(
            "POST",
            "https://example.test/upload",
            files=[("file", b"content", "text/plain", {"X-Custom": "yes"})],
        )
        assert req._files == [("file", b"content", "text/plain", {"X-Custom": "yes"})]

    def test_data_plus_files_multipart(self):
        req = Request(
            "POST",
            "https://example.test/upload",
            data={"field": "value"},
            files={"file": b"content"},
        )
        body = Request._encode_files(req._files, data=req._multipart_data)
        assert b"field" in body
        assert b"file" in body
        assert b"value" in body
        assert b"content" in body

    def test_repeated_file_fields(self):
        files = [("files", b"one"), ("files", b"two")]
        body = Request._encode_files(files)
        assert body.count(b"files") >= 2

    def test_large_streaming_file_not_eagerly_buffered(self):
        """Request construction with a file-like should not eagerly read the file."""

        class FakeFile:
            def __init__(self):
                self.read_called = False

            def read(self, n=-1):
                self.read_called = True
                return b"data"

            def __iter__(self):
                return iter([b"data"])

        f = FakeFile()
        req = Request("POST", "https://example.test/upload", content=f)
        assert not f.read_called
