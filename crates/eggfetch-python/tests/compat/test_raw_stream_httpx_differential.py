"""Pinned HTTPX observations for the compatibility raw-stream surface."""

from __future__ import annotations

import gzip
import inspect
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler

import httpx
import pytest

from eggfetch.compat.httpx import AsyncClient, Client, Response

assert httpx.__version__ == "0.28.1", (
    f"Expected httpx 0.28.1, got {httpx.__version__}"
)


class _SyncChunks(httpx.SyncByteStream):
    def __init__(self, chunks: list[bytes], fail_after: int | None = None) -> None:
        self.chunks = chunks
        self.fail_after = fail_after
        self.close_count = 0

    def __iter__(self):
        for index, chunk in enumerate(self.chunks):
            yield chunk
            if self.fail_after == index:
                raise ValueError("source failure")

    def close(self) -> None:
        self.close_count += 1


class _AsyncChunks(httpx.AsyncByteStream):
    def __init__(self, chunks: list[bytes], fail_after: int | None = None) -> None:
        self.chunks = chunks
        self.fail_after = fail_after
        self.close_count = 0

    async def __aiter__(self):
        for index, chunk in enumerate(self.chunks):
            yield chunk
            if self.fail_after == index:
                raise ValueError("source failure")

    async def aclose(self) -> None:
        self.close_count += 1


def _elapsed_available(response) -> bool:
    try:
        response.elapsed
    except RuntimeError:
        return False
    return True


def _state(response, source) -> tuple[bool, bool, int, bool, int]:
    return (
        response.is_stream_consumed,
        response.is_closed,
        response.num_bytes_downloaded,
        _elapsed_available(response),
        source.close_count,
    )


def _exception(action) -> tuple[str, str]:
    try:
        action()
    except Exception as exc:  # noqa: BLE001 - normalize both public runtimes.
        name = type(exc).__name__
        if name == "StreamConsumed":
            return name, "stream-consumed"
        return name, (str(exc).splitlines() or [""])[0]
    return "", ""


def _sync_case(response_type, source_type, chunks, chunk_size):
    source = source_type(chunks)
    response = response_type(200, stream=source)
    before = _state(response, source)
    iterator = response.iter_raw(chunk_size)
    constructed = _state(response, source)
    first = next(iterator, None)
    after_first = _state(response, source)
    rest = list(iterator)
    exhausted = _state(response, source)
    return {
        "before": before,
        "constructed": constructed,
        "first": first,
        "after_first": after_first,
        "rest": rest,
        "exhausted": exhausted,
    }


@pytest.mark.parametrize("chunks, chunk_size", [([b"abcd"], 1), ([b"a", b"b", b"c"], 3)])
def test_sync_raw_state_and_accounting_match_httpx(chunks, chunk_size):
    reference = _sync_case(httpx.Response, _SyncChunks, chunks, chunk_size)
    candidate = _sync_case(Response, _SyncChunks, chunks, chunk_size)
    assert candidate == reference


@pytest.mark.parametrize("chunks", [[], [b"", b"a", b""]])
def test_sync_empty_chunks_match_httpx(chunks):
    reference = _sync_case(httpx.Response, _SyncChunks, chunks, None)
    candidate = _sync_case(Response, _SyncChunks, chunks, None)
    assert candidate == reference


def test_sync_partial_finalization_and_explicit_close_match_httpx():
    observations = []
    for response_type in (httpx.Response, Response):
        source = _SyncChunks([b"a", b"b"])
        response = response_type(200, stream=source)
        iterator = response.iter_raw(chunk_size=1)
        assert next(iterator) == b"a"
        partial = _state(response, source)
        iterator.close()
        finalized = _state(response, source)
        second = _exception(lambda: list(response.iter_raw()))
        response.close()
        closed = _state(response, source)
        observations.append((partial, finalized, second, closed))
    assert observations[1] == observations[0]


