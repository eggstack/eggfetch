"""eggfetch - Python bindings for the eggfetch HTTP client engine.

This is a sync-only API built on top of the async Rust core via PyO3.
Network I/O releases the Python GIL so other threads can make progress.
"""

from eggfetch._native import (
    __version__,
    Client,
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
)

__all__ = [
    "__version__",
    "Client",
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
]
