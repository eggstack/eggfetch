import eggfetch
from collections.abc import AsyncIterator


def sync_use() -> bytes:
    with eggfetch.Client() as client:
        response: eggfetch.Response = client.get("http://127.0.0.1/")
        return response.content


async def async_use() -> None:
    async def body() -> AsyncIterator[bytes]:
        yield b"chunk"

    async with eggfetch.AsyncClient() as client:
        response: eggfetch.Response = await client.post(
            "http://127.0.0.1/", content=body()
        )
        assert response.status_code >= 0
