"""HTTPX-compatible Client and AsyncClient for eggfetch."""

from __future__ import annotations

import asyncio
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


def _convert_timeout(timeout):
    if isinstance(timeout, Timeout):
        return eggfetch.Timeout(
            connect=timeout.connect,
            read=timeout.read,
            write=timeout.write,
            pool=timeout.pool,
        )
    if isinstance(timeout, (int, float)):
        return eggfetch.Timeout(connect=timeout, read=timeout, write=timeout, pool=timeout)
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
        return dict(headers.items())
    if isinstance(headers, dict):
        return headers
    return None


def _convert_cookies(cookies):
    if isinstance(cookies, Cookies):
        return dict(cookies.items())
    if isinstance(cookies, dict):
        return cookies
    return None


def _convert_params(params):
    if isinstance(params, QueryParams):
        return dict(params.items())
    if isinstance(params, dict):
        return params
    return None


def _convert_proxy(proxy):
    if isinstance(proxy, Proxy):
        return str(proxy.url)
    if isinstance(proxy, str):
        return proxy
    return None


def _map_exception(native_exc, compat_request=None):
    exc_type = type(native_exc)
    exc_name = exc_type.__name__
    msg = str(native_exc)

    if exc_name == "TimeoutException" or isinstance(native_exc, eggfetch.TimeoutException):
        detail = getattr(native_exc, "detail", None)
        if detail:
            if "connect" in str(detail).lower():
                return ConnectTimeout(message=msg, request=compat_request)
            if "read" in str(detail).lower():
                return ReadTimeout(message=msg, request=compat_request)
            if "write" in str(detail).lower():
                return WriteTimeout(message=msg, request=compat_request)
            if "pool" in str(detail).lower():
                return PoolTimeout(message=msg, request=compat_request)
        return TimeoutException(message=msg, request=compat_request)

    if exc_name == "ConnectError" or exc_name == "NetworkError":
        return ConnectError(message=msg, request=compat_request)

    if exc_name == "ProtocolError":
        return ProtocolError(message=msg, request=compat_request)

    if exc_name == "TooManyRedirects":
        return TooManyRedirects(message=msg, request=compat_request)

    if exc_name == "InvalidUrl":
        return InvalidURL(msg)

    if exc_name == "ProxyError":
        return ProxyError(message=msg, request=compat_request)

    return RequestError(message=msg, request=compat_request)


def _wrap_response(native_resp, compat_request=None, default_encoding="utf-8"):
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

    # Build extensions: preserve request extensions + map standard keys
    extensions: dict = {}
    if compat_request is not None and hasattr(compat_request, "extensions"):
        extensions.update(compat_request.extensions)

    # Map standard extension keys from native response
    if hasattr(native_resp, "http_version") and native_resp.http_version:
        extensions["http_version"] = native_resp.http_version
    if hasattr(native_resp, "reason_phrase") and native_resp.reason_phrase:
        extensions["reason_phrase"] = native_resp.reason_phrase

    return Response(
        status_code,
        headers=header_list,
        content=content,
        request=compat_request,
        history=history,
        default_encoding=default_encoding,
        extensions=extensions if extensions else None,
    )


def _wrap_streaming_response(native_resp, compat_request=None, default_encoding="utf-8"):
    # If native_resp is already a compat Response (e.g. from MockTransport),
    # extract its stream for proper iteration.
    if isinstance(native_resp, Response):
        status_code = native_resp.status_code
        header_list = native_resp.headers.multi_items()
        stream_obj = native_resp._stream
        history = native_resp.history or []
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

    # Build extensions: preserve request extensions + map standard keys
    extensions: dict = {}
    if compat_request is not None and hasattr(compat_request, "extensions"):
        extensions.update(compat_request.extensions)

    if hasattr(native_resp, "http_version") and native_resp.http_version:
        extensions["http_version"] = native_resp.http_version
    if hasattr(native_resp, "reason_phrase") and native_resp.reason_phrase:
        extensions["reason_phrase"] = native_resp.reason_phrase

    response = Response(
        status_code,
        headers=header_list,
        stream=stream_obj,
        request=compat_request,
        history=history,
        default_encoding=default_encoding,
        extensions=extensions if extensions else None,
    )
    # Only set _native_stream for objects with a .read() method (native
    # eggfetch streams).  Python generators/iterables go through _stream
    # iteration instead.
    if hasattr(stream_obj, "read"):
        response._native_stream = stream_obj
    return response


