"""HTTPX-compatible Request class for eggfetch."""

from __future__ import annotations

import json as _json
import typing
from urllib.parse import urlencode, urlparse, urlunparse, parse_qs, quote

from eggfetch.compat.httpx._urls import URL, QueryParams
from eggfetch.compat.httpx._headers import Headers
from eggfetch.compat.httpx._cookies import Cookies
from eggfetch.compat.httpx._exceptions import RequestNotRead, StreamConsumed
from eggfetch.compat.httpx._stream import ByteStream

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
        "_files",
        "_multipart_data",
        "_explicit_cookie_header",
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
        # ── Body-source mutual exclusion (HTTPX rules) ──────────────
        # content is exclusive with data, files, json
        # json is exclusive with content, data, files
        # data + files is VALID (multipart)
        # stream is exclusive with everything
        has_content = content is not None
        has_json = json is not None
        has_data = data is not None
        has_files = files is not None
        has_stream = stream is not None

        if has_stream:
            if has_content or has_json or has_data or has_files:
                raise ValueError(
                    "stream= is mutually exclusive with content, data, files, and json."
                )
        elif has_content and has_json:
            raise ValueError(
                "content and json are mutually exclusive."
            )
        elif has_content and has_data:
            raise ValueError(
                "content and data are mutually exclusive."
            )
        elif has_content and has_files:
            raise ValueError(
                "content and files are mutually exclusive."
            )
        elif has_json and has_data:
            raise ValueError(
                "json and data are mutually exclusive."
            )
        elif has_json and has_files:
            raise ValueError(
                "json and files are mutually exclusive."
            )
        # data + files is allowed (multipart)

        self._method = method.upper()
        self._http_version = "HTTP/1.1"
        self._is_stream_consumed = False
        self._stream_consumed = False
        self._stream = stream

        # Track whether the Cookie header was explicitly provided by the user.
        # This is used during redirects to decide whether to carry the header
        # or regenerate from the client jar.
        self._explicit_cookie_header: str | None = None

        # Build headers from user-provided value
        if isinstance(headers, Headers):
            self._headers = headers
        else:
            self._headers = Headers(headers)

        # Capture explicit Cookie header before jar merge happens.
        # Used during redirects to decide whether to carry or regenerate.
        self._explicit_cookie_header = self._headers.get("cookie")

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

        # ── Handle body content ──────────────────────────────────────
        self._content: bytes | None = None
        self._files = None
        self._multipart_data = None

        if has_content:
            if hasattr(content, "read") and not isinstance(content, (bytes, bytearray)):
                def _file_reader():
                    while True:
                        chunk = content.read(8192)
                        if not chunk:
                            break
                        if isinstance(chunk, str):
                            chunk = chunk.encode("utf-8")
                        yield chunk

                self._stream = _file_reader()
            elif hasattr(content, "__iter__") and not isinstance(content, (bytes, str)):
                self._stream = content
            else:
                self._content = content if isinstance(content, bytes) else content.encode("utf-8")
        elif has_json:
            self._content = _json.dumps(
                json, separators=(",", ":"), ensure_ascii=False
            ).encode("utf-8")
            if "content-type" not in self._headers:
                self._headers["content-type"] = "application/json"
        elif has_data and has_files:
            # data + files → multipart: store both for later encoding
            self._files = files
            self._multipart_data = data
            self._content = None
        elif has_data:
            if isinstance(data, dict):
                self._content = urlencode(data).encode("utf-8")
                if "content-type" not in self._headers:
                    self._headers["content-type"] = "application/x-www-form-urlencoded"
            elif isinstance(data, (list, tuple)):
                self._content = urlencode(data).encode("utf-8")
                if "content-type" not in self._headers:
                    self._headers["content-type"] = "application/x-www-form-urlencoded"
            elif isinstance(data, str):
                self._content = data.encode("utf-8")
            elif isinstance(data, bytes):
                self._content = data
            else:
                self._content = str(data).encode("utf-8")
        elif has_files:
            self._files = files

        # ── Apply params to URL ─────────────────────────────────────
        self._url = self._apply_params_to_url(
            URL(url) if not isinstance(url, URL) else url,
            self._params,
        )

        # ── Auto-headers ────────────────────────────────────────────
        host = self._url.host
        if host is not None and not has_stream:
            port = self._url.port
            scheme = self._url.scheme
            if port and (
                (scheme == "http" and port != 80) or (scheme == "https" and port != 443)
            ):
                host = f"{host}:{port}"
            if "host" not in self._headers:
                self._headers["host"] = host

        # Only add Transfer-Encoding: chunked for stream= if it wasn't
        # explicitly provided.  HTTPX does NOT auto-add it for low-level
        # stream= — only for encoded content paths.
        # (Removed auto Transfer-Encoding injection for stream=)

        # Content-Length for encoded content
        if self._content is not None:
            if "content-length" not in self._headers:
                self._headers["content-length"] = str(len(self._content))

        elif self._stream is None and self._method in {"POST", "PUT", "PATCH"}:
            if "content-length" not in self._headers and "transfer-encoding" not in self._headers:
                self._headers["content-length"] = "0"

        # Extensions
        self._extensions: dict = extensions if extensions is not None else {}

    @staticmethod
    def _apply_params_to_url(url: URL, params: QueryParams) -> URL:
        """Merge query params into the URL, matching HTTPX behavior.

        HTTPX replaces existing query parameters with those provided via
        the ``params`` argument for direct Request construction.
        """
        if not params or not params.multi_items():
            return url

        raw = str(url)
        parsed = urlparse(raw)

        # Merge: params replace existing query
        new_query = urlencode(params.multi_items(), doseq=True)
        rebuilt = urlunparse((
            parsed.scheme,
            parsed.netloc,
            parsed.path,
            parsed.params,
            new_query,
            "",  # fragment
        ))
        return URL(rebuilt)

    @staticmethod
    def _encode_files(files, data=None) -> bytes:
        """Encode files (and optional data fields) as multipart/form-data.

        Supports HTTPX file tuple forms:
        - file object or bytes
        - (filename, fileobj)
        - (filename, fileobj, content_type)
        - (filename, fileobj, content_type, headers)
        """
        boundary = "----eggfetchboundary"
        parts: list[bytes] = []

        # Encode data fields first (if any)
        if data is not None:
            if isinstance(data, dict):
                items = list(data.items())
            elif isinstance(data, (list, tuple)):
                items = data
            else:
                items = []
            for key, value in items:
                if isinstance(value, (list, tuple)):
                    for v in value:
                        parts.append(
                            (
                                f"--{boundary}\r\n"
                                f'Content-Disposition: form-data; name="{key}"\r\n\r\n'
                            ).encode("utf-8")
                            + (
                                v.encode("utf-8") if isinstance(v, str) else str(v).encode("utf-8")
                            )
                            + b"\r\n"
                        )
                else:
                    parts.append(
                        (
                            f"--{boundary}\r\n"
                            f'Content-Disposition: form-data; name="{key}"\r\n\r\n'
                        ).encode("utf-8")
                        + (
                            value.encode("utf-8")
                            if isinstance(value, str)
                            else str(value).encode("utf-8")
                        )
                        + b"\r\n"
                    )

        # Encode files
        if isinstance(files, dict):
            file_items = list(files.items())
        elif isinstance(files, (list, tuple)):
            file_items = list(files)
        else:
            file_items = [("file", files)]

        for field_name, file_spec in file_items:
            file_headers: dict[str, str] = {}

            if isinstance(file_spec, tuple):
                if len(file_spec) >= 4:
                    filename, fileobj, content_type, file_h = (
                        file_spec[0],
                        file_spec[1],
                        file_spec[2],
                        file_spec[3],
                    )
                    if isinstance(file_h, dict):
                        file_headers = file_h
                elif len(file_spec) == 3:
                    filename, fileobj, content_type = file_spec
                elif len(file_spec) == 2:
                    filename, fileobj = file_spec
                    content_type = "application/octet-stream"
                else:
                    filename = file_spec[0] if file_spec else "file"
                    fileobj = file_spec[1] if len(file_spec) > 1 else b""
                    content_type = "application/octet-stream"
            else:
                filename = str(field_name)
                fileobj = file_spec
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

            # Build part header
            header_lines = [
                f"--{boundary}\r\n",
                f'Content-Disposition: form-data; name="{field_name}"',
            ]
            if filename:
                header_lines[1] += f'; filename="{filename}"'
            header_lines[1] += "\r\n"
            header_lines.append(f"Content-Type: {content_type}\r\n")
            for hk, hv in file_headers.items():
                header_lines.append(f"{hk}: {hv}\r\n")
            header_lines.append("\r\n")

            part = "".join(header_lines).encode("utf-8") + body + b"\r\n"
            parts.append(part)

        parts.append(f"--{boundary}--\r\n".encode("utf-8"))
        return b"".join(parts)

    @property
    def method(self) -> str:
        return self._method

    @property
    def url(self) -> URL:
        return self._url

    @url.setter
    def url(self, value: URL | str) -> None:
        self._url = URL(value) if not isinstance(value, URL) else value

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
    def content(self) -> bytes:
        if self._content is None and self._stream is not None:
            raise RequestNotRead("Attempted to access streaming request content, without having called `read()`.")
        return self._content or b""

    @property
    def stream(self):
        return self._stream

    @property
    def files(self):
        return self._files

    @property
    def extensions(self) -> dict:
        return self._extensions

    @property
    def is_stream_consumed(self) -> bool:
        return self._is_stream_consumed

    @property
    def http_version(self) -> str:
        return self._http_version

    @http_version.setter
    def http_version(self, value: str) -> None:
        self._http_version = value

    def read(self) -> bytes:
        if self._stream is not None and self._content is None:
            if self._stream_consumed:
                raise StreamConsumed()
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
            self._stream = ByteStream(self._content)
        if self._content is None:
            return b""
        return self._content

    async def aread(self) -> bytes:
        if self._stream is not None and self._content is None:
            if self._stream_consumed:
                raise StreamConsumed()
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
            self._stream = ByteStream(self._content)
        if self._content is None:
            return b""
        return self._content

    def __repr__(self) -> str:
        return f"<Request({self._method!r}, {str(self._url)!r})>"
