"""Category: asyncio SDK/client behavior — exercises AsyncClient async dispatch."""

import asyncio
import sys

sys.path.insert(0, "crates/eggfetch-python/python")

from eggfetch.compat.httpx import AsyncClient


async def _async_get(http_server):
    async with AsyncClient() as c:
        r = await c.get(f"{http_server}/get")
        assert r.status_code == 200
        data = r.json()
        assert data["method"] == "GET"


async def _async_post_json(http_server):
    async with AsyncClient() as c:
        r = await c.post(f"{http_server}/post", json={"key": "value"})
        assert r.status_code == 200
        data = r.json()
        assert data["data"] == {"key": "value"}


async def _async_request_method(http_server):
    async with AsyncClient() as c:
        r = await c.request("GET", f"{http_server}/get")
        assert r.status_code == 200


def test_async_get(http_server):
    asyncio.run(_async_get(http_server))


def test_async_post_json(http_server):
    asyncio.run(_async_post_json(http_server))


def test_async_dispatch(http_server):
    asyncio.run(_async_request_method(http_server))
