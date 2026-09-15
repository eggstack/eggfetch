"""Typing boundary for the intentionally public HTTPX 0.28.1 facade.

The facade remains a compatibility implementation, not a second native API
contract.  Its public value/client types are declared here; implementation
modules remain private and are not promised as typed import paths.
"""

from collections.abc import AsyncIterator, Iterator
from typing import Any, AsyncContextManager, Self

__description__: str
__title__: str
__version__: str


class Response:
    status_code: int
    content: bytes
    text: str
    def raise_for_status(self) -> Self: ...
    def iter_bytes(self, chunk_size: int | None = ...) -> Iterator[bytes]: ...
    def iter_text(self, chunk_size: int | None = ...) -> Iterator[str]: ...
    def iter_lines(self) -> Iterator[str]: ...
    async def aiter_bytes(self, chunk_size: int | None = ...) -> AsyncIterator[bytes]: ...
    async def aiter_text(self, chunk_size: int | None = ...) -> AsyncIterator[str]: ...
    async def aiter_lines(self) -> AsyncIterator[str]: ...


class Client:
    def __init__(self, **kwargs: Any) -> None: ...
    def __enter__(self) -> Self: ...
    def __exit__(self, exc_type: Any, exc_value: Any, traceback: Any) -> None: ...
    def get(self, url: str, **kwargs: Any) -> Response: ...
    def request(self, method: str, url: str, **kwargs: Any) -> Response: ...
    def stream(self, method: str, url: str, **kwargs: Any) -> Any: ...
    def close(self) -> None: ...


class AsyncClient:
    def __init__(self, **kwargs: Any) -> None: ...
    async def __aenter__(self) -> Self: ...
    async def __aexit__(self, exc_type: Any, exc_value: Any, traceback: Any) -> None: ...
    async def get(self, url: str, **kwargs: Any) -> Response: ...
    async def request(self, method: str, url: str, **kwargs: Any) -> Response: ...
    def stream(self, method: str, url: str, **kwargs: Any) -> AsyncContextManager[Response]: ...
    async def aclose(self) -> None: ...


def get(url: str, **kwargs: Any) -> Response: ...
def request(method: str, url: str, **kwargs: Any) -> Response: ...


# Remaining names are public compatibility imports whose detailed typing is
# inherited from the pinned reference and is outside this native pass.
ASGITransport: Any
AsyncBaseTransport: Any
AsyncByteStream: Any
AsyncHTTPTransport: Any
Auth: Any
BasicAuth: Any
ByteStream: Any
CloseError: Any
codes: Any
COMPATIBILITY_INFO: Any
CompatibilityInfo: Any
ConnectError: Any
ConnectTimeout: Any
CookieConflict: Any
Cookies: Any
create_ssl_context: Any
DecodingError: Any
delete: Any
diagnostics_summary: Any
DigestAuth: Any
get_compatibility_info: Any
head: Any
Headers: Any
HTTPError: Any
HTTPStatusError: Any
HTTPTransport: Any
InvalidURL: Any
Limits: Any
LocalProtocolError: Any
main: Any
MockTransport: Any
NetRCAuth: Any
NetworkError: Any
options: Any
patch: Any
PoolTimeout: Any
post: Any
ProtocolError: Any
Proxy: Any
ProxyError: Any
put: Any
QueryParams: Any
ReadError: Any
ReadTimeout: Any
RemoteProtocolError: Any
Request: Any
RequestError: Any
RequestNotRead: Any
ResponseNotRead: Any
stream: Any
StreamClosed: Any
StreamConsumed: Any
StreamError: Any
SyncByteStream: Any
Timeout: Any
TimeoutException: Any
TooManyRedirects: Any
TransportError: Any
UnsupportedProtocol: Any
URL: Any
USE_CLIENT_DEFAULT: Any
WriteError: Any
WriteTimeout: Any
WSGITransport: Any
_build_response: Any
