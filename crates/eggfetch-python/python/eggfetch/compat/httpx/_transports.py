"""HTTPX-compatible transport protocols for eggfetch."""

from __future__ import annotations

import typing

import eggfetch

from eggfetch.compat.httpx._request import Request
from eggfetch.compat.httpx._response import Response
from eggfetch.compat.httpx._urls import URL
from eggfetch.compat.httpx._headers import Headers
from eggfetch.compat.httpx._timeout import Timeout
from eggfetch.compat.httpx._limits import Limits
from eggfetch.compat.httpx._proxy import Proxy
from eggfetch.compat.httpx._exceptions import (
    CloseError,
    ConnectError,
    ConnectTimeout,
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
from eggfetch.compat.httpx._client import (
    _convert_headers,
    _convert_cookies,
    _convert_params,
    _convert_timeout,
    _convert_limits,
    _convert_proxy,
    _map_exception,
    _wrap_response,
    _wrap_streaming_response,
    _validate_transport_options,
    _validate_protocol_options,
)

if typing.TYPE_CHECKING:
    pass


class BaseTransport:
    def handle_request(self, request: Request) -> Response:
        raise NotImplementedError(
            "eggfetch does not support custom transports"
        )

    def close(self) -> None:
        pass

    def __enter__(self) -> BaseTransport:
        return self

    def __exit__(self, *exc) -> None:
        self.close()


class AsyncBaseTransport:
    async def handle_async_request(self, request: Request) -> Response:
        raise NotImplementedError(
            "eggfetch does not support custom transports"
        )

    async def aclose(self) -> None:
        pass

    async def __aenter__(self) -> AsyncBaseTransport:
        return self

    async def __aexit__(self, *exc) -> None:
        await self.aclose()


class HTTPTransport(BaseTransport):
    """Transport backed by eggfetch-core for synchronous requests.

    The transport always returns a stream-backed Response so that the
    higher-level client layer can decide whether to buffer or iterate.

    Note: ``local_address``, ``socket_options``, and ``uds`` are
    accepted for API compatibility but are **not forwarded** to
    eggfetch-core, which does not support them.
    """

    def __init__(
        self,
        verify: bool = True,
        cert: str | tuple[str, str] | None = None,
        trust_env: bool = True,
        http1: bool = True,
        http2: bool = False,
        limits: Limits = Limits(max_connections=100, max_keepalive_connections=20, keepalive_expiry=5.0),
        proxy: str | Proxy | None = None,
        uds: str | None = None,
        local_address: str | None = None,
        retries: int = 0,
        socket_options: typing.Any | None = None,
    ) -> None:
        _validate_protocol_options(http1, http2)
        _validate_transport_options(
            uds=uds, local_address=local_address, socket_options=socket_options,
        )
        self._verify = verify
        self._cert = cert
        self._trust_env = trust_env
        self._http1 = http1
        self._http2 = http2
        self._proxy = proxy
        if isinstance(limits, Limits):
            self._limits = limits
        elif limits is not None:
            self._limits = Limits(limits)
        else:
            self._limits = Limits(max_connections=100, max_keepalive_connections=20, keepalive_expiry=5.0)
        self._timeout = None
        self._local_address = local_address
        self._retries = retries
        self._socket_options = socket_options
        self._uds = uds
        self._native_client: eggfetch.Client | None = None
        self._is_closed: bool = False

    def _ensure_client(self) -> eggfetch.Client:
        if self._is_closed:
            raise RuntimeError("Transport is closed")
        if self._native_client is None:
            kwargs: dict[str, typing.Any] = {}
            if self._verify is not True:
                kwargs["verify"] = self._verify
            if self._cert is not None:
                kwargs["cert"] = self._cert
            if self._trust_env is not True:
                kwargs["trust_env"] = self._trust_env
            if self._http2:
                kwargs["http2"] = self._http2
            kwargs["limits"] = _convert_limits(self._limits)
            if self._timeout is not None:
                kwargs["timeout"] = _convert_timeout(self._timeout)
            if self._proxy is not None:
                kwargs["proxy"] = _convert_proxy(self._proxy)
            if self._retries:
                kwargs["retries"] = self._retries
            self._native_client = eggfetch.Client(**kwargs)
        return self._native_client

    def handle_request(self, request: Request) -> Response:
        client = self._ensure_client()
        kwargs: dict[str, typing.Any] = {
            "method": request.method,
            "url": str(request.url),
        }
        if request.headers:
            kwargs["headers"] = _convert_headers(request.headers)
        if isinstance(request.extensions.get("timeout"), dict):
            t = request.extensions["timeout"]
            kwargs["timeout"] = _convert_timeout(Timeout(timeout=None, connect=t.get("connect"), read=t.get("read"), write=t.get("write"), pool=t.get("pool")))
        if request._stream is not None and request._content is None:
            kwargs["content"] = request._stream
        elif request._content is not None:
            kwargs["content"] = request._content
        if request._files is not None:
            kwargs["files"] = request._files

        try:
            native_resp = client.stream(**kwargs)
        except Exception as exc:
            raise _map_exception(exc, request) from exc

        return _wrap_streaming_response(native_resp, request)

    def close(self) -> None:
        if self._native_client is not None and not self._is_closed:
            try:
                self._native_client.close()
            except Exception:
                pass
        self._is_closed = True


class AsyncHTTPTransport(AsyncBaseTransport):
    """Transport backed by eggfetch-core for asynchronous requests.

    The transport always returns a stream-backed Response so that the
    higher-level client layer can decide whether to buffer or iterate.

    Note: ``local_address``, ``socket_options``, and ``uds`` are
    accepted for API compatibility but are **not forwarded** to
    eggfetch-core, which does not support them.
    """

    def __init__(
        self,
        verify: bool = True,
        cert: str | tuple[str, str] | None = None,
        trust_env: bool = True,
        http1: bool = True,
        http2: bool = False,
        limits: Limits = Limits(max_connections=100, max_keepalive_connections=20, keepalive_expiry=5.0),
        proxy: str | Proxy | None = None,
        uds: str | None = None,
        local_address: str | None = None,
        retries: int = 0,
        socket_options: typing.Any | None = None,
    ) -> None:
        _validate_protocol_options(http1, http2)
        _validate_transport_options(
            uds=uds, local_address=local_address, socket_options=socket_options,
        )
        self._verify = verify
        self._cert = cert
        self._trust_env = trust_env
        self._http1 = http1
        self._http2 = http2
        self._proxy = proxy
        if isinstance(limits, Limits):
            self._limits = limits
        elif limits is not None:
            self._limits = Limits(limits)
        else:
            self._limits = Limits(max_connections=100, max_keepalive_connections=20, keepalive_expiry=5.0)
        self._timeout = None
        self._local_address = local_address
        self._retries = retries
        self._socket_options = socket_options
        self._uds = uds
        self._native_client: eggfetch.AsyncClient | None = None
        self._is_closed: bool = False

    def _ensure_client(self) -> eggfetch.AsyncClient:
        if self._is_closed:
            raise RuntimeError("Transport is closed")
        if self._native_client is None:
            kwargs: dict[str, typing.Any] = {}
            if self._verify is not True:
                kwargs["verify"] = self._verify
            if self._cert is not None:
                kwargs["cert"] = self._cert
            if self._trust_env is not True:
                kwargs["trust_env"] = self._trust_env
            if self._http2:
                kwargs["http2"] = self._http2
            kwargs["limits"] = _convert_limits(self._limits)
            if self._timeout is not None:
                kwargs["timeout"] = _convert_timeout(self._timeout)
            if self._proxy is not None:
                kwargs["proxy"] = _convert_proxy(self._proxy)
            if self._retries:
                kwargs["retries"] = self._retries
            self._native_client = eggfetch.AsyncClient(**kwargs)
        return self._native_client

    async def handle_async_request(self, request: Request) -> Response:
        client = self._ensure_client()
        kwargs: dict[str, typing.Any] = {
            "method": request.method,
            "url": str(request.url),
        }
        if request.headers:
            kwargs["headers"] = _convert_headers(request.headers)
        if isinstance(request.extensions.get("timeout"), dict):
            t = request.extensions["timeout"]
            kwargs["timeout"] = _convert_timeout(Timeout(timeout=None, connect=t.get("connect"), read=t.get("read"), write=t.get("write"), pool=t.get("pool")))
        if request._stream is not None and request._content is None:
            kwargs["content"] = request._stream
        elif request._content is not None:
            kwargs["content"] = request._content
        if request._files is not None:
            kwargs["files"] = request._files

        try:
            native_resp = await client.stream(**kwargs)
        except Exception as exc:
            raise _map_exception(exc, request) from exc

        return _wrap_streaming_response(native_resp, request)

    async def aclose(self) -> None:
        if self._native_client is not None and not self._is_closed:
            try:
                await self._native_client.aclose()
            except Exception:
                pass
        self._is_closed = True
