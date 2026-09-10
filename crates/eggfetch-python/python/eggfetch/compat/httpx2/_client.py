"""HTTPX2 2.12.0-compatible Client/AsyncClient for eggfetch.

Subclasses the 0.28.1-compatible clients (single shared Rust engine) and
adds the HTTPX2 delta: ``query()``, ``sse()``, and ``websocket()``.
All other behavior (redirects, cookies, auth, proxies, timeouts, retries)
is inherited unchanged. Importing this module never mutates
``eggfetch.compat.httpx``.
"""

from __future__ import annotations

import typing
from contextlib import asynccontextmanager, contextmanager

from eggfetch.compat.httpx._client import AsyncClient as _BaseAsyncClient
from eggfetch.compat.httpx._client import Client as _BaseClient
from eggfetch.compat.httpx._client import _USE_CLIENT_DEFAULT as USE_CLIENT_DEFAULT

UseClientDefault = typing.Any

if typing.TYPE_CHECKING:
    from eggfetch.compat.httpx2._sse import EventSource
    from eggfetch.compat.httpx2.websockets._api import (
        AsyncWebSocketSession,
        WebSocketSession,
    )


class Client(_BaseClient):
    """Sync HTTPX2-compatible client (Rust engine, httpx2 surface)."""

    def query(
        self,
        url,
        *,
        content=None,
        data=None,
        files=None,
        json=None,
        params=None,
        headers=None,
        cookies=None,
        auth=USE_CLIENT_DEFAULT,
        follow_redirects=USE_CLIENT_DEFAULT,
        timeout=USE_CLIENT_DEFAULT,
        extensions=None,
    ):
        """Send a ``QUERY`` request."""
        return self.request(
            "QUERY",
            url,
            content=content,
            data=data,
            files=files,
            json=json,
            params=params,
            headers=headers,
            cookies=cookies,
            auth=auth,
            follow_redirects=follow_redirects,
            timeout=timeout,
            extensions=extensions,
        )

    @contextmanager
    def sse(
        self,
        url,
        *,
        method: str = "GET",
        content=None,
        data=None,
        files=None,
        json=None,
        params=None,
        headers=None,
        cookies=None,
        auth=USE_CLIENT_DEFAULT,
        follow_redirects=USE_CLIENT_DEFAULT,
        timeout=USE_CLIENT_DEFAULT,
        extensions=None,
        max_event_size: int | None = None,
    ):
        """Connect to an SSE endpoint and yield an ``EventSource``."""
        from eggfetch.compat.httpx2._config import DEFAULT_MAX_EVENT_SIZE_BYTES
        from eggfetch.compat.httpx2._headers import Headers
        from eggfetch.compat.httpx2._sse import EventSource

        if max_event_size is None:
            max_event_size = DEFAULT_MAX_EVENT_SIZE_BYTES
        with self.stream(
            method,
            url,
            content=content,
            data=data,
            files=files,
            json=json,
            params=params,
            headers={"Accept": "text/event-stream", "Cache-Control": "no-store"}
            | Headers(headers or {}),
            cookies=cookies,
            auth=auth,
            follow_redirects=follow_redirects,
            timeout=timeout,
            extensions=extensions,
        ) as response:
            yield EventSource(response, max_event_size=max_event_size)

    @contextmanager
    def websocket(
        self,
        url,
        *,
        max_message_size_bytes: int = 65_536,
        queue_size: int = 512,
        keepalive_ping_interval_seconds: float | None = 20.0,
        keepalive_ping_timeout_seconds: float | None = 20.0,
        subprotocols: list[str] | None = None,
        params=None,
        headers=None,
        cookies=None,
        auth=USE_CLIENT_DEFAULT,
        follow_redirects=USE_CLIENT_DEFAULT,
        timeout=USE_CLIENT_DEFAULT,
        extensions=None,
    ):
        """Open a WebSocket session over the existing 101 upgrade path."""
        try:
            from eggfetch.compat.httpx2.websockets._api import connect_ws
        except ImportError as e:  # pragma: no cover
            raise ImportError(
                "WebSocket support requires the `wsproto` package. "
                "Install it with `pip install httpx2[ws]`."
            ) from e
        with connect_ws(
            str(url),
            self,
            max_message_size_bytes=max_message_size_bytes,
            queue_size=queue_size,
            keepalive_ping_interval_seconds=keepalive_ping_interval_seconds,
            keepalive_ping_timeout_seconds=keepalive_ping_timeout_seconds,
            subprotocols=subprotocols,
            params=params,
            headers=headers,
            cookies=cookies,
            auth=auth,
            follow_redirects=follow_redirects,
            timeout=timeout,
            extensions=extensions,
        ) as ws:
            yield ws


