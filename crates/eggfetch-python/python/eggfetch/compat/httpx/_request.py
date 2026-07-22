"""HTTPX-compatible Request class for eggfetch."""

from __future__ import annotations

import typing
from urllib.parse import urlparse, urlencode

from eggfetch.compat.httpx._urls import URL, QueryParams
from eggfetch.compat.httpx._headers import Headers
from eggfetch.compat.httpx._cookies import Cookies

if typing.TYPE_CHECKING:
    from typing import AsyncIterator, Iterator


class Request:
    """HTTPX-compatible Request object."""

    __slots__ = (
        "_method",
        "_url",
        "_headers",
        "_cookies",
        "_params",
        "_content",
        "_stream",
        "_extensions",
        "_http_version",
        "_is_stream_consumed",
        "_stream_consumed",
    )

    def __init__(
        self,
        method: str,
        url: URL | str,
        *,
        params=None,
        headers=None,
        cookies=None,
        content: bytes | None = None,
        data=None,
        files=None,
        json=None,
        stream=None,
        extensions: dict | None = None,
    ) -> None:
        # Validate mutual exclusion of body sources
        body_sources = [
            name
            for name, val in [
                ("content", content),
                ("data", data),
                ("files", files),
                ("json", json),
                ("stream", stream),
            ]
            if val is not None
        ]
        if len(body_sources) > 1:
            raise ValueError(
                f"Conflicting body sources: {', '.join(body_sources)}. "
                "Only one of content, data, files, json, or stream may be provided."
            )

        self._method = method.upper()
        self._url = URL(url) if not isinstance(url, URL) else url
        self._http_version = "HTTP/1.1"
        self._is_stream_consumed = False
        self._stream_consumed = False
        self._stream = stream

        # Build headers from user-provided value
        if isinstance(headers, Headers):
            self._headers = headers
        else:
            self._headers = Headers(headers)

        # Build params
        if isinstance(params, QueryParams):
            self._params = params
        else:
            self._params = QueryParams(params)

        # Build cookies
        if isinstance(cookies, Cookies):
            self._cookies = cookies
        else:
            self._cookies = Cookies(cookies)

        # Handle body content
        self._content: bytes | None = None

        if content is not None:
            self._content = content if isinstance(content, bytes) else content.encode("utf-8")
        elif json is not None:
            import json as _json

            self._content = _json.dumps(json).encode("utf-8")
            if "content-type" not in self._headers:
                self._headers["content-type"] = "application/json"
        elif data is not None:
            if isinstance(data, dict):
                encoded = urlencode(data)
            elif isinstance(data, (list, tuple)):
                encoded = urlencode(data)
            elif isinstance(data, str):
                encoded = data
            elif isinstance(data, bytes):
                self._content = data
                encoded = None
            else:
                encoded = str(data)
            if self._content is None and encoded is not None:
                self._content = encoded.encode("utf-8")
            if "content-type" not in self._headers:
                self._headers["content-type"] = "application/x-www-form-urlencoded"
        elif files is not None:
            # Simplified: serialize files as multipart-like placeholder
            self._content = self._encode_files(files)
            if "content-type" not in self._headers:
                self._headers["content-type"] = "multipart/form-data"

        # Auto-headers
        host = self._url.host
        if host is not None:
            port = self._url.port
            scheme = self._url.scheme
            if port and (
                (scheme == "http" and port != 80) or (scheme == "https" and port != 443)
            ):
                host = f"{host}:{port}"
            if "host" not in self._headers:
                self._headers["host"] = host

        if stream is not None:
            if "transfer-encoding" not in self._headers:
                self._headers["transfer-encoding"] = "chunked"
        elif self._content is not None:
            if "content-length" not in self._headers:
                self._headers["content-length"] = str(len(self._content))

        # Extensions
        self._extensions: dict = extensions if extensions is not None else {}

    @staticmethod
    def _encode_files(files) -> bytes:
        """Minimal multipart encoding for files parameter."""
        boundary = "----eggfetchboundary"
        parts: list[bytes] = []

        if isinstance(files, dict):
            file_items = files.items()
        elif isinstance(files, (list, tuple)):
            file_items = files
        else:
            file_items = [("file", files)]

        for field_name, file_tuple in file_items:
            if isinstance(file_tuple, tuple):
                if len(file_tuple) == 3:
                    filename, fileobj, content_type = file_tuple
                elif len(file_tuple) == 2:
                    filename, fileobj = file_tuple
                    content_type = "application/octet-stream"
                else:
                    filename = file_tuple[0]
                    fileobj = file_tuple[1] if len(file_tuple) > 1 else b""
                    content_type = "application/octet-stream"
            else:
                filename = str(field_name)
                fileobj = file_tuple
                content_type = "application/octet-stream"

            if isinstance(fileobj, (bytes, bytearray)):
                body = bytes(fileobj)
            elif isinstance(fileobj, str):
                body = fileobj.encode("utf-8")
            elif hasattr(fileobj, "read"):
                body = fileobj.read()
                if isinstance(body, str):
                    body = body.encode("utf-8")
            else:
                body = str(fileobj).encode("utf-8")

            part = (
                f"--{boundary}\r\n"
                f'Content-Disposition: form-data; name="{field_name}"; filename="{filename}"\r\n'
                f"Content-Type: {content_type}\r\n\r\n"
            ).encode("utf-8")
            part += body + b"\r\n"
            parts.append(part)

        parts.append(f"--{boundary}--\r\n".encode("utf-8"))
        return b"".join(parts)

    @property
    def method(self) -> str:
        return self._method

    @property
    def url(self) -> URL:
        return self._url

    @property
    def headers(self) -> Headers:
        return self._headers

    @property
    def cookies(self) -> Cookies:
        return self._cookies

    @property
    def params(self) -> QueryParams:
        return self._params

    @property
    def content(self) -> bytes | None:
        return self._content

    @property
    def stream(self):
        return self._stream

    @property
    def extensions(self) -> dict:
        return self._extensions

    @property
    def is_stream_consumed(self) -> bool:
        return self._is_stream_consumed

    @property
    def http_version(self) -> str:
        return self._http_version

    def read(self) -> bytes:
        if self._stream is not None and self._content is None:
            chunks: list[bytes] = []
            for chunk in self._stream:
                if isinstance(chunk, bytes):
                    chunks.append(chunk)
                elif isinstance(chunk, str):
                    chunks.append(chunk.encode("utf-8"))
                else:
                    chunks.append(str(chunk).encode("utf-8"))
            self._content = b"".join(chunks)
            self._is_stream_consumed = True
            self._stream_consumed = True
        if self._content is None:
            return b""
        return self._content

    async def aread(self) -> bytes:
        if self._stream is not None and self._content is None:
            chunks: list[bytes] = []
            async for chunk in self._stream:  # type: ignore[union-attr]
                if isinstance(chunk, bytes):
                    chunks.append(chunk)
                elif isinstance(chunk, str):
                    chunks.append(chunk.encode("utf-8"))
                else:
                    chunks.append(str(chunk).encode("utf-8"))
            self._content = b"".join(chunks)
            self._is_stream_consumed = True
            self._stream_consumed = True
        if self._content is None:
            return b""
        return self._content

    def __repr__(self) -> str:
        return f"<Request({self._method!r}, {str(self._url)!r})>"
