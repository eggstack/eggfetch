"""Pinned HTTPX observations for the compatibility raw-stream surface."""

from __future__ import annotations

import gzip
import inspect
import asyncio
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler

import httpx
import pytest

from eggfetch.compat.httpx import AsyncClient, Client, Limits, Response

assert httpx.__version__ == "0.28.1", (
    f"Expected httpx 0.28.1, got {httpx.__version__}"
)


class _SyncChunks(httpx.SyncByteStream):
    def __init__(
        self,
        chunks: list[bytes],
        fail_after: int | None = None,
        fail_before: bool = False,
    ) -> None:
        self.chunks = chunks
        self.fail_after = fail_after
        self.fail_before = fail_before
        self.close_count = 0

    def __iter__(self):
        if self.fail_before:
            raise ValueError("source failure")
        for index, chunk in enumerate(self.chunks):
            yield chunk
            if self.fail_after == index:
                raise ValueError("source failure")

    def close(self) -> None:
        self.close_count += 1


class _AsyncChunks(httpx.AsyncByteStream):
    def __init__(
        self,
        chunks: list[bytes],
        fail_after: int | None = None,
        fail_before: bool = False,
    ) -> None:
        self.chunks = chunks
        self.fail_after = fail_after
        self.fail_before = fail_before
        self.close_count = 0

    async def __aiter__(self):
        if self.fail_before:
            raise ValueError("source failure")
        for index, chunk in enumerate(self.chunks):
            yield chunk
            if self.fail_after == index:
                raise ValueError("source failure")

    async def aclose(self) -> None:
        self.close_count += 1


class _BlockingAsyncChunks(httpx.AsyncByteStream):
    def __init__(self) -> None:
        self.started = asyncio.Event()
        self.release = asyncio.Event()
        self.close_count = 0

    async def __aiter__(self):
        self.started.set()
        await self.release.wait()
        yield b"released"

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


async def _async_exception(action) -> tuple[str, str]:
    try:
        await action()
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


def test_sync_immediate_source_failure_matches_httpx():
    observations = []
    for response_type in (httpx.Response, Response):
        source = _SyncChunks([], fail_before=True)
        response = response_type(200, stream=source)
        error = _exception(lambda: list(response.iter_raw()))
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
        second = await _async_exception(lambda: anext(response.aiter_raw()))
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
async def test_async_immediate_source_failure_matches_httpx():
    observations = []
    for response_type in (httpx.Response, Response):
        source = _AsyncChunks([], fail_before=True)
        response = response_type(200, stream=source)
        try:
            await anext(response.aiter_raw())
        except Exception as exc:  # noqa: BLE001 - public exception comparison.
            error = (type(exc).__name__, (str(exc).splitlines() or [""])[0])
        observations.append((error, _state(response, source)))
    assert observations[1] == observations[0]


@pytest.mark.asyncio
async def test_async_raw_cancellation_and_close_match_httpx():
    observations = []
    for response_type in (httpx.Response, Response):
        source = _BlockingAsyncChunks()
        response = response_type(200, stream=source)
        iterator = response.aiter_raw()
        task = asyncio.create_task(anext(iterator))
        await source.started.wait()
        task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await task
        await iterator.aclose()
        await response.aclose()
        observations.append(_state(response, source))
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
            body = gzip.compress(original, mtime=0)
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


def _stable_multi_items(headers):
    """Header items with the volatile `Date` value removed.

    The test server stamps `Date` per request, so two sequential
    requests (reference vs candidate) can straddle a second boundary
    and differ by one second. Date *presence* is asserted separately;
    everything else must match exactly.
    """
    return [(k, v) for k, v in headers.multi_items() if k.lower() != "date"]


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
                assert "date" in expected.headers and "date" in actual.headers
                assert _stable_multi_items(actual.headers) == _stable_multi_items(
                    expected.headers
                )


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
                assert "date" in expected.headers and "date" in actual.headers
                assert _stable_multi_items(actual.headers) == _stable_multi_items(
                    expected.headers
                )


