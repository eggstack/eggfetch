"""HTTPX-compatible Limits class for eggfetch."""

from __future__ import annotations


class Limits:
    """HTTPX-compatible Limits class.

    Limits(max_connections=None, max_keepalive_connections=None, keepalive_expiry=5.0)
    """

    __slots__ = ("_max_connections", "_max_keepalive_connections", "_keepalive_expiry")

    def __init__(
        self,
        max_connections=None,
        max_keepalive_connections=None,
        keepalive_expiry=5.0,
    ):
        self._max_connections = max_connections
        self._max_keepalive_connections = max_keepalive_connections
        self._keepalive_expiry = keepalive_expiry

    @property
    def max_connections(self) -> int | None:
        return self._max_connections

    @property
    def max_keepalive_connections(self) -> int | None:
        return self._max_keepalive_connections

    @property
    def keepalive_expiry(self) -> float | None:
        return self._keepalive_expiry

    def __eq__(self, other):
        if isinstance(other, Limits):
            return (
                self._max_connections == other._max_connections
                and self._max_keepalive_connections == other._max_keepalive_connections
                and self._keepalive_expiry == other._keepalive_expiry
            )
        return NotImplemented

    def __repr__(self) -> str:
        parts = []
        if self._max_connections is not None:
            parts.append(f"max_connections={self._max_connections!r}")
        if self._max_keepalive_connections is not None:
            parts.append(f"max_keepalive_connections={self._max_keepalive_connections!r}")
        if self._keepalive_expiry is not None:
            parts.append(f"keepalive_expiry={self._keepalive_expiry!r}")
        return f"Limits({', '.join(parts)})"
