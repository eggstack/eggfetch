"""HTTPX-compatible Client and AsyncClient for eggfetch."""

from __future__ import annotations

import asyncio
import enum
import socket
import sys
import typing
import urllib.parse
import time
from contextlib import contextmanager, asynccontextmanager

import eggfetch

from eggfetch.compat.httpx._urls import URL, QueryParams
from eggfetch.compat.httpx._headers import Headers
from eggfetch.compat.httpx._cookies import Cookies
from eggfetch.compat.httpx._timeout import Timeout
from eggfetch.compat.httpx._limits import Limits
from eggfetch.compat.httpx._proxy import Proxy
from eggfetch.compat.httpx._auth import Auth, BasicAuth
from eggfetch.compat.httpx._request import Request
from eggfetch.compat.httpx._response import Response
from eggfetch.compat.httpx._stream import ByteStream
from eggfetch.compat.httpx._exceptions import (
    ConnectTimeout,
    ConnectError,
    CookieConflict,
    HTTPStatusError,
    InvalidURL,
    NetworkError,
    PoolTimeout,
    ProtocolError,
    ProxyError,
    ReadTimeout,
    RequestError,
    ResponseNotRead,
    StreamConsumed,
    TimeoutException,
    TooManyRedirects,
    TransportError,
    UnsupportedProtocol,
    WriteTimeout,
)

if typing.TYPE_CHECKING:
    from typing import Any, AsyncIterator, Iterator

_USE_CLIENT_DEFAULT = object()

# Sentinel distinguishing "no match found" from "matched a None transport".
_MOUNT_NO_MATCH = object()


# ── Client state ────────────────────────────────────────────────────────

class _ClientState(enum.Enum):
    UNOPENED = "unopened"
    OPENED = "opened"
    CLOSED = "closed"


# ── Auth normalization ──────────────────────────────────────────────────

class _FunctionAuth(Auth):
    """Adapter wrapping a callable as a compatibility Auth object.

    Matches HTTPX's callable-auth behavior: the callable receives the
    request and must return a request (sync) or await a request (async).
    """

    def __init__(self, func: typing.Callable[..., Any]) -> None:
        self._func = func

    def sync_auth_flow(self, request: Request):  # type: ignore[override]
        result = self._func(request)
        if isinstance(result, Request):
            yield result
        elif hasattr(result, "__iter__"):
            yield from result
        else:
            yield request

    async def async_auth_flow(self, request: Request):  # type: ignore[override]
        if asyncio.iscoroutinefunction(self._func):
            result = await self._func(request)
        else:
            result = self._func(request)
        if isinstance(result, Request):
            yield result
        elif hasattr(result, "__aiter__"):
            async for req in result:
                yield req
        elif hasattr(result, "__iter__"):
            for req in result:
                yield req
        else:
            yield request

    def __repr__(self) -> str:
        return f"_FunctionAuth({self._func!r})"


def _build_auth(auth: typing.Any) -> Auth | None:
    """Normalize an auth value into a compatibility Auth object.

    Returns ``None`` for no-auth.  Raises ``TypeError`` for unsupported
    types.
    """
    if auth is None:
        return None
    if isinstance(auth, Auth):
        return auth
    if isinstance(auth, tuple):
        if len(auth) != 2:
            raise TypeError(
                f"auth tuple must be (username, password), got {len(auth)} elements"
            )
        username, password = auth
        return BasicAuth(username=str(username), password=str(password))
    if callable(auth):
        return _FunctionAuth(auth)
    raise TypeError(
        f"auth must be None, Auth instance, (username, password) tuple, or callable, "
        f"got {type(auth).__name__}"
    )


def _extract_url_credentials(url: URL) -> Auth | None:
    """Extract Basic Auth from URL user-info, or return None."""
    url_str = str(url)
    parsed = urllib.parse.urlsplit(url_str)
    if parsed.username:
        return BasicAuth(
            username=urllib.parse.unquote(parsed.username),
            password=urllib.parse.unquote(parsed.password or ""),
        )
    return None


# ── Protocol validation ─────────────────────────────────────────────────

def _validate_protocol_options(http1: bool, http2: bool) -> None:
    """Validate protocol combination.  Raises ValueError on invalid combos."""
    if not http1 and not http2:
        raise ValueError(
            "At least one of http1 or http2 must be True"
        )
    if not http1 and http2:
        raise NotImplementedError(
            "eggfetch does not support http1=False, http2=True (H2-only mode)"
        )


# ── Transport option validation ───────────────────────────────────────

def _convert_socket_option(option: tuple) -> tuple[int, int, bytes]:
    """Convert a socket option tuple to (level, option, value) ints.

    Accepts:
    - ``(level_int, option_int, value_int_or_bytes)``
    - ``(level_str, option_str, value_int_or_bytes)``

    Returns ``(level, option, value_bytes)``.
    """
    if len(option) != 3:
        raise ValueError(
            "socket_options must be a list of (level, option, value) triples"
        )
    level, opt, value = option

    # Resolve level.
    if isinstance(level, str):
        try:
            level = getattr(socket, level)
        except AttributeError as exc:
            raise ValueError(f"unknown socket level: {level!r}") from exc

    # Resolve option.
    if isinstance(opt, str):
        try:
            opt = getattr(socket, opt)
        except AttributeError as exc:
            raise ValueError(f"unknown socket option: {opt!r}") from exc

    # Convert value to bytes.
    if isinstance(value, int):
        value = value.to_bytes(4, byteorder=sys.byteorder, signed=True)
    elif isinstance(value, bytes):
        pass
    else:
        raise ValueError(
            f"socket option value must be int or bytes, got {type(value).__name__}"
        )

    return (int(level), int(opt), value)


def _validate_transport_options(
    *,
    uds: str | None = None,
    local_address: str | None = None,
    socket_options: typing.Any | None = None,
) -> None:
    """Validate transport options format (no longer rejected)."""
    if uds is not None and not isinstance(uds, str):
        raise TypeError("uds must be a string path")
    if local_address is not None:
        if not isinstance(local_address, str):
            raise TypeError("local_address must be a string")
        # HTTPX accepts an address only; the OS selects the source port.
        try:
            import ipaddress
            ipaddress.ip_address(local_address)
        except ValueError as exc:
            raise ValueError(f"invalid local_address {local_address!r}") from exc
    if socket_options is not None:
        if not isinstance(socket_options, (list, tuple)):
            raise TypeError("socket_options must be a list of tuples")
        for opt in socket_options:
            if not isinstance(opt, (list, tuple)) or len(opt) != 3:
                raise ValueError(
                    "socket_options must be a list of (level, option, value) triples"
                )


# ── Default headers ─────────────────────────────────────────────────────

def _default_httpx_headers() -> Headers:
    """Return HTTPX-equivalent default headers."""
    headers = Headers()
    headers["accept"] = "*/*"
    headers["accept-encoding"] = "gzip, deflate, br"
    headers["connection"] = "keep-alive"
    headers["user-agent"] = "python-httpx/0.28.1"
    return headers


def _merge_default_headers(user_headers) -> Headers:
    """Merge user-provided headers with HTTPX default headers.

    User headers override defaults using duplicate-preserving semantics.
    """
    defaults = _default_httpx_headers()
    if user_headers is not None:
        if isinstance(user_headers, Headers):
            defaults.update(user_headers)
        elif isinstance(user_headers, dict):
            defaults.update(user_headers)
        elif isinstance(user_headers, (list, tuple)):
            defaults.update(user_headers)
    return defaults


def _convert_timeout(timeout):
    if isinstance(timeout, Timeout):
        kwargs = {
            "connect": timeout.connect,
            "read": timeout.read,
            "write": timeout.write,
            "pool": timeout.pool,
        }
        kwargs["total"] = timeout.total
        return eggfetch.Timeout(**kwargs)
    if isinstance(timeout, (int, float)):
        return eggfetch.Timeout(connect=timeout, read=timeout, write=timeout, pool=timeout, total=timeout)
    return None


def _convert_limits(limits):
    if isinstance(limits, Limits):
        return eggfetch.Limits(
            max_connections=limits.max_connections,
            max_keepalive_connections=limits.max_keepalive_connections,
            keepalive_expiry=limits.keepalive_expiry,
        )
    return None


def _convert_headers(headers):
    if isinstance(headers, Headers):
        return headers.multi_items()
    if isinstance(headers, dict):
        return list(headers.items())
    return None


def _convert_cookies(cookies):
    if isinstance(cookies, Cookies):
        return dict(cookies.items())
    if isinstance(cookies, dict):
        return cookies
    return None


def _convert_params(params):
    if params is None:
        return None
    if isinstance(params, QueryParams):
        return list(params.multi_items())
    if isinstance(params, dict):
        return list(params.items())
    if isinstance(params, str):
        from urllib.parse import parse_qsl
        return parse_qsl(params)
    return None


