"""HTTPX-compatible Timeout class for eggfetch."""

from __future__ import annotations

import copy


class Timeout:
    """HTTPX-compatible Timeout class.

    Timeout(timeout=5.0, *, connect=None, read=None, write=None, pool=None)
    """

    __slots__ = ("_connect", "_read", "_write", "_pool", "_total")

    def __init__(self, timeout=5.0, *, connect=None, read=None, write=None, pool=None):
        if connect is None:
            connect = timeout
        if read is None:
            read = timeout
        if write is None:
            write = timeout
        if pool is None:
            pool = timeout

        self._validate_value(connect, "connect")
        self._validate_value(read, "read")
        self._validate_value(write, "write")
        self._validate_value(pool, "pool")

        self._connect = connect
        self._read = read
        self._write = write
        self._pool = pool
        self._total = timeout

    @staticmethod
    def _validate_value(value, name: str) -> None:
        if value is not None:
            if not isinstance(value, (int, float)):
                raise TypeError(f"Timeout {name} must be None or a number, got {type(value).__name__}")
            if value < 0:
                raise ValueError(f"Timeout {name} must be a positive number, got {value}")
            if isinstance(value, float) and (value != value):  # NaN check
                raise ValueError(f"Timeout {name} must be a finite number, got NaN")

    @property
    def connect(self) -> float | None:
        return self._connect

    @property
    def read(self) -> float | None:
        return self._read

    @property
    def write(self) -> float | None:
        return self._write

    @property
    def pool(self) -> float | None:
        return self._pool

    @property
    def total(self) -> float | None:
        return self._total

    @property
    def as_dict(self) -> dict:
        return {
            "connect": self._connect,
            "read": self._read,
            "write": self._write,
            "pool": self._pool,
        }

    def __eq__(self, other):
        if isinstance(other, Timeout):
            return (
                self._connect == other._connect
                and self._read == other._read
                and self._write == other._write
                and self._pool == other._pool
                and self._total == other._total
            )
        return NotImplemented

    def __repr__(self) -> str:
        parts = []
        if self._connect is not None:
            parts.append(f"connect={self._connect!r}")
        if self._read is not None:
            parts.append(f"read={self._read!r}")
        if self._write is not None:
            parts.append(f"write={self._write!r}")
        if self._pool is not None:
            parts.append(f"pool={self._pool!r}")
        return f"Timeout({', '.join(parts)})"

    def __copy__(self):
        new = Timeout.__new__(Timeout)
        new._connect = self._connect
        new._read = self._read
        new._write = self._write
        new._pool = self._pool
        new._total = self._total
        return new

    def __deepcopy__(self, memo):
        new = Timeout.__new__(Timeout)
        memo[id(self)] = new
        new._connect = copy.deepcopy(self._connect, memo)
        new._read = copy.deepcopy(self._read, memo)
        new._write = copy.deepcopy(self._write, memo)
        new._pool = copy.deepcopy(self._pool, memo)
        new._total = copy.deepcopy(self._total, memo)
        return new