class AsyncClient(_BaseAsyncClient):
    """Async HTTPX2-compatible client (Rust engine, httpx2 surface)."""

    async def query(
        self,
        url,
        *,
        content=None,
        data=None,
        files=None,
        json=None,
        params=None,
        headers=None,
        cookies=None,
        auth=USE_CLIENT_DEFAULT,
        follow_redirects=USE_CLIENT_DEFAULT,
        timeout=USE_CLIENT_DEFAULT,
        extensions=None,
    ):
        """Send a ``QUERY`` request."""
        return await self.request(
            "QUERY",
            url,
            content=content,
            data=data,
            files=files,
            json=json,
            params=params,
            headers=headers,
            cookies=cookies,
            auth=auth,
            follow_redirects=follow_redirects,
            timeout=timeout,
            extensions=extensions,
        )

    @asynccontextmanager
    async def sse(
        self,
        url,
        *,
        method: str = "GET",
        content=None,
        data=None,
        files=None,
        json=None,
        params=None,
        headers=None,
        cookies=None,
        auth=USE_CLIENT_DEFAULT,
        follow_redirects=USE_CLIENT_DEFAULT,
        timeout=USE_CLIENT_DEFAULT,
        extensions=None,
        max_event_size: int | None = None,
    ):
        """Connect to an SSE endpoint and yield an ``EventSource``."""
        from eggfetch.compat.httpx2._config import DEFAULT_MAX_EVENT_SIZE_BYTES
        from eggfetch.compat.httpx2._headers import Headers
        from eggfetch.compat.httpx2._sse import EventSource

        if max_event_size is None:
            max_event_size = DEFAULT_MAX_EVENT_SIZE_BYTES
        async with self.stream(
            method,
            url,
            content=content,
            data=data,
            files=files,
            json=json,
            params=params,
            headers={"Accept": "text/event-stream", "Cache-Control": "no-store"}
            | Headers(headers or {}),
            cookies=cookies,
            auth=auth,
            follow_redirects=follow_redirects,
            timeout=timeout,
            extensions=extensions,
        ) as response:
            yield EventSource(response, max_event_size=max_event_size)

    @asynccontextmanager
    async def websocket(
        self,
        url,
        *,
        max_message_size_bytes: int = 65_536,
        queue_size: int = 512,
        keepalive_ping_interval_seconds: float | None = 20.0,
        keepalive_ping_timeout_seconds: float | None = 20.0,
        subprotocols: list[str] | None = None,
        params=None,
        headers=None,
        cookies=None,
        auth=USE_CLIENT_DEFAULT,
        follow_redirects=USE_CLIENT_DEFAULT,
        timeout=USE_CLIENT_DEFAULT,
        extensions=None,
    ):
        """Open an async WebSocket session over the existing upgrade path."""
        try:
            from eggfetch.compat.httpx2.websockets._api import aconnect_ws
        except ImportError as e:  # pragma: no cover
            raise ImportError(
                "WebSocket support requires the `wsproto` package. "
                "Install it with `pip install httpx2[ws]`."
            ) from e
        async with aconnect_ws(
            str(url),
            self,
            max_message_size_bytes=max_message_size_bytes,
            queue_size=queue_size,
            keepalive_ping_interval_seconds=keepalive_ping_interval_seconds,
            keepalive_ping_timeout_seconds=keepalive_ping_timeout_seconds,
            subprotocols=subprotocols,
            params=params,
            headers=headers,
            cookies=cookies,
            auth=auth,
            follow_redirects=follow_redirects,
            timeout=timeout,
            extensions=extensions,
        ) as ws:
            yield ws


__all__ = ["AsyncClient", "Client", "USE_CLIENT_DEFAULT", "UseClientDefault"]
