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
    """

    def __init__(self, handler: Callable[[Request], Response]) -> None:
        self._handler = handler
        self._is_closed = False

    def handle_request(self, request: Request) -> Response:
        """Call the handler with the request and return its response."""
        if self._is_closed:
            raise RuntimeError("MockTransport is closed")
        response = self._handler(request)
        if not isinstance(response, Response):
            raise TypeError(
                f"Handler must return a Response, got {type(response)}"
            )
        if response.request is None:
            response._request = request  # type: ignore[attr-defined]
        return response

    async def handle_async_request(self, request: Request) -> Response:
        """Async variant - calls the same handler."""
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
) -> Response:
    """Helper to build mock responses easily."""
    if text is not None:
        return Response(status_code, headers=headers, text=text)
    if json is not None:
        return Response(status_code, headers=headers, json=json)
    return Response(status_code, headers=headers, content=content or b"")
