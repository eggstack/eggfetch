"""This fixture must fail: sync APIs reject async-only request bodies."""

from collections.abc import AsyncIterator

import eggfetch


async def body() -> AsyncIterator[bytes]:
    yield b"chunk"


def invalid_sync_request() -> None:
    with eggfetch.Client() as client:
        client.post("http://127.0.0.1/", content=body())
