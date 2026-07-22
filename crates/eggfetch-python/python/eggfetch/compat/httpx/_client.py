"""HTTPX-compatible Client and AsyncClient for eggfetch."""

from __future__ import annotations

import asyncio
import typing
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
            seconds=timeout.total,
            connect=timeout.connect,
            read=timeout.read,
            write=timeout.write,
            pool=timeout.pool,
        )
    if isinstance(timeout, (int, float)):
        return eggfetch.Timeout(seconds=timeout)
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

    return Response(
        status_code,
        headers=header_list,
        content=content,
        request=compat_request,
        history=history,
        default_encoding=default_encoding,
    )


def _wrap_streaming_response(native_resp, compat_request=None, default_encoding="utf-8"):
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

    response = Response(
        status_code,
        headers=header_list,
        stream=native_resp,
        request=compat_request,
        history=history,
        default_encoding=default_encoding,
    )
    response._native_stream = native_resp
    return response


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
        self._mounts = mounts
        self._timeout = timeout if isinstance(timeout, Timeout) else Timeout(timeout)
        self._follow_redirects = follow_redirects
        self._limits = limits if isinstance(limits, Limits) else Limits(limits)
        self._max_redirects = max_redirects
        self._event_hooks = event_hooks or {"request": [], "response": []}
        self._base_url = URL(base_url) if not isinstance(base_url, URL) else base_url
        self._transport = transport
        self._default_encoding = default_encoding
        self._native_client = None
        self._is_closed = False

    def _ensure_client(self):
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

    def send(self, request, *, stream=False, auth=_USE_CLIENT_DEFAULT,
             follow_redirects=None, timeout=None):
        self._ensure_client()

        kwargs = {}
        if isinstance(request, Request):
            kwargs["method"] = request.method
            kwargs["url"] = str(request.url)
            if request.headers:
                kwargs["headers"] = _convert_headers(request.headers)
            if request.params:
                kwargs["params"] = _convert_params(request.params)
            # Pass stream directly to native client for lazy iteration.
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

        for hook in self._event_hooks.get("request", []):
            hook(request)

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

        for hook in self._event_hooks.get("response", []):
            hook(response)

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
        try:
            yield self.send(req, stream=True)
        finally:
            pass

    def close(self) -> None:
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
            return None
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
        self._mounts = mounts
        self._timeout = timeout if isinstance(timeout, Timeout) else Timeout(timeout)
        self._follow_redirects = follow_redirects
        self._limits = limits if isinstance(limits, Limits) else Limits(limits)
        self._max_redirects = max_redirects
        self._event_hooks = event_hooks or {"request": [], "response": []}
        self._base_url = URL(base_url) if not isinstance(base_url, URL) else base_url
        self._transport = transport
        self._default_encoding = default_encoding
        self._native_client = None
        self._is_closed = False

    def _ensure_client(self):
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

    async def send(self, request, *, stream=False, auth=_USE_CLIENT_DEFAULT,
                   follow_redirects=None, timeout=None):
        self._ensure_client()

        kwargs = {}
        if isinstance(request, Request):
            kwargs["method"] = request.method
            kwargs["url"] = str(request.url)
            if request.headers:
                kwargs["headers"] = _convert_headers(request.headers)
            if request.params:
                kwargs["params"] = _convert_params(request.params)
            # Pass stream directly to native client for lazy iteration.
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

        for hook in self._event_hooks.get("request", []):
            if asyncio.iscoroutinefunction(hook):
                await hook(request)
            else:
                hook(request)

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

        for hook in self._event_hooks.get("response", []):
            if asyncio.iscoroutinefunction(hook):
                await hook(response)
            else:
                hook(response)

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
        try:
            yield await self.send(req, stream=True)
        finally:
            pass

    async def close(self) -> None:
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
            return None
        return request_extensions
