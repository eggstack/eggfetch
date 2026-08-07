"""HTTPX-compatible URL and QueryParams classes for eggfetch."""

from __future__ import annotations

import urllib.parse
from collections.abc import Mapping


def _is_http_url(url: str) -> bool:
    return url.startswith("http://") or url.startswith("https://")


def _urlsplit(url: str) -> urllib.parse.SplitResult:
    return urllib.parse.urlsplit(url)


class QueryParams(Mapping):
    """HTTPX-compatible QueryParams class.

    Internal storage: list of (key, value) tuples to preserve order and duplicates.
    """

    __slots__ = ("_items",)

    def __init__(self, *args, **kwargs):
        self._items: list[tuple[str, str]] = []
        if args:
            if len(args) == 1:
                value = args[0]
                if value is None:
                    pass  # Empty QueryParams
                elif isinstance(value, str):
                    self._parse_string(value)
                elif isinstance(value, dict):
                    self._items = [(k, v) for k, v in value.items()]
                elif isinstance(value, (list, tuple)):
                    self._items = [(str(k), str(v)) for k, v in value]
                elif isinstance(value, QueryParams):
                    self._items = list(value._items)
                else:
                    raise TypeError(
                        f"QueryParams() argument must be str, dict, list, tuple, or QueryParams, "
                        f"not {type(value).__name__}"
                    )
            else:
                raise TypeError(
                    f"QueryParams() accepts at most 1 positional argument, got {len(args)}"
                )
        if kwargs:
            for k, v in kwargs.items():
                self._items.append((k, str(v)))

    def _parse_string(self, s: str) -> None:
        if not s:
            return
        parsed = urllib.parse.parse_qsl(s, keep_blank_values=True)
        self._items = parsed

    def get(self, key: str, default=None):
        for k, v in reversed(self._items):
            if k == key:
                return v
        return default

    def get_list(self, key: str) -> list[str]:
        return [v for k, v in self._items if k == key]

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

    def add(self, key: str, value) -> None:
        self._items.append((key, str(value)))

    def set(self, key: str, value) -> None:
        new_items = [(k, v) for k, v in self._items if k != key]
        new_items.append((key, str(value)))
        self._items = new_items

    def remove(self, key: str) -> None:
        self._items = [(k, v) for k, v in self._items if k != key]

    def update(self, params) -> None:
        if isinstance(params, QueryParams):
            new_items = [(k, v) for k, v in self._items if k not in dict(params._items)]
            new_items.extend(params._items)
            self._items = new_items
        elif isinstance(params, dict):
            for k, v in params.items():
                self.set(k, v)
        elif isinstance(params, (list, tuple)):
            for k, v in params:
                self.set(k, str(v))
        else:
            raise TypeError(
                f"update() argument must be QueryParams, dict, or list of tuples, "
                f"not {type(params).__name__}"
            )

    def merge(self, params) -> None:
        if isinstance(params, QueryParams):
            self._items.extend(params._items)
        elif isinstance(params, dict):
            for k, v in params.items():
                self.add(k, v)
        elif isinstance(params, (list, tuple)):
            for k, v in params:
                self.add(k, str(v))
        else:
            raise TypeError(
                f"merge() argument must be QueryParams, dict, or list of tuples, "
                f"not {type(params).__name__}"
            )

    def __getitem__(self, key: str) -> str:
        for k, v in reversed(self._items):
            if k == key:
                return v
        raise KeyError(key)

    def __setitem__(self, key: str, value) -> None:
        self.set(key, value)

    def __delitem__(self, key: str) -> None:
        before = len(self._items)
        self.remove(key)
        if len(self._items) == before:
            raise KeyError(key)

    def __contains__(self, key: object) -> bool:
        return any(k == key for k, _ in self._items)

    def __len__(self) -> int:
        return len(self._items)

    def __iter__(self):
        return iter(self.keys())

    def __bool__(self) -> bool:
        return len(self._items) > 0

    def __eq__(self, other):
        if isinstance(other, QueryParams):
            return self._items == other._items
        return NotImplemented

    def __hash__(self):
        return hash(tuple(self._items))

    def __str__(self) -> str:
        return urllib.parse.urlencode(self._items)

    def __repr__(self) -> str:
        return f"QueryParams({self!s})"