def _build_native_kwargs(request, follow_redirects=None, timeout=None):
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
        if request.cookies:
            kwargs["cookies"] = _convert_cookies(request.cookies)
    else:
        raise TypeError(f"send() requires a Request object, got {type(request).__name__}")

    if follow_redirects is not None:
        kwargs["follow_redirects"] = follow_redirects
    if timeout is not None:
        kwargs["timeout"] = _convert_timeout(timeout)

    return kwargs


def _parse_mount_pattern(pattern: str):
    """Parse a mount pattern into (scheme, host, port, path) components.

    Handles patterns like:
    - ``all://`` → ("", None, None, "")
    - ``http://`` → ("http", None, None, "")
    - ``https://`` → ("https", None, None, "")
    - ``http://example.com`` → ("http", "example.com", None, "")
    - ``http://example.com:8080`` → ("http", "example.com", 8080, "")
    - ``http://example.com/api`` → ("http", "example.com", None, "/api")
    """
    if pattern == "all://":
        return ("", None, None, "")

    parsed = urllib.parse.urlsplit(pattern)
    scheme = parsed.scheme.lower()
    host = parsed.hostname
    port = parsed.port
    path = parsed.path.rstrip("/") or ""
    return (scheme, host, port, path)


def _match_mount(url, mounts):
    """Find the best matching transport for *url* from *mounts* dict.

    Uses component-based matching (scheme, host, port, path) rather than
    string prefix matching.  Scoring priority (highest wins):

    - Full URL match (scheme + host + port + path)             — 10 000
    - Scheme + host + path                                      — 200 + len(path)
    - Scheme + host + port                                      — 205
    - Scheme + host                                             — 200
    - Scheme only (``http://`` or ``https://``)                  — 10
    - Catch-all (``all://``)                                    — 0

    If no mount matches, return ``None``.
    """
    if not mounts:
        return None

    url_str = str(url)
    url_parts = urllib.parse.urlsplit(url_str)
    url_scheme = url_parts.scheme.lower()
    url_host = url_parts.hostname
    url_port = url_parts.port
    url_path = url_parts.path.rstrip("/") or ""

    best_match: str | None = None
    best_score: int = -1

    for pattern in mounts:
        pat_scheme, pat_host, pat_port, pat_path = _parse_mount_pattern(pattern)

        # Catch-all: always matches, lowest priority
        if pat_scheme == "" and pat_host is None:
            score = 0
            if score > best_score:
                best_score = score
                best_match = pattern
            continue

        # Scheme must match (or pattern has no scheme)
        if pat_scheme and pat_scheme != url_scheme:
            continue

        # Host must match (or pattern has no host)
        if pat_host is not None:
            if url_host is None:
                continue
            if pat_host.lower() != url_host.lower():
                continue

        # Port must match (or pattern has no port)
        if pat_port is not None:
            if url_port != pat_port:
                continue

        # Path must be a prefix (or pattern has no path)
        if pat_path:
            # Exact match or prefix followed by /
            if url_path != pat_path and not url_path.startswith(pat_path + "/"):
                continue

        # Compute score — more specific matches get higher scores.
        # Each additional component (host, port, path) adds specificity.
        if pat_host is None and not pat_path:
            # Scheme-only pattern (e.g. ``http://``)
            score = 10
        elif pat_host is not None and not pat_path and pat_port is None:
            # Host pattern without port or path
            score = 200
        elif pat_host is not None and pat_port is not None and not pat_path:
            # Host + port pattern
            score = 205
        elif pat_host is not None and pat_path:
            # Host + path (with or without port)
            base = 205 if pat_port is not None else 200
            score = base + len(pat_path)
        else:
            score = 0

        if score > best_score:
            best_score = score
            best_match = pattern

    if best_match is not None:
        return mounts[best_match]
    return None


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
        self._auth = auth
        self._params = params if isinstance(params, QueryParams) else QueryParams(params)
        self._headers = headers if isinstance(headers, Headers) else Headers(headers)
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
        self._is_closed = False

        self._mounts: dict[str, Any] = {}
        if mounts:
            for pattern, transport_obj in mounts.items():
                self._mounts[pattern] = transport_obj

    def _ensure_client(self):
        if self._transport is not None:
            return
        if self._native_client is None or self._is_closed:
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
            self._is_closed = False

    @property
    def auth(self):
        return self._auth

    @property
    def base_url(self) -> URL:
        return self._base_url

    @property
    def cookies(self) -> Cookies:
        return self._cookies

    @property
    def event_hooks(self) -> dict:
        return self._event_hooks

    @property
    def headers(self) -> Headers:
        return self._headers

    @property
    def is_closed(self) -> bool:
        return self._is_closed

    @property
    def params(self) -> QueryParams:
        return self._params

    @property
    def timeout(self) -> Timeout:
        return self._timeout

    @property
    def trust_env(self) -> bool:
        return self._trust_env

    def build_request(self, method, url, **kwargs):
        merged_url = self._merge_url(url)
        merged_params = self._merge_params(kwargs.get("params"))
        merged_headers = self._merge_headers(kwargs.get("headers"))
        merged_cookies = self._merge_cookies(kwargs.get("cookies"))
        merged_extensions = self._merge_extensions(kwargs.get("extensions"))

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

    def _dispatch_request(self, request, *, stream=False, follow_redirects=None,
                          timeout=None):
        transport = _match_mount(request.url, self._mounts)
        if transport is not None:
            return self._send_via_transport(
                transport, request, stream=stream
            )
        if self._transport is not None:
            return self._send_via_transport(
                self._transport, request, stream=stream
            )
        return self._send_via_native(
            request, stream=stream,
            follow_redirects=follow_redirects, timeout=timeout,
        )

    def _send_via_transport(self, transport, request, *, stream=False):
        native_resp = transport.handle_request(request)
        if stream:
            return _wrap_streaming_response(native_resp, request, self._default_encoding)
        return _wrap_response(native_resp, request, self._default_encoding)

    def _send_via_native(self, request, *, stream=False, follow_redirects=None,
                         timeout=None):
        self._ensure_client()
        kwargs = _build_native_kwargs(request, follow_redirects=follow_redirects,
                                       timeout=timeout)
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

    def send(self, request, *, stream=False, auth=_USE_CLIENT_DEFAULT,
             follow_redirects=None, timeout=None):
        if self._is_closed:
            raise RuntimeError("Client is closed")

        if not isinstance(request, Request):
            raise TypeError(f"send() requires a Request object, got {type(request).__name__}")

        # 1. Resolve auth
        if auth is _USE_CLIENT_DEFAULT:
            resolved_auth = self._auth
        else:
            resolved_auth = auth

        # 2. Execute request hooks BEFORE auth and dispatch
        for hook in self._event_hooks.get("request", []):
            hook(request)

        # 3. Run auth flow (generator pattern)
        # Auth is NOT applied when a custom transport or mount transport handles
        # the request — transports own their own auth.
        transport = _match_mount(request.url, self._mounts)
        use_auth = resolved_auth is not None and transport is None and self._transport is None

        auth_flow_gen = None
        if use_auth:
            auth_flow_gen = resolved_auth.auth_flow(request)
            try:
                request = next(auth_flow_gen)
            except StopIteration:
                auth_flow_gen = None

        # 4. Dispatch loop — feed each auth response back and dispatch follow-ups
        while True:
            response = self._dispatch_request(
                request, stream=stream,
                follow_redirects=follow_redirects, timeout=timeout,
            )

            if auth_flow_gen is None:
                break

            try:
                request = auth_flow_gen.send(response)
            except StopIteration:
                auth_flow_gen = None
                break

        # 5. Execute response hooks
        for hook in self._event_hooks.get("response", []):
            try:
                hook(response)
            except Exception:
                response.close()
                raise

        return response

    def request(self, method, url, *, params=None, headers=None, cookies=None,
                content=None, data=None, files=None, json=None,
                follow_redirects=None, timeout=None, extensions=None):
        req = self.build_request(
            method, url,
            params=params, headers=headers, cookies=cookies,
            content=content, data=data, files=files, json=json,
            extensions=extensions,
        )
        return self.send(
            req,
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
    def stream(self, method, url, **kwargs):
        self._ensure_client()
        req = self.build_request(method, url, **kwargs)
        response = None
        try:
            response = self.send(req, stream=True)
            yield response
        finally:
            if response is not None:
                response.close()

    def close(self) -> None:
        if self._transport is not None and hasattr(self._transport, "close"):
            try:
                self._transport.close()
            except Exception:
                pass
        for transport in self._mounts.values():
            if hasattr(transport, "close"):
                try:
                    transport.close()
                except Exception:
                    pass
        if self._native_client is not None:
            try:
                self._native_client.close()
            except Exception:
                pass
        self._is_closed = True

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
        merged = QueryParams(self._params)
        for key in req.keys():
            merged.set(key, req[key])
        return merged

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
        self._auth = auth
        self._params = params if isinstance(params, QueryParams) else QueryParams(params)
        self._headers = headers if isinstance(headers, Headers) else Headers(headers)
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
        self._is_closed = False

        self._mounts: dict[str, Any] = {}
        if mounts:
            for pattern, transport_obj in mounts.items():
                self._mounts[pattern] = transport_obj

    def _ensure_client(self):
        if self._transport is not None or self._async_transport is not None:
            return
        if self._native_client is None or self._is_closed:
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
            self._is_closed = False

    @property
    def auth(self):
        return self._auth

    @property
    def base_url(self) -> URL:
        return self._base_url

    @property
    def cookies(self) -> Cookies:
        return self._cookies

    @property
    def event_hooks(self) -> dict:
        return self._event_hooks

    @property
    def headers(self) -> Headers:
        return self._headers

    @property
    def is_closed(self) -> bool:
        return self._is_closed

    @property
    def params(self) -> QueryParams:
        return self._params

    @property
    def timeout(self) -> Timeout:
        return self._timeout

    @property
    def trust_env(self) -> bool:
        return self._trust_env

    def build_request(self, method, url, **kwargs):
        merged_url = self._merge_url(url)
        merged_params = self._merge_params(kwargs.get("params"))
        merged_headers = self._merge_headers(kwargs.get("headers"))
        merged_cookies = self._merge_cookies(kwargs.get("cookies"))
        merged_extensions = self._merge_extensions(kwargs.get("extensions"))

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

    async def _dispatch_request(self, request, *, stream=False, follow_redirects=None,
                                timeout=None):
        transport = _match_mount(request.url, self._mounts)
        if transport is not None:
            return await self._send_via_transport(
                transport, request, stream=stream
            )
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
            follow_redirects=follow_redirects, timeout=timeout,
        )

    async def _send_via_transport(self, transport, request, *, stream=False):
        native_resp = await transport.handle_async_request(request)
        if stream:
            return _wrap_streaming_response(native_resp, request, self._default_encoding)
        return _wrap_response(native_resp, request, self._default_encoding)

    def _send_via_transport_sync(self, transport, request, *, stream=False):
        native_resp = transport.handle_request(request)
        if stream:
            return _wrap_streaming_response(native_resp, request, self._default_encoding)
        return _wrap_response(native_resp, request, self._default_encoding)

    async def _send_via_native(self, request, *, stream=False, follow_redirects=None,
                               timeout=None):
        self._ensure_client()
        kwargs = _build_native_kwargs(request, follow_redirects=follow_redirects,
                                       timeout=timeout)
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

    async def send(self, request, *, stream=False, auth=_USE_CLIENT_DEFAULT,
                   follow_redirects=None, timeout=None):
        if self._is_closed:
            raise RuntimeError("Client is closed")

        if not isinstance(request, Request):
            raise TypeError(f"send() requires a Request object, got {type(request).__name__}")

        # 1. Resolve auth
        if auth is _USE_CLIENT_DEFAULT:
            resolved_auth = self._auth
        else:
            resolved_auth = auth

        # 2. Execute request hooks BEFORE auth and dispatch
        for hook in self._event_hooks.get("request", []):
            if asyncio.iscoroutinefunction(hook):
                await hook(request)
            else:
                hook(request)

        # 3. Run auth flow (generator pattern)
        # Auth is NOT applied when a custom/mount transport handles the request.
        transport = _match_mount(request.url, self._mounts)
        has_custom_transport = (transport is not None or
                               self._async_transport is not None or
                               self._transport is not None)
        use_auth = resolved_auth is not None and not has_custom_transport

        auth_flow_gen = None
        if use_auth:
            auth_flow_gen = resolved_auth.auth_flow(request)
            try:
                request = next(auth_flow_gen)
            except StopIteration:
                auth_flow_gen = None

        # 4. Dispatch loop — feed each auth response back and dispatch follow-ups
        while True:
            response = await self._dispatch_request(
                request, stream=stream,
                follow_redirects=follow_redirects, timeout=timeout,
            )

            if auth_flow_gen is None:
                break

            try:
                request = auth_flow_gen.send(response)
            except StopIteration:
                auth_flow_gen = None
                break

        # 5. Execute response hooks
        for hook in self._event_hooks.get("response", []):
            try:
                if asyncio.iscoroutinefunction(hook):
                    await hook(response)
                else:
                    hook(response)
            except Exception:
                response.close()
                raise

        return response

    async def request(self, method, url, *, params=None, headers=None, cookies=None,
                      content=None, data=None, files=None, json=None,
                      follow_redirects=None, timeout=None, extensions=None):
        req = self.build_request(
            method, url,
            params=params, headers=headers, cookies=cookies,
            content=content, data=data, files=files, json=json,
            extensions=extensions,
        )
        return await self.send(
            req,
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
    async def stream(self, method, url, **kwargs):
        self._ensure_client()
        req = self.build_request(method, url, **kwargs)
        response = None
        try:
            response = await self.send(req, stream=True)
            yield response
        finally:
            if response is not None:
                if hasattr(response, "aclose"):
                    await response.aclose()
                else:
                    response.close()

    async def close(self) -> None:
        if self._async_transport is not None and hasattr(self._async_transport, "aclose"):
            try:
                await self._async_transport.aclose()
            except Exception:
                pass
        if self._transport is not None and hasattr(self._transport, "close"):
            try:
                self._transport.close()
            except Exception:
                pass
        for transport in self._mounts.values():
            if hasattr(transport, "aclose"):
                try:
                    await transport.aclose()
                except Exception:
                    pass
            elif hasattr(transport, "close"):
                try:
                    transport.close()
                except Exception:
                    pass
        if self._native_client is not None:
            try:
                await self._native_client.aclose()
            except Exception:
                pass
        self._is_closed = True

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
        merged = QueryParams(self._params)
        for key in req.keys():
            merged.set(key, req[key])
        return merged

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
