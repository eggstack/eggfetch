"""HTTPX transport integration backed by EggFetch's Rust engine.

Install ``eggfetch[httpx-transport]`` to use this module. Pass the transport
to a real :class:`httpx.Client`; the SDK-facing client remains an HTTPX
instance while request I/O is delegated to EggFetch.

The integration currently supports synchronous HTTPX clients. Configure
network options on ``EggFetchTransport`` because HTTPX does not forward
``Client`` TLS, limits, or proxy settings to a custom transport.
"""

from __future__ import annotations

from collections.abc import Iterable, Iterator
from typing import Any

import httpx

from eggfetch.compat.httpx import (
    ConnectError as EggConnectError,
    ConnectTimeout as EggConnectTimeout,
    HTTPTransport as _EggHTTPTransport,
    Limits as _EggLimits,
    NetworkError as EggNetworkError,
    PoolTimeout as EggPoolTimeout,
    ProtocolError as EggProtocolError,
    ProxyError as EggProxyError,
    ReadTimeout as EggReadTimeout,
    Request as _EggRequest,
    RequestError as EggRequestError,
    Response as _EggResponse,
    TimeoutException as EggTimeoutException,
    TransportError as EggTransportError,
    UnsupportedProtocol as EggUnsupportedProtocol,
    WriteTimeout as EggWriteTimeout,
)


def _translate_error(error: Exception, request: httpx.Request) -> Exception:
    """Convert an EggFetch compatibility exception to HTTPX's exception type."""
    message = str(error)
    pairs: tuple[tuple[type[BaseException], type[BaseException]], ...] = (
        (EggConnectTimeout, httpx.ConnectTimeout),
        (EggReadTimeout, httpx.ReadTimeout),
        (EggWriteTimeout, httpx.WriteTimeout),
        (EggPoolTimeout, httpx.PoolTimeout),
        (EggTimeoutException, httpx.TimeoutException),
        (EggProxyError, httpx.ProxyError),
        (EggUnsupportedProtocol, httpx.UnsupportedProtocol),
        (EggProtocolError, httpx.ProtocolError),
        (EggConnectError, httpx.ConnectError),
        (EggNetworkError, httpx.NetworkError),
        (EggTransportError, httpx.TransportError),
        (EggRequestError, httpx.RequestError),
    )
    for egg_type, httpx_type in pairs:
        if isinstance(error, egg_type):
            return httpx_type(message, request=request)
    return httpx.TransportError(message or "EggFetch transport failure", request=request)


class _EggFetchResponseStream(httpx.SyncByteStream):
    """Expose EggFetch's raw response stream as HTTPX's public stream type."""

    def __init__(self, response: _EggResponse, request: httpx.Request) -> None:
        self._response = response
        self._request = request
        self._closed = False

    def __iter__(self) -> Iterator[bytes]:
        try:
            yield from self._response.iter_raw()
        except Exception as error:
            translated = _translate_error(error, self._request)
            if translated is error:
                raise
            raise translated from error
        finally:
            self.close()

    def close(self) -> None:
        if not self._closed:
            self._closed = True
            self._response.close()


class EggFetchTransport(httpx.BaseTransport):
    """An HTTPX sync transport that delegates HTTP requests to EggFetch.

    Args mirror the network-relevant subset of ``httpx.HTTPTransport``.
    The HTTPX client remains responsible for SDK-facing URL construction,
    authentication, hooks, redirects, and retry policy. EggFetch transport
    retries default to zero so retry ownership remains with the SDK/application.
    """

    def __init__(
        self,
        *,
        verify: bool | str | Any = True,
        cert: str | tuple[str, str] | None = None,
        trust_env: bool = True,
        http1: bool = True,
        http2: bool = False,
        limits: httpx.Limits | None = None,
        proxy: str | httpx.Proxy | None = None,
        uds: str | None = None,
        local_address: str | None = None,
        socket_options: Iterable[tuple[int, ...]] | None = None,
    ) -> None:
        limits = limits or httpx.Limits(
            max_connections=100,
            max_keepalive_connections=20,
            keepalive_expiry=5.0,
        )
        egg_limits = _EggLimits(
            max_connections=limits.max_connections,
            max_keepalive_connections=limits.max_keepalive_connections,
            keepalive_expiry=limits.keepalive_expiry,
        )
        if isinstance(proxy, httpx.Proxy):
            proxy = str(proxy.url)
        self._transport = _EggHTTPTransport(
            verify=verify,
            cert=cert,
            trust_env=trust_env,
            http1=http1,
            http2=http2,
            limits=egg_limits,
            proxy=proxy,
            uds=uds,
            local_address=local_address,
            retries=0,
            socket_options=socket_options,
        )
        self._closed = False

    def handle_request(self, request: httpx.Request) -> httpx.Response:
        if self._closed:
            raise RuntimeError("EggFetchTransport is closed")

        timeout = request.extensions.get("timeout")
        extensions = {"timeout": timeout} if isinstance(timeout, dict) else {}
        try:
            content: bytes | None = request.content
        except httpx.RequestNotRead:
            content = None
        stream = None
        if content is None:
            if "content-length" in request.headers:
                # EggFetch deliberately rejects an unknown-length iterator
                # paired with Content-Length. Buffer this uncommon
                # HTTPX-stream shape so the declared framing stays truthful.
                content = request.read()
            else:
                stream = iter(request.stream)
        egg_request = _EggRequest(
            request.method,
            str(request.url),
            headers=request.headers.multi_items(),
            content=content,
            stream=stream,
            extensions=extensions,
        )

        try:
            egg_response = self._transport.handle_request(egg_request)
        except Exception as error:
            translated = _translate_error(error, request)
            if translated is error:
                raise
            raise translated from error

        stream = _EggFetchResponseStream(egg_response, request)
        response_extensions: dict[str, Any] = {}
        for key in ("http_version", "reason_phrase"):
            value = egg_response.extensions.get(key)
            if value is not None:
                response_extensions[key] = (
                    value.encode("ascii", errors="replace")
                    if isinstance(value, str)
                    else value
                )
        return httpx.Response(
            egg_response.status_code,
            headers=egg_response.headers.multi_items(),
            stream=stream,
            request=request,
            extensions=response_extensions,
        )

    def close(self) -> None:
        if not self._closed:
            self._closed = True
            self._transport.close()


__all__ = ["EggFetchTransport"]
