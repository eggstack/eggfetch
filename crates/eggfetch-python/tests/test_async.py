"""Tests for the eggfetch Python async API."""

import asyncio
import http.server
import json
import threading
import urllib.parse

import pytest

import eggfetch


# ---------------------------------------------------------------------------
# Local test server
# ---------------------------------------------------------------------------

class _Handler(http.server.BaseHTTPRequestHandler):
    """Minimal test server that echoes request details."""

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        body = json.dumps({
            "method": "GET",
            "path": parsed.path,
            "query": parsed.query,
            "headers": dict(self.headers),
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        raw = self.rfile.read(length) if length else b""
        body = json.dumps({
            "method": "POST",
            "path": self.path,
            "headers": dict(self.headers),
            "body": raw.decode(errors="replace"),
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_PUT(self):
        self.do_POST()

    def do_PATCH(self):
        self.do_POST()

    def do_DELETE(self):
        self.do_GET()

    def do_HEAD(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("X-Echo", "head-ok")
        self.end_headers()

    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header("Allow", "GET, POST, OPTIONS")
        self.end_headers()

    def log_message(self, format, *args):
        pass  # suppress logs during tests


@pytest.fixture(scope="module")
def server():
    """Start a local HTTP server for the test module."""
    srv = http.server.HTTPServer(("127.0.0.1", 0), _Handler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# Package imports
# ---------------------------------------------------------------------------

class TestAsyncPackageImports:
    def test_import_async_client(self):
        assert hasattr(eggfetch, "AsyncClient")

    def test_async_client_is_class(self):
        assert isinstance(eggfetch.AsyncClient, type)


# ---------------------------------------------------------------------------
# AsyncClient basic behavior
# ---------------------------------------------------------------------------

class TestAsyncClientBasic:
    def test_async_context_manager(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/hello")
                assert r.status_code == 200
        asyncio.run(_test())

    def test_async_get_returns_response(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/hello")
                assert isinstance(r, eggfetch.Response)
                assert r.status_code == 200
        asyncio.run(_test())

    def test_async_get_body(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/hello")
                data = json.loads(r.text)
                assert data["method"] == "GET"
                assert data["path"] == "/hello"
        asyncio.run(_test())

    def test_async_get_is_success(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/hello")
                assert r.is_success
        asyncio.run(_test())

    def test_async_get_url_property(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/hello")
                assert r.url == f"{server}/hello"
        asyncio.run(_test())


# ---------------------------------------------------------------------------
# AsyncClient POST
# ---------------------------------------------------------------------------

class TestAsyncClientPost:
    def test_async_post_content(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.post(f"{server}/api", content=b"hello world")
                assert r.status_code == 200
                data = json.loads(r.text)
                assert data["method"] == "POST"
                assert data["body"] == "hello world"
        asyncio.run(_test())


# ---------------------------------------------------------------------------
# Headers and params
# ---------------------------------------------------------------------------

class TestAsyncHeadersAndParams:
    def test_headers_reach_server(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(
                    f"{server}/hello", headers={"X-Custom": "test-value"}
                )
                data = json.loads(r.text)
                assert data["headers"].get("x-custom") == "test-value"
        asyncio.run(_test())

    def test_response_headers(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/hello")
                assert "content-type" in r.headers
        asyncio.run(_test())

    def test_params_serialized(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(
                    f"{server}/search", params={"q": "hello", "page": "1"}
                )
                data = json.loads(r.text)
                assert "q=hello" in data["query"]
                assert "page=1" in data["query"]
        asyncio.run(_test())

    def test_params_sequence_of_pairs(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(
                    f"{server}/search",
                    params=[("q", "test"), ("q", "other")],
                )
                data = json.loads(r.text)
                assert "q=test" in data["query"]
                assert "q=other" in data["query"]
        asyncio.run(_test())

    def test_headers_as_sequence_of_pairs(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(
                    f"{server}/hello",
                    headers=[("X-Pair", "val1"), ("X-Pair", "val2")],
                )
                data = json.loads(r.text)
                assert data["headers"].get("x-pair") == "val2"
        asyncio.run(_test())

    def test_request_headers_override_client_default(self, server):
        async def _test():
            async with eggfetch.AsyncClient(
                headers={"X-Foo": "from-client"}
            ) as client:
                r = await client.get(
                    f"{server}/hello",
                    headers={"X-Foo": "override", "X-Bar": "from-request"},
                )
                data = json.loads(r.text)
                assert data["headers"].get("x-bar") == "from-request"
        asyncio.run(_test())

    def test_params_with_existing_query(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(
                    f"{server}/search?existing=1",
                    params={"q": "hello"},
                )
                data = json.loads(r.text)
                assert "existing=1" in data["query"]
                assert "q=hello" in data["query"]
        asyncio.run(_test())

    def test_invalid_params_type_raises(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                with pytest.raises(TypeError):
                    await client.get(f"{server}/search", params=123)
        asyncio.run(_test())

    def test_invalid_header_value_raises(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                with pytest.raises(eggfetch.RequestError):
                    await client.get(
                        f"{server}/hello", headers={"X-Bad": "val\nue"}
                    )
        asyncio.run(_test())


# ---------------------------------------------------------------------------
# Client reuse and default headers
# ---------------------------------------------------------------------------

class TestAsyncClientReuse:
    def test_client_reuses_connection(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r1 = await client.get(f"{server}/hello")
                r2 = await client.get(f"{server}/hello")
                assert r1.status_code == 200
                assert r2.status_code == 200
        asyncio.run(_test())

    def test_client_default_headers(self, server):
        async def _test():
            async with eggfetch.AsyncClient(
                headers={"X-Client-Header": "from-client"}
            ) as client:
                r = await client.get(f"{server}/hello")
                data = json.loads(r.text)
                assert data["headers"].get("x-client-header") == "from-client"
        asyncio.run(_test())


# ---------------------------------------------------------------------------
# HTTP methods
# ---------------------------------------------------------------------------

class TestAsyncHTTPMethods:
    def test_put(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.put(f"{server}/api", content=b"put-data")
                assert r.status_code == 200
        asyncio.run(_test())

    def test_patch(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.patch(f"{server}/api", content=b"patch-data")
                assert r.status_code == 200
        asyncio.run(_test())

    def test_delete(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.delete(f"{server}/resource")
                assert r.status_code == 200
        asyncio.run(_test())

    def test_head(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.head(f"{server}/hello")
                assert r.status_code == 200
        asyncio.run(_test())

    def test_options(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.options(f"{server}/hello")
                assert r.status_code == 200
        asyncio.run(_test())


# ---------------------------------------------------------------------------
# Closed client
# ---------------------------------------------------------------------------

class TestAsyncClientClosed:
    def test_closed_client_raises(self, server):
        async def _test():
            client = eggfetch.AsyncClient()
            client.close()
            with pytest.raises(ValueError, match="closed"):
                await client.get(f"{server}/hello")
        asyncio.run(_test())

    def test_client_is_closed_property(self):
        async def _test():
            client = eggfetch.AsyncClient()
            assert not client.is_closed
            client.close()
            assert client.is_closed
        asyncio.run(_test())

    def test_aclose_is_idempotent(self):
        async def _test():
            client = eggfetch.AsyncClient()
            client.close()
            client.close()  # should not raise
            assert client.is_closed
        asyncio.run(_test())


# ---------------------------------------------------------------------------
# Error mapping
# ---------------------------------------------------------------------------

class TestAsyncErrors:
    def test_invalid_url(self):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                with pytest.raises(ValueError):
                    await client.get("not-a-url")
        asyncio.run(_test())

    def test_unsupported_scheme(self):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                with pytest.raises(eggfetch.RequestError, match="not supported"):
                    await client.get("ftp://example.com")
        asyncio.run(_test())


# ---------------------------------------------------------------------------
# Timeout
# ---------------------------------------------------------------------------

class TestAsyncTimeout:
    def test_scalar_timeout(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.get(f"{server}/hello", timeout=10.0)
                assert r.status_code == 200
        asyncio.run(_test())

    def test_request_timeout_overrides_client_default(self, server):
        async def _test():
            async with eggfetch.AsyncClient(timeout=0.001) as client:
                # Client timeout is very short; request-level override should succeed
                r = await client.get(f"{server}/hello", timeout=10.0)
                assert r.status_code == 200
        asyncio.run(_test())


# ---------------------------------------------------------------------------
# Concurrent requests
# ---------------------------------------------------------------------------

class TestAsyncConcurrent:
    def test_many_concurrent_requests(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                tasks = [
                    client.get(f"{server}/hello") for _ in range(10)
                ]
                responses = await asyncio.gather(*tasks)
                assert len(responses) == 10
                for r in responses:
                    assert r.status_code == 200
        asyncio.run(_test())


# ---------------------------------------------------------------------------
# Cancellation
# ---------------------------------------------------------------------------

class TestAsyncCancellation:
    def test_cancellation_does_not_poison_client(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                # Start a request and cancel it
                async def do_get():
                    return await client.get(f"{server}/hello")

                task = asyncio.create_task(do_get())
                await asyncio.sleep(0)  # let it start
                task.cancel()
                try:
                    await task
                except asyncio.CancelledError:
                    pass

                # A later request should still succeed
                r = await client.get(f"{server}/hello")
                assert r.status_code == 200
        asyncio.run(_test())


# ---------------------------------------------------------------------------
# Unsupported kwargs
# ---------------------------------------------------------------------------

class TestAsyncUnsupportedKwargs:
    def test_unsupported_kwarg(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                with pytest.raises(TypeError):
                    await client.get(f"{server}/hello", json={"key": "value"})
        asyncio.run(_test())


# ---------------------------------------------------------------------------
# Body kwargs: content, data, json
# ---------------------------------------------------------------------------

class TestAsyncContent:
    def test_content_bytes(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.post(f"{server}/api", content=b"raw bytes")
                data = json.loads(r.text)
                assert data["body"] == "raw bytes"
        asyncio.run(_test())

    def test_content_str(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.post(f"{server}/api", content="string body")
                data = json.loads(r.text)
                assert data["body"] == "string body"
        asyncio.run(_test())


class TestAsyncFormData:
    def test_form_dict(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.post(f"{server}/api", data={"a": "1", "b": "2"})
                data = json.loads(r.text)
                assert "a=1" in data["body"]
                assert "b=2" in data["body"]
                assert data["headers"].get("content-type") == "application/x-www-form-urlencoded"
        asyncio.run(_test())

    def test_form_sequence_of_pairs(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.post(
                    f"{server}/api",
                    data=[("a", "1"), ("a", "2")],
                )
                data = json.loads(r.text)
                assert "a=1" in data["body"]
                assert "a=2" in data["body"]
        asyncio.run(_test())


class TestAsyncJsonBody:
    def test_json_dict(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.post(f"{server}/api", json={"hello": "world"})
                data = json.loads(r.text)
                body = json.loads(data["body"])
                assert body == {"hello": "world"}
                assert data["headers"].get("content-type") == "application/json"
        asyncio.run(_test())

    def test_json_list(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.post(f"{server}/api", json=[1, 2, 3])
                data = json.loads(r.text)
                body = json.loads(data["body"])
                assert body == [1, 2, 3]
        asyncio.run(_test())

    def test_json_unserializable_raises(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                with pytest.raises(TypeError):
                    await client.post(f"{server}/api", json=object())
        asyncio.run(_test())

    def test_json_preserves_user_content_type(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.post(
                    f"{server}/api",
                    headers={"Content-Type": "custom/json"},
                    json={"a": 1},
                )
                data = json.loads(r.text)
                assert data["headers"].get("content-type") == "custom/json"
        asyncio.run(_test())


class TestAsyncBodyConflict:
    def test_content_and_json_raises(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                with pytest.raises(TypeError, match="only one of content, data, or json"):
                    await client.post(f"{server}/api", content=b"raw", json={"a": 1})
        asyncio.run(_test())

    def test_data_and_json_raises(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                with pytest.raises(TypeError, match="only one of content, data, or json"):
                    await client.post(f"{server}/api", data={"a": "1"}, json={"b": 2})
        asyncio.run(_test())


# ---------------------------------------------------------------------------
# AsyncClient body kwargs
# ---------------------------------------------------------------------------

class TestAsyncClientBodyKwargs:
    def test_client_json(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.post(f"{server}/api", json={"key": "value"})
                data = json.loads(r.text)
                body = json.loads(data["body"])
                assert body == {"key": "value"}
        asyncio.run(_test())

    def test_client_put_json(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.put(f"{server}/api", json={"updated": True})
                data = json.loads(r.text)
                body = json.loads(data["body"])
                assert body == {"updated": True}
        asyncio.run(_test())

    def test_client_patch_form_data(self, server):
        async def _test():
            async with eggfetch.AsyncClient() as client:
                r = await client.patch(f"{server}/api", data={"a": "1"})
                data = json.loads(r.text)
                assert "a=1" in data["body"]
        asyncio.run(_test())
