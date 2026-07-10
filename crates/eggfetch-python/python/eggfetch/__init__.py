"""eggfetch - Python bindings for the eggfetch HTTP client engine.

Provides both sync and async APIs over the async Rust core via PyO3.
The sync API blocks on the async engine while releasing the GIL.
The async API integrates with asyncio.
"""

from eggfetch._native import (
    __version__,
    AsyncClient,
    Client,
    Cookie,
    Cookies,
    Headers,
    Response,
    Timeout,
    # Top-level helpers
    request,
    get,
    post,
    put,
    patch,
    delete,
    head,
    options,
    # Exceptions
    EggfetchError,
    RequestError,
    InvalidUrl,
    TimeoutException,
    PoolTimeout,
    ConnectTimeout,
    ReadTimeout,
    WriteTimeout,
    NetworkError,
    ProtocolError,
    BodyError,
    HTTPStatusError,
    UnsupportedKwarg,
    TooManyRedirects,
)

__all__ = [
    "__version__",
    "AsyncClient",
    "Client",
    "Cookie",
    "Cookies",
    "Headers",
    "Response",
    "Timeout",
    "request",
    "get",
    "post",
    "put",
    "patch",
    "delete",
    "head",
    "options",
    "EggfetchError",
    "RequestError",
    "InvalidUrl",
    "TimeoutException",
    "PoolTimeout",
    "ConnectTimeout",
    "ReadTimeout",
    "WriteTimeout",
    "NetworkError",
    "ProtocolError",
    "BodyError",
    "HTTPStatusError",
    "UnsupportedKwarg",
    "TooManyRedirects",
]
