"""HTTPX 0.28.1-compatible facade for eggfetch.

This package provides a drop-in compatibility layer so that existing
HTTPX code can run against the eggfetch Rust engine with minimal changes.
"""

from __future__ import annotations

import typing
from contextlib import contextmanager

__description__ = "A HTTPX-compatible facade for eggfetch."
__title__ = "eggfetch[httpx-compat]"
__version__ = "0.28.1"

# ── Phase 2: pure-Python value objects ──────────────────────────────────
from eggfetch.compat.httpx._urls import URL, QueryParams
from eggfetch.compat.httpx._headers import Headers
from eggfetch.compat.httpx._cookies import Cookies
from eggfetch.compat.httpx._timeout import Timeout
from eggfetch.compat.httpx._limits import Limits
from eggfetch.compat.httpx._proxy import Proxy
from eggfetch.compat.httpx._status_codes import codes
from eggfetch.compat.httpx._exceptions import (
    CloseError,
    ConnectError,
    ConnectTimeout,
    CookieConflict,
    DecodingError,
    HTTPError,
    HTTPStatusError,
    InvalidURL,
    LocalProtocolError,
    NetworkError,
    PoolTimeout,
    ProtocolError,
    ProxyError,
    ReadError,
    ReadTimeout,
    RemoteProtocolError,
    RequestError,
    RequestNotRead,
    ResponseNotRead,
    StreamClosed,
    StreamConsumed,
    StreamError,
    TimeoutException,
    TooManyRedirects,
    TransportError,
    UnsupportedProtocol,
    WriteError,
    WriteTimeout,
)

# ── Phase 3+ / 4+: transports, streams, client ───────────────────────
# These exist so the import line ``from eggfetch.compat.httpx import …``
# works.

_USE_CLIENT_DEFAULT = object()


def _stub_factory(name: str, msg: str | None = None):
    """Return a class that raises NotImplementedError on instantiation."""
    _msg = msg or f"eggfetch does not support {name}"

    class _Stub:
        def __init_subclass__(cls, **kwargs):
            super().__init_subclass__(**kwargs)

        def __init__(self, *args, **kwargs):
            raise NotImplementedError(_msg)

    _Stub.__name__ = name
    _Stub.__qualname__ = name
    return _Stub


# Diagnostics
from eggfetch.compat.httpx._diagnostics import (
    CompatibilityInfo,
    COMPATIBILITY_INFO,
    get_compatibility_info,
    diagnostics_summary,
)

# Auth (Phase 3)
from eggfetch.compat.httpx._auth import Auth, BasicAuth, DigestAuth, NetRCAuth

# Transport implementations (Phase 4)
from eggfetch.compat.httpx._transports import (
    BaseTransport,
    AsyncBaseTransport,
    HTTPTransport,
    AsyncHTTPTransport,
)
from eggfetch.compat.httpx._mock import MockTransport, _build_response
from eggfetch.compat.httpx._wsgi import WSGITransport
from eggfetch.compat.httpx._asgi import ASGITransport

# Stream base classes (Phase 3)
from eggfetch.compat.httpx._stream import ByteStream, SyncByteStream, AsyncByteStream


# Phase 2: Request / Response
from eggfetch.compat.httpx._request import Request
from eggfetch.compat.httpx._response import Response

# Client / AsyncClient
from eggfetch.compat.httpx._client import Client, AsyncClient


# Top-level convenience functions — explicit signatures matching HTTPX 0.28.1


def request(
    method,
    url,
    *,
    params=None,
    content=None,
    data=None,
    files=None,
    json=None,
    headers=None,
    cookies=None,
    auth=None,
    proxy=None,
    timeout=Timeout(5.0),
    follow_redirects=False,
    verify=True,
    trust_env=True,
    extensions=None,
) -> Response:
    with Client(
        cookies=cookies,
        proxy=proxy,
        verify=verify,
        timeout=timeout,
        trust_env=trust_env,
    ) as client:
        return client.request(
            method,
            url,
            params=params,
            content=content,
            data=data,
            files=files,
            json=json,
            headers=headers,
            auth=auth,
            follow_redirects=follow_redirects,
            extensions=extensions,
        )


