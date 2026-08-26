"""HTTPX-compatible Headers class for eggfetch."""

from __future__ import annotations

from collections.abc import MutableMapping

# Header names whose values are redacted by ``__repr__`` so that logging a
# response never dumps credentials (mirrors the native PyHeaders rule).
_REDACTED_HEADER_NAMES = frozenset(
    {"authorization", "proxy-authorization", "cookie", "set-cookie"}
)


class Headers(MutableMapping):
    """HTTPX-compatible Headers class.

    Internal storage: list of (name, value) tuples.
    Names are stored in lowercase for case-insensitive lookup.
    """

    __slots__ = ("_items", "_encoding")

    def __init__(self, headers=None, encoding: str | None = None):
        self._items: list[tuple[str, str]] = []
        self._encoding = encoding if encoding is not None else "utf-8"
        if headers is not None:
            self._init_from(headers)

    def _init_from(self, headers) -> None:
        if isinstance(headers, Headers):
            self._items = list(headers._items)
            self._encoding = headers._encoding
            return
        if isinstance(headers, dict):
            items = headers.items()
        elif isinstance(headers, (list, tuple)):
            items = headers
        else:
            raise TypeError(
                f"Headers() argument must be dict, list, or Headers, "
                f"not {type(headers).__name__}"
            )
        for name, value in items:
            if isinstance(name, bytes):
                name = name.decode("ascii")
            if isinstance(value, bytes):
                value = value.decode("ascii")
            self._validate(name, value)
            self._items.append((name.lower(), value))

    @staticmethod
    def _validate(name: str, value: str) -> None:
        if "\r" in name or "\n" in name:
            raise ValueError(f"Header name {name!r} contains invalid character CR/LF")
        if "\r" in value or "\n" in value:
            raise ValueError(f"Header value for {name!r} contains invalid character CR/LF")

    def _normalize_name(self, name) -> str:
        if isinstance(name, bytes):
            return name.decode("ascii").lower()
        return name.lower()

    def get(self, name: str, default=None):
        norm = self._normalize_name(name)
        values = [v for k, v in self._items if k == norm]
        if not values:
            return default
        return ", ".join(values)

    def get_list(self, name: str) -> list[str]:
        norm = self._normalize_name(name)
        return [v for k, v in self._items if k == norm]

    def multi_items(self) -> list[tuple[str, str]]:
        return list(self._items)

    def keys(self) -> list[str]:
        return list(dict.fromkeys(k for k, _ in self._items))

    def values(self) -> list[str]:
        seen = set()
        result = []
        for k, v in self._items:
            if k not in seen:
                seen.add(k)
                result.append(v)
        return result

    def items(self) -> list[tuple[str, str]]:
        seen = set()
        result = []
        for k, v in self._items:
            if k not in seen:
                seen.add(k)
                result.append((k, v))
        return result

    def __iter__(self):
        return iter(self.keys())

    def __len__(self) -> int:
        return len(self._items)

    def __contains__(self, key: object) -> bool:
        norm = self._normalize_name(key)
        return any(k == norm for k, _ in self._items)

    def __setitem__(self, name: str, value: str) -> None:
        self._validate(name, value)
        norm = name.lower()
        self._items = [(k, v) for k, v in self._items if k != norm]
        self._items.append((norm, value))

    def __delitem__(self, name: str) -> None:
        norm = self._normalize_name(name)
        before = len(self._items)
        self._items = [(k, v) for k, v in self._items if k != norm]
        if len(self._items) == before:
            raise KeyError(name)

    def __getitem__(self, name: str) -> str:
        norm = self._normalize_name(name)
        values = [v for k, v in self._items if k == norm]
        if not values:
            raise KeyError(name)
        return ", ".join(values)

    def update(self, headers) -> None:
        if isinstance(headers, Headers):
            items = headers._items
        elif isinstance(headers, dict):
            items = [(k.lower(), v) for k, v in headers.items()]
        elif isinstance(headers, (list, tuple)):
            items = [
                (self._normalize_name(k), v) if isinstance(k, str) else (k.decode("ascii").lower(), v)
                for k, v in headers
            ]
        else:
            raise TypeError(
                f"update() argument must be Headers, dict, or list of tuples, "
                f"not {type(headers).__name__}"
            )
        incoming_keys = {k for k, _ in items}
        self._items = [(k, v) for k, v in self._items if k not in incoming_keys]
        self._items.extend(items)

    def pop(self, name: str, default=None):
        norm = self._normalize_name(name)
        found = False
        new_items = []
        result = default
        for k, v in self._items:
            if k == norm and not found:
                found = True
                result = v
            else:
                new_items.append((k, v))
        self._items = new_items
        return result

    def setdefault(self, name: str, default: str = "") -> str:
        norm = self._normalize_name(name)
        for k, v in self._items:
            if k == norm:
                return v
        self._items.append((norm, default))
        return default

    def popitem(self) -> tuple[str, str]:
        if not self._items:
            raise KeyError("popitem(): Headers is empty")
        name, value = self._items.pop(0)
        return name, value

    def append(self, name: str, value: str) -> None:
        self._validate(name, value)
        self._items.append((name.lower(), value))

    def clear(self) -> None:
        self._items.clear()

    def copy(self) -> "Headers":
        new = Headers(encoding=self._encoding)
        new._items = list(self._items)
        return new

    @property
    def raw(self) -> list[tuple[bytes, bytes]]:
        return [(k.encode("ascii"), v.encode("ascii")) for k, v in self._items]

    @property
    def encoding(self) -> str:
        return self._encoding

    def __eq__(self, other):
        if isinstance(other, Headers):
            return self._items == other._items
        return NotImplemented

    def __repr__(self) -> str:
        items_str = ", ".join(
            f"{k!r}: '<redacted>'" if k in _REDACTED_HEADER_NAMES else f"{k!r}: {v!r}"
            for k, v in self._items
        )
        return f"Headers({items_str})"
