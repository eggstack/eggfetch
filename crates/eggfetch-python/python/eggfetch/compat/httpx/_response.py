"""HTTPX-compatible Response class for eggfetch."""

from __future__ import annotations

import codecs
import json as _json
import time
import typing
from datetime import timedelta
from typing import Any, AsyncIterator, Iterator

from eggfetch.compat.httpx._urls import URL
from eggfetch.compat.httpx._headers import Headers
from eggfetch.compat.httpx._cookies import Cookies
from eggfetch.compat.httpx._exceptions import (
    HTTPStatusError,
    ResponseNotRead,
    StreamClosed,
    StreamConsumed,
)


class _RawByteChunker:
    """Adapt raw source chunks without changing source-byte accounting."""

    def __init__(self, chunk_size: int | None) -> None:
        self._buffer = bytearray()
        self._chunk_size = chunk_size

    def decode(self, content: bytes) -> list[bytes]:
        """Return complete output chunks for one consumed source chunk."""
        if self._chunk_size is None:
            return [content] if content else []

        self._buffer.extend(content)
        if len(self._buffer) >= self._chunk_size:
            value = bytes(self._buffer)
            chunks = [
                value[index : index + self._chunk_size]
                for index in range(0, len(value), self._chunk_size)
            ]
            if len(chunks[-1]) == self._chunk_size:
                self._buffer.clear()
                return chunks

            self._buffer.clear()
            self._buffer.extend(chunks[-1])
            return chunks[:-1]
        return []

    def flush(self) -> list[bytes]:
        """Return the final incomplete chunk, if any."""
        value = bytes(self._buffer)
        self._buffer.clear()
        return [value] if value else []