def _convert_proxy(proxy):
    if isinstance(proxy, Proxy):
        url = str(proxy.url)
        auth = proxy.raw_auth
        if auth is not None and proxy.url.scheme in ("socks5", "socks5h"):
            username, password = auth
            parsed = urllib.parse.urlsplit(url)
            userinfo = "{}:{}@".format(
                urllib.parse.quote(str(username), safe=""),
                urllib.parse.quote(str(password), safe=""),
            )
            url = urllib.parse.urlunsplit(
                (parsed.scheme, userinfo + parsed.netloc, parsed.path, parsed.query, parsed.fragment)
            )
        return url
    if isinstance(proxy, str):
        return proxy
    return None


def _map_exception(native_exc, compat_request=None):
    msg = str(native_exc)

    if not isinstance(native_exc, eggfetch.EggfetchError):
        raise

    if isinstance(native_exc, eggfetch.PoolTimeout):
        return PoolTimeout(message=msg, request=compat_request)
    if isinstance(native_exc, eggfetch.ConnectTimeout):
        return ConnectTimeout(message=msg, request=compat_request)
    if isinstance(native_exc, eggfetch.ReadTimeout):
        return ReadTimeout(message=msg, request=compat_request)
    if isinstance(native_exc, eggfetch.WriteTimeout):
        return WriteTimeout(message=msg, request=compat_request)
    if isinstance(native_exc, eggfetch.TimeoutException):
        msg_lower = msg.lower()
        if "pool timeout" in msg_lower:
            return PoolTimeout(message=msg, request=compat_request)
        if "proxy connect" in msg_lower or "proxy tls" in msg_lower or "connect timeout" in msg_lower:
            return ConnectTimeout(message=msg, request=compat_request)
        if "write timeout" in msg_lower:
            return WriteTimeout(message=msg, request=compat_request)
        if "read timeout" in msg_lower or "total timeout" in msg_lower:
            return ReadTimeout(message=msg, request=compat_request)
        return TimeoutException(message=msg, request=compat_request)

    if isinstance(native_exc, eggfetch.NetworkError):
        return ConnectError(message=msg, request=compat_request)

    if isinstance(native_exc, eggfetch.ProtocolError):
        return ProtocolError(message=msg, request=compat_request)

    if isinstance(native_exc, eggfetch.TooManyRedirects):
        return TooManyRedirects(message=msg, request=compat_request)

    if isinstance(native_exc, eggfetch.InvalidUrl):
        return InvalidURL(msg)

    if isinstance(native_exc, eggfetch.ProxyError):
        return ProxyError(message=msg, request=compat_request)

    return RequestError(message=msg, request=compat_request)


def _wrap_response(native_resp, compat_request=None, default_encoding="utf-8"):
    from datetime import timedelta

    status_code = native_resp.status_code
    native_headers = native_resp.headers

    header_list = []
    if hasattr(native_headers, "multi_items"):
        header_list = native_headers.multi_items()
    elif hasattr(native_headers, "items"):
        header_list = native_headers.items()
    _overlay_wire_headers(header_list, native_resp)

    try:
        content = native_resp.content
    except ResponseNotRead:
        content = native_resp.read()

    history = []
    if hasattr(native_resp, "history") and native_resp.history:
        for h in native_resp.history:
            history.append(_wrap_response(h, default_encoding=default_encoding))

    # Start with handler-provided extensions, then overlay standard keys.
    extensions: dict = {}
    if hasattr(native_resp, "extensions") and native_resp.extensions:
        extensions.update(native_resp.extensions)

    if hasattr(native_resp, "http_version") and native_resp.http_version:
        extensions.setdefault("http_version", native_resp.http_version)
    if hasattr(native_resp, "reason_phrase") and native_resp.reason_phrase:
        extensions.setdefault("reason_phrase", native_resp.reason_phrase)

    resp = Response(
        status_code,
        headers=header_list,
        content=content,
        request=compat_request,
        history=history,
        default_encoding=default_encoding,
        extensions=extensions if extensions else None,
    )

    start = compat_request.extensions.get("_eggfetch_started_at") if compat_request is not None else None
    if start is not None:
        resp.elapsed = timedelta(seconds=max(0.0, time.monotonic() - start))
    elif hasattr(native_resp, "elapsed") and native_resp.elapsed is not None:
        resp.elapsed = native_resp.elapsed

    return resp


def _overlay_wire_headers(header_list, native_resp):
    """Restore only wire metadata hidden by core automatic decompression."""
    present = {str(name).lower() for name, _value in header_list}
    for attribute, name in (
        ("_wire_content_encoding", "content-encoding"),
        ("_wire_content_length", "content-length"),
    ):
        value = getattr(native_resp, attribute, None)
        if value is not None and name not in present:
            header_list.append((name, value))
            present.add(name)


def _wrap_streaming_response(native_resp, compat_request=None, default_encoding="utf-8"):
    if isinstance(native_resp, Response):
        status_code = native_resp.status_code
        header_list = native_resp.headers.multi_items()
        stream_obj = native_resp._stream
        history = native_resp.history or []
        existing_content = native_resp._content
    else:
        status_code = native_resp.status_code
        native_headers = native_resp.headers

        header_list = []
        if hasattr(native_headers, "multi_items"):
            header_list = native_headers.multi_items()
        elif hasattr(native_headers, "items"):
            header_list = native_headers.items()
        _overlay_wire_headers(header_list, native_resp)

        history = []
        if hasattr(native_resp, "history") and native_resp.history:
            for h in native_resp.history:
                history.append(_wrap_response(h, default_encoding=default_encoding))
        stream_obj = native_resp
        existing_content = None

    # Start with handler-provided extensions, then overlay standard keys.
    extensions: dict = {}
    if hasattr(native_resp, "extensions") and native_resp.extensions:
        extensions.update(native_resp.extensions)

    if hasattr(native_resp, "http_version") and native_resp.http_version:
        extensions.setdefault("http_version", native_resp.http_version)
    if hasattr(native_resp, "reason_phrase") and native_resp.reason_phrase:
        extensions.setdefault("reason_phrase", native_resp.reason_phrase)

    # Preserve buffered content when wrapping a compat Response that was
    # constructed with content= but is being streamed.  This matches
    # HTTPX's expectation that a buffered response still yields its body
    # through streaming iteration.
    if existing_content is not None and stream_obj is None:
        def _buffer_iter():
            yield existing_content
        stream_obj = _buffer_iter()

    response = Response(
        status_code,
        headers=header_list,
        stream=stream_obj,
        request=compat_request,
        history=history,
        default_encoding=default_encoding,
        extensions=extensions if extensions else None,
    )
    if hasattr(stream_obj, "read"):
        response._native_stream = stream_obj

    if compat_request is not None and "_eggfetch_started_at" in compat_request.extensions:
        response.extensions["_eggfetch_started_at"] = compat_request.extensions["_eggfetch_started_at"]

    return response


def _build_native_kwargs(request, follow_redirects=None, timeout=_USE_CLIENT_DEFAULT):
    kwargs: dict[str, Any] = {}
    if isinstance(request, Request):
        kwargs["method"] = request.method
        kwargs["url"] = str(request.url)
        if request.headers:
            kwargs["headers"] = _convert_headers(request.headers)
        # Request construction already serialized params into request.url.
        if request._stream is not None and request._content is None:
            kwargs["content"] = request._stream
        elif request._content is not None:
            kwargs["content"] = request._content
        if request._files is not None:
            kwargs["files"] = request._files
            # When data + files are both present (multipart), pass the
            # data fields so the native multipart encoder can include them.
            if hasattr(request, "_multipart_data") and request._multipart_data is not None:
                kwargs["data"] = request._multipart_data
    else:
        raise TypeError(f"send() requires a Request object, got {type(request).__name__}")

    if follow_redirects is not None:
        kwargs["follow_redirects"] = follow_redirects
    if timeout is _USE_CLIENT_DEFAULT:
        pass
    elif timeout is None:
        kwargs["timeout"] = None
    else:
        kwargs["timeout"] = _convert_timeout(timeout)

    return kwargs


# ── Mount pattern matching (Track 3) ───────────────────────────────────

def _parse_mount_pattern(pattern: str):
    """Parse a mount pattern into ``(scheme, host, port, path)`` components.

    Supports HTTPX 0.28.1 patterns:
    - ``all://`` → catch-all
    - ``http://`` / ``https://`` → scheme-only
    - ``http://example.com`` → exact host
    - ``http://example.com:8080`` → host + port
    - ``all://*.example.com`` → wildcard domain
    - ``http://example.com/api`` → host + path prefix
    """
    if pattern == "all://":
        return ("", None, None, "", False)

    parsed = urllib.parse.urlsplit(pattern)
    scheme = parsed.scheme.lower()
    host = parsed.hostname
    port = parsed.port
    path = parsed.path.rstrip("/") or ""

    # Detect wildcard domain: all://*.example.com
    raw_host = parsed.hostname or ""
    is_wildcard = False
    if raw_host.startswith("*."):
        is_wildcard = True
        host = raw_host[2:]
    elif raw_host == "*":
        is_wildcard = True
        host = ""

    # Normalize "all" scheme to empty string for scheme-agnostic matching
    if scheme == "all":
        scheme = ""

    return (scheme, host, port, path, is_wildcard)


