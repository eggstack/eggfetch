"""HTTPX-compatible Response class for eggfetch."""

from __future__ import annotations

import json as _json
import typing
from datetime import timedelta

from eggfetch.compat.httpx._urls import URL
from eggfetch.compat.httpx._headers import Headers
from eggfetch.compat.httpx._cookies import Cookies
from eggfetch.compat.httpx._exceptions import HTTPStatusError

if typing.TYPE_CHECKING:
    from typing import Any, AsyncIterator, Iterator


class Response:
    """HTTPX-compatible Response object."""

    __slots__ = (
        "_status_code",
        "_headers",
        "_url",
        "_reason_phrase",
        "_http_version",
        "_encoding",
        "_default_encoding",
        "_content",
        "_text",
        "_json",
        "_cookies",
        "_request",
        "_extensions",
        "_history",
        "_elapsed",
        "_num_bytes_downloaded",
        "_stream",
        "_is_closed",
        "_stream_consumed",
        "_native_stream",
    )

    _REASON_PHRASES: dict[int, str] = {
        100: "Continue",
        101: "Switching Protocols",
        102: "Processing",
        103: "Early Hints",
        200: "OK",
        201: "Created",
        202: "Accepted",
        203: "Non-Authoritative Information",
        204: "No Content",
        205: "Reset Content",
        206: "Partial Content",
        207: "Multi-Status",
        208: "Already Reported",
        226: "IM Used",
        300: "Multiple Choices",
        301: "Moved Permanently",
        302: "Found",
        303: "See Other",
        304: "Not Modified",
        305: "Use Proxy",
        307: "Temporary Redirect",
        308: "Permanent Redirect",
        400: "Bad Request",
        401: "Unauthorized",
        402: "Payment Required",
        403: "Forbidden",
        404: "Not Found",
        405: "Method Not Allowed",
        406: "Not Acceptable",
        407: "Proxy Authentication Required",
        408: "Request Timeout",
        409: "Conflict",
        410: "Gone",
        411: "Length Required",
        412: "Precondition Failed",
        413: "Content Too Large",
        414: "URI Too Long",
        415: "Unsupported Media Type",
        416: "Range Not Satisfiable",
        417: "Expectation Failed",
        418: "I'm a Teapot",
        421: "Misdirected Request",
        422: "Unprocessable Content",
        423: "Locked",
        424: "Failed Dependency",
        425: "Too Early",
        426: "Upgrade Required",
        428: "Precondition Required",
        429: "Too Many Requests",
        431: "Request Header Fields Too Large",
        451: "Unavailable For Legal Reasons",
        500: "Internal Server Error",
        501: "Not Implemented",
        502: "Bad Gateway",
        503: "Service Unavailable",
        504: "Gateway Timeout",
        505: "HTTP Version Not Supported",
        506: "Variant Also Negotiates",
        507: "Insufficient Storage",
        508: "Loop Detected",
        510: "Not Extended",
        511: "Network Authentication Required",
    }

    def __init__(
        self,
        status_code: int,
        *,
        headers=None,
        content: bytes | None = None,
        text: str | None = None,
        html: str | None = None,
        json: Any = None,
        stream=None,
        request=None,
        extensions: dict | None = None,
        history: list | None = None,
        default_encoding: str = "utf-8",
    ) -> None:
        # NOTE: ``extensions`` here are *response* extensions (http_version,
        # reason_phrase, etc.).  Request extensions live on
        # ``request.extensions`` and must NOT be merged into response
        # extensions (Track 5.3).
        # Validate mutual exclusion of content sources
        body_sources = [
            name
            for name, val in [
                ("content", content),
                ("text", text),
                ("html", html),
                ("json", json),
                ("stream", stream),
            ]
            if val is not None
        ]
        if len(body_sources) > 1:
            raise ValueError(
                f"Conflicting body sources: {', '.join(body_sources)}. "
                "Only one of content, text, html, json, or stream may be provided."
            )

        self._status_code = status_code
        self._default_encoding = default_encoding
        self._request = request
        self._extensions: dict = extensions if extensions is not None else {}
        self._history: list[Response] = list(history) if history else []
        self._is_closed = False
        self._stream_consumed = False
        self._stream = stream
        self._native_stream = None
        self._num_bytes_downloaded = 0

        # Build headers
        if isinstance(headers, Headers):
            self._headers = headers
        else:
            self._headers = Headers(headers)

        # Build content
        self._content: bytes | None = None
        self._json: Any = None
        self._text: str | None = None
        self._encoding: str | None = None

        if content is not None:
            self._content = content if isinstance(content, bytes) else content.encode("utf-8")
            self._num_bytes_downloaded = len(self._content)
        elif text is not None:
            self._text = text
            enc = self.charset_encoding or default_encoding
            self._content = text.encode(enc)
            self._num_bytes_downloaded = len(self._content)
        elif html is not None:
            self._text = html
            self._encoding = "utf-8"
            self._content = html.encode("utf-8")
            self._num_bytes_downloaded = len(self._content)
            if "content-type" not in self._headers:
                self._headers["content-type"] = "text/html; charset=utf-8"
        elif json is not None:
            self._json = json
            self._content = _json.dumps(json).encode("utf-8")
            self._num_bytes_downloaded = len(self._content)
            if "content-type" not in self._headers:
                self._headers["content-type"] = "application/json"
        elif stream is None:
            self._content = b""
            self._num_bytes_downloaded = 0

        # Derived properties
        self._reason_phrase = self._REASON_PHRASES.get(status_code, "")
        self._http_version = "HTTP/1.1"
        self._url = URL("")
        self._elapsed = timedelta(0)

        # Extract URL from request
        if request is not None and hasattr(request, "url"):
            self._url = request.url

        # Build cookies from headers
        self._cookies = Cookies()
        self._cookies.set_cookie_header(self)

    @property
    def status_code(self) -> int:
        return self._status_code

    @property
    def headers(self) -> Headers:
        return self._headers

    @property
    def url(self) -> URL:
        return self._url

    @property
    def reason_phrase(self) -> str:
        return self._reason_phrase

    @property
    def http_version(self) -> str:
        return self._http_version

    @property
    def encoding(self) -> str | None:
        return self._encoding

    @encoding.setter
    def encoding(self, value: str) -> None:
        self._encoding = value

    @property
    def charset_encoding(self) -> str | None:
        content_type = self._headers.get("content-type", "")
        if not content_type:
            return None
        for part in content_type.split(";"):
            part = part.strip()
            if part.lower().startswith("charset="):
                return part.split("=", 1)[1].strip()
        return None

    @property
    def content(self) -> bytes:
        if self._content is None:
            if self._stream is not None:
                raise RuntimeError(
                    "Response content has not been read. Call .read() or .aread() first."
                )
            return b""
        return self._content

    @property
    def text(self) -> str:
        if self._content is None:
            if self._stream is not None:
                raise RuntimeError(
                    "Response content has not been read. Call .read() or .aread() first."
                )
            return ""
        enc = self._encoding or self.charset_encoding or self._default_encoding
        return self._content.decode(enc)

    @property
    def cookies(self) -> Cookies:
        return self._cookies

    @property
    def elapsed(self) -> timedelta:
        return self._elapsed

    @property
    def history(self) -> list[Response]:
        return self._history

    @property
    def request(self):
        return self._request

    @property
    def extensions(self) -> dict:
        return self._extensions

    @property
    def links(self) -> dict:
        link_header = self._headers.get("link", "")
        if not link_header:
            return {}
        result: dict[str, dict[str, str]] = {}
        for part in link_header.split(","):
            part = part.strip()
            if not part:
                continue
            url_part, *param_parts = part.split(";")
            url_str = url_part.strip().strip("<>")
            attrs: dict[str, str] = {}
            for param in param_parts:
                param = param.strip()
                if "=" in param:
                    key, _, value = param.partition("=")
                    attrs[key.strip()] = value.strip().strip('"')
            rel = attrs.pop("rel", "alternate")
            result[rel] = {"url": url_str, **attrs}
        return result

    @property
    def num_bytes_downloaded(self) -> int:
        return self._num_bytes_downloaded

    @property
    def is_success(self) -> bool:
        return 200 <= self._status_code < 300

    @property
    def is_redirect(self) -> bool:
        return self._status_code in (301, 302, 303, 307, 308) or (
            300 <= self._status_code < 400
        )

    @property
    def is_client_error(self) -> bool:
        return 400 <= self._status_code < 500

    @property
    def is_server_error(self) -> bool:
        return 500 <= self._status_code < 600

    @property
    def is_error(self) -> bool:
        return self.is_client_error or self.is_server_error

    @property
    def is_informational(self) -> bool:
        return 100 <= self._status_code < 200

    @property
    def is_closed(self) -> bool:
        return self._is_closed

    @property
    def has_redirect_location(self) -> bool:
        return "location" in self._headers and self.is_redirect

    def json(self, **kwargs) -> Any:
        if self._json is not None:
            return self._json
        if self._content is None:
            if self._stream is not None:
                raise RuntimeError(
                    "Response content has not been read. Call .read() or .aread() first."
                )
            raise RuntimeError("Response content is empty.")
        return _json.loads(self._content, **kwargs)

    def raise_for_status(self) -> Response:
        if self.is_error:
            message = f"Server error {self._status_code}"
            if self._reason_phrase:
                message = f"Client error {self._status_code}" if self.is_client_error else f"Server error {self._status_code}"
            raise HTTPStatusError(message=message, request=self._request, response=self)
        return self

    def read(self) -> bytes:
        if self._native_stream is not None and not self._stream_consumed:
            content = self._native_stream.read()
            if isinstance(content, bytes):
                self._content = content
            else:
                self._content = bytes(content)
            self._num_bytes_downloaded = len(self._content)
            self._stream_consumed = True
            return self._content
        if self._stream is not None and not self._stream_consumed:
            chunks: list[bytes] = []
            for chunk in self._stream:
                if isinstance(chunk, bytes):
                    chunks.append(chunk)
                elif isinstance(chunk, str):
                    chunks.append(chunk.encode("utf-8"))
                else:
                    chunks.append(str(chunk).encode("utf-8"))
            self._content = b"".join(chunks)
            self._num_bytes_downloaded = len(self._content)
            self._stream_consumed = True
        if self._content is None:
            return b""
        return self._content

    async def aread(self) -> bytes:
        if self._native_stream is not None and not self._stream_consumed:
            content = await self._native_stream.aread()
            if isinstance(content, bytes):
                self._content = content
            else:
                self._content = bytes(content)
            self._num_bytes_downloaded = len(self._content)
            self._stream_consumed = True
            return self._content
        if self._stream is not None and not self._stream_consumed:
            chunks: list[bytes] = []
            async for chunk in self._stream:  # type: ignore[union-attr]
                if isinstance(chunk, bytes):
                    chunks.append(chunk)
                elif isinstance(chunk, str):
                    chunks.append(chunk.encode("utf-8"))
                else:
                    chunks.append(str(chunk).encode("utf-8"))
            self._content = b"".join(chunks)
            self._num_bytes_downloaded = len(self._content)
            self._stream_consumed = True
        if self._content is None:
            return b""
        return self._content

    def close(self) -> None:
        self._is_closed = True
        if self._native_stream is not None and hasattr(self._native_stream, "close"):
            self._native_stream.close()
        if hasattr(self._stream, "close"):
            self._stream.close()

    async def aclose(self) -> None:
        self._is_closed = True
        if self._native_stream is not None and hasattr(self._native_stream, "aclose"):
            await self._native_stream.aclose()
        if hasattr(self._stream, "aclose"):
            await self._stream.aclose()

    def iter_bytes(self, chunk_size: int = 8192) -> Iterator[bytes]:
        if self._native_stream is not None and not self._stream_consumed:
            yield from self._native_stream.iter_bytes(chunk_size=chunk_size)
            return
        data = self.content
        for i in range(0, len(data), chunk_size):
            yield data[i : i + chunk_size]

    def iter_text(self, chunk_size: int = 8192) -> Iterator[str]:
        if self._native_stream is not None and not self._stream_consumed:
            yield from self._native_stream.iter_text(chunk_size=chunk_size)
            return
        enc = self._encoding or self.charset_encoding or self._default_encoding
        data = self.content.decode(enc)
        for i in range(0, len(data), chunk_size):
            yield data[i : i + chunk_size]

    def iter_lines(self) -> Iterator[str]:
        if self._native_stream is not None and not self._stream_consumed:
            yield from self._native_stream.iter_lines()
            return
        text = self.text
        for line in text.split("\n"):
            if line.endswith("\r"):
                line = line[:-1]
            yield line

    def iter_raw(self, chunk_size: int = 8192) -> Iterator[bytes]:
        if self._native_stream is not None and not self._stream_consumed:
            yield from self._native_stream.iter_raw(chunk_size=chunk_size)
            return
        yield from self.iter_bytes(chunk_size)

    async def aiter_bytes(self, chunk_size: int = 8192) -> AsyncIterator[bytes]:
        if self._native_stream is not None and not self._stream_consumed:
            async for chunk in self._native_stream.aiter_bytes(chunk_size=chunk_size):
                yield chunk
            return
        data = self.content
        for i in range(0, len(data), chunk_size):
            yield data[i : i + chunk_size]

    async def aiter_text(self, chunk_size: int = 8192) -> AsyncIterator[str]:
        if self._native_stream is not None and not self._stream_consumed:
            async for chunk in self._native_stream.aiter_text(chunk_size=chunk_size):
                yield chunk
            return
        enc = self._encoding or self.charset_encoding or self._default_encoding
        data = self.content.decode(enc)
        for i in range(0, len(data), chunk_size):
            yield data[i : i + chunk_size]

    async def aiter_lines(self) -> AsyncIterator[str]:
        if self._native_stream is not None and not self._stream_consumed:
            async for line in self._native_stream.aiter_lines():
                yield line
            return
        text = self.text
        for line in text.split("\n"):
            if line.endswith("\r"):
                line = line[:-1]
            yield line

    async def aiter_raw(self, chunk_size: int = 8192) -> AsyncIterator[bytes]:
        if self._native_stream is not None and not self._stream_consumed:
            async for chunk in self._native_stream.aiter_raw(chunk_size=chunk_size):
                yield chunk
            return
        async for chunk in self.aiter_bytes(chunk_size):
            yield chunk

    def __repr__(self) -> str:
        return f"<Response [{self._status_code}]>"