class Response:
    """HTTPX-compatible Response object."""

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
        default_encoding: str | typing.Callable[[bytes], str] = "utf-8",
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
        self._is_closed = stream is None
        self._stream_consumed = stream is None
        self._stream = stream
        self._native_stream = None
        self._num_bytes_downloaded = 0
        self._next_request = None

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
            self._num_bytes_downloaded = 0
        elif text is not None:
            self._text = text
            enc = self.charset_encoding or (
                self._default_encoding
                if isinstance(self._default_encoding, str)
                else "utf-8"
            )
            self._content = text.encode(enc)
            self._num_bytes_downloaded = 0
        elif html is not None:
            self._text = html
            self._encoding = "utf-8"
            self._content = html.encode("utf-8")
            self._num_bytes_downloaded = 0
            if "content-type" not in self._headers:
                self._headers["content-type"] = "text/html; charset=utf-8"
        elif json is not None:
            self._json = json
            self._content = _json.dumps(json).encode("utf-8")
            self._num_bytes_downloaded = 0
            if "content-type" not in self._headers:
                self._headers["content-type"] = "application/json"
        elif stream is None:
            self._content = b""
            self._num_bytes_downloaded = 0

        if stream is None and "content-length" not in self._headers:
            self._headers["content-length"] = str(len(self._content or b""))

        # ── Protocol metadata from extensions ───────────────────────
        ext_http_version = self._extensions.get("http_version")
        if ext_http_version is not None:
            if isinstance(ext_http_version, bytes):
                self._http_version = ext_http_version.decode("ascii", errors="replace")
            else:
                self._http_version = str(ext_http_version)
        else:
            self._http_version = "HTTP/1.1"

        ext_reason = self._extensions.get("reason_phrase")
        if ext_reason is not None:
            if isinstance(ext_reason, bytes):
                self._reason_phrase = ext_reason.decode("utf-8", errors="replace")
            else:
                self._reason_phrase = str(ext_reason)
        else:
            self._reason_phrase = self._REASON_PHRASES.get(status_code, "")

        # URL derives from request (Track 3.2)
        if request is not None and hasattr(request, "url"):
            self._url = request.url
        else:
            self._url = None

        # Elapsed — undefined until read/close for streaming,
        # timedelta(0) for buffered responses
        if stream is None:
            self._elapsed: timedelta | None = timedelta(0)
        else:
            self._elapsed = None

        # Build cookies from Set-Cookie headers
        self._cookies = Cookies()
        self._cookies.extract_cookies(self)

    @property
    def status_code(self) -> int:
        return self._status_code

    @property
    def headers(self) -> Headers:
        return self._headers

    @property
    def url(self) -> URL:
        if self._request is None:
            raise RuntimeError("The request instance has not been set on this response.")
        return self._url

    @url.setter
    def url(self, value: URL | str) -> None:
        self._url = URL(value) if not isinstance(value, URL) else value

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
        if self._text is not None:
            raise ValueError(
                "The `response.encoding` cannot be set after "
                "`response.text` has been accessed."
            )
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

    def _resolve_encoding(self) -> str:
        """Resolve the effective encoding string."""
        if self._encoding is not None:
            return self._encoding
        cs = self.charset_encoding
        if cs is not None:
            return cs
        if callable(self._default_encoding):
            # HTTPX calls the callable with the content bytes
            if self._content is not None:
                return self._default_encoding(self._content)
            return "utf-8"
        return self._default_encoding

    @property
    def content(self) -> bytes:
        if self._content is None:
            if self._stream is not None or self._native_stream is not None:
                raise ResponseNotRead()
            return b""
        return self._content

    @property
    def text(self) -> str:
        if self._text is not None:
            return self._text
        if self._content is None:
            if self._stream is not None or self._native_stream is not None:
                raise ResponseNotRead()
            return ""
        enc = self._resolve_encoding()
        self._text = self._content.decode(enc)
        return self._text

    @property
    def cookies(self) -> Cookies:
        return self._cookies

    @property
    def elapsed(self) -> timedelta:
        if self._elapsed is None:
            raise RuntimeError(
                "The `response.elapsed` attribute is not available until "
                "the response has been read or closed."
            )
        return self._elapsed

    @elapsed.setter
    def elapsed(self, value: timedelta) -> None:
        self._elapsed = value

    @property
    def history(self) -> list[Response]:
        return self._history

    @history.setter
    def history(self, value: list[Response]) -> None:
        self._history = list(value) if value is not None else []

    @property
    def request(self):
        if self._request is None:
            raise RuntimeError("The request instance has not been set on this response.")
        return self._request

    @request.setter
    def request(self, value) -> None:
        self._request = value
        if value is not None and hasattr(value, "url"):
            self._url = value.url

    @property
    def next_request(self):
        return self._next_request

    @next_request.setter
    def next_request(self, value) -> None:
        self._next_request = value

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
        return self._status_code in (301, 302, 303, 307, 308)

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
    def is_stream_consumed(self) -> bool:
        return self._stream_consumed

    @property
    def has_redirect_location(self) -> bool:
        # HTTPX: only True for redirect statuses where Location is present
        # and the status is one HTTPX treats as a "has redirect location"
        return self._status_code in {301, 302, 303, 307, 308} and "location" in self._headers

    def json(self, **kwargs) -> Any:
        if self._json is not None:
            return self._json
        if self._content is None:
            if self._stream is not None or self._native_stream is not None:
                raise ResponseNotRead()
            raise RuntimeError("Response content is empty.")
        return _json.loads(self._content, **kwargs)

    def raise_for_status(self) -> Response:
        if not self.is_success:
            if self._request is None:
                raise RuntimeError(
                    "No request has been attached to this response. "
                    "Cannot call raise_for_status() without a request."
                )
            if self.is_informational:
                message = (
                    f"Informational response {self._status_code}"
                )
            elif self.is_redirect:
                loc = self.headers.get("location", "")
                message = (
                    f"Redirect response {self._status_code}"
                )
                if loc:
                    message += f" to {loc}"
            elif self.is_client_error:
                message = f"Client error {self._status_code}"
            elif self.is_server_error:
                message = f"Server error {self._status_code}"
            else:
                message = f"Response {self._status_code}"
            raise HTTPStatusError(
                message=message,
                request=self._request,
                response=self,
            )
        return self

    def read(self) -> bytes:
        if self._stream_consumed:
            if self._content is not None:
                return self._content
            raise StreamConsumed()
        if self._native_stream is not None:
            self._content = self._native_stream.read()
        elif self._stream is not None:
            self._content = b"".join(chunk if isinstance(chunk, bytes) else str(chunk).encode() for chunk in self._stream)
        else:
            self._content = self._content or b""
        self._num_bytes_downloaded = len(self._content)
        self._stream_consumed = True
        self.close()
        return self._content

    async def aread(self) -> bytes:
        if self._stream_consumed:
            if self._content is not None:
                return self._content
            raise StreamConsumed()
        if self._native_stream is not None:
            self._content = await self._native_stream.aread()
        elif self._stream is not None:
            chunks = []
            async for chunk in self._stream:
                chunks.append(chunk if isinstance(chunk, bytes) else str(chunk).encode())
            self._content = b"".join(chunks)
        else:
            self._content = self._content or b""
        self._num_bytes_downloaded = len(self._content)
        self._stream_consumed = True
        await self.aclose()
        return self._content

    def close(self) -> None:
        if self._is_closed:
            return
        self._is_closed = True
        if self._native_stream is not None and hasattr(self._native_stream, "close"):
            self._native_stream.close()
        if hasattr(self._stream, "close"):
            self._stream.close()
        if self._elapsed is None:
            start = self._extensions.get("_eggfetch_started_at")
            if start is not None:
                self._elapsed = timedelta(seconds=max(0.0, time.monotonic() - start))

    async def aclose(self) -> None:
        if self._is_closed:
            return
        self._is_closed = True
        if self._native_stream is not None and hasattr(self._native_stream, "aclose"):
            await self._native_stream.aclose()
        if hasattr(self._stream, "aclose"):
            await self._stream.aclose()
        if self._elapsed is None:
            start = self._extensions.get("_eggfetch_started_at")
            if start is not None:
                self._elapsed = timedelta(seconds=max(0.0, time.monotonic() - start))

    def iter_bytes(self, chunk_size: int | None = 8192) -> Iterator[bytes]:
        if self._stream_consumed and self._content is None:
            raise StreamConsumed()
        try:
            if self._native_stream is not None:
                for chunk in self._native_stream.iter_bytes(chunk_size=chunk_size):
                    self._num_bytes_downloaded += len(chunk)
                    yield chunk
            elif self._stream is not None:
                iterator = iter(self._stream)
                if chunk_size is None:
                    for chunk in iterator:
                        self._num_bytes_downloaded += len(chunk)
                        yield chunk
                else:
                    pending = bytearray()
                    for chunk in iterator:
                        pending.extend(chunk)
                        while len(pending) >= chunk_size:
                            chunk = bytes(pending[:chunk_size])
                            self._num_bytes_downloaded += len(chunk)
                            yield chunk
                            del pending[:chunk_size]
                    if pending:
                        chunk = bytes(pending)
                        self._num_bytes_downloaded += len(chunk)
                        yield chunk
            else:
                data = self._content or b""
                size = chunk_size or 8192
                for i in range(0, len(data), size):
                    chunk = data[i:i + size]
                    self._num_bytes_downloaded += len(chunk)
                    yield chunk
            self._stream_consumed = True
        finally:
            self._stream_consumed = True
            self.close()

    def iter_text(self, chunk_size: int | None = 8192) -> Iterator[str]:
        decoder = codecs.getincrementaldecoder(self._resolve_encoding())()
        for chunk in self.iter_bytes(chunk_size):
            text = decoder.decode(chunk, final=False)
            if text:
                yield text
        tail = decoder.decode(b"", final=True)
        if tail:
            yield tail

    def iter_lines(self) -> Iterator[str]:
        pending = ""
        for chunk in self.iter_text():
            pending += chunk
            lines = pending.split("\n")
            pending = lines.pop()
            for line in lines:
                yield line.removesuffix("\r")
        if pending:
            yield pending.removesuffix("\r")

    def iter_raw(self, chunk_size: int | None = None) -> Iterator[bytes]:
        if self._stream_consumed:
            raise StreamConsumed()
        if self._is_closed:
            raise StreamClosed()
        if self._native_stream is None and self._stream is not None and not hasattr(
            self._stream, "__iter__"
        ):
            raise RuntimeError("Attempted to call a sync iterator on an async stream.")

        self._stream_consumed = True
        self._num_bytes_downloaded = 0
        chunker = _RawByteChunker(chunk_size)

        if self._native_stream is not None:
            source = self._native_stream.iter_raw(chunk_size=None)
        elif self._stream is not None:
            source = iter(self._stream)
        else:
            # Buffered responses are consumed and closed at construction.
            raise StreamConsumed()

        for raw_stream_bytes in source:
            self._num_bytes_downloaded += len(raw_stream_bytes)
            yield from chunker.decode(raw_stream_bytes)

        yield from chunker.flush()
        self.close()

    async def aiter_bytes(self, chunk_size: int | None = 8192) -> AsyncIterator[bytes]:
        if self._stream_consumed and self._content is None:
            raise StreamConsumed()
        try:
            if self._native_stream is not None:
                async for chunk in self._native_stream.aiter_bytes(chunk_size=chunk_size):
                    self._num_bytes_downloaded += len(chunk)
                    yield chunk
            elif self._content is not None:
                data = self._content
                size = chunk_size or 8192
                for i in range(0, len(data), size):
                    chunk = data[i:i + size]
                    self._num_bytes_downloaded += len(chunk)
                    yield chunk
            elif self._stream is not None:
                if hasattr(self._stream, '__aiter__'):
                    pending = bytearray()
                    async for chunk in self._stream:
                        pending.extend(chunk)
                        if chunk_size is None:
                            chunk = bytes(pending)
                            self._num_bytes_downloaded += len(chunk)
                            yield chunk
                            pending.clear()
                        else:
                            while len(pending) >= chunk_size:
                                chunk = bytes(pending[:chunk_size])
                                self._num_bytes_downloaded += len(chunk)
                                yield chunk
                                del pending[:chunk_size]
                    if pending:
                        chunk = bytes(pending)
                        self._num_bytes_downloaded += len(chunk)
                        yield chunk
                else:
                    pending = bytearray()
                    for chunk in self._stream:
                        pending.extend(chunk)
                        if chunk_size is None:
                            chunk = bytes(pending)
                            self._num_bytes_downloaded += len(chunk)
                            yield chunk
                            pending.clear()
                        else:
                            while len(pending) >= chunk_size:
                                chunk = bytes(pending[:chunk_size])
                                self._num_bytes_downloaded += len(chunk)
                                yield chunk
                                del pending[:chunk_size]
                    if pending:
                        chunk = bytes(pending)
                        self._num_bytes_downloaded += len(chunk)
                        yield chunk
            else:
                data = self._content or b""
                size = chunk_size or 8192
                for i in range(0, len(data), size):
                    chunk = data[i:i + size]
                    self._num_bytes_downloaded += len(chunk)
                    yield chunk
            self._stream_consumed = True
        finally:
            self._stream_consumed = True
            await self.aclose()

    async def aiter_text(self, chunk_size: int | None = 8192) -> AsyncIterator[str]:
        decoder = codecs.getincrementaldecoder(self._resolve_encoding())()
        async for chunk in self.aiter_bytes(chunk_size):
            text = decoder.decode(chunk, final=False)
            if text:
                yield text
        tail = decoder.decode(b"", final=True)
        if tail:
            yield tail

    async def aiter_lines(self) -> AsyncIterator[str]:
        pending = ""
        async for chunk in self.aiter_text():
            pending += chunk
            lines = pending.split("\n")
            pending = lines.pop()
            for line in lines:
                yield line.removesuffix("\r")
        if pending:
            yield pending.removesuffix("\r")

    async def aiter_raw(self, chunk_size: int | None = None) -> AsyncIterator[bytes]:
        if self._stream_consumed:
            raise StreamConsumed()
        if self._is_closed:
            raise StreamClosed()
        if self._native_stream is None and self._stream is not None and not hasattr(
            self._stream, "__aiter__"
        ):
            raise RuntimeError("Attempted to call an async iterator on an sync stream.")

        self._stream_consumed = True
        self._num_bytes_downloaded = 0
        chunker = _RawByteChunker(chunk_size)

        if self._native_stream is not None:
            source = self._native_stream.aiter_raw(chunk_size=None)
        elif self._stream is not None:
            source = self._stream
        else:
            # Buffered responses are consumed and closed at construction.
            raise StreamConsumed()

        async for raw_stream_bytes in source:
            self._num_bytes_downloaded += len(raw_stream_bytes)
            for chunk in chunker.decode(raw_stream_bytes):
                yield chunk

        for chunk in chunker.flush():
            yield chunk
        await self.aclose()

    def __repr__(self) -> str:
        return f"<Response [{self._status_code} {self._reason_phrase}]>"
