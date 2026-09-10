"""HTTPX2 Timeout/Limits/Proxy for eggfetch.

Timeout phases are identical (connect/read/write/pool, omitted-vs-None
preserved, reject bare Timeout()); the error message names httpx2 per
reference. Limits/Proxy semantics are identical; NO_PROXY profile split
lives in core (see H2X-PROXY-001) and does not change these signatures.
"""

from __future__ import annotations

import eggfetch.compat.httpx._timeout as _httpx_timeout
from eggfetch.compat.httpx._limits import Limits
from eggfetch.compat.httpx._proxy import Proxy

_UNSET = _httpx_timeout._UNSET


class Timeout(_httpx_timeout.Timeout):
    """httpx2-compatible Timeout (message names httpx2)."""

    def __init__(
        self,
        timeout=_UNSET,
        *,
        connect=_UNSET,
        read=_UNSET,
        write=_UNSET,
        pool=_UNSET,
    ):
        try:
            super().__init__(
                timeout, connect=connect, read=read, write=write, pool=pool
            )
        except ValueError as e:
            raise ValueError(
                str(e).replace("httpx.Timeout", "httpx2.Timeout")
            ) from e


__all__ = ["Limits", "Proxy", "Timeout"]

DEFAULT_TIMEOUT_CONFIG = Timeout(timeout=5.0)
DEFAULT_LIMITS = Limits(max_connections=100, max_keepalive_connections=20)
DEFAULT_MAX_REDIRECTS = 20
DEFAULT_MAX_EVENT_SIZE_BYTES = 1024 * 1024
DEFAULT_MAX_MESSAGE_SIZE_BYTES = 65_536
DEFAULT_QUEUE_SIZE = 512
DEFAULT_KEEPALIVE_PING_INTERVAL_SECONDS = 20.0
DEFAULT_KEEPALIVE_PING_TIMEOUT_SECONDS = 20.0