def _host_matches(url_host: str | None, pat_host: str | None, is_wildcard: bool) -> bool:
    """Check whether *url_host* matches *pat_host* (with optional wildcard)."""
    if pat_host is None:
        return True
    if url_host is None:
        return False
    if is_wildcard:
        url_host_lower = url_host.lower()
        pat_host_lower = pat_host.lower()
        return url_host_lower.endswith("." + pat_host_lower)
    return pat_host.lower() == url_host.lower()


def _match_mount(url, mounts):
    """Find the best matching transport for *url* from *mounts* dict.

    Priority follows HTTPX 0.28.1 ordering (highest wins):

    1. Exact host + port + path                                      — 10 000
    2. Wildcard domain + port + path                                  — 9 500
    3. Exact host + port (no path)                                    — 500
    4. Wildcard domain + port (no path)                               — 450
    5. Exact host + path (no port)                                    — 200 + len(path)
    6. Wildcard domain + path (no port)                               — 150 + len(path)
    7. Exact host only                                                — 200
    8. Wildcard domain only                                           — 150
    9. Scheme only (``http://`` or ``https://``)                       — 10
    10. Catch-all (``all://``)                                         — 0

    Returns:
    - The matched transport object, or
    - ``None`` if a pattern matched ``None`` (explicit bypass), or
    - ``_MOUNT_NO_MATCH`` if nothing matched.
    """
    if not mounts:
        return _MOUNT_NO_MATCH

    url_str = str(url)
    url_parts = urllib.parse.urlsplit(url_str)
    url_scheme = url_parts.scheme.lower()
    url_host = url_parts.hostname
    url_port = url_parts.port
    url_path = url_parts.path.rstrip("/") or ""

    best_match: str | None = None
    best_score: int = -1

    for pattern in mounts:
        parsed = _parse_mount_pattern(pattern)
        pat_scheme = parsed[0]
        pat_host = parsed[1]
        pat_port = parsed[2]
        pat_path = parsed[3]
        is_wildcard = parsed[4]

        # Catch-all: always matches, lowest priority
        if pat_scheme == "" and pat_host is None:
            score = 0
            if score > best_score:
                best_score = score
                best_match = pattern
            continue

        # Scheme must match — "all" is scheme-agnostic
        if pat_scheme and pat_scheme != "all" and pat_scheme != url_scheme:
            continue

        # Host must match
        if not _host_matches(url_host, pat_host, is_wildcard):
            continue

        # Port must match (or pattern has no port)
        if pat_port is not None:
            if url_port != pat_port:
                continue

        # Path must be a prefix (or pattern has no path)
        if pat_path:
            if url_path != pat_path and not url_path.startswith(pat_path + "/"):
                continue

        # Compute score
        if pat_host is None and not pat_path:
            score = 10
        elif is_wildcard:
            if pat_port is not None and not pat_path:
                score = 450
            elif pat_port is not None and pat_path:
                score = 9500
            elif pat_path:
                score = 150 + len(pat_path)
            else:
                score = 150
        else:
            if pat_port is not None and not pat_path:
                score = 500
            elif pat_port is not None and pat_path:
                score = 10000
            elif pat_path:
                base = 200
                score = base + len(pat_path)
            else:
                score = 200

        if score > best_score:
            best_score = score
            best_match = pattern

    if best_match is not None:
        return mounts[best_match]
    return _MOUNT_NO_MATCH


def _validate_mount_pattern(pattern: str) -> None:
    """Validate a mount pattern at client construction time."""
    if pattern == "all://":
        return
    parsed = urllib.parse.urlsplit(pattern)
    if not parsed.scheme:
        raise ValueError(
            f"Mount pattern must include a scheme: {pattern!r}"
        )
    raw_host = parsed.hostname or ""
    if raw_host == "*" or raw_host.startswith("*."):
        remainder = raw_host[2:] if raw_host.startswith("*.") else ""
        if not remainder or "." not in remainder:
            raise ValueError(
                f"Wildcard mount pattern requires a valid domain after '*.': {pattern!r}"
            )
    if parsed.port is not None and parsed.port < 0:
        raise ValueError(
            f"Invalid port in mount pattern: {pattern!r}"
        )


# ── Effective timeout extension (Track 1.3) ────────────────────────────

def _timeout_mapping(timeout):
    if isinstance(timeout, Timeout):
        return timeout.as_dict
    if isinstance(timeout, (int, float)):
        return Timeout(timeout).as_dict
    if timeout is None:
        return Timeout(None).as_dict
    raise TypeError(f"Invalid timeout value: {type(timeout).__name__}")

def _prepare_cookie_header(request):
    """Merge request-local, explicit, and scoped jar cookies once."""
    explicit = request.headers.get("cookie")
    request.headers.pop("cookie", None)
    request._cookies.set_cookie_header(request)
    generated = request.headers.get("cookie")
    if explicit and generated:
        names = {part.split("=", 1)[0].strip() for part in explicit.split(";")}
        additions = [part.strip() for part in generated.split(";") if part.split("=", 1)[0].strip() not in names]
        request.headers["cookie"] = "; ".join([explicit, *additions]) if additions else explicit
    elif explicit:
        request.headers["cookie"] = explicit


def _request_timeout(request, fallback):
    value = request.extensions.get("timeout")
    if isinstance(value, dict):
        if isinstance(fallback, Timeout) and value == fallback.as_dict:
            return fallback
        return Timeout(timeout=None, connect=value.get("connect"), read=value.get("read"), write=value.get("write"), pool=value.get("pool"))
    return fallback

def _ensure_timeout_extension(request: Request, timeout) -> None:
    if "timeout" not in request.extensions:
        request.extensions["timeout"] = _timeout_mapping(timeout)


# ── Redirect helpers (Phase 4, Track 2) ────────────────────────────────

_REDIRECT_STATUSES = frozenset({301, 302, 303, 307, 308})


def _is_redirect_status(status_code: int) -> bool:
    """Return True if *status_code* is a redirect that may need following."""
    return status_code in _REDIRECT_STATUSES


def _port_or_default(url: URL) -> int | None:
    """Return the port, or the default port for the scheme."""
    if url.port is not None:
        return url.port
    if url.scheme == "https":
        return 443
    if url.scheme == "http":
        return 80
    return None


def _same_origin(url: URL, other: URL) -> bool:
    """Return True if *url* and *other* share the same origin."""
    return (
        url.scheme == other.scheme
        and url.host == other.host
        and _port_or_default(url) == _port_or_default(other)
    )


def _is_https_redirect(url: URL, location: URL) -> bool:
    """Return True if *location* is an HTTPS upgrade of *url*."""
    if url.host != location.host:
        return False
    return (
        url.scheme == "http"
        and _port_or_default(url) == 80
        and location.scheme == "https"
        and _port_or_default(location) == 443
    )


# ── Body replay classifier (Track 3) ──────────────────────────────────

class _BodyReplay:
    """Classify a Request body for redirect replay.

    Classifications:
    - ``empty``: no body present
    - ``buffered``: stable ``_content`` bytes, always replayable
    - ``reusable-stream``: ``ByteStream`` with known-good content
    - ``multipart-reconstructable``: ``_files`` + ``_multipart_data`` with
      all-immutable parts (bytes/tuple with bytes content)
    - ``one-shot``: generator or iterator that cannot be replayed
    - ``unsupported``: body state that cannot be safely replayed
    """

    __slots__ = ("kind", "content", "files", "multipart_data")

    def __init__(
        self,
        kind: str,
        *,
        content: bytes | None = None,
        files=None,
        multipart_data=None,
    ) -> None:
        self.kind = kind
        self.content = content
        self.files = files
        self.multipart_data = multipart_data

    def __repr__(self) -> str:
        return f"_BodyReplay(kind={self.kind!r})"


def _classify_body(request: Request) -> _BodyReplay:
    """Classify a Request body for redirect replay."""
    # Multipart (data + files): check if all parts are immutable/reconstructable
    if request._files is not None:
        if _is_multipart_reconstructable(request._files, request._multipart_data):
            return _BodyReplay(
                "multipart-reconstructable",
                files=request._files,
                multipart_data=request._multipart_data,
            )
        return _BodyReplay("unsupported")

    # Buffered bytes
    if request._content is not None:
        return _BodyReplay("buffered", content=request._content)

    # ByteStream (reusable)
    if isinstance(request._stream, ByteStream):
        return _BodyReplay(
            "reusable-stream",
            content=request._stream._content,
        )

    # Generator or iterator (one-shot)
    if request._stream is not None:
        return _BodyReplay("one-shot")

    return _BodyReplay("empty")


def _is_multipart_reconstructable(files, multipart_data) -> bool:
    """Check if multipart data can be safely reconstructed.

    Returns True only when every part is represented by immutable reusable
    values (bytes, str, or tuples with bytes/str content).
    """
    if files is None:
        return False

    # Check data fields
    if multipart_data is not None:
        if isinstance(multipart_data, dict):
            items = list(multipart_data.items())
        elif isinstance(multipart_data, (list, tuple)):
            items = multipart_data
        else:
            return False
        for _key, value in items:
            if isinstance(value, (list, tuple)):
                for v in value:
                    if not _is_immutable_value(v):
                        return False
            elif not _is_immutable_value(value):
                return False

    # Check file fields
    file_items = files if isinstance(files, (list, tuple)) else list(files.items()) if isinstance(files, dict) else [("file", files)]
    for field_name, file_spec in file_items:
        if isinstance(file_spec, tuple):
            if len(file_spec) >= 2:
                fileobj = file_spec[1]
                if not _is_immutable_file_value(fileobj):
                    return False
            # tuple with 3+: filename, fileobj, content_type, headers
        elif not _is_immutable_file_value(file_spec):
            return False

    return True


