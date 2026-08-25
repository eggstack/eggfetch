"""HTTPX-compatible Timeout class for eggfetch."""

from __future__ import annotations

import copy
import math


class _UnsetType:
    """Private sentinel preserving whether a timeout argument was omitted."""

    def __repr__(self) -> str:
        return "UnsetType"


_UNSET = _UnsetType()


class Timeout:
    """HTTPX-compatible Timeout class.

    Timeout(timeout=UNSET, *, connect=UNSET, read=UNSET, write=UNSET, pool=UNSET)
    """

    __slots__ = ("_connect", "_read", "_write", "_pool", "_total")

    def __init__(
        self,
        timeout=_UNSET,
        *,
        connect=_UNSET,
        read=_UNSET,
        write=_UNSET,
        pool=_UNSET,
    ):
        if isinstance(timeout, Timeout):
            if any(value is not _UNSET for value in (connect, read, write, pool)):
                raise TypeError(
                    "Cannot combine a Timeout instance with explicit phase values"
                )
            connect, read, write, pool = (
                timeout.connect,
                timeout.read,
                timeout.write,
                timeout.pool,
            )
            total = timeout.total
        elif isinstance(timeout, tuple):
            if any(value is not _UNSET for value in (connect, read, write, pool)):
                raise TypeError(
                    "Cannot combine a timeout tuple with explicit phase values"
                )
            connect, read = timeout[0], timeout[1]
            write = timeout[2] if len(timeout) >= 3 else None
            pool = timeout[3] if len(timeout) >= 4 else None
            total = None
        elif all(value is not _UNSET for value in (connect, read, write, pool)):
            total = None if timeout is _UNSET else timeout
        else:
            if timeout is _UNSET:
                raise ValueError(
                    "httpx.Timeout must either include a default, or set all "
                    "four parameters explicitly."
                )
            connect = timeout if connect is _UNSET else connect
            read = timeout if read is _UNSET else read
            write = timeout if write is _UNSET else write
            pool = timeout if pool is _UNSET else pool
            total = timeout

        self._validate_value(connect, "connect")
        self._validate_value(read, "read")
        self._validate_value(write, "write")
        self._validate_value(pool, "pool")

        self._connect = connect
        self._read = read
        self._write = write
        self._pool = pool
        self._total = total

    @staticmethod
    def _validate_value(value, name: str) -> None:
        if value is not None:
            if not isinstance(value, (int, float)):
                raise TypeError(f"Timeout {name} must be None or a number, got {type(value).__name__}")
            if not math.isfinite(value):
                # Reject NaN and ±inf here so callers get a consistent
                # error from the layer they constructed the Timeout in;
                # the native engine rejects non-finite values too.
                raise ValueError(f"Timeout {name} must be a finite number, got {value}")
            if value < 0:
                raise ValueError(f"Timeout {name} must be a positive number, got {value}")

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
        """The scalar timeout recorded at construction, if any.

        Informational only: like HTTPX, this layer enforces the four
        phases (connect/read/write/pool) and does not synthesize a
        native outer deadline from ``total``. Native callers may set an
        explicit engine-level total separately.
        """
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
            )
        return NotImplemented

    def __repr__(self) -> str:
        if len({self._connect, self._read, self._write, self._pool}) == 1:
            return f"Timeout(timeout={self._connect!r})"
        return (
            f"Timeout(connect={self._connect!r}, read={self._read!r}, "
            f"write={self._write!r}, pool={self._pool!r})"
        )

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
