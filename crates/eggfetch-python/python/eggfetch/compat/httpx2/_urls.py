"""HTTPX2 2.12.0-compatible URL/Origin classes for eggfetch.

``QueryParams`` is identical and re-exported. ``URL`` subclasses the
0.28.1-compatible URL with an added ``origin`` property. ``Origin`` mirrors
httpx2==2.12.0 (frozen dataclass, normalized scheme/host/effective port).
"""

from __future__ import annotations

import ipaddress
import urllib.parse
from dataclasses import dataclass

from eggfetch.compat.httpx._urls import QueryParams
from eggfetch.compat.httpx._urls import URL as _BaseURL

try:
    import idna as _idna
except ImportError:  # pragma: no cover
    _idna = None  # type: ignore[assignment]

_ORIGIN_DEFAULT_PORTS = {"http": 80, "https": 443, "ws": 80, "wss": 443}


@dataclass(frozen=True, slots=True, init=False)
class Origin:
    """Normalized, immutable, hashable URL origin (RFC 9110 §4.3.1)."""

    scheme: str
    host: str
    port: int | None

    def __init__(self, url) -> None:
        from eggfetch.compat.httpx2._urls import URL as _H2URL

        if isinstance(url, Origin):
            object.__setattr__(self, "scheme", url.scheme)
            object.__setattr__(self, "host", url.host)
            object.__setattr__(self, "port", url.port)
            return
        if not isinstance(url, _BaseURL):
            url = _H2URL(str(url))
        if url.is_relative_url:
            raise ValueError("URL must be absolute to have an origin")
        host = url.host or ""
        host = host.lower()
        if host.startswith("[") and host.endswith("]"):
            host = host[1:-1]
        try:
            parsed_ip = ipaddress.ip_address(host)
            host = str(parsed_ip)
        except ValueError:
            if _idna is not None and "xn--" in host:
                try:
                    host = _idna.decode(host)
                except Exception:
                    pass
        port = url.port
        if port is None:
            port = _ORIGIN_DEFAULT_PORTS.get(url.scheme)
        object.__setattr__(self, "scheme", url.scheme)
        object.__setattr__(self, "host", host)
        object.__setattr__(self, "port", port)


class URL(_BaseURL):
    """HTTPX2-compatible URL with ``origin`` property."""

    @property
    def origin(self) -> Origin:
        """Return the URL origin as an immutable, hashable ``Origin``."""
        return Origin(self)


__all__ = ["Origin", "QueryParams", "URL"]
