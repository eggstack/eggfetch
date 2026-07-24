"""HTTPX-compatible exception hierarchy for eggfetch."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .._models import Request


class HTTPError(Exception):
    """Base exception for HTTPX-compatible errors."""

    def __init__(self, message: str) -> None:
        self.message = message
        self._request: Request | None = None
        super().__init__(self.message)

    @property
    def request(self) -> Request:
        if self._request is None:
            raise RuntimeError(
                "The request instance has not been set on this exception."
            )
        return self._request

    @request.setter
    def request(self, request: Request) -> None:
        self._request = request

    def __repr__(self) -> str:
        return f"{type(self).__name__}({self.message!r})"


class HTTPStatusError(HTTPError):
    """Exception raised when an HTTP response indicates an error status."""

    def __init__(
        self,
        message: str,
        *,
        request: Request | None = None,
        response: object | None = None,
    ) -> None:
        super().__init__(message)
        self._request = request
        self.response = response

    def __repr__(self) -> str:
        return (
            f"{type(self).__name__}({self.message!r}, "
            f"request={self.request!r}, response={self.response!r})"
        )


class RequestError(HTTPError):
    """Base exception for request-related errors."""

    def __init__(
        self, message: str, *, request: Request | None = None
    ) -> None:
        super().__init__(message)
        self._request = request


class TransportError(RequestError):
    """Base exception for transport-related errors."""

    def __init__(
        self, message: str, *, request: Request | None = None
    ) -> None:
        super().__init__(message, request=request)


class TimeoutException(TransportError):
    """Exception raised when a request times out."""


class ConnectTimeout(TimeoutException):
    """Exception raised when a connection attempt times out."""


class ReadTimeout(TimeoutException):
    """Exception raised when reading from the network times out."""


class WriteTimeout(TimeoutException):
    """Exception raised when writing to the network times out."""


class PoolTimeout(TimeoutException):
    """Exception raised when waiting for a connection from the pool times out."""


class NetworkError(TransportError):
    """Base exception for network-related errors."""


class CloseError(NetworkError):
    """Exception raised when closing a connection fails."""


class ConnectError(NetworkError):
    """Exception raised when establishing a connection fails."""


class ReadError(NetworkError):
    """Exception raised when reading from the network fails."""


class WriteError(NetworkError):
    """Exception raised when writing to the network fails."""


class ProtocolError(TransportError):
    """Base exception for protocol-related errors."""


class LocalProtocolError(ProtocolError):
    """Exception raised when a local protocol error occurs."""


class RemoteProtocolError(ProtocolError):
    """Exception raised when a remote protocol error occurs."""


class ProxyError(TransportError):
    """Exception raised when a proxy error occurs."""


class UnsupportedProtocol(TransportError):
    """Exception raised when an unsupported protocol is used."""


class DecodingError(RequestError):
    """Exception raised when response decoding fails."""


class TooManyRedirects(RequestError):
    """Exception raised when too many redirects are followed."""


class StreamError(Exception):
    """Base exception for stream-related errors."""

    def __init__(self, *args: object) -> None:
        super().__init__(*args)


class RequestNotRead(StreamError):
    """Exception raised when attempting to read a request that hasn't been read."""


class ResponseNotRead(StreamError):
    """Exception raised when attempting to read a response that hasn't been read."""


class StreamClosed(StreamError):
    """Exception raised when attempting to read from a closed stream."""


class StreamConsumed(StreamError):
    """Exception raised when attempting to read from a consumed stream."""


class InvalidURL(Exception):
    """Exception raised when an invalid URL is provided."""

    def __init__(self, message: str = "", **kwargs: object) -> None:
        self.message = message if message is not None else ""
        super().__init__(self.message)

    def __repr__(self) -> str:
        return f"{type(self).__name__}({self.message!r})"


class CookieConflict(Exception):
    """Exception raised when a cookie conflict occurs."""

    def __init__(self, message: str = "", **kwargs: object) -> None:
        self.message = message if message is not None else ""
        super().__init__(self.message)

    def __repr__(self) -> str:
        return f"{type(self).__name__}({self.message!r})"