class URL:
    """HTTPX-compatible URL class using urllib.parse internally."""

    __slots__ = ("_parts", "_raw")

    def __new__(cls, url="", *, base_url=None):
        if isinstance(url, URL):
            return url
        instance = object.__new__(cls)
        instance._init(url, base_url)
        return instance

    def _init(self, url, base_url):
        if url is None:
            url = ""
        if isinstance(url, bytes):
            url = url.decode("ascii", errors="replace")
        if isinstance(url, URL):
            self._parts = url._parts
            self._raw = url._raw
            return

        if base_url is not None:
            if isinstance(base_url, URL):
                base_str = str(base_url)
            else:
                base_str = str(base_url)
            url = urllib.parse.urljoin(base_str, url)

        self._raw = url
        if _is_http_url(url):
            self._parts = _urlsplit(url)
        else:
            parts = urllib.parse.urlsplit(url)
            if parts.scheme:
                self._parts = parts
            else:
                self._parts = _urlsplit(f"http://{url}")

    @property
    def scheme(self) -> str:
        return self._parts.scheme

    @property
    def host(self) -> str | None:
        return self._parts.hostname

    @property
    def port(self) -> int | None:
        return self._parts.port

    @property
    def path(self) -> str:
        return self._parts.path

    @property
    def query(self) -> bytes:
        if self._parts.query:
            return self._parts.query.encode("utf-8")
        return b""

    @property
    def fragment(self) -> str:
        return self._parts.fragment

    @property
    def username(self) -> str | None:
        return self._parts.username

    @property
    def password(self) -> str | None:
        return self._parts.password

    @property
    def netloc(self) -> bytes:
        return self._parts.netloc.encode("utf-8")

    @property
    def userinfo(self) -> bytes:
        if self._parts.username:
            userinfo = self._parts.username
            if self._parts.password:
                userinfo += ":" + self._parts.password
            return userinfo.encode("utf-8")
        return b""

    @property
    def raw_host(self) -> bytes | None:
        if self._parts.hostname:
            return self._parts.hostname.encode("utf-8")
        return None

    @property
    def raw_path(self) -> bytes:
        path = self._parts.path
        if not path:
            path = "/"
        return path.encode("utf-8")

    @property
    def raw_scheme(self) -> bytes:
        return self._parts.scheme.encode("utf-8")

    @property
    def raw(self) -> tuple[bytes, bytes | None, int | None, bytes]:
        """Return raw URL components as ``(scheme, host, port, path)``.

        Matches the HTTPX 0.28.1 ``URL.raw`` public property.
        Default ports (80/http, 443/https) are returned as ``None``.
        The path includes the query string if present.
        """
        scheme = self._parts.scheme.encode("ascii")
        host = self._parts.hostname.encode("ascii") if self._parts.hostname else None
        port = self._parts.port
        if port is not None:
            if (self._parts.scheme == "http" and port == 80) or (
                self._parts.scheme == "https" and port == 443
            ):
                port = None
        raw_path = self._parts.path.encode("utf-8") if self._parts.path else b"/"
        if self._parts.query:
            raw_path = raw_path + b"?" + self._parts.query.encode("utf-8")
        return (scheme, host, port, raw_path)

    @property
    def is_absolute_url(self) -> bool:
        return bool(self._parts.scheme and self._parts.netloc)

    @property
    def is_relative_url(self) -> bool:
        return not self.is_absolute_url

    @property
    def params(self) -> QueryParams:
        if self._parts.query:
            return QueryParams(self._parts.query)
        return QueryParams()

    def copy_with(self, url=None, params=None):
        if url is not None:
            new = URL(url)
        else:
            new = URL(self)

        if params is not None:
            if isinstance(params, QueryParams):
                query_str = str(params)
            elif isinstance(params, dict):
                query_str = urllib.parse.urlencode(params)
            else:
                query_str = urllib.parse.urlencode(params)
            parts = _urlsplit(str(new))
            new_parts = urllib.parse.SplitResult(
                parts.scheme, parts.netloc, parts.path, query_str, parts.fragment
            )
            new = object.__new__(URL)
            new._parts = new_parts
            new._raw = urllib.parse.urlunsplit(new_parts)
        return new

    def copy_set_param(self, key: str, value) -> "URL":
        params = self.params
        params.set(key, value)
        return self.copy_with(params=params)

    def copy_remove_param(self, key: str) -> "URL":
        params = self.params
        params.remove(key)
        return self.copy_with(params=params)

    def copy_merge_params(self, params) -> "URL":
        existing = self.params
        existing.merge(params)
        return self.copy_with(params=existing)

    def copy_add_param(self, key: str, value) -> "URL":
        params = self.params
        params.add(key, value)
        return self.copy_with(params=params)

    def join(self, other: "URL") -> "URL":
        return URL(str(self) + str(other))

    def _default_port(self):
        if self._parts.port is None:
            return None
        if self._parts.scheme == "http" and self._parts.port == 80:
            return 80
        if self._parts.scheme == "https" and self._parts.port == 443:
            return 443
        return None

    def _display_str(self) -> str:
        parts = self._parts
        netloc = parts.netloc
        default_port = self._default_port()
        if default_port is not None and parts.port is not None:
            hostname = parts.hostname or ""
            if parts.password:
                netloc = f"{parts.username}:{parts.password}@{hostname}"
            elif parts.username:
                netloc = f"{parts.username}@{hostname}"
            else:
                netloc = hostname
        query = f"?{parts.query}" if parts.query else ""
        fragment = f"#{parts.fragment}" if parts.fragment else ""
        return f"{parts.scheme}://{netloc}{parts.path}{query}{fragment}"

    def __str__(self) -> str:
        return self._display_str()

    def __bytes__(self) -> bytes:
        return str(self).encode("utf-8")

    def __repr__(self) -> str:
        display = self._display_str()
        if self.password:
            display = display.replace(f":{self.password}@", ":***@")
        return f"URL({display!r})"

    def __eq__(self, other):
        if isinstance(other, URL):
            return str(self) == str(other)
        if isinstance(other, str):
            return str(self) == str(URL(other))
        return NotImplemented

    def __hash__(self):
        return hash(str(self))

    def __lt__(self, other):
        if isinstance(other, URL):
            return str(self) < str(other)
        return NotImplemented

    def __bool__(self) -> bool:
        return bool(self._raw)