def get(
    url,
    *,
    params=None,
    headers=None,
    cookies=None,
    auth=None,
    proxy=None,
    follow_redirects=False,
    verify=True,
    timeout=Timeout(5.0),
    trust_env=True,
    extensions=None,
) -> Response:
    with Client(
        cookies=cookies,
        proxy=proxy,
        verify=verify,
        timeout=timeout,
        trust_env=trust_env,
    ) as client:
        return client.get(
            url,
            params=params,
            headers=headers,
            auth=auth,
            follow_redirects=follow_redirects,
            extensions=extensions,
        )


def post(
    url,
    *,
    content=None,
    data=None,
    files=None,
    json=None,
    params=None,
    headers=None,
    cookies=None,
    auth=None,
    proxy=None,
    follow_redirects=False,
    verify=True,
    timeout=Timeout(5.0),
    trust_env=True,
    extensions=None,
) -> Response:
    with Client(
        cookies=cookies,
        proxy=proxy,
        verify=verify,
        timeout=timeout,
        trust_env=trust_env,
    ) as client:
        return client.post(
            url,
            content=content,
            data=data,
            files=files,
            json=json,
            params=params,
            headers=headers,
            auth=auth,
            follow_redirects=follow_redirects,
            extensions=extensions,
        )


def put(
    url,
    *,
    content=None,
    data=None,
    files=None,
    json=None,
    params=None,
    headers=None,
    cookies=None,
    auth=None,
    proxy=None,
    follow_redirects=False,
    verify=True,
    timeout=Timeout(5.0),
    trust_env=True,
    extensions=None,
) -> Response:
    with Client(
        cookies=cookies,
        proxy=proxy,
        verify=verify,
        timeout=timeout,
        trust_env=trust_env,
    ) as client:
        return client.put(
            url,
            content=content,
            data=data,
            files=files,
            json=json,
            params=params,
            headers=headers,
            auth=auth,
            follow_redirects=follow_redirects,
            extensions=extensions,
        )


def patch(
    url,
    *,
    content=None,
    data=None,
    files=None,
    json=None,
    params=None,
    headers=None,
    cookies=None,
    auth=None,
    proxy=None,
    follow_redirects=False,
    verify=True,
    timeout=Timeout(5.0),
    trust_env=True,
    extensions=None,
) -> Response:
    with Client(
        cookies=cookies,
        proxy=proxy,
        verify=verify,
        timeout=timeout,
        trust_env=trust_env,
    ) as client:
        return client.patch(
            url,
            content=content,
            data=data,
            files=files,
            json=json,
            params=params,
            headers=headers,
            auth=auth,
            follow_redirects=follow_redirects,
            extensions=extensions,
        )


def delete(
    url,
    *,
    params=None,
    headers=None,
    cookies=None,
    auth=None,
    proxy=None,
    follow_redirects=False,
    timeout=Timeout(5.0),
    verify=True,
    trust_env=True,
    extensions=None,
) -> Response:
    with Client(
        cookies=cookies,
        proxy=proxy,
        verify=verify,
        timeout=timeout,
        trust_env=trust_env,
    ) as client:
        return client.delete(
            url,
            params=params,
            headers=headers,
            auth=auth,
            follow_redirects=follow_redirects,
            extensions=extensions,
        )


def head(
    url,
    *,
    params=None,
    headers=None,
    cookies=None,
    auth=None,
    proxy=None,
    follow_redirects=False,
    verify=True,
    timeout=Timeout(5.0),
    trust_env=True,
    extensions=None,
) -> Response:
    with Client(
        cookies=cookies,
        proxy=proxy,
        verify=verify,
        timeout=timeout,
        trust_env=trust_env,
    ) as client:
        return client.head(
            url,
            params=params,
            headers=headers,
            auth=auth,
            follow_redirects=follow_redirects,
            extensions=extensions,
        )


