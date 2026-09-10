"""HTTPX2 2.12.0-compatible facade for eggfetch.

Sibling of ``eggfetch.compat.httpx`` (HTTPX 0.28.1). Shares the single Rust
engine; profile-specific semantics (FunctionAuth, Origin, QUERY, header
operators, SSE, WebSocket, truststore default, status aliases) live here.
Importing this package never mutates ``eggfetch.compat.httpx`` or
``sys.modules['httpx']`` (see ``alias_httpx()`` for explicit opt-in).
"""

from __future__ import annotations

__description__ = "An HTTPX2-compatible facade for eggfetch."
__title__ = "eggfetch[httpx2-compat]"
__version__ = "2.12.0"

from eggfetch.compat.httpx2._alias import alias_httpx
from eggfetch.compat.httpx2._auth import Auth, BasicAuth, DigestAuth, FunctionAuth, NetRCAuth
from eggfetch.compat.httpx2._client import AsyncClient, Client, USE_CLIENT_DEFAULT
from eggfetch.compat.httpx2._config import Limits, Proxy, Timeout
from eggfetch.compat.httpx2._exceptions import (
    CloseError,
    ConnectError,
    ConnectTimeout,
    CookieConflict,
    DecodingError,
    HTTPError,
    HTTPStatusError,
    HTTPXDeprecationWarning,
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
from eggfetch.compat.httpx2._headers import Headers
from eggfetch.compat.httpx2._status_codes import codes
from eggfetch.compat.httpx2._urls import Origin, QueryParams, URL

# Shared transports/streams/models (identical semantics).
from eggfetch.compat.httpx._asgi import ASGITransport
from eggfetch.compat.httpx._cookies import Cookies
from eggfetch.compat.httpx._mock import MockTransport
from eggfetch.compat.httpx._request import Request
from eggfetch.compat.httpx._response import Response
from eggfetch.compat.httpx._stream import AsyncByteStream, ByteStream, SyncByteStream
from eggfetch.compat.httpx._transports import (
    AsyncBaseTransport,
    AsyncHTTPTransport,
    BaseTransport,
    HTTPTransport,
)
from eggfetch.compat.httpx._wsgi import WSGITransport

# SSE surface (Python framing over streamed responses).
from eggfetch.compat.httpx2._sse import EventSource, ServerSentEvent, SSEError

# Top-level helpers (including QUERY + websocket).
from eggfetch.compat.httpx2._api import (
    delete,
    get,
    head,
    options,
    patch,
    post,
    put,
    query,
    request,
    stream,
    websocket,
)


def create_ssl_context(verify=True, cert=None, trust_env=True):
    """Create an ``ssl.SSLContext`` matching HTTPX2 2.12.0 behavior.

    Default (``verify=True``) uses the OS trust store via ``truststore``
    (matching httpx2), falling back to certifi only when truststore is
    unavailable. All other arguments mirror the 0.28.1 helper; contexts
    are classified by the same safe rustls boundary (unrepresentable
    state fails closed with ``TypeError`` before dispatch).
    """
    import os
    import ssl as _ssl

    from eggfetch.compat.httpx._ssl_context import _eggfetch_ssl_registry

    if verify is True:
        if trust_env and os.environ.get("SSL_CERT_FILE"):
            ctx = _ssl.create_default_context(cafile=os.environ["SSL_CERT_FILE"])
        elif trust_env and os.environ.get("SSL_CERT_DIR"):
            ctx = _ssl.create_default_context(capath=os.environ["SSL_CERT_DIR"])
        else:
            try:
                import truststore as _truststore

                ctx = _truststore.SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
            except ImportError:
                import certifi

                ctx = _ssl.create_default_context(cafile=certifi.where())
    elif verify is False:
        ctx = _ssl.SSLContext(_ssl.PROTOCOL_TLS_CLIENT)
        ctx.check_hostname = False
        ctx.verify_mode = _ssl.CERT_NONE
    elif isinstance(verify, str):
        import warnings as _warnings

        _warnings.warn(
            "`verify=<str>` is deprecated. "
            "Use `verify=ssl.create_default_context(cafile=...)` "
            "or `verify=ssl.create_default_context(capath=...)` instead.",
            HTTPXDeprecationWarning,
            stacklevel=2,
        )
        if os.path.isdir(verify):
            ctx = _ssl.create_default_context(capath=verify)
        else:
            ctx = _ssl.create_default_context(cafile=verify)
    elif isinstance(verify, _ssl.SSLContext):
        ctx = verify
    else:
        raise TypeError(
            f"verify must be bool, str, or ssl.SSLContext, got {type(verify).__name__}"
        )

    cert_path = None
    key_path = None
    passthrough = isinstance(verify, _ssl.SSLContext)

    if cert and not passthrough:
        import warnings as _warnings

        _warnings.warn(
            "`cert=...` is deprecated. Use `verify=<ssl_context>` instead,"
            "with `.load_cert_chain()` to configure the certificate chain.",
            HTTPXDeprecationWarning,
            stacklevel=2,
        )
        if isinstance(cert, str):
            if isinstance(verify, str) and cert:
                raise TypeError(
                    "`verify=<str>` cannot be combined with `cert=...`. "
                    "Build an `ssl.SSLContext` and pass it as `verify=<ctx>`, "
                    "using `.load_cert_chain()` to configure the certificate chain."
                )
            ctx.load_cert_chain(cert)
            cert_path = cert
        else:
            ctx.load_cert_chain(*cert)
            cert_path = cert[0]
            key_path = cert[1]

    _eggfetch_ssl_registry.register(
        ctx,
        cert_path=cert_path,
        key_path=key_path,
        verify=verify,
        trust_env=trust_env,
        passthrough=passthrough,
    )
    return ctx


__all__ = [
    "__description__",
    "__title__",
    "__version__",
    "alias_httpx",
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
    "create_ssl_context",
    "DecodingError",
    "delete",
    "DigestAuth",
    "EventSource",
    "FunctionAuth",
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
    "Origin",
    "patch",
    "PoolTimeout",
    "post",
    "ProtocolError",
    "Proxy",
    "ProxyError",
    "put",
    "query",
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
    "ServerSentEvent",
    "SSEError",
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
    "websocket",
    "WriteError",
    "WriteTimeout",
    "WSGITransport",
]