def test_actual_native_sync_compressed_raw_and_decoded_match_httpx():
    with _native_server() as base:
        original = b"hello " * 100
        with httpx.Client() as reference, Client() as candidate:
            with reference.stream("GET", f"{base}/gzip") as expected:
                expected_raw = b"".join(expected.iter_raw())
                expected_count = expected.num_bytes_downloaded
            with candidate.stream("GET", f"{base}/gzip") as actual:
                actual_raw = b"".join(actual.iter_raw())
                assert actual_raw == expected_raw
                assert actual_raw != original
                assert actual_raw.startswith(b"\x1f\x8b")
                assert actual.num_bytes_downloaded == expected_count
                assert actual.is_closed

            with reference.stream("GET", f"{base}/gzip") as expected:
                expected_decoded = expected.read()
            with candidate.stream("GET", f"{base}/gzip") as actual:
                actual_decoded = actual.read()
                assert actual_decoded == expected_decoded == original
                assert actual.is_closed


def _sync_compressed_metadata(response, mode):
    before = (
        response.headers.get("content-encoding"),
        response.headers.get("content-length"),
    )
    body = b"".join(response.iter_raw()) if mode == "raw" else response.read()
    after = (
        response.headers.get("content-encoding"),
        response.headers.get("content-length"),
    )
    return before, after, len(body)


@pytest.mark.parametrize("mode", ["raw", "decoded"])
def test_actual_native_sync_compressed_metadata_matches_httpx(mode):
    with _native_server() as base:
        with httpx.Client() as reference, Client() as candidate:
            with reference.stream("GET", f"{base}/gzip") as expected:
                expected_observation = _sync_compressed_metadata(expected, mode)
            with candidate.stream("GET", f"{base}/gzip") as actual:
                actual_observation = _sync_compressed_metadata(actual, mode)
            assert actual_observation == expected_observation
            assert actual_observation[0][0] == "gzip"
            assert actual_observation[0][1] is not None
            assert actual_observation[0][1] == actual_observation[1][1]
            if mode == "raw":
                assert actual_observation[2] == int(actual_observation[0][1])
            else:
                assert actual_observation[2] != int(actual_observation[0][1])

def test_actual_native_sync_compressed_raw_chunk_adaptation_matches_httpx():
    with _native_server() as base:
        with httpx.Client() as reference, Client() as candidate:
            with reference.stream("GET", f"{base}/gzip") as expected:
                expected_chunks = list(expected.iter_raw(chunk_size=1))
                expected_count = expected.num_bytes_downloaded
            with candidate.stream("GET", f"{base}/gzip") as actual:
                actual_chunks = list(actual.iter_raw(chunk_size=1))
                assert actual_chunks == expected_chunks
                assert actual.num_bytes_downloaded == expected_count


@pytest.mark.asyncio
async def test_actual_native_async_compressed_raw_and_decoded_match_httpx():
    with _native_server() as base:
        async with httpx.AsyncClient() as reference, AsyncClient() as candidate:
            async with reference.stream("GET", f"{base}/gzip") as expected:
                expected_raw = b"".join([chunk async for chunk in expected.aiter_raw()])
                expected_count = expected.num_bytes_downloaded
            async with candidate.stream("GET", f"{base}/gzip") as actual:
                actual_raw = b"".join([chunk async for chunk in actual.aiter_raw()])
                assert actual_raw == expected_raw
                assert actual_raw != b"hello " * 100
                assert actual_raw.startswith(b"\x1f\x8b")
                assert actual.num_bytes_downloaded == expected_count
                assert actual.is_closed

            async with reference.stream("GET", f"{base}/gzip") as expected:
                expected_decoded = await expected.aread()
            async with candidate.stream("GET", f"{base}/gzip") as actual:
                actual_decoded = await actual.aread()
                assert actual_decoded == expected_decoded == b"hello " * 100
                assert actual.is_closed