def options(
    url,
    *,
    params=None,
    headers=None,
    cookies=None,
    auth=None,
    proxy=None,
    follow_redirects=False,
    verify=True,
    timeout=Timeout(5.0),
    trust_env=True,
    extensions=None,
) -> Response:
    with Client(
        cookies=cookies,
        proxy=proxy,
        verify=verify,
        timeout=timeout,
        trust_env=trust_env,
    ) as client:
        return client.options(
            url,
            params=params,
            headers=headers,
            auth=auth,
            follow_redirects=follow_redirects,
            extensions=extensions,
        )


@contextmanager
def stream(
    method,
    url,
    *,
    params=None,
    content=None,
    data=None,
    files=None,
    json=None,
    headers=None,
    cookies=None,
    auth=None,
    proxy=None,
    timeout=Timeout(5.0),
    follow_redirects=False,
    verify=True,
    trust_env=True,
    extensions=None,
) -> typing.Iterator[Response]:
    with Client(
        cookies=cookies,
        proxy=proxy,
        verify=verify,
        timeout=timeout,
        trust_env=trust_env,
    ) as client:
        with client.stream(
            method,
            url,
            params=params,
            content=content,
            data=data,
            files=files,
            json=json,
            headers=headers,
            auth=auth,
            follow_redirects=follow_redirects,
            extensions=extensions,
        ) as response:
            yield response


USE_CLIENT_DEFAULT = _USE_CLIENT_DEFAULT


def main():
    """HTTPX CLI entry point stub.

    eggfetch does not implement the HTTPX command-line interface.
    """
    raise NotImplementedError(
        "eggfetch does not implement the httpx CLI entry point."
    )


def create_ssl_context(
    verify=True,
    cert=None,
    trust_env=True,
):
    """Create an SSL context stub.

    eggfetch manages TLS internally via the Rust engine. This function
    exists for API compatibility but does not create a usable context
    for external consumers.
    """
    raise NotImplementedError(
        "eggfetch manages TLS internally; create_ssl_context is not supported."
    )


__all__ = [
    "__description__",
    "__title__",
    "__version__",
    "ASGITransport",
    "AsyncBaseTransport",
    "AsyncByteStream",
    "AsyncClient",
    "AsyncHTTPTransport",
    "Auth",
    "BaseTransport",
    "BasicAuth",
    "ByteStream",
    "Client",
    "CloseError",
    "codes",
    "COMPATIBILITY_INFO",
    "CompatibilityInfo",
    "ConnectError",
    "ConnectTimeout",
    "CookieConflict",
    "Cookies",
    "create_ssl_context",
    "DecodingError",
    "delete",
    "diagnostics_summary",
    "DigestAuth",
    "get",
    "get_compatibility_info",
    "head",
    "Headers",
    "HTTPError",
    "HTTPStatusError",
    "HTTPTransport",
    "InvalidURL",
    "Limits",
    "LocalProtocolError",
    "main",
    "MockTransport",
    "NetRCAuth",
    "NetworkError",
    "options",
    "patch",
    "PoolTimeout",
    "post",
    "ProtocolError",
    "Proxy",
    "ProxyError",
    "put",
    "QueryParams",
    "ReadError",
    "ReadTimeout",
    "RemoteProtocolError",
    "request",
    "Request",
    "RequestError",
    "RequestNotRead",
    "Response",
    "ResponseNotRead",
    "stream",
    "StreamClosed",
    "StreamConsumed",
    "StreamError",
    "SyncByteStream",
    "Timeout",
    "TimeoutException",
    "TooManyRedirects",
    "TransportError",
    "UnsupportedProtocol",
    "URL",
    "USE_CLIENT_DEFAULT",
    "WriteError",
    "WriteTimeout",
    "WSGITransport",
    "_build_response",
]
