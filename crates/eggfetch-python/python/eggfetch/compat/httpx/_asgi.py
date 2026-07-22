"""ASGI transport for testing ASGI applications with the HTTPX-compatible client."""

from __future__ import annotations

import asyncio
import concurrent.futures
import typing
from typing import Any, Callable

from eggfetch.compat.httpx._request import Request
from eggfetch.compat.httpx._response import Response

if typing.TYPE_CHECKING:
    pass

# Default chunk size for streaming request bodies through the ASGI
# receive channel (64 KiB).
_CHUNK_SIZE = 65_536


class ASGITransport:
    """Transport for making requests to an ASGI application.

    Usage::

        async def app(scope, receive, send):
            ...

        async with AsyncClient(transport=ASGITransport(app)) as client:
            response = await client.get("http://testserver/")
    """

    def __init__(
        self,
        app: Callable,
        raise_app_exceptions: bool = True,
        root_path: str = "",
        client: tuple[str, int] | None = ("127.0.0.1", 12345),
    ) -> None:
        self._app = app
        self._raise_app_exceptions = raise_app_exceptions
        self._root_path = root_path
        self._client = client

    async def handle_async_request(self, request: Request) -> Response:
        """Convert HTTPX request to ASGI scope/receive/send and call the app."""
        scope = self._build_scope(request)

        status_code = 200
        headers: list[tuple[str, str]] = []
        body_parts: list[bytes] = []

        body = request.content or b""
        body_offset = 0

        async def receive() -> dict:
            nonlocal body_offset
            if body_offset < len(body):
                chunk = body[body_offset : body_offset + _CHUNK_SIZE]
                body_offset += len(chunk)
                more_body = body_offset < len(body)
                return {
                    "type": "http.request",
                    "body": chunk,
                    "more_body": more_body,
                }
            # All body delivered — return empty final frame
            return {
                "type": "http.request",
                "body": b"",
                "more_body": False,
            }

        async def send(message: dict) -> None:
            nonlocal status_code, headers
            if message["type"] == "http.response.start":
                status_code = message["status"]
                headers = [
                    (
                        name.decode() if isinstance(name, bytes) else name,
                        value.decode() if isinstance(value, bytes) else value,
                    )
                    for name, value in message.get("headers", [])
                ]
            elif message["type"] == "http.response.body":
                body_chunk = message.get("body", b"")
                if body_chunk:
                    body_parts.append(body_chunk)

        try:
            await self._app(scope, receive, send)
        except Exception:
            if self._raise_app_exceptions:
                raise
            return Response(500, content=b"Internal Server Error")

        body_bytes = b"".join(body_parts)

        return Response(
            status_code,
            headers=headers,
            content=body_bytes,
        )

    def handle_request(self, request: Request) -> Response:
        """Sync variant — runs async app in an event loop."""
        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            loop = None

        if loop is not None and loop.is_running():
            with concurrent.futures.ThreadPoolExecutor() as pool:
                future = pool.submit(
                    asyncio.run, self.handle_async_request(request)
                )
                return future.result(timeout=30)
        else:
            return asyncio.run(self.handle_async_request(request))

    def _build_scope(self, request: Request) -> dict:
        """Build an ASGI HTTP scope from an HTTPX Request."""
        url = request.url

        scheme = url.scheme or "http"
        host = url.host or "localhost"
        port = url.port or (443 if scheme == "https" else 80)
        path = url.path or "/"

        query_string = url.query or b""
        if isinstance(query_string, str):
            query_string = query_string.encode()

        headers = [
            [name.lower().encode(), value.encode()]
            for name, value in request.headers.items()
        ]

        # raw_path preserves the original path bytes without normalization
        raw_path = path.encode("utf-8") if isinstance(path, str) else path

        scope: dict[str, Any] = {
            "type": "http",
            "asgi": {"version": "3.0", "spec_version": "2.3"},
            "http_version": "1.1",
            "method": request.method,
            "scheme": scheme,
            "path": path,
            "raw_path": raw_path,
            "root_path": self._root_path,
            "query_string": query_string,
            "headers": headers,
            "server": (host, port),
            "client": tuple(self._client) if self._client else None,
            "extensions": {},
        }

        return scope

    async def aclose(self) -> None:
        pass

    async def __aenter__(self) -> ASGITransport:
        return self

    async def __aexit__(self, *args: typing.Any) -> None:
        await self.aclose()
