"""Raw stream lifecycle and accounting tests (final closure 02).

Covers Tracks 0-7 of the raw stream lifecycle plan. Tier 1 tests use only
eggfetch compat objects with no network or httpx dependency.
"""

import gzip

import pytest

from eggfetch.compat.httpx import AsyncClient, Client, MockTransport, Request, Response
from eggfetch.compat.httpx._exceptions import ResponseNotRead, StreamConsumed


# ── Track 0 / Track 7: Tier 1 pinned-reference cases ──────────────────


class TestSyncRawIterationLifecycle:
    """Sync iter_raw() must mark consumed, count bytes, and close exactly once."""

    def test_raw_iteration_marks_consumed_on_exhaustion(self):
        resp = Response(200, stream=iter([b"abc", b"def"]))
        assert not resp.is_stream_consumed
        chunks = list(resp.iter_raw())
        assert resp.is_stream_consumed
        assert b"".join(chunks) == b"abcdef"

    def test_raw_iteration_closes_on_exhaustion(self):
        resp = Response(200, stream=iter([b"abc", b"def"]))
        list(resp.iter_raw())
        assert resp.is_closed

    def test_raw_iteration_increments_num_bytes(self):
        resp = Response(200, stream=iter([b"abc", b"def"]))
        list(resp.iter_raw())
        assert resp.num_bytes_downloaded == 6

    def test_raw_iteration_chunk_size_none(self):
        resp = Response(200, stream=iter([b"abc", b"def"]))
        chunks = list(resp.iter_raw(chunk_size=None))
        assert chunks == [b"abc", b"def"]
        assert resp.num_bytes_downloaded == 6
        assert resp.is_stream_consumed

    def test_raw_iteration_chunk_size_splits_large_chunks(self):
        resp = Response(200, stream=iter([b"abcdefghij"]))
        chunks = list(resp.iter_raw(chunk_size=3))
        assert chunks == [b"abc", b"def", b"ghi", b"j"]
        assert resp.num_bytes_downloaded == 10

    def test_raw_iteration_coalesces_small_chunks(self):
        resp = Response(200, stream=iter([b"a", b"b", b"c", b"d", b"e"]))
        chunks = list(resp.iter_raw(chunk_size=3))
        assert chunks == [b"abc", b"de"]
        assert resp.num_bytes_downloaded == 5

    def test_raw_iteration_empty_chunks(self):
        resp = Response(200, stream=iter([b"", b"abc", b""]))
        chunks = list(resp.iter_raw())
        assert chunks == [b"abc"]
        assert resp.num_bytes_downloaded == 3

    def test_raw_iteration_zero_length_body(self):
        resp = Response(200, stream=iter([]))
        chunks = list(resp.iter_raw())
        assert chunks == []
        assert resp.num_bytes_downloaded == 0
        assert resp.is_stream_consumed

    def test_raw_iteration_partial_break(self):
        resp = Response(200, stream=iter([b"a", b"b", b"c"]))
        gen = resp.iter_raw(chunk_size=1)
        first = next(gen)
        assert first == b"a"
        assert not resp.is_stream_consumed
        gen.close()
        assert resp.is_stream_consumed
        assert resp.is_closed

    def test_raw_iteration_explicit_close_without_exhaustion(self):
        resp = Response(200, stream=iter([b"a", b"b", b"c"]))
        gen = resp.iter_raw(chunk_size=1)
        next(gen)
        gen.close()
        assert resp.is_closed
        assert resp.is_stream_consumed

    def test_second_raw_iteration_after_full_consumption(self):
        resp = Response(200, stream=iter([b"abc"]))
        list(resp.iter_raw())
        with pytest.raises(StreamConsumed):
            list(resp.iter_raw())

    def test_second_raw_iteration_after_partial_consumption(self):
        resp = Response(200, stream=iter([b"a", b"b", b"c"]))
        gen = resp.iter_raw()
        next(gen)
        gen.close()
        with pytest.raises(StreamConsumed):
            list(resp.iter_raw())

    def test_read_after_partial_raw_iteration(self):
        resp = Response(200, stream=iter([b"abc", b"def"]))
        gen = resp.iter_raw(chunk_size=1)
        next(gen)
        gen.close()
        with pytest.raises(StreamConsumed):
            resp.read()

    def test_content_after_partial_raw_iteration(self):
        resp = Response(200, stream=iter([b"abc", b"def"]))
        gen = resp.iter_raw(chunk_size=1)
        next(gen)
        gen.close()
        with pytest.raises(ResponseNotRead):
            _ = resp.content

    def test_unread_response_close(self):
        resp = Response(200, stream=iter([b"abc"]))
        resp.close()
        assert resp.is_closed
        assert resp.num_bytes_downloaded == 0

    def test_num_bytes_before_during_after_iteration(self):
        resp = Response(200, stream=iter([b"abc", b"def", b"ghi"]))
        assert resp.num_bytes_downloaded == 0
        gen = resp.iter_raw()
        first = next(gen)
        assert resp.num_bytes_downloaded == len(first)
        rest = list(gen)
        assert resp.num_bytes_downloaded == 9

    def test_elapsed_becomes_available_after_raw_iteration(self):
        resp = Response(200, stream=iter([b"abc"]))
        assert resp._elapsed is None
        list(resp.iter_raw())
        assert resp.elapsed is not None