def _is_immutable_value(value) -> bool:
    """Check if a form value is immutable and safe for reconstruction."""
    return isinstance(value, (bytes, str, int, float))


def _is_immutable_file_value(fileobj) -> bool:
    """Check if a file value is immutable and safe for reconstruction."""
    if isinstance(fileobj, (bytes, bytearray)):
        return True
    if isinstance(fileobj, str):
        return True
    # file-like objects, generators, etc. are NOT safe
    return False


def _redirect_method(request: Request, response: Response) -> str:
    """Determine the method for a redirect request (HTTPX 0.28.1 rules)."""
    method = request.method

    # 303 See Other: change non-HEAD to GET
    if response.status_code == 303 and method != "HEAD":
        method = "GET"

    # 302 Found: browser-compatible conversion to GET except HEAD
    if response.status_code == 302 and method != "HEAD":
        method = "GET"

    # 301 Moved Permanently: convert POST to GET
    if response.status_code == 301 and method == "POST":
        method = "GET"

    return method


def _redirect_url(request: Request, response: Response) -> URL:
    """Resolve the redirect URL from the Location header."""
    location = response.headers.get("location", "")
    if not location:
        return request.url

    try:
        url = URL(location)
    except Exception:
        raise InvalidURL(f"Invalid URL in location header: {location}")

    # Handle malformed 'Location' headers that are "absolute" form but have no host
    if url.scheme and not url.host:
        # Reconstruct with the request's host
        parts = urllib.parse.urlsplit(str(url))
        new_netloc = request.url.host
        if request.url.port is not None:
            new_netloc = f"{new_netloc}:{request.url.port}"
        reconstructed = urllib.parse.urlunsplit((
            parts.scheme, new_netloc, parts.path, parts.query, parts.fragment,
        ))
        url = URL(reconstructed)

    # Handle relative URLs
    if url.is_relative_url:
        url = request.url.join(url)

    # Attach previous fragment if needed (RFC 7231 7.1.2)
    if request.url.fragment and not url.fragment:
        parts = urllib.parse.urlsplit(str(url))
        reconstructed = urllib.parse.urlunsplit((
            parts.scheme, parts.netloc, parts.path, parts.query, request.url.fragment,
        ))
        url = URL(reconstructed)

    return url


def _redirect_headers(request: Request, url: URL, method: str) -> Headers:
    """Build headers for a redirect request, stripping sensitive headers."""
    headers = Headers(request.headers)

    # Always strip Cookie header on redirect — it will be regenerated from
    # the client jar for the destination URL.  This matches HTTPX 0.28.1:
    # explicit Cookie headers are not carried across redirects.
    headers.pop("Cookie", None)
    headers.pop("cookie", None)

    if not _same_origin(url, request.url):
        if not _is_https_redirect(request.url, url):
            # Strip Authorization when redirecting away from origin
            headers.pop("Authorization", None)
        # Update Host header
        raw_host = url.raw_host
        if raw_host is not None:
            host_val = raw_host.decode("ascii") if isinstance(raw_host, bytes) else str(raw_host)
            if url.port is not None and url.port not in (80, 443):
                host_val = f"{host_val}:{url.port}"
            headers["Host"] = host_val

    if method != request.method and method == "GET":
        # Strip body-related headers when switching to GET
        headers.pop("Content-Length", None)
        headers.pop("Transfer-Encoding", None)

    return headers


def _redirect_stream(request: Request, method: str):
    """Determine the body source for a redirect request.

    Returns:
    - ``None`` when the body should be dropped (method rewrite to GET)
    - ``None`` when ``request._content`` exists (replay via content kwarg)
    - A fresh ``ByteStream`` for ``ByteStream`` streams
    - Raises ``StreamConsumed`` for unreplayable streams

    For multipart and other complex bodies, use ``_classify_body()`` instead.
    """
    if method != request.method and method == "GET":
        return None
    if request._content is not None:
        return None
    if isinstance(request._stream, ByteStream):
        return ByteStream(request._stream._content)
    if request._stream is not None:
        raise StreamConsumed()
    return None


def _build_redirect_request(
    client_cookies: Cookies,
    request: Request,
    response: Response,
) -> Request:
    """Build a new Request for a redirect response (HTTPX 0.28.1 rules)."""
    method = _redirect_method(request, response)
    url = _redirect_url(request, response)
    headers = _redirect_headers(request, url, method)

    # Classify the body before building the redirect request.
    body = _classify_body(request)

    # Method rewrites (301→GET for POST, 302, 303): drop body
    if method != request.method and method == "GET":
        return Request(
            method=method,
            url=url,
            headers=headers,
            cookies=Cookies(client_cookies),
            extensions=_clean_redirect_extensions(request),
        )

    # Retained method: must have a valid body source
    if body.kind == "empty":
        return Request(
            method=method,
            url=url,
            headers=headers,
            cookies=Cookies(client_cookies),
            extensions=_clean_redirect_extensions(request),
        )

    if body.kind == "buffered":
        return Request(
            method=method,
            url=url,
            headers=headers,
            cookies=Cookies(client_cookies),
            content=body.content,
            extensions=_clean_redirect_extensions(request),
        )

    if body.kind == "reusable-stream":
        return Request(
            method=method,
            url=url,
            headers=headers,
            cookies=Cookies(client_cookies),
            stream=ByteStream(body.content),
            extensions=_clean_redirect_extensions(request),
        )

    if body.kind == "multipart-reconstructable":
        return Request(
            method=method,
            url=url,
            headers=headers,
            cookies=Cookies(client_cookies),
            data=body.multipart_data,
            files=body.files,
            extensions=_clean_redirect_extensions(request),
        )

    # One-shot or unsupported: fail before second dispatch
    raise StreamConsumed()


def _clean_redirect_extensions(request: Request) -> dict:
    """Build extensions dict for a redirect request."""
    extensions = dict(request.extensions)
    extensions.pop("_eggfetch_started_at", None)
    return extensions


# ── Client ──────────────────────────────────────────────────────────────

