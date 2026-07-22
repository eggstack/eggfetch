"""HTTPX-compatible exception hierarchy for eggfetch."""


class HTTPError(Exception):
    """Base exception for HTTPX-compatible errors."""

    def __init__(self, *args, request=None, message=""):
        self._request = request
        self._message = message or (args[0] if args else "")
        super().__init__(self._message if self._message else str(self))

    @property
    def request(self):
        return self._request

    def __repr__(self):
        return f"{type(self).__name__}({self._message!r})"


class HTTPStatusError(HTTPError):
    """Exception raised when an HTTP response indicates an error status."""

    def __init__(self, message, *, request, response):
        super().__init__(message=message, request=request)
        self._response = response

    @property
    def response(self):
        return self._response

    def __repr__(self):
        return (
            f"{type(self).__name__}({self._message!r}, "
            f"request={self._request!r}, response={self._response!r})"
        )


class RequestError(HTTPError):
    """Base exception for request-related errors."""


class TransportError(RequestError):
    """Base exception for transport-related errors."""


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

    def __init__(self, message):
        super().__init__(message)


class CookieConflict(Exception):
    """Exception raised when a cookie conflict occurs."""

    def __init__(self, message):
        super().__init__(message)
