"""HTTPX-compatible stream base classes for eggfetch."""

from __future__ import annotations

import typing

if typing.TYPE_CHECKING:
    from typing import AsyncIterator, Iterator


class ByteStream:
    def __init__(self, content: bytes = b"") -> None:
        self._content = content
        self._is_closed = False

    def __iter__(self) -> Iterator[bytes]:
        yield self._content

    def close(self) -> None:
        self._is_closed = True

    def __enter__(self) -> ByteStream:
        return self

    def __exit__(self, *args) -> None:
        self.close()


class SyncByteStream(ByteStream):
    pass


class AsyncByteStream:
    def __init__(self, content: bytes = b"") -> None:
        self._content = content
        self._is_closed = False

    async def __aiter__(self) -> AsyncIterator[bytes]:
        yield self._content

    async def aclose(self) -> None:
        self._is_closed = True

    async def __aenter__(self) -> AsyncByteStream:
        return self

    async def __aexit__(self, *args) -> None:
        await self.aclose()
