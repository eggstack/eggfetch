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
    """Create an ``ssl.SSLContext`` matching HTTPX 0.28.1 behavior.

    Returns a genuine Python ``ssl.SSLContext`` that can be inspected
    and classified by eggfetch's compatibility translation layer.

    When a context created by this helper is passed back as the
    ``verify`` argument to ``Client`` or ``AsyncClient``, eggfetch
    reconstructs an equivalent ``TlsConfig`` via the weak registry
    metadata.

    Parameters match HTTPX 0.28.1:
    - ``verify=True``: default secure context (certifi CA bundle,
      or ``SSL_CERT_FILE``/``SSL_CERT_DIR`` when ``trust_env=True``).
    - ``verify=False``: disable certificate and hostname verification.
    - ``verify=<str>``: **deprecated**; load CA from path.
    - ``verify=<ssl.SSLContext>``: use the provided context directly.
    - ``cert=<str>`` or ``cert=(cert, key)``: **deprecated**; load
      client certificate chain.
    """
    import os
    import ssl as _ssl

    from eggfetch.compat.httpx._ssl_context import (
        _eggfetch_ssl_registry,
    )

    if verify is True:
        if trust_env and os.environ.get("SSL_CERT_FILE"):
            ctx = _ssl.create_default_context(
                cafile=os.environ["SSL_CERT_FILE"]
            )
        elif trust_env and os.environ.get("SSL_CERT_DIR"):
            ctx = _ssl.create_default_context(
                capath=os.environ["SSL_CERT_DIR"]
            )
        else:
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
            DeprecationWarning,
            stacklevel=2,
        )
        if os.path.isdir(verify):
            ctx = _ssl.create_default_context(capath=verify)
        else:
            ctx = _ssl.create_default_context(cafile=verify)
    elif isinstance(verify, _ssl.SSLContext):
        # Caller-supplied passthrough: we did not construct this
        # context, so we have no cert/key path provenance.  The
        # returned context is the caller-supplied object itself; the
        # registry must treat it as an external context for
        # translation purposes.  A construction fingerprint is still
        # captured for stale-entry detection, but stored metadata
        # carries no ``cert_path`` and no special ``verify`` kwarg.
        ctx = verify
    else:
        raise TypeError(
            f"verify must be bool, str, or ssl.SSLContext, "
            f"got {type(verify).__name__}"
        )

    cert_path = None
    key_path = None
    passthrough = isinstance(verify, _ssl.SSLContext)

    if cert and not passthrough:
        import warnings as _warnings

        _warnings.warn(
            "`cert=...` is deprecated. Use `verify=<ssl_context>` "
            "instead, with `.load_cert_chain()` to configure the "
            "certificate chain.",
            DeprecationWarning,
            stacklevel=2,
        )
        if isinstance(cert, str):
            ctx.load_cert_chain(cert)
            cert_path = cert
        else:
            ctx.load_cert_chain(*cert)
            cert_path = cert[0]
            key_path = cert[1]

    if passthrough:
        # Register the passthrough so the registry knows the context
        # is a caller-supplied object that we did not construct.  No
        # verify kwarg, no cert path — those are unknowable here.
        _eggfetch_ssl_registry.register(
            ctx,
            cert_path=None,
            key_path=None,
            verify=True,
            trust_env=trust_env,
            passthrough=True,
        )
    else:
        # Helper-constructed context: record reconstruction metadata
        # along with a public-state fingerprint so we can detect
        # post-construction mutation at translation time.
        _eggfetch_ssl_registry.register(
            ctx,
            cert_path=cert_path,
            key_path=key_path,
            verify=verify,
            trust_env=trust_env,
            passthrough=False,
        )

    return ctx


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
