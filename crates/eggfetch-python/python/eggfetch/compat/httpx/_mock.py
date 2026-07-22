"""HTTPX-compatible MockTransport for eggfetch."""

from __future__ import annotations

import asyncio
import typing

from eggfetch.compat.httpx._request import Request
from eggfetch.compat.httpx._response import Response

if typing.TYPE_CHECKING:
    from typing import Any, Callable


class MockTransport:
    """A transport that uses a handler function to produce responses.

    Usage::

        def handler(request):
            return Response(200, text="Hello")

        with Client(transport=MockTransport(handler)) as client:
            response = client.get("http://testserver/")

    The handler may also return a :class:`Response` backed by a custom
    stream (sync or async iterator).  Streaming responses are consumed
    once, matching real network behaviour.
    """

    def __init__(self, handler: Callable[[Request], Response]) -> None:
        self._handler = handler
        self._is_closed = False

    def handle_request(self, request: Request) -> Response:
        """Call the handler with the request and return its response.

        Raises :class:`RuntimeError` if the transport is closed, and
        :class:`TypeError` if the handler does not return a
        :class:`Response`.
        """
        if self._is_closed:
            raise RuntimeError("MockTransport is closed")
        if asyncio.iscoroutinefunction(self._handler):
            raise RuntimeError(
                "Cannot use an async handler with a synchronous client. "
                "Use AsyncClient instead."
            )
        response = self._handler(request)
        if not isinstance(response, Response):
            raise TypeError(
                f"Handler must return a Response, got {type(response)}"
            )
        if response.request is None:
            response._request = request  # type: ignore[attr-defined]
        return response

    async def handle_async_request(self, request: Request) -> Response:
        """Async variant — calls the same handler.

        If the handler is a plain (non-async) function it is called
        directly.  If the handler is a coroutine function it is awaited.
        """
        if self._is_closed:
            raise RuntimeError("MockTransport is closed")
        if asyncio.iscoroutinefunction(self._handler):
            response = await self._handler(request)
        else:
            response = self._handler(request)
        if not isinstance(response, Response):
            raise TypeError(
                f"Handler must return a Response, got {type(response)}"
            )
        if response.request is None:
            response._request = request  # type: ignore[attr-defined]
        return response

    def close(self) -> None:
        """Close the transport (idempotent)."""
        self._is_closed = True

    async def aclose(self) -> None:
        """Async close the transport (idempotent)."""
        self._is_closed = True

    def __enter__(self) -> MockTransport:
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()

    async def __aenter__(self) -> MockTransport:
        return self

    async def __aexit__(self, *args: Any) -> None:
        await self.aclose()


def _build_response(
    status_code: int = 200,
    *,
    headers: dict | None = None,
    content: bytes | None = None,
    text: str | None = None,
    json: Any = None,
    stream: Any = None,
) -> Response:
    """Helper to build mock responses easily.

    Supports the same body parameters as :class:`Response`, plus an
    optional *stream* for streaming mock responses.
    """
    if stream is not None:
        return Response(status_code, headers=headers, stream=stream)
    if text is not None:
        return Response(status_code, headers=headers, text=text)
    if json is not None:
        return Response(status_code, headers=headers, json=json)
    return Response(status_code, headers=headers, content=content or b"")