def test_sync_source_failure_preserves_primary_exception_and_state():
    observations = []
    for response_type in (httpx.Response, Response):
        source = _SyncChunks([b"a", b"b"], fail_after=0)
        response = response_type(200, stream=source)
        iterator = response.iter_raw()
        assert next(iterator) == b"a"
        error = _exception(lambda: next(iterator))
        observations.append((error, _state(response, source)))
    assert observations[1] == observations[0]


@pytest.mark.parametrize("chunk_size", [0, -1, 1.5, "1", False])
def test_sync_invalid_chunk_sizes_match_httpx(chunk_size):
    observations = []
    for response_type in (httpx.Response, Response):
        source = _SyncChunks([b"x"])
        response = response_type(200, stream=source)
        iterator = response.iter_raw(chunk_size)
        observations.append((_exception(lambda: next(iterator)), _state(response, source)))
    assert observations[1] == observations[0]


@pytest.mark.asyncio
async def async_raw_case(response_type, source_type, chunks, chunk_size):
    source = source_type(chunks)
    response = response_type(200, stream=source)
    before = _state(response, source)
    iterator = response.aiter_raw(chunk_size)
    constructed = _state(response, source)
    try:
        first = await anext(iterator)
    except StopAsyncIteration:
        first = None
    after_first = _state(response, source)
    rest = [chunk async for chunk in iterator]
    exhausted = _state(response, source)
    return {
        "before": before,
        "constructed": constructed,
        "first": first,
        "after_first": after_first,
        "rest": rest,
        "exhausted": exhausted,
    }


@pytest.mark.asyncio
@pytest.mark.parametrize("chunks, chunk_size", [([b"abcd"], 1), ([b"a", b"b", b"c"], 3), ([], None)])
async def test_async_raw_state_and_accounting_match_httpx(chunks, chunk_size):
    reference = await async_raw_case(httpx.Response, _AsyncChunks, chunks, chunk_size)
    candidate = await async_raw_case(Response, _AsyncChunks, chunks, chunk_size)
    assert candidate == reference


@pytest.mark.asyncio
async def test_async_partial_finalization_and_explicit_close_match_httpx():
    observations = []
    for response_type in (httpx.Response, Response):
        source = _AsyncChunks([b"a", b"b"])
        response = response_type(200, stream=source)
        iterator = response.aiter_raw(chunk_size=1)
        assert await anext(iterator) == b"a"
        partial = _state(response, source)
        await iterator.aclose()
        finalized = _state(response, source)
        second = _exception(lambda: response.aiter_raw())
        await response.aclose()
        closed = _state(response, source)
        observations.append((partial, finalized, second, closed))
    assert observations[1] == observations[0]


@pytest.mark.asyncio
async def test_async_source_failure_preserves_primary_exception_and_state():
    observations = []
    for response_type in (httpx.Response, Response):
        source = _AsyncChunks([b"a", b"b"], fail_after=0)
        response = response_type(200, stream=source)
        iterator = response.aiter_raw()
        assert await anext(iterator) == b"a"
        try:
            await anext(iterator)
        except Exception as exc:  # noqa: BLE001 - public exception comparison.
            error = (type(exc).__name__, (str(exc).splitlines() or [""])[0])
        observations.append((error, _state(response, source)))
    assert observations[1] == observations[0]


@pytest.mark.asyncio
@pytest.mark.parametrize("chunk_size", [0, -1, 1.5, "1", False])
async def test_async_invalid_chunk_sizes_match_httpx(chunk_size):
    observations = []
    for response_type in (httpx.Response, Response):
        source = _AsyncChunks([b"x"])
        response = response_type(200, stream=source)
        iterator = response.aiter_raw(chunk_size)
        try:
            await anext(iterator)
        except Exception as exc:  # noqa: BLE001 - normalize reference behavior.
            error = (type(exc).__name__, (str(exc).splitlines() or [""])[0])
        else:
            error = ("", "")
        observations.append((error, _state(response, source)))
    assert observations[1] == observations[0]


