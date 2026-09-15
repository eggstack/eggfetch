"""Typing boundary for the intentionally public HTTPX2 2.12.0 facade."""

from typing import Any, Self

from eggfetch.compat.httpx import (
    Client as _HttpxClient,
    Response,
)

__description__: str
__title__: str
__version__: str


class Client(_HttpxClient): ...


class AsyncClient:
    def __init__(self, **kwargs: Any) -> None: ...
    async def __aenter__(self) -> Self: ...
    async def __aexit__(self, exc_type: Any, exc_value: Any, traceback: Any) -> None: ...
    async def get(self, url: str, **kwargs: Any) -> Response: ...
    async def request(self, method: str, url: str, **kwargs: Any) -> Response: ...
    async def aclose(self) -> None: ...


def get(url: str, **kwargs: Any) -> Response: ...
def request(method: str, url: str, **kwargs: Any) -> Response: ...


# Profile-specific and shared compatibility exports.  The implementation
# modules remain private; Any here records presence without promising a
# native-level contract for the large HTTPX2 surface.
alias_httpx: Any
ASGITransport: Any
AsyncBaseTransport: Any
AsyncHTTPTransport: Any
Auth: Any
BasicAuth: Any
BaseTransport: Any
ByteStream: Any
CloseError: Any
codes: Any
ConnectError: Any
ConnectTimeout: Any
CookieConflict: Any
Cookies: Any
create_ssl_context: Any
DecodingError: Any
delete: Any
DigestAuth: Any
EventSource: Any
FunctionAuth: Any
head: Any
Headers: Any
HTTPError: Any
HTTPStatusError: Any
HTTPTransport: Any
InvalidURL: Any
Limits: Any
LocalProtocolError: Any
MockTransport: Any
NetRCAuth: Any
NetworkError: Any
options: Any
Origin: Any
patch: Any
PoolTimeout: Any
post: Any
ProtocolError: Any
Proxy: Any
ProxyError: Any
put: Any
query: Any
QueryParams: Any
ReadError: Any
ReadTimeout: Any
RemoteProtocolError: Any
Request: Any
RequestError: Any
RequestNotRead: Any
ResponseNotRead: Any
ServerSentEvent: Any
SSEError: Any
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
websocket: Any
WriteError: Any
WriteTimeout: Any
WSGITransport: Any
