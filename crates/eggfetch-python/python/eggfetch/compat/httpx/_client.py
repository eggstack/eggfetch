"""HTTPX-compatible Client and AsyncClient for eggfetch."""

from __future__ import annotations

import asyncio
import enum
import typing
import urllib.parse
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


# ── Transport unsupported-option validation ─────────────────────────────

def _validate_transport_options(
    *,
    uds: str | None = None,
    local_address: str | None = None,
    socket_options: typing.Any | None = None,
) -> None:
    """Reject unsupported transport options before any network activity."""
    if uds is not None:
        raise NotImplementedError(
            "eggfetch does not support Unix domain sockets (uds)"
        )
    if local_address is not None:
        raise NotImplementedError(
            "eggfetch does not support local_address"
        )
    if socket_options is not None:
        raise NotImplementedError(
            "eggfetch does not support socket_options"
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
        if timeout.total is not None:
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
        return str(proxy.url)
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

    content = native_resp.content

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

    # Elapsed time: use native elapsed if available, else measure zero
    if hasattr(native_resp, "elapsed") and native_resp.elapsed is not None:
        resp.elapsed = native_resp.elapsed
    else:
        resp.elapsed = timedelta(0)

    return resp


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

    # Elapsed time for streaming: set to zero initially (will be updated
    # after close/read per HTTPX semantics)
    from datetime import timedelta
    response.elapsed = timedelta(0)

    return response


def _build_native_kwargs(request, follow_redirects=None, timeout=_USE_CLIENT_DEFAULT):
    kwargs: dict[str, Any] = {}
    if isinstance(request, Request):
        kwargs["method"] = request.method
        kwargs["url"] = str(request.url)
        if request.headers:
            kwargs["headers"] = _convert_headers(request.headers)
        if request.params:
            kwargs["params"] = _convert_params(request.params)
        if request._stream is not None and request._content is None:
            kwargs["content"] = request._stream
        elif request.content is not None:
            kwargs["content"] = request.content
        if request._files is not None:
            kwargs["files"] = request._files
            # When data + files are both present (multipart), pass the
            # data fields so the native multipart encoder can include them.
            if hasattr(request, "_multipart_data") and request._multipart_data is not None:
                kwargs["data"] = request._multipart_data
        if request.cookies:
            kwargs["cookies"] = _convert_cookies(request.cookies)
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

def _ensure_timeout_extension(request: Request, timeout) -> None:
    """Put effective timeout into request extensions if not already set.

    Custom transports may read ``extensions["timeout"]`` to configure
    their own timeout logic, matching HTTPX's transport contract.
    """
    if "timeout" in request.extensions:
        return
    if timeout is _USE_CLIENT_DEFAULT:
        return
    if timeout is None:
        request.extensions["timeout"] = Timeout(None)
    elif isinstance(timeout, Timeout):
        request.extensions["timeout"] = timeout
    elif isinstance(timeout, (int, float)):
        request.extensions["timeout"] = Timeout(timeout)


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
            if self._cookies:
                kwargs["cookies"] = _convert_cookies(self._cookies)
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
            request, follow_redirects=False, timeout=self._timeout,
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

    def _run_request_hooks(self, request):
        """Run request hooks, returning the (possibly mutated) request.

        If a hook raises, no dispatch occurs and the exception propagates.
        """
        for hook in self._event_hooks.get("request", []):
            hook(request)
        return request

    def _run_response_hooks(self, response):
        """Run response hooks on the final response.

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

        # 1. Resolve auth
        if auth is _USE_CLIENT_DEFAULT:
            resolved_auth = self._auth
        elif auth is None:
            resolved_auth = None
        else:
            resolved_auth = _build_auth(auth)

        # 2. Initialize auth flow generator
        use_auth = resolved_auth is not None
        auth_flow_gen = None
        if use_auth:
            auth_flow_gen = resolved_auth.sync_auth_flow(request)
            try:
                request = next(auth_flow_gen)
            except StopIteration:
                auth_flow_gen = None

        # 3. Per-hop dispatch loop
        #    For each hop: hooks → dispatch → response hooks → auth/redirect decision
        while True:
            # 3a. Request hooks (see the transported Request)
            request = self._run_request_hooks(request)

            # 3b. One-hop transport dispatch
            response = self._dispatch_one_hop(
                request, stream=stream,
            )

            # 3c. Response hooks (see the response before auth decides)
            self._run_response_hooks(response)

            # 3d. Auth/redirect state machine decision
            if auth_flow_gen is None:
                break

            try:
                request = auth_flow_gen.send(response)
            except StopAsyncIteration:
                auth_flow_gen = None
                break
            except StopIteration:
                auth_flow_gen = None
                break

            # Intermediate auth response — drain body and close before
            # dispatching the follow-up request.
            response.read()
            response.close()

        return response

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

    def get(self, url, **kwargs):
        return self.request("GET", url, **kwargs)

    def post(self, url, **kwargs):
        return self.request("POST", url, **kwargs)

    def put(self, url, **kwargs):
        return self.request("PUT", url, **kwargs)

    def patch(self, url, **kwargs):
        return self.request("PATCH", url, **kwargs)

    def delete(self, url, **kwargs):
        return self.request("DELETE", url, **kwargs)

    def head(self, url, **kwargs):
        return self.request("HEAD", url, **kwargs)

    def options(self, url, **kwargs):
        return self.request("OPTIONS", url, **kwargs)

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
        self._transport = transport
        self._async_transport = async_transport
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
            if self._cookies:
                kwargs["cookies"] = _convert_cookies(self._cookies)
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
            request, follow_redirects=False, timeout=self._timeout,
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

    # ── Per-hop send (Track 4) ──────────────────────────────────────────

    async def _run_request_hooks(self, request):
        """Run request hooks, awaiting if necessary.

        Matches HTTPX: callable objects returning awaitables are awaited
        in async mode.  Plain callables are called directly.
        """
        for hook in self._event_hooks.get("request", []):
            result = hook(request)
            if asyncio.iscoroutine(result) or asyncio.isfuture(result):
                await result
        return request

    async def _run_response_hooks(self, response):
        """Run response hooks on the final response.

        If a hook raises, the response is closed and the exception propagates.
        """
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

        # 1. Resolve auth
        if auth is _USE_CLIENT_DEFAULT:
            resolved_auth = self._auth
        elif auth is None:
            resolved_auth = None
        else:
            resolved_auth = _build_auth(auth)

        # 2. Initialize auth flow generator
        use_auth = resolved_auth is not None
        auth_flow_gen = None
        if use_auth:
            auth_flow_gen = resolved_auth.async_auth_flow(request)
            try:
                request = await auth_flow_gen.__anext__()
            except StopAsyncIteration:
                auth_flow_gen = None

        # 3. Per-hop dispatch loop
        while True:
            # 3a. Request hooks (see the transported Request)
            request = await self._run_request_hooks(request)

            # 3b. One-hop transport dispatch
            response = await self._dispatch_one_hop(
                request, stream=stream,
            )

            # 3c. Response hooks (see the response before auth decides)
            await self._run_response_hooks(response)

            # 3d. Auth/redirect state machine decision
            if auth_flow_gen is None:
                break

            try:
                request = await auth_flow_gen.asend(response)
            except StopAsyncIteration:
                auth_flow_gen = None
                break

            # Intermediate auth response — drain body and close before
            # dispatching the follow-up request.
            await response.aread()
            await response.aclose()

        return response

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

    async def get(self, url, **kwargs):
        return await self.request("GET", url, **kwargs)

    async def post(self, url, **kwargs):
        return await self.request("POST", url, **kwargs)

    async def put(self, url, **kwargs):
        return await self.request("PUT", url, **kwargs)

    async def patch(self, url, **kwargs):
        return await self.request("PATCH", url, **kwargs)

    async def delete(self, url, **kwargs):
        return await self.request("DELETE", url, **kwargs)

    async def head(self, url, **kwargs):
        return await self.request("HEAD", url, **kwargs)

    async def options(self, url, **kwargs):
        return await self.request("OPTIONS", url, **kwargs)

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