def test_buffered_raw_is_consumed_but_decoded_reads_remain_repeatable():
    reference = httpx.Response(200, content=b"abc")
    candidate = Response(200, content=b"abc")
    assert (reference.is_stream_consumed, reference.is_closed) == (
        candidate.is_stream_consumed,
        candidate.is_closed,
    )
    assert _exception(lambda: list(reference.iter_raw()))[0] == _exception(
        lambda: list(candidate.iter_raw())
    )[0]
    assert candidate.content == reference.content == b"abc"
    assert candidate.read() == reference.read() == b"abc"


def test_raw_defaults_match_pinned_signature():
    reference = inspect.signature(httpx.Response.iter_raw)
    candidate = inspect.signature(Response.iter_raw)
    assert candidate.parameters["chunk_size"].default == reference.parameters["chunk_size"].default
    reference_async = inspect.signature(httpx.Response.aiter_raw)
    candidate_async = inspect.signature(Response.aiter_raw)
    assert candidate_async.parameters["chunk_size"].default == reference_async.parameters["chunk_size"].default


def test_sync_iteration_rejects_async_source_without_consuming():
    async def source():
        yield b"x"

    observations = []
    for response_type in (httpx.Response, Response):
        response = response_type(200, stream=source())
        iterator = response.iter_raw()
        observations.append((_exception(lambda: next(iterator)), response.is_stream_consumed))
    assert observations[1] == observations[0]


@pytest.mark.asyncio
async def test_async_iteration_rejects_sync_source_without_consuming():
    observations = []
    for response_type in (httpx.Response, Response):
        source = _SyncChunks([b"x"])
        response = response_type(200, stream=source)
        iterator = response.aiter_raw()
        try:
            await anext(iterator)
        except Exception as exc:  # noqa: BLE001 - normalize reference behavior.
            error = (type(exc).__name__, str(exc).splitlines()[0])
        observations.append((error, response.is_stream_consumed))
    assert observations[1] == observations[0]


class _RawHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        original = b"hello " * 100
        if self.path == "/gzip":
            body = gzip.compress(original)
            self.send_response(200)
            self.send_header("Content-Encoding", "gzip")
        else:
            body = b"native raw body"
            self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()

    def log_message(self, format, *args):
        pass


@contextmanager
def _native_server():
    from .native_fixtures import local_http_server

    with local_http_server(_RawHandler) as (host, port):
        yield f"http://{host}:{port}"


def test_actual_native_sync_raw_stream_reaches_unadapted_path():
    with _native_server() as base:
        with httpx.Client() as reference, Client() as candidate:
            with reference.stream("GET", f"{base}/raw") as expected, candidate.stream(
                "GET", f"{base}/raw"
            ) as actual:
                expected_chunks = list(expected.iter_raw())
                actual_chunks = list(actual.iter_raw())
                assert b"".join(actual_chunks) == b"".join(expected_chunks)
                assert actual.num_bytes_downloaded == expected.num_bytes_downloaded


@pytest.mark.asyncio
async def test_actual_native_async_raw_stream_reaches_unadapted_path():
    with _native_server() as base:
        async with httpx.AsyncClient() as reference, AsyncClient() as candidate:
            async with reference.stream("GET", f"{base}/raw") as expected, candidate.stream(
                "GET", f"{base}/raw"
            ) as actual:
                expected_chunks = [chunk async for chunk in expected.aiter_raw()]
                actual_chunks = [chunk async for chunk in actual.aiter_raw()]
                assert b"".join(actual_chunks) == b"".join(expected_chunks)
                assert actual.num_bytes_downloaded == expected.num_bytes_downloaded


def test_native_compressed_raw_boundary_is_explicitly_unresolved():
    """Keep the core adapter stop condition executable until it is reviewed."""
    with _native_server() as base:
        original = b"hello " * 100
        with httpx.Client() as reference, Client() as candidate:
            with reference.stream("GET", f"{base}/gzip") as expected, candidate.stream(
                "GET", f"{base}/gzip"
            ) as actual:
                expected_raw = b"".join(expected.iter_raw())
                actual_raw = b"".join(actual.iter_raw())
                assert expected_raw != actual_raw
                assert actual_raw == original
