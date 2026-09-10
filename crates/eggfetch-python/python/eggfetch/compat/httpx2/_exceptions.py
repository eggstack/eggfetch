"""HTTPX2 2.12.0-compatible exception hierarchy for eggfetch.

Re-exports the HTTPX 0.28.1-compatible hierarchy (identical taxonomy) and
adds ``HTTPXDeprecationWarning`` per httpx2==2.12.0.
"""

from __future__ import annotations

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


class HTTPXDeprecationWarning(UserWarning):
    """Custom deprecation warning for HTTPX2.

    Unlike the built-in ``DeprecationWarning``, this inherits from
    ``UserWarning`` to ensure it is visible by default, matching
    httpx2==2.12.0.
    """


__all__ = [
    "HTTPXDeprecationWarning",
    "HTTPError",
    "HTTPStatusError",
    "RequestError",
    "TransportError",
    "TimeoutException",
    "ConnectTimeout",
    "ReadTimeout",
    "WriteTimeout",
    "PoolTimeout",
    "NetworkError",
    "CloseError",
    "ConnectError",
    "ReadError",
    "WriteError",
    "ProtocolError",
    "LocalProtocolError",
    "RemoteProtocolError",
    "ProxyError",
    "UnsupportedProtocol",
    "DecodingError",
    "TooManyRedirects",
    "StreamError",
    "RequestNotRead",
    "ResponseNotRead",
    "StreamClosed",
    "StreamConsumed",
    "InvalidURL",
    "CookieConflict",
]