async def _async_compressed_metadata(response, mode):
    before = (
        response.headers.get("content-encoding"),
        response.headers.get("content-length"),
    )
    if mode == "raw":
        body = b"".join([chunk async for chunk in response.aiter_raw()])
    else:
        body = await response.aread()
    after = (
        response.headers.get("content-encoding"),
        response.headers.get("content-length"),
    )
    return before, after, len(body)


@pytest.mark.asyncio
@pytest.mark.parametrize("mode", ["raw", "decoded"])
async def test_actual_native_async_compressed_metadata_matches_httpx(mode):
    with _native_server() as base:
        async with httpx.AsyncClient() as reference, AsyncClient() as candidate:
            async with reference.stream("GET", f"{base}/gzip") as expected:
                expected_observation = await _async_compressed_metadata(expected, mode)
            async with candidate.stream("GET", f"{base}/gzip") as actual:
                actual_observation = await _async_compressed_metadata(actual, mode)
            assert actual_observation == expected_observation
            assert actual_observation[0][0] == "gzip"
            assert actual_observation[0][1] is not None
            assert actual_observation[0][1] == actual_observation[1][1]
            if mode == "raw":
                assert actual_observation[2] == int(actual_observation[0][1])
            else:
                assert actual_observation[2] != int(actual_observation[0][1])

@pytest.mark.asyncio
async def test_actual_native_async_compressed_raw_chunk_adaptation_matches_httpx():
    with _native_server() as base:
        async with httpx.AsyncClient() as reference, AsyncClient() as candidate:
            async with reference.stream("GET", f"{base}/gzip") as expected:
                expected_chunks = [chunk async for chunk in expected.aiter_raw(chunk_size=1)]
                expected_count = expected.num_bytes_downloaded
            async with candidate.stream("GET", f"{base}/gzip") as actual:
                actual_chunks = [chunk async for chunk in actual.aiter_raw(chunk_size=1)]
                assert actual_chunks == expected_chunks
                assert actual.num_bytes_downloaded == expected_count


def test_native_compressed_stream_selection_is_one_shot():
    with _native_server() as base:
        with Client() as client:
            with client.stream("GET", f"{base}/gzip") as response:
                assert next(response.iter_raw())
                assert _exception(response.read)[0] == "StreamConsumed"
            with client.stream("GET", f"{base}/gzip") as response:
                assert response.read() == b"hello " * 100
                assert _exception(lambda: list(response.iter_raw()))[0] == "StreamConsumed"


async def _native_async_cancellation_case(client_type, limits):
    from .native_fixtures import blocking_gzip_handler, local_http_server

    handler = blocking_gzip_handler()
    with local_http_server(handler) as (host, port):
        base = f"http://{host}:{port}"
        async with client_type(limits=limits) as client:
            response_context = client.stream("GET", f"{base}/gzip-blocked")
            response = await response_context.__aenter__()
            iterator = response.aiter_raw()
            first = await asyncio.wait_for(anext(iterator), timeout=1)
            assert first.startswith(b"\x1f")
            assert handler.first_body_sent.wait(timeout=1)

            pending = asyncio.create_task(anext(iterator))
            assert handler.body_blocked.wait(timeout=1)
            await asyncio.sleep(0)
            pending.cancel()
            with pytest.raises(asyncio.CancelledError):
                await pending

            await iterator.aclose()
            await response.aclose()
            second_selection = _async_exception(lambda: anext(response.aiter_raw()))
            handler.release_body.set()
            follow_up = await asyncio.wait_for(client.get(f"{base}/follow-up"), timeout=2)
            return (
                first,
                response.is_stream_consumed,
                response.is_closed,
                await second_selection,
                follow_up.status_code,
                follow_up.content,
            )
        handler.release_body.set()


@pytest.mark.asyncio
async def test_native_async_compressed_cancellation_releases_pool_lease():
    reference = await _native_async_cancellation_case(
        httpx.AsyncClient,
        httpx.Limits(max_connections=1, max_keepalive_connections=1),
    )
    candidate = await _native_async_cancellation_case(
        AsyncClient,
        Limits(max_connections=1, max_keepalive_connections=1),
    )
    assert candidate == reference
