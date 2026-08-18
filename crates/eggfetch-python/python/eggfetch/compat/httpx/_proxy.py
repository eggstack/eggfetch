"""HTTPX-compatible Proxy class for eggfetch."""

from __future__ import annotations

_SENSITIVE_HEADER_NAMES = frozenset(
    {"authorization", "proxy-authorization", "cookie", "set-cookie"}
)


def _is_sensitive_header_name(name: str) -> bool:
    return name.lower() in _SENSITIVE_HEADER_NAMES


class Proxy:
    """HTTPX-compatible Proxy class.

    Accepts the same constructor signature as httpx.Proxy:
    ``Proxy(url, *, headers=None, auth=None, ssl_context=None)``.

    ``headers`` accepts the same input forms that HTTPX's ``Headers`` type
    accepts: a mapping, a sequence of two-tuples, or ``None``.  Duplicate
    header names are preserved.
    """

    __slots__ = ("_url", "_headers", "_auth", "_ssl_context")

    def __init__(self, url, *, headers=None, auth=None, ssl_context=None):
        from ._urls import URL

        if isinstance(url, URL):
            self._url = url
        elif isinstance(url, str):
            self._url = URL(url)
        else:
            raise TypeError(
                f"Proxy() url must be str or URL, got {type(url).__name__}"
            )

        self._headers = _normalize_headers(headers)
        self._auth = auth
        self._ssl_context = ssl_context

    @property
    def url(self):
        return self._url

    @property
    def headers(self) -> list[tuple[str, str]]:
        return self._headers

    @property
    def auth(self):
        return self._auth

    @property
    def raw_auth(self):
        if self._auth is None:
            return None
        if isinstance(self._auth, (list, tuple)):
            return tuple(self._auth)
        return None

    @property
    def ssl_context(self):
        return self._ssl_context

    def __repr__(self) -> str:
        url_str = str(self._url)
        if self._url.password:
            url_str = url_str.replace(f":{self._url.password}@", ":***@")
        parts = [f"url={url_str!r}"]
        if self._headers:
            redacted_headers = [
                (name, "<redacted>" if _is_sensitive_header_name(name) else value)
                for name, value in self._headers
            ]
            parts.append(f"headers={redacted_headers!r}")
        if self._auth is not None:
            parts.append("auth=***")
        return f"Proxy({', '.join(parts)})"


def _normalize_headers(headers) -> list[tuple[str, str]]:
    """Normalize proxy headers to a list of ``(name, value)`` tuples.

    Accepts the same input forms HTTPX's ``Headers`` type accepts:
    - ``None`` → empty list
    - a ``dict`` → list of tuples (preserving insertion order)
    - a sequence of two-tuples → copied as-is
    - anything else → ``TypeError``

    Header names and values are converted to ``str``.
    Duplicate names are preserved (no deduplication).
    """
    if headers is None:
        return []
    if isinstance(headers, dict):
        return [(str(k), str(v)) for k, v in headers.items()]
    if isinstance(headers, (list, tuple)):
        result = []
        for pair in headers:
            if isinstance(pair, (list, tuple)) and len(pair) == 2:
                result.append((str(pair[0]), str(pair[1])))
            else:
                raise TypeError(
                    "Proxy() headers sequence items must be (name, value) pairs, "
                    f"got {type(pair).__name__}"
                )
        return result
    raise TypeError(
        f"Proxy() headers must be dict, sequence of pairs, or None, "
        f"got {type(headers).__name__}"
    )