class TestAsyncRawIterationLifecycle:
    """Async aiter_raw() must mirror sync lifecycle behavior."""

    @pytest.mark.asyncio
    async def test_async_raw_marks_consumed_on_exhaustion(self):
        async def agen():
            yield b"abc"
            yield b"def"

        resp = Response(200, stream=agen())
        assert not resp.is_stream_consumed
        chunks = []
        async for chunk in resp.aiter_raw():
            chunks.append(chunk)
        assert resp.is_stream_consumed
        assert b"".join(chunks) == b"abcdef"

    @pytest.mark.asyncio
    async def test_async_raw_closes_on_exhaustion(self):
        async def agen():
            yield b"abc"
            yield b"def"

        resp = Response(200, stream=agen())
        async for _ in resp.aiter_raw():
            pass
        assert resp.is_closed

    @pytest.mark.asyncio
    async def test_async_raw_increments_num_bytes(self):
        async def agen():
            yield b"abc"
            yield b"def"

        resp = Response(200, stream=agen())
        async for _ in resp.aiter_raw():
            pass
        assert resp.num_bytes_downloaded == 6

    @pytest.mark.asyncio
    async def test_async_raw_chunk_size_none(self):
        async def agen():
            yield b"abc"
            yield b"def"

        resp = Response(200, stream=agen())
        chunks = []
        async for chunk in resp.aiter_raw(chunk_size=None):
            chunks.append(chunk)
        assert chunks == [b"abc", b"def"]
        assert resp.num_bytes_downloaded == 6

    @pytest.mark.asyncio
    async def test_async_raw_chunk_size_splits(self):
        async def agen():
            yield b"abcdefghij"

        resp = Response(200, stream=agen())
        chunks = []
        async for chunk in resp.aiter_raw(chunk_size=3):
            chunks.append(chunk)
        assert chunks == [b"abc", b"def", b"ghi", b"j"]
        assert resp.num_bytes_downloaded == 10

    @pytest.mark.asyncio
    async def test_async_raw_coalesces_small_chunks(self):
        async def agen():
            yield b"a"
            yield b"b"
            yield b"c"
            yield b"d"
            yield b"e"

        resp = Response(200, stream=agen())
        chunks = []
        async for chunk in resp.aiter_raw(chunk_size=3):
            chunks.append(chunk)
        assert chunks == [b"abc", b"de"]
        assert resp.num_bytes_downloaded == 5

    @pytest.mark.asyncio
    async def test_async_raw_empty_chunks(self):
        async def agen():
            yield b""
            yield b"abc"
            yield b""

        resp = Response(200, stream=agen())
        chunks = []
        async for chunk in resp.aiter_raw():
            chunks.append(chunk)
        assert chunks == [b"abc"]
        assert resp.num_bytes_downloaded == 3

    @pytest.mark.asyncio
    async def test_async_raw_zero_length_body(self):
        async def agen():
            return
            yield  # make it async generator

        resp = Response(200, stream=agen())
        chunks = []
        async for chunk in resp.aiter_raw():
            chunks.append(chunk)
        assert chunks == []
        assert resp.num_bytes_downloaded == 0
        assert resp.is_stream_consumed

    @pytest.mark.asyncio
    async def test_async_raw_second_iteration_after_consumption(self):
        async def agen():
            yield b"abc"

        resp = Response(200, stream=agen())
        async for _ in resp.aiter_raw():
            pass
        with pytest.raises(StreamConsumed):
            async for _ in resp.aiter_raw():
                pass

    @pytest.mark.asyncio
    async def test_async_raw_elapsed_available(self):
        async def agen():
            yield b"abc"

        resp = Response(200, stream=agen())
        assert resp._elapsed is None
        async for _ in resp.aiter_raw():
            pass
        assert resp.elapsed is not None


# ── Track 1: Raw vs decoded paths are structurally distinct ───────────


