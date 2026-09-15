"""Downstream-style smoke fixture for the intentionally public facades."""

from eggfetch.compat.httpx import Client, Response
from eggfetch.compat.httpx2 import AsyncClient


def sync_facade() -> Response:
    with Client() as client:
        return client.get("http://127.0.0.1/")


async def async_facade() -> None:
    async with AsyncClient() as client:
        response = await client.get("http://127.0.0.1/")
        assert response.status_code >= 0
