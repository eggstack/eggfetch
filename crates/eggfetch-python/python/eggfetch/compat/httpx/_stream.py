"""HTTPX-compatible stream base classes for eggfetch."""

from __future__ import annotations

import typing

if typing.TYPE_CHECKING:
    from typing import AsyncIterator, Iterator


class SyncByteStream:
    def __init__(self, *args, **kwargs):
        pass

    def close(self) -> None:
        pass


class AsyncByteStream:
    def __init__(self, *args, **kwargs):
        pass

    async def aclose(self) -> None:
        pass


class ByteStream(AsyncByteStream, SyncByteStream):
    def __init__(self, stream: bytes | None = None) -> None:
        self._content = stream if stream is not None else b""
        self._is_closed = False

    def __iter__(self) -> Iterator[bytes]:
        yield self._content

    async def __aiter__(self) -> AsyncIterator[bytes]:
        yield self._content

    def close(self) -> None:
        self._is_closed = True

    async def aclose(self) -> None:
        self._is_closed = True

    def __enter__(self) -> ByteStream:
        return self

    def __exit__(self, *args) -> None:
        self.close()

    async def __aenter__(self) -> ByteStream:
        return self

    async def __aexit__(self, *args) -> None:
        await self.aclose()