class TestRawVsDecodedPaths:
    """Raw and decoded iteration must use distinct paths."""

    def test_raw_and_decoded_use_separate_iterations(self):
        resp = Response(200, stream=iter([b"hello world"]))
        raw_chunks = list(resp.iter_raw())
        assert b"".join(raw_chunks) == b"hello world"

    def test_raw_after_full_decoded_raises(self):
        resp = Response(200, stream=iter([b"abc"]))
        list(resp.iter_bytes())
        with pytest.raises(StreamConsumed):
            list(resp.iter_raw())

    def test_decoded_after_full_raw_raises(self):
        resp = Response(200, stream=iter([b"abc"]))
        list(resp.iter_raw())
        with pytest.raises(StreamConsumed):
            list(resp.iter_bytes())

    def test_raw_chunk_size_handling_independent_of_decoded(self):
        resp1 = Response(200, stream=iter([b"abcdefghij"]))
        resp2 = Response(200, stream=iter([b"abcdefghij"]))
        raw = list(resp1.iter_raw(chunk_size=3))
        decoded = list(resp2.iter_bytes(chunk_size=5))
        assert raw == [b"abc", b"def", b"ghi", b"j"]
        assert decoded == [b"abcde", b"fghij"]

    def test_iter_text_after_raw_raises(self):
        resp = Response(200, stream=iter([b"abc"]))
        list(resp.iter_raw())
        with pytest.raises(StreamConsumed):
            list(resp.iter_text())

    def test_iter_lines_after_raw_raises(self):
        resp = Response(200, stream=iter([b"line1\nline2\n"]))
        list(resp.iter_raw())
        with pytest.raises(StreamConsumed):
            list(resp.iter_lines())


# ── Track 3: Byte accounting is raw-authoritative ──────────────────────


class TestRawByteAccounting:
    """num_bytes_downloaded must track raw transport bytes."""

    def test_accounting_increments_per_chunk(self):
        resp = Response(200, stream=iter([b"aa", b"bb", b"cc"]))
        gen = resp.iter_raw(chunk_size=1)
        assert resp.num_bytes_downloaded == 0
        first = next(gen)
        assert resp.num_bytes_downloaded == 1
        second = next(gen)
        assert resp.num_bytes_downloaded == 2
        rest = list(gen)
        assert resp.num_bytes_downloaded == 6

    def test_accounting_with_coalescing(self):
        resp = Response(200, stream=iter([b"a", b"b", b"c"]))
        chunks = list(resp.iter_raw(chunk_size=5))
        assert chunks == [b"abc"]
        assert resp.num_bytes_downloaded == 3

    def test_accounting_with_splitting(self):
        resp = Response(200, stream=iter([b"abcdef"]))
        chunks = list(resp.iter_raw(chunk_size=2))
        assert chunks == [b"ab", b"cd", b"ef"]
        assert resp.num_bytes_downloaded == 6

    def test_buffered_response_accounting(self):
        resp = Response(200, content=b"hello")
        assert resp.num_bytes_downloaded == 0
        list(resp.iter_raw())
        assert resp.num_bytes_downloaded == 5


# ── Track 5: Close exactly once ────────────────────────────────────────


class TestRawCloseBehavior:
    """Close must happen exactly once on exhaustion and be idempotent."""

    def test_close_on_normal_exhaustion(self):
        resp = Response(200, stream=iter([b"abc"]))
        list(resp.iter_raw())
        assert resp.is_closed
        resp.close()
        assert resp.is_closed

    def test_close_idempotent_after_partial(self):
        resp = Response(200, stream=iter([b"a", b"b", b"c"]))
        gen = resp.iter_raw()
        next(gen)
        resp.close()
        assert resp.is_closed
        resp.close()
        assert resp.is_closed

    @pytest.mark.asyncio
    async def test_async_close_on_exhaustion(self):
        async def agen():
            yield b"abc"

        resp = Response(200, stream=agen())
        async for _ in resp.aiter_raw():
            pass
        assert resp.is_closed


# ── Track 6: Decoded iteration not broken ──────────────────────────────


class TestDecodedIterationIntact:
    """Existing decoded iteration must remain correct after raw fixes."""

    def test_iter_bytes_still_works(self):
        resp = Response(200, stream=iter([b"hello", b" ", b"world"]))
        chunks = list(resp.iter_bytes(chunk_size=5))
        assert b"".join(chunks) == b"hello world"
        assert resp.is_stream_consumed

    def test_iter_text_still_works(self):
        resp = Response(200, stream=iter([b"hello", b" ", b"world"]))
        text = "".join(resp.iter_text())
        assert text == "hello world"

    def test_iter_lines_still_works(self):
        resp = Response(200, stream=iter([b"line1\nline2\nline3\n"]))
        lines = list(resp.iter_lines())
        assert lines == ["line1", "line2", "line3"]

    def test_split_utf8_still_works(self):
        value = "€".encode("utf-8")
        resp = Response(200, stream=iter([value[:1], value[1:]]))
        assert list(resp.iter_text(chunk_size=1)) == ["€"]
        assert resp.num_bytes_downloaded == len(value)

    def test_read_after_iter_bytes_streaming(self):
        resp = Response(200, stream=iter([b"abc"]))
        list(resp.iter_bytes())
        with pytest.raises(StreamConsumed):
            resp.read()

    def test_read_after_iter_bytes_buffered(self):
        resp = Response(200, content=b"abc")
        list(resp.iter_bytes())
        assert resp.read() == b"abc"


