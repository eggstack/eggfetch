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

    __slots__ = ()

    def __new__(cls, url="", *, base_url=None):
        """Return an httpx2 ``URL`` even when handed a 0.28.1 ``URL``.

        The 0.28.1 constructor returns its argument unchanged when that
        argument is already a ``URL``.  Inherited unchanged, that would hand
        a 0.28.1 URL back to an httpx2 caller, and a 0.28.1 URL has no
        ``origin``.  A foreign URL is adopted with its parsed parts copied
        verbatim, exactly as :func:`_as_httpx2_url` does, so the result is an
        httpx2 URL without re-parsing.
        """
        if isinstance(url, _BaseURL) and not isinstance(url, cls):
            instance = object.__new__(cls)
            instance._parts = url._parts
            instance._raw = url._raw
            return instance
        return super().__new__(cls, url, base_url=base_url)

    @property
    def origin(self) -> Origin:
        """Return the URL origin as an immutable, hashable ``Origin``."""
        return Origin(self)

    def copy_with(self, url=None, params=None):
        """Return a copy of this URL as an httpx2 ``URL``.

        The 0.28.1 class rebuilds copies with its own ``URL`` class, which
        would drop ``origin`` from the copy.
        """
        return _as_httpx2_url(super().copy_with(url=url, params=params))

    def join(self, other: "URL") -> "URL":
        """Resolve *other* against *self*, returning an httpx2 ``URL``."""
        return _as_httpx2_url(super().join(other))


def _as_httpx2_url(value) -> URL:
    """Return *value* as an httpx2 :class:`URL`.

    Mirrors the 0.28.1 URL coercion: a 0.28.1 URL is adopted with its
    parsed parts unchanged, anything else is parsed by the httpx2 ``URL``
    class itself.  The httpx2 facade never hands out a URL without
    ``origin``.
    """
    if isinstance(value, URL):
        return value
    if isinstance(value, _BaseURL):
        adopted = object.__new__(URL)
        adopted._parts = value._parts
        adopted._raw = value._raw
        return adopted
    return URL(value)


def _upgrade_httpx2_url(value):
    """Upgrade a 0.28.1 URL to an httpx2 URL, leaving other values alone.

    Used where the 0.28.1 class copies a foreign ``request.url`` verbatim
    instead of coercing it, so a non-URL value has to survive unchanged.
    """
    if isinstance(value, _BaseURL):
        return _as_httpx2_url(value)
    return value


__all__ = ["Origin", "QueryParams", "URL"]