class Client:
    def __init__(
        self,
        *,
        auth=None,
        params=None,
        headers=None,
        cookies=None,
        verify=True,
        cert=None,
        trust_env=True,
        http1=True,
        http2=False,
        proxy=None,
        mounts=None,
        timeout=Timeout(5.0),
        follow_redirects=False,
        limits=Limits(100, 20, 5.0),
        max_redirects=20,
        event_hooks=None,
        base_url="",
        transport=None,
        default_encoding="utf-8",
        extensions=None,
    ):
        _validate_protocol_options(http1, http2)
        self._auth = _build_auth(auth)
        self._params = params if isinstance(params, QueryParams) else QueryParams(params)
        self._headers = _merge_default_headers(headers)
        self._cookies = cookies if isinstance(cookies, Cookies) else Cookies(cookies)
        self._verify = verify
        self._cert = cert
        self._trust_env = trust_env
        self._http1 = http1
        self._http2 = http2
        self._proxy = proxy
        self._timeout = timeout if isinstance(timeout, Timeout) else Timeout(timeout)
        self._follow_redirects = follow_redirects
        self._limits = limits if isinstance(limits, Limits) else Limits(limits)
        self._max_redirects = max_redirects
        self._event_hooks = event_hooks or {"request": [], "response": []}
        self._base_url = URL(base_url) if not isinstance(base_url, URL) else base_url
        self._transport = transport
        self._default_encoding = default_encoding
        self._extensions = extensions or {}
        self._native_client = None
        self._state = _ClientState.UNOPENED

        # Track which transport objects we own for close deduplication.
        self._owned_transports: set[int] = set()

        self._mounts: dict[str, Any] = {}
        if mounts:
            for pattern, transport_obj in mounts.items():
                _validate_mount_pattern(pattern)
                self._mounts[pattern] = transport_obj
                if transport_obj is not None:
                    self._owned_transports.add(id(transport_obj))

        if self._transport is not None:
            self._owned_transports.add(id(self._transport))

    def _ensure_client(self):
        if self._transport is not None:
            if self._state == _ClientState.UNOPENED:
                self._state = _ClientState.OPENED
            return
        if self._state == _ClientState.CLOSED:
            raise RuntimeError("Client is closed")
        if self._native_client is None:
            kwargs = {}
            if self._headers:
                kwargs["headers"] = _convert_headers(self._headers)
            if self._timeout:
                kwargs["timeout"] = _convert_timeout(self._timeout)
            if self._follow_redirects:
                kwargs["follow_redirects"] = self._follow_redirects
            if self._max_redirects is not None:
                kwargs["max_redirects"] = self._max_redirects
            if self._verify is not True:
                kwargs["verify"] = self._verify
            if self._cert is not None:
                kwargs["cert"] = self._cert
            if self._trust_env is not True:
                kwargs["trust_env"] = self._trust_env
            if self._limits:
                kwargs["limits"] = _convert_limits(self._limits)
            if self._proxy is not None:
                kwargs["proxy"] = _convert_proxy(self._proxy)
            if self._http2:
                kwargs["http2"] = self._http2
            self._native_client = eggfetch.Client(**kwargs)
            self._state = _ClientState.OPENED

    @property
    def auth(self):
        return self._auth

    @auth.setter
    def auth(self, value):
        self._auth = _build_auth(value)

    @property
    def base_url(self) -> URL:
        return self._base_url

    @base_url.setter
    def base_url(self, value):
        if isinstance(value, str):
            self._base_url = URL(value)
        elif isinstance(value, URL):
            self._base_url = value
        else:
            raise TypeError(f"base_url must be str or URL, got {type(value).__name__}")

    @property
    def cookies(self) -> Cookies:
        return self._cookies

    @cookies.setter
    def cookies(self, value):
        if isinstance(value, Cookies):
            self._cookies = value
        elif isinstance(value, dict):
            self._cookies = Cookies(value)
        else:
            self._cookies = Cookies(value)

    @property
    def event_hooks(self) -> dict:
        return self._event_hooks

    @event_hooks.setter
    def event_hooks(self, value):
        if not isinstance(value, dict):
            raise TypeError(f"event_hooks must be dict, got {type(value).__name__}")
        self._event_hooks = {
            "request": list(value.get("request", [])),
            "response": list(value.get("response", [])),
        }

    @property
    def headers(self) -> Headers:
        return self._headers

    @headers.setter
    def headers(self, value):
        if isinstance(value, Headers):
            self._headers = value
        elif isinstance(value, dict):
            self._headers = Headers(value)
        else:
            self._headers = Headers(value)

    @property
    def is_closed(self) -> bool:
        return self._state == _ClientState.CLOSED

    @property
    def params(self) -> QueryParams:
        return self._params

    @params.setter
    def params(self, value):
        if isinstance(value, QueryParams):
            self._params = value
        else:
            self._params = QueryParams(value)

    @property
    def timeout(self) -> Timeout:
        return self._timeout

    @timeout.setter
    def timeout(self, value):
        if isinstance(value, Timeout):
            self._timeout = value
        else:
            self._timeout = Timeout(value)

    @property
    def trust_env(self) -> bool:
        return self._trust_env

    def build_request(self, method, url, **kwargs):
        merged_url = self._merge_url(url)
        merged_params = self._merge_params(kwargs.get("params"))
        merged_headers = self._merge_headers(kwargs.get("headers"))
        merged_cookies = self._merge_cookies(kwargs.get("cookies"))
        merged_extensions = self._merge_extensions(kwargs.get("extensions"))

        if merged_params:
            merged_url = merged_url.copy_with(params=merged_params)

        return Request(
            method,
            merged_url,
            params=merged_params,
            headers=merged_headers,
            cookies=merged_cookies,
            content=kwargs.get("content"),
            data=kwargs.get("data"),
            files=kwargs.get("files"),
            json=kwargs.get("json"),
            stream=kwargs.get("stream"),
            extensions=merged_extensions if merged_extensions else None,
        )

    # ── One-hop dispatch (Track 1) ──────────────────────────────────────

    def _dispatch_one_hop(self, request, *, stream=False):
        """Send exactly one prepared Request through exactly one transport.

        This function must not:
        - apply auth
        - follow redirects
        - run user event hooks
        - mutate cookie state
        - append history

        Native dispatches always use ``follow_redirects=False``.
        """
        request.extensions.setdefault("_eggfetch_started_at", time.monotonic())
        _ensure_timeout_extension(request, self._timeout)

        transport = _match_mount(request.url, self._mounts)
        if transport is _MOUNT_NO_MATCH:
            # No mount matched — use default transport or native client.
            if self._transport is not None:
                return self._send_via_transport(
                    self._transport, request, stream=stream
                )
            return self._send_via_native(
                request, stream=stream,
            )
        if transport is None:
            # Explicit None mount — bypass to default direct transport.
            if self._transport is not None:
                return self._send_via_transport(
                    self._transport, request, stream=stream
                )
            return self._send_via_native(
                request, stream=stream,
            )
        # A specific transport was matched.
        return self._send_via_transport(
            transport, request, stream=stream
        )

    def _send_via_transport(self, transport, request, *, stream=False):
        try:
            native_resp = transport.handle_request(request)
        except Exception as exc:
            if not isinstance(exc, (RequestError, TransportError)):
                raise _map_exception(exc, request) from exc
            raise
        if stream:
            return _wrap_streaming_response(native_resp, request, self._default_encoding)
        return _wrap_response(native_resp, request, self._default_encoding)

    def _send_via_native(self, request, *, stream=False):
        self._ensure_client()
        # Track 1.2: Force one-hop mode — never follow redirects internally.
        kwargs = _build_native_kwargs(
            request, follow_redirects=False, timeout=_request_timeout(request, self._timeout),
        )
        try:
            if stream:
                native_resp = self._native_client.stream(**kwargs)
            else:
                native_resp = self._native_client.request(**kwargs)
        except Exception as exc:
            raise _map_exception(exc, request) from exc
        if stream:
            return _wrap_streaming_response(native_resp, request, self._default_encoding)
        return _wrap_response(native_resp, request, self._default_encoding)

    # ── Per-hop send (Track 4) ──────────────────────────────────────────

    def _send_single_request(self, request, *, stream=False):
        """Dispatch a single request through the transport layer.

        Before dispatching:
        - Set the Cookie header from the client jar (domain/path selection).
        After receiving the response:
        - Extract cookies from ``Set-Cookie`` headers into the client jar.

        This matches HTTPX's ``_send_single_request``.
        """
        request.extensions.setdefault("_eggfetch_started_at", time.monotonic())
        _ensure_timeout_extension(request, self._timeout)

        _prepare_cookie_header(request)

        transport = _match_mount(request.url, self._mounts)

        if transport is not _MOUNT_NO_MATCH and transport is not None:
            # A specific transport was matched.
            try:
                native_resp = transport.handle_request(request)
            except Exception as exc:
                if not isinstance(exc, (RequestError, TransportError)):
                    raise _map_exception(exc, request) from exc
                raise
        elif self._transport is not None:
            # No match or explicit None — use default transport.
            try:
                native_resp = self._transport.handle_request(request)
            except Exception as exc:
                if not isinstance(exc, (RequestError, TransportError)):
                    raise _map_exception(exc, request) from exc
                raise
        else:
            self._ensure_client()
            kwargs = _build_native_kwargs(
                request, follow_redirects=False, timeout=_request_timeout(request, self._timeout),
            )
            try:
                if stream:
                    native_resp = self._native_client.stream(**kwargs)
                else:
                    native_resp = self._native_client.request(**kwargs)
            except Exception as exc:
                raise _map_exception(exc, request) from exc

        if stream:
            response = _wrap_streaming_response(native_resp, request, self._default_encoding)
        else:
            response = _wrap_response(native_resp, request, self._default_encoding)

        # Extract cookies from response Set-Cookie headers into client jar
        self._cookies.extract_cookies(response)

        return response

    def _run_request_hooks(self, request):
        """Run request hooks, returning the (possibly mutated) request.

        If a hook raises, no dispatch occurs and the exception propagates.
        """
        for hook in self._event_hooks.get("request", []):
            hook(request)
        return request

    def _run_response_hooks(self, response):
        """Run response hooks on the response.

        If a hook raises, the response is closed and the exception propagates.
        """
        for hook in self._event_hooks.get("response", []):
            try:
                hook(response)
            except Exception:
                response.close()
                raise

    def send(self, request, *, stream=False, auth=_USE_CLIENT_DEFAULT,
             follow_redirects=None, timeout=_USE_CLIENT_DEFAULT):
        if self._state == _ClientState.CLOSED:
            raise RuntimeError("Client is closed")

        if not isinstance(request, Request):
            raise TypeError(f"send() requires a Request object, got {type(request).__name__}")

        effective_timeout = self._timeout if timeout is _USE_CLIENT_DEFAULT else timeout
        _ensure_timeout_extension(request, effective_timeout)

        # 1. Resolve effective follow_redirects
        effective_follow = (
            self._follow_redirects
            if follow_redirects is None
            else follow_redirects
        )

        # 2. Resolve auth
        if auth is _USE_CLIENT_DEFAULT:
            resolved_auth = self._auth
        elif auth is None:
            resolved_auth = None
        else:
            resolved_auth = _build_auth(auth)

        # If no explicit auth, check URL credentials
        if resolved_auth is None:
            resolved_auth = _extract_url_credentials(request.url)

        # 3. Auth flow + redirect handling (HTTPX 0.28.1 order)
        #    _send_handling_auth wraps _send_handling_redirects,
        #    which wraps _send_single_request (one transport hop).
        history: list[Response] = []

        if resolved_auth is not None:
            auth_flow_gen = resolved_auth.sync_auth_flow(request)
            try:
                request = next(auth_flow_gen)
            except StopIteration:
                auth_flow_gen = None
        else:
            auth_flow_gen = None

        try:
            while True:
                response = self._send_handling_redirects(
                    request,
                    follow_redirects=effective_follow,
                    history=history,
                    stream=stream,
                )

                if auth_flow_gen is None:
                    break

                try:
                    request = auth_flow_gen.send(response)
                except StopIteration:
                    auth_flow_gen = None
                    break

                # Auth produced a follow-up request — read and close
                # the intermediate response, then continue.
                response.read()
                response.close()
                history.append(response)
        finally:
            if auth_flow_gen is not None:
                auth_flow_gen.close()

        if not stream:
            response.read()

        return response

    def _send_handling_redirects(
        self,
        request: Request,
        *,
        follow_redirects: bool,
        history: list[Response],
        stream: bool = False,
    ) -> Response:
        """Follow redirects until a non-redirect response is received.

        This implements the redirect loop that was previously delegated to
        the native Rust engine.  Each hop runs request/response hooks and
        dispatches through ``_send_single_request``.
        """
        while True:
            if len(history) > self._max_redirects:
                raise TooManyRedirects(
                    "Exceeded maximum allowed redirects.", request=request
                )

            # Request hooks
            request = self._run_request_hooks(request)

            # One transport hop
            response = self._send_single_request(request, stream=stream)
            try:
                # Response hooks
                self._run_response_hooks(response)

                # Snapshot history for the response
                response.history = list(history)

                if not _is_redirect_status(response.status_code):
                    return response

                # Build the redirect request
                request = _build_redirect_request(self._cookies, request, response)
                history = history + [response]

                if follow_redirects:
                    # Drain the redirect response before following
                    response.read()
                else:
                    # Manual redirect: expose next_request, don't follow
                    response.next_request = request
                    return response

            except BaseException as exc:
                response.close()
                raise exc

    def request(self, method, url, *, auth=_USE_CLIENT_DEFAULT, params=None,
                headers=None, cookies=None,
                content=None, data=None, files=None, json=None,
                follow_redirects=None, timeout=_USE_CLIENT_DEFAULT, extensions=None):
        req = self.build_request(
            method, url,
            params=params, headers=headers, cookies=cookies,
            content=content, data=data, files=files, json=json,
            extensions=extensions,
        )
        return self.send(
            req,
            auth=auth,
            follow_redirects=follow_redirects,
            timeout=timeout,
        )

    def get(self, url, *, params=None, headers=None, cookies=None,
            auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
            timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return self.request(
            "GET", url,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    def post(self, url, *, content=None, data=None, files=None, json=None,
             params=None, headers=None, cookies=None,
             auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
             timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return self.request(
            "POST", url,
            content=content, data=data, files=files, json=json,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    def put(self, url, *, content=None, data=None, files=None, json=None,
            params=None, headers=None, cookies=None,
            auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
            timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return self.request(
            "PUT", url,
            content=content, data=data, files=files, json=json,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    def patch(self, url, *, content=None, data=None, files=None, json=None,
              params=None, headers=None, cookies=None,
              auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
              timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return self.request(
            "PATCH", url,
            content=content, data=data, files=files, json=json,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    def delete(self, url, *, params=None, headers=None, cookies=None,
               auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
               timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return self.request(
            "DELETE", url,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    def head(self, url, *, params=None, headers=None, cookies=None,
             auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
             timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return self.request(
            "HEAD", url,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    def options(self, url, *, params=None, headers=None, cookies=None,
                auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
                timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return self.request(
            "OPTIONS", url,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    @contextmanager
    def stream(self, method, url, *, content=None, data=None, files=None,
               json=None, params=None, headers=None, cookies=None,
               auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
               timeout=_USE_CLIENT_DEFAULT, extensions=None):
        if self._state == _ClientState.CLOSED:
            raise RuntimeError("Client is closed")
        req = self.build_request(
            method, url,
            content=content, data=data, files=files, json=json,
            params=params, headers=headers, cookies=cookies,
            extensions=extensions,
        )
        response = None
        try:
            response = self.send(
                req, stream=True,
                auth=auth,
                follow_redirects=follow_redirects,
                timeout=timeout,
            )
            yield response
        finally:
            if response is not None:
                response.close()

    # ── Ownership and close (Track 6) ───────────────────────────────────

    def close(self) -> None:
        if self._state == _ClientState.CLOSED:
            return

        last_error: Exception | None = None

        # Close default transport (if owned).
        if self._transport is not None and id(self._transport) in self._owned_transports:
            if hasattr(self._transport, "close"):
                try:
                    self._transport.close()
                except Exception as exc:
                    last_error = exc

        # Close mounted transports — deduplicate by id.
        seen: set[int] = set()
        for transport in self._mounts.values():
            if transport is None:
                continue
            tid = id(transport)
            if tid in seen:
                continue
            seen.add(tid)
            if hasattr(transport, "close"):
                try:
                    transport.close()
                except Exception as exc:
                    last_error = exc

        # Close native client.
        if self._native_client is not None:
            try:
                self._native_client.close()
            except Exception as exc:
                last_error = exc

        self._state = _ClientState.CLOSED
        if last_error is not None:
            raise last_error

    def __enter__(self) -> Client:
        self._ensure_client()
        return self

    def __exit__(self, *exc) -> None:
        self.close()

    def _merge_url(self, url):
        if self._base_url and self._base_url._raw:
            return URL(url, base_url=self._base_url)
        return URL(url) if not isinstance(url, URL) else url

    def _merge_params(self, request_params):
        if request_params is None:
            return self._params
        if isinstance(request_params, QueryParams):
            req = request_params
        else:
            req = QueryParams(request_params)
        req_keys = {k for k, _ in req.multi_items()}
        merged_items = [
            (k, v) for k, v in self._params.multi_items() if k not in req_keys
        ]
        merged_items.extend(req.multi_items())
        return QueryParams(merged_items)

    def _merge_headers(self, request_headers):
        merged = self._headers.copy()
        if request_headers is not None:
            if isinstance(request_headers, Headers):
                merged.update(request_headers)
            elif isinstance(request_headers, dict):
                merged.update(request_headers)
        return merged

    def _merge_cookies(self, request_cookies):
        if request_cookies is None:
            return self._cookies
        merged = Cookies(self._cookies)
        if isinstance(request_cookies, Cookies):
            merged.update(request_cookies)
        elif isinstance(request_cookies, dict):
            merged.update(request_cookies)
        return merged

    def _merge_extensions(self, request_extensions):
        if request_extensions is None:
            if self._extensions:
                return dict(self._extensions)
            return None
        if self._extensions:
            merged = dict(self._extensions)
            merged.update(request_extensions)
            return merged
        return request_extensions


class AsyncClient:
    def __init__(
        self,
        *,
        auth=None,
        params=None,
        headers=None,
        cookies=None,
        verify=True,
        cert=None,
        http1=True,
        http2=False,
        proxy=None,
        mounts=None,
        timeout=Timeout(5.0),
        follow_redirects=False,
        limits=Limits(100, 20, 5.0),
        max_redirects=20,
        event_hooks=None,
        base_url="",
        transport=None,
        trust_env=True,
        async_transport=None,
        default_encoding="utf-8",
        extensions=None,
    ):
        _validate_protocol_options(http1, http2)
        self._auth = _build_auth(auth)
        self._params = params if isinstance(params, QueryParams) else QueryParams(params)
        self._headers = _merge_default_headers(headers)
        self._cookies = cookies if isinstance(cookies, Cookies) else Cookies(cookies)
        self._verify = verify
        self._cert = cert
        self._trust_env = trust_env
        self._http1 = http1
        self._http2 = http2
        self._proxy = proxy
        self._timeout = timeout if isinstance(timeout, Timeout) else Timeout(timeout)
        self._follow_redirects = follow_redirects
        self._limits = limits if isinstance(limits, Limits) else Limits(limits)
        self._max_redirects = max_redirects
        self._event_hooks = event_hooks or {"request": [], "response": []}
        self._base_url = URL(base_url) if not isinstance(base_url, URL) else base_url
        # HTTPX's ``transport=`` argument is the async transport for
        # AsyncClient.  Keep the legacy explicit ``async_transport`` alias,
        # but dispatch the public argument through the async path as well.
        self._transport = None if transport is not None else transport
        self._async_transport = async_transport or transport
        self._default_encoding = default_encoding
        self._extensions = extensions or {}
        self._native_client = None
        self._state = _ClientState.UNOPENED

        # Track which transport objects we own for close deduplication.
        self._owned_transports: set[int] = set()

        self._mounts: dict[str, Any] = {}
        if mounts:
            for pattern, transport_obj in mounts.items():
                _validate_mount_pattern(pattern)
                self._mounts[pattern] = transport_obj
                if transport_obj is not None:
                    self._owned_transports.add(id(transport_obj))

        if self._transport is not None:
            self._owned_transports.add(id(self._transport))
        if self._async_transport is not None:
            self._owned_transports.add(id(self._async_transport))

    def _ensure_client(self):
        if self._transport is not None or self._async_transport is not None:
            if self._state == _ClientState.UNOPENED:
                self._state = _ClientState.OPENED
            return
        if self._state == _ClientState.CLOSED:
            raise RuntimeError("Client is closed")
        if self._native_client is None:
            kwargs = {}
            if self._headers:
                kwargs["headers"] = _convert_headers(self._headers)
            if self._timeout:
                kwargs["timeout"] = _convert_timeout(self._timeout)
            if self._follow_redirects:
                kwargs["follow_redirects"] = self._follow_redirects
            if self._max_redirects is not None:
                kwargs["max_redirects"] = self._max_redirects
            if self._verify is not True:
                kwargs["verify"] = self._verify
            if self._cert is not None:
                kwargs["cert"] = self._cert
            if self._trust_env is not True:
                kwargs["trust_env"] = self._trust_env
            if self._limits:
                kwargs["limits"] = _convert_limits(self._limits)
            if self._proxy is not None:
                kwargs["proxy"] = _convert_proxy(self._proxy)
            if self._http2:
                kwargs["http2"] = self._http2
            self._native_client = eggfetch.AsyncClient(**kwargs)
            self._state = _ClientState.OPENED

    @property
    def auth(self):
        return self._auth

    @auth.setter
    def auth(self, value):
        self._auth = _build_auth(value)

    @property
    def base_url(self) -> URL:
        return self._base_url

    @base_url.setter
    def base_url(self, value):
        if isinstance(value, str):
            self._base_url = URL(value)
        elif isinstance(value, URL):
            self._base_url = value
        else:
            raise TypeError(f"base_url must be str or URL, got {type(value).__name__}")

    @property
    def cookies(self) -> Cookies:
        return self._cookies

    @cookies.setter
    def cookies(self, value):
        if isinstance(value, Cookies):
            self._cookies = value
        elif isinstance(value, dict):
            self._cookies = Cookies(value)
        else:
            self._cookies = Cookies(value)

    @property
    def event_hooks(self) -> dict:
        return self._event_hooks

    @event_hooks.setter
    def event_hooks(self, value):
        if not isinstance(value, dict):
            raise TypeError(f"event_hooks must be dict, got {type(value).__name__}")
        self._event_hooks = {
            "request": list(value.get("request", [])),
            "response": list(value.get("response", [])),
        }

    @property
    def headers(self) -> Headers:
        return self._headers

    @headers.setter
    def headers(self, value):
        if isinstance(value, Headers):
            self._headers = value
        elif isinstance(value, dict):
            self._headers = Headers(value)
        else:
            self._headers = Headers(value)

    @property
    def is_closed(self) -> bool:
        return self._state == _ClientState.CLOSED

    @property
    def params(self) -> QueryParams:
        return self._params

    @params.setter
    def params(self, value):
        if isinstance(value, QueryParams):
            self._params = value
        else:
            self._params = QueryParams(value)

    @property
    def timeout(self) -> Timeout:
        return self._timeout

    @timeout.setter
    def timeout(self, value):
        if isinstance(value, Timeout):
            self._timeout = value
        else:
            self._timeout = Timeout(value)

    @property
    def trust_env(self) -> bool:
        return self._trust_env

    def build_request(self, method, url, **kwargs):
        merged_url = self._merge_url(url)
        merged_params = self._merge_params(kwargs.get("params"))
        merged_headers = self._merge_headers(kwargs.get("headers"))
        merged_cookies = self._merge_cookies(kwargs.get("cookies"))
        merged_extensions = self._merge_extensions(kwargs.get("extensions"))

        if merged_params:
            merged_url = merged_url.copy_with(params=merged_params)

        return Request(
            method,
            merged_url,
            params=merged_params,
            headers=merged_headers,
            cookies=merged_cookies,
            content=kwargs.get("content"),
            data=kwargs.get("data"),
            files=kwargs.get("files"),
            json=kwargs.get("json"),
            stream=kwargs.get("stream"),
            extensions=merged_extensions if merged_extensions else None,
        )

    # ── One-hop dispatch (Track 1) ──────────────────────────────────────

    async def _dispatch_one_hop(self, request, *, stream=False):
        """Send exactly one prepared Request through exactly one transport.

        This function must not:
        - apply auth
        - follow redirects
        - run user event hooks
        - mutate cookie state
        - append history

        Native dispatches always use ``follow_redirects=False``.
        """
        request.extensions.setdefault("_eggfetch_started_at", time.monotonic())
        _ensure_timeout_extension(request, self._timeout)

        transport = _match_mount(request.url, self._mounts)
        if transport is _MOUNT_NO_MATCH:
            if self._async_transport is not None:
                return await self._send_via_transport(
                    self._async_transport, request, stream=stream
                )
            if self._transport is not None:
                return self._send_via_transport_sync(
                    self._transport, request, stream=stream
                )
            return await self._send_via_native(
                request, stream=stream,
            )
        if transport is None:
            # Explicit None mount — bypass to default direct transport.
            if self._async_transport is not None:
                return await self._send_via_transport(
                    self._async_transport, request, stream=stream
                )
            if self._transport is not None:
                return self._send_via_transport_sync(
                    self._transport, request, stream=stream
                )
            return await self._send_via_native(
                request, stream=stream,
            )
        # A specific transport was matched.
        return await self._send_via_transport(
            transport, request, stream=stream
        )

    async def _send_via_transport(self, transport, request, *, stream=False):
        try:
            native_resp = await transport.handle_async_request(request)
        except Exception as exc:
            if not isinstance(exc, (RequestError, TransportError)):
                raise _map_exception(exc, request) from exc
            raise
        if stream:
            return _wrap_streaming_response(native_resp, request, self._default_encoding)
        return _wrap_response(native_resp, request, self._default_encoding)

    def _send_via_transport_sync(self, transport, request, *, stream=False):
        try:
            native_resp = transport.handle_request(request)
        except Exception as exc:
            if not isinstance(exc, (RequestError, TransportError)):
                raise _map_exception(exc, request) from exc
            raise
        if stream:
            return _wrap_streaming_response(native_resp, request, self._default_encoding)
        return _wrap_response(native_resp, request, self._default_encoding)

    async def _send_via_native(self, request, *, stream=False):
        self._ensure_client()
        # Track 1.2: Force one-hop mode — never follow redirects internally.
        kwargs = _build_native_kwargs(
            request, follow_redirects=False, timeout=_request_timeout(request, self._timeout),
        )
        try:
            if stream:
                native_resp = await self._native_client.stream(**kwargs)
            else:
                native_resp = await self._native_client.request(**kwargs)
        except Exception as exc:
            raise _map_exception(exc, request) from exc
        if stream:
            return _wrap_streaming_response(native_resp, request, self._default_encoding)
        return _wrap_response(native_resp, request, self._default_encoding)

    async def _send_single_request(self, request, *, stream=False):
        """Dispatch a single request through the transport layer (async).

        Before dispatching:
        - Set the Cookie header from the client jar (domain/path selection).
        After receiving the response:
        - Extract cookies from ``Set-Cookie`` headers into the client jar.
        """
        request.extensions.setdefault("_eggfetch_started_at", time.monotonic())
        _ensure_timeout_extension(request, self._timeout)

        _prepare_cookie_header(request)

        transport = _match_mount(request.url, self._mounts)

        if transport is not _MOUNT_NO_MATCH and transport is not None:
            # A specific transport was matched.
            try:
                if hasattr(transport, "handle_async_request"):
                    native_resp = await transport.handle_async_request(request)
                else:
                    native_resp = transport.handle_request(request)
            except Exception as exc:
                if not isinstance(exc, (RequestError, TransportError)):
                    raise _map_exception(exc, request) from exc
                raise
        elif self._async_transport is not None:
            try:
                native_resp = await self._async_transport.handle_async_request(request)
            except Exception as exc:
                if not isinstance(exc, (RequestError, TransportError)):
                    raise _map_exception(exc, request) from exc
                raise
        elif self._transport is not None:
            try:
                native_resp = self._transport.handle_request(request)
            except Exception as exc:
                if not isinstance(exc, (RequestError, TransportError)):
                    raise _map_exception(exc, request) from exc
                raise
        else:
            self._ensure_client()
            kwargs = _build_native_kwargs(
                request, follow_redirects=False, timeout=_request_timeout(request, self._timeout),
            )
            try:
                if stream:
                    native_resp = await self._native_client.stream(**kwargs)
                else:
                    native_resp = await self._native_client.request(**kwargs)
            except Exception as exc:
                raise _map_exception(exc, request) from exc

        if stream:
            response = _wrap_streaming_response(native_resp, request, self._default_encoding)
        else:
            response = _wrap_response(native_resp, request, self._default_encoding)

        # Extract cookies from response Set-Cookie headers into client jar
        self._cookies.extract_cookies(response)

        return response

    # ── Per-hop send (Track 4) ──────────────────────────────────────────

    async def _run_request_hooks(self, request):
        """Run request hooks, awaiting if necessary."""
        for hook in self._event_hooks.get("request", []):
            result = hook(request)
            if asyncio.iscoroutine(result) or asyncio.isfuture(result):
                await result
        return request

    async def _run_response_hooks(self, response):
        """Run response hooks on the response."""
        for hook in self._event_hooks.get("response", []):
            try:
                result = hook(response)
                if asyncio.iscoroutine(result) or asyncio.isfuture(result):
                    await result
            except Exception:
                await response.aclose()
                raise

    async def send(self, request, *, stream=False, auth=_USE_CLIENT_DEFAULT,
                   follow_redirects=None, timeout=_USE_CLIENT_DEFAULT):
        if self._state == _ClientState.CLOSED:
            raise RuntimeError("Client is closed")

        if not isinstance(request, Request):
            raise TypeError(f"send() requires a Request object, got {type(request).__name__}")

        effective_timeout = self._timeout if timeout is _USE_CLIENT_DEFAULT else timeout
        _ensure_timeout_extension(request, effective_timeout)

        # 1. Resolve effective follow_redirects
        effective_follow = (
            self._follow_redirects
            if follow_redirects is None
            else follow_redirects
        )

        # 2. Resolve auth
        if auth is _USE_CLIENT_DEFAULT:
            resolved_auth = self._auth
        elif auth is None:
            resolved_auth = None
        else:
            resolved_auth = _build_auth(auth)

        # If no explicit auth, check URL credentials
        if resolved_auth is None:
            resolved_auth = _extract_url_credentials(request.url)

        # 3. Auth flow + redirect handling (HTTPX 0.28.1 order)
        history: list[Response] = []

        if resolved_auth is not None:
            auth_flow_gen = resolved_auth.async_auth_flow(request)
            try:
                request = await auth_flow_gen.__anext__()
            except StopAsyncIteration:
                auth_flow_gen = None
        else:
            auth_flow_gen = None

        try:
            while True:
                response = await self._send_handling_redirects(
                    request,
                    follow_redirects=effective_follow,
                    history=history,
                    stream=stream,
                )

                if auth_flow_gen is None:
                    break

                try:
                    request = await auth_flow_gen.asend(response)
                except StopAsyncIteration:
                    auth_flow_gen = None
                    break

                # Auth produced a follow-up — read and close intermediate response
                await response.aread()
                await response.aclose()
                history.append(response)
        finally:
            if auth_flow_gen is not None:
                await auth_flow_gen.aclose()

        if not stream:
            await response.aread()

        return response

    async def _send_handling_redirects(
        self,
        request: Request,
        *,
        follow_redirects: bool,
        history: list[Response],
        stream: bool = False,
    ) -> Response:
        """Follow redirects until a non-redirect response is received."""
        while True:
            if len(history) > self._max_redirects:
                raise TooManyRedirects(
                    "Exceeded maximum allowed redirects.", request=request
                )

            # Request hooks
            request = await self._run_request_hooks(request)

            # One transport hop
            response = await self._send_single_request(request, stream=stream)
            try:
                # Response hooks
                await self._run_response_hooks(response)

                # Snapshot history
                response.history = list(history)

                if not _is_redirect_status(response.status_code):
                    return response

                # Build the redirect request
                request = _build_redirect_request(self._cookies, request, response)
                history = history + [response]

                if follow_redirects:
                    await response.aread()
                else:
                    response.next_request = request
                    return response

            except BaseException as exc:
                await response.aclose()
                raise exc

    async def request(self, method, url, *, auth=_USE_CLIENT_DEFAULT, params=None,
                      headers=None, cookies=None,
                      content=None, data=None, files=None, json=None,
                      follow_redirects=None, timeout=_USE_CLIENT_DEFAULT, extensions=None):
        req = self.build_request(
            method, url,
            params=params, headers=headers, cookies=cookies,
            content=content, data=data, files=files, json=json,
            extensions=extensions,
        )
        return await self.send(
            req,
            auth=auth,
            follow_redirects=follow_redirects,
            timeout=timeout,
        )

    async def get(self, url, *, params=None, headers=None, cookies=None,
                  auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
                  timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return await self.request(
            "GET", url,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    async def post(self, url, *, content=None, data=None, files=None, json=None,
                   params=None, headers=None, cookies=None,
                   auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
                   timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return await self.request(
            "POST", url,
            content=content, data=data, files=files, json=json,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    async def put(self, url, *, content=None, data=None, files=None, json=None,
                  params=None, headers=None, cookies=None,
                  auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
                  timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return await self.request(
            "PUT", url,
            content=content, data=data, files=files, json=json,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    async def patch(self, url, *, content=None, data=None, files=None, json=None,
                    params=None, headers=None, cookies=None,
                    auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
                    timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return await self.request(
            "PATCH", url,
            content=content, data=data, files=files, json=json,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    async def delete(self, url, *, params=None, headers=None, cookies=None,
                     auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
                     timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return await self.request(
            "DELETE", url,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    async def head(self, url, *, params=None, headers=None, cookies=None,
                   auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
                   timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return await self.request(
            "HEAD", url,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    async def options(self, url, *, params=None, headers=None, cookies=None,
                      auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
                      timeout=_USE_CLIENT_DEFAULT, extensions=None):
        return await self.request(
            "OPTIONS", url,
            params=params, headers=headers, cookies=cookies,
            auth=auth, follow_redirects=follow_redirects,
            timeout=timeout, extensions=extensions,
        )

    @asynccontextmanager
    async def stream(self, method, url, *, content=None, data=None, files=None,
                     json=None, params=None, headers=None, cookies=None,
                     auth=_USE_CLIENT_DEFAULT, follow_redirects=None,
                     timeout=_USE_CLIENT_DEFAULT, extensions=None):
        if self._state == _ClientState.CLOSED:
            raise RuntimeError("Client is closed")
        req = self.build_request(
            method, url,
            content=content, data=data, files=files, json=json,
            params=params, headers=headers, cookies=cookies,
            extensions=extensions,
        )
        response = None
        try:
            response = await self.send(
                req, stream=True,
                auth=auth,
                follow_redirects=follow_redirects,
                timeout=timeout,
            )
            yield response
        finally:
            if response is not None:
                if hasattr(response, "aclose"):
                    await response.aclose()
                else:
                    response.close()

    # ── Ownership and close (Track 6) ───────────────────────────────────

    async def close(self) -> None:
        if self._state == _ClientState.CLOSED:
            return

        last_error: Exception | None = None

        # Close async default transport (if owned).
        if self._async_transport is not None and id(self._async_transport) in self._owned_transports:
            if hasattr(self._async_transport, "aclose"):
                try:
                    await self._async_transport.aclose()
                except Exception as exc:
                    last_error = exc

        # Close sync default transport (if owned).
        if self._transport is not None and id(self._transport) in self._owned_transports:
            if hasattr(self._transport, "close"):
                try:
                    self._transport.close()
                except Exception as exc:
                    last_error = exc

        # Close mounted transports — deduplicate by id.
        seen: set[int] = set()
        for transport in self._mounts.values():
            if transport is None:
                continue
            tid = id(transport)
            if tid in seen:
                continue
            seen.add(tid)
            if hasattr(transport, "aclose"):
                try:
                    await transport.aclose()
                except Exception as exc:
                    last_error = exc
            elif hasattr(transport, "close"):
                try:
                    transport.close()
                except Exception as exc:
                    last_error = exc

        # Close native client.
        if self._native_client is not None:
            try:
                await self._native_client.aclose()
            except Exception as exc:
                last_error = exc

        self._state = _ClientState.CLOSED
        if last_error is not None:
            raise last_error

    async def aclose(self) -> None:
        await self.close()

    async def __aenter__(self) -> AsyncClient:
        self._ensure_client()
        return self

    async def __aexit__(self, *exc) -> None:
        await self.close()

    def _merge_url(self, url):
        if self._base_url and self._base_url._raw:
            return URL(url, base_url=self._base_url)
        return URL(url) if not isinstance(url, URL) else url

    def _merge_params(self, request_params):
        if request_params is None:
            return self._params
        if isinstance(request_params, QueryParams):
            req = request_params
        else:
            req = QueryParams(request_params)
        req_keys = {k for k, _ in req.multi_items()}
        merged_items = [
            (k, v) for k, v in self._params.multi_items() if k not in req_keys
        ]
        merged_items.extend(req.multi_items())
        return QueryParams(merged_items)

    def _merge_headers(self, request_headers):
        merged = self._headers.copy()
        if request_headers is not None:
            if isinstance(request_headers, Headers):
                merged.update(request_headers)
            elif isinstance(request_headers, dict):
                merged.update(request_headers)
        return merged

    def _merge_cookies(self, request_cookies):
        if request_cookies is None:
            return self._cookies
        merged = Cookies(self._cookies)
        if isinstance(request_cookies, Cookies):
            merged.update(request_cookies)
        elif isinstance(request_cookies, dict):
            merged.update(request_cookies)
        return merged

    def _merge_extensions(self, request_extensions):
        if request_extensions is None:
            if self._extensions:
                return dict(self._extensions)
            return None
        if self._extensions:
            merged = dict(self._extensions)
            merged.update(request_extensions)
            return merged
        return request_extensions