# ── Native stream integration (Track 7.3: must exercise native path) ──


class TestNativeRawStreamIntegration:
    """Use Client.stream() to exercise the native transport/raw path."""

    def test_native_sync_raw_iter_marks_consumed(self):
        def handler(request):
            return Response(200, content=b"hello world")

        with Client(transport=MockTransport(handler)) as client:
            with client.stream("GET", "https://example.com") as resp:
                assert not resp.is_stream_consumed
                chunks = list(resp.iter_raw())
                assert resp.is_stream_consumed
                assert b"".join(chunks) == b"hello world"
                assert resp.num_bytes_downloaded == 11

    def test_native_sync_raw_iter_closes(self):
        def handler(request):
            return Response(200, content=b"data")

        with Client(transport=MockTransport(handler)) as client:
            with client.stream("GET", "https://example.com") as resp:
                list(resp.iter_raw())
                assert resp.is_closed

    def test_native_sync_raw_chunk_size(self):
        def handler(request):
            return Response(200, content=b"abcdefghij")

        with Client(transport=MockTransport(handler)) as client:
            with client.stream("GET", "https://example.com") as resp:
                chunks = list(resp.iter_raw(chunk_size=3))
                assert chunks == [b"abc", b"def", b"ghi", b"j"]

    def test_native_sync_raw_second_iteration_raises(self):
        def handler(request):
            return Response(200, content=b"abc")

        with Client(transport=MockTransport(handler)) as client:
            with client.stream("GET", "https://example.com") as resp:
                list(resp.iter_raw())
                with pytest.raises(StreamConsumed):
                    list(resp.iter_raw())

    @pytest.mark.asyncio
    async def test_native_async_raw_aiter_marks_consumed(self):
        def handler(request):
            return Response(200, content=b"hello world")

        async with AsyncClient(transport=MockTransport(handler)) as client:
            async with client.stream("GET", "https://example.com") as resp:
                assert not resp.is_stream_consumed
                chunks = []
                async for chunk in resp.aiter_raw():
                    chunks.append(chunk)
                assert resp.is_stream_consumed
                assert b"".join(chunks) == b"hello world"
                assert resp.num_bytes_downloaded == 11

    @pytest.mark.asyncio
    async def test_native_async_raw_aiter_closes(self):
        def handler(request):
            return Response(200, content=b"data")

        async with AsyncClient(transport=MockTransport(handler)) as client:
            async with client.stream("GET", "https://example.com") as resp:
                async for _ in resp.aiter_raw():
                    pass
                assert resp.is_closed

    @pytest.mark.asyncio
    async def test_native_async_raw_chunk_size(self):
        def handler(request):
            return Response(200, content=b"abcdefghij")

        async with AsyncClient(transport=MockTransport(handler)) as client:
            async with client.stream("GET", "https://example.com") as resp:
                chunks = []
                async for chunk in resp.aiter_raw(chunk_size=3):
                    chunks.append(chunk)
                assert chunks == [b"abc", b"def", b"ghi", b"j"]

    @pytest.mark.asyncio
    async def test_native_async_raw_second_iteration_raises(self):
        def handler(request):
            return Response(200, content=b"abc")

        async with AsyncClient(transport=MockTransport(handler)) as client:
            async with client.stream("GET", "https://example.com") as resp:
                async for _ in resp.aiter_raw():
                    pass
                with pytest.raises(StreamConsumed):
                    async for _ in resp.aiter_raw():
                        pass


# ── Compressed response raw vs decoded distinction ─────────────────────


class TestCompressedRawVsDecoded:
    """Compressed responses must show raw != decoded for raw iteration."""

    def test_gzip_raw_and_decoded_differ(self):
        original = b"hello " * 100
        compressed = gzip.compress(original)

        def handler(request):
            return Response(
                200,
                content=compressed,
                headers={"Content-Encoding": "gzip"},
            )

        with Client(transport=MockTransport(handler)) as client:
            with client.stream("GET", "https://example.com") as resp:
                raw_chunks = list(resp.iter_raw())
                raw_data = b"".join(raw_chunks)
                assert raw_data == compressed
                assert len(raw_data) < len(original)
