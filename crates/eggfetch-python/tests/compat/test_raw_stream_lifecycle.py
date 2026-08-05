"""Small candidate regressions not owned by the pinned differential matrix."""

import pytest

from eggfetch.compat.httpx import AsyncClient, Client, Response
from eggfetch.compat.httpx._exceptions import StreamConsumed


class TestRawStreamKernel:
    def test_consumption_and_source_accounting_happen_before_adaptation(self):
        response = Response(200, stream=iter([b"abcd"]))
        iterator = response.iter_raw(chunk_size=1)
        assert not response.is_stream_consumed
        assert next(iterator) == b"a"
        assert response.is_stream_consumed
        assert response.num_bytes_downloaded == 4

        response = Response(200, stream=iter([b"a", b"b", b"c"]))
        iterator = response.iter_raw(chunk_size=3)
        assert next(iterator) == b"abc"
        assert response.num_bytes_downloaded == 3

    def test_buffered_raw_iteration_is_rejected_without_affecting_reads(self):
        response = Response(200, content=b"body")
        with pytest.raises(StreamConsumed):
            list(response.iter_raw())
        assert response.content == b"body"
        assert response.read() == b"body"

    def test_partial_raw_finalization_does_not_close_response(self):
        response = Response(200, stream=iter([b"a", b"b"]))
        iterator = response.iter_raw()
        assert next(iterator) == b"a"
        iterator.close()
        assert response.is_stream_consumed
        assert not response.is_closed
        response.close()
        assert response.is_closed

    def test_normal_raw_exhaustion_closes_response(self):
        response = Response(200, stream=iter([b"body"]))
        assert list(response.iter_raw()) == [b"body"]
        assert response.is_closed

    @pytest.mark.asyncio
    async def test_async_consumption_and_source_accounting_happen_before_adaptation(self):
        async def source():
            yield b"abcd"

        response = Response(200, stream=source())
        iterator = response.aiter_raw(chunk_size=1)
        assert not response.is_stream_consumed
        assert await anext(iterator) == b"a"
        assert response.is_stream_consumed
        assert response.num_bytes_downloaded == 4

    @pytest.mark.asyncio
    async def test_async_partial_finalization_does_not_close_response(self):
        async def source():
            yield b"a"
            yield b"b"

        response = Response(200, stream=source())
        iterator = response.aiter_raw()
        assert await anext(iterator) == b"a"
        await iterator.aclose()
        assert response.is_stream_consumed
        assert not response.is_closed
        await response.aclose()
        assert response.is_closed


class TestNativeRawStreamKernel:
    """These cases use the repository's loopback server and Rust transport."""

    def test_sync_native_raw_iteration(self):
        from .native_fixtures import local_http_server

        with local_http_server() as (host, port):
            with Client() as client:
                with client.stream("GET", f"http://{host}:{port}/health") as response:
                    iterator = response.iter_raw(chunk_size=1)
                    assert next(iterator) == b"o"
                    assert response.is_stream_consumed
                    assert response.num_bytes_downloaded >= 2
                    assert list(iterator) == [b"k"]
                    assert response.is_closed

    @pytest.mark.asyncio
    async def test_async_native_raw_iteration(self):
        from .native_fixtures import local_http_server

        with local_http_server() as (host, port):
            async with AsyncClient() as client:
                async with client.stream(
                    "GET", f"http://{host}:{port}/health"
                ) as response:
                    iterator = response.aiter_raw(chunk_size=1)
                    assert await anext(iterator) == b"o"
                    assert response.is_stream_consumed
                    assert response.num_bytes_downloaded >= 2
                    assert [chunk async for chunk in iterator] == [b"k"]
                    assert response.is_closed
