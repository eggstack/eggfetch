"""HTTPX 0.28.1-compatible facade for eggfetch.

This package provides a drop-in compatibility layer so that existing
HTTPX code can run against the eggfetch Rust engine with minimal changes.
"""

from __future__ import annotations

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

# ── Phase 3+ / 4+: stubs for transports, auth, streams, client ────────
# These exist so the import line ``from eggfetch.compat.httpx import …``
# works.  Real implementations arrive in later phases.

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


# Auth base class (Phase 3+)
class Auth:
    """Base class for authentication."""

    def auth_flow(self, request):
        raise NotImplementedError("eggfetch auth flows not yet implemented")


class _NetRCAuth(Auth):
    """Stub for NetRCAuth."""

    def __init__(self, *args, **kwargs):
        raise NotImplementedError("eggfetch does not support NetRCAuth")


NetRCAuth = _NetRCAuth


class _BasicAuth(Auth):
    """HTTPX-compatible BasicAuth (Phase 2 stub)."""

    def __init__(self, username: str = "", password: str = "", *, encoding="latin-1"):
        self._username = username
        self._password = password
        self._encoding = encoding

    def auth_flow(self, request):
        raise NotImplementedError("eggfetch BasicAuth flow not yet implemented")

    def __repr__(self):
        return f"BasicAuth(username={self._username!r})"


BasicAuth = _BasicAuth


class _DigestAuth(Auth):
    """HTTPX-compatible DigestAuth (Phase 2 stub)."""

    def __init__(self, username: str, password: str):
        self._username = username
        self._password = password

    def auth_flow(self, request):
        raise NotImplementedError("eggfetch DigestAuth flow not yet implemented")

    def __repr__(self):
        return f"DigestAuth(username={self._username!r})"


DigestAuth = _DigestAuth

# Transport stubs (Phase 4+)
class _BaseTransport:
    def handle_request(self, request):
        raise NotImplementedError("eggfetch does not support custom transports")

    def close(self):
        pass


class _AsyncBaseTransport:
    async def handle_async_request(self, request):
        raise NotImplementedError("eggfetch does not support custom transports")

    async def aclose(self):
        pass


BaseTransport = _BaseTransport
AsyncBaseTransport = _AsyncBaseTransport

HTTPTransport = _stub_factory("HTTPTransport")
AsyncHTTPTransport = _stub_factory("AsyncHTTPTransport")
MockTransport = _stub_factory("MockTransport")
ASGITransport = _stub_factory("ASGITransport")
WSGITransport = _stub_factory("WSGITransport")

# Stream base classes (Phase 3)
from eggfetch.compat.httpx._stream import ByteStream, SyncByteStream, AsyncByteStream


# Phase 2: Request / Response
from eggfetch.compat.httpx._request import Request
from eggfetch.compat.httpx._response import Response

# Client / AsyncClient
from eggfetch.compat.httpx._client import Client, AsyncClient


# Top-level convenience functions
def request(*args, **kwargs):
    with Client() as client:
        return client.request(*args, **kwargs)


def get(*args, **kwargs):
    with Client() as client:
        return client.get(*args, **kwargs)


def post(*args, **kwargs):
    with Client() as client:
        return client.post(*args, **kwargs)


def put(*args, **kwargs):
    with Client() as client:
        return client.put(*args, **kwargs)


def patch(*args, **kwargs):
    with Client() as client:
        return client.patch(*args, **kwargs)


def delete(*args, **kwargs):
    with Client() as client:
        return client.delete(*args, **kwargs)


def head(*args, **kwargs):
    with Client() as client:
        return client.head(*args, **kwargs)


def options(*args, **kwargs):
    with Client() as client:
        return client.options(*args, **kwargs)


def stream(*args, **kwargs):
    with Client() as client:
        with client.stream(*args, **kwargs) as response:
            return response


USE_CLIENT_DEFAULT = _USE_CLIENT_DEFAULT

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
    "ConnectError",
    "ConnectTimeout",
    "CookieConflict",
    "Cookies",
    "DecodingError",
    "delete",
    "DigestAuth",
    "get",
    "head",
    "Headers",
    "HTTPError",
    "HTTPStatusError",
    "HTTPTransport",
    "InvalidURL",
    "Limits",
    "LocalProtocolError",
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
]
