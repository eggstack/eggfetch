"""HTTPX-compatible Proxy class for eggfetch."""

from __future__ import annotations


class Proxy:
    """HTTPX-compatible Proxy class.

    Phase 2: accept and store; Phase 4 executes.
    """

    __slots__ = ("_url", "_headers", "_auth")

    def __init__(self, url, *, headers=None, auth=None):
        from ._urls import URL

        if isinstance(url, URL):
            self._url = url
        elif isinstance(url, str):
            self._url = URL(url)
        else:
            raise TypeError(
                f"Proxy() url must be str or URL, got {type(url).__name__}"
            )

        self._headers = {}
        if headers is not None:
            if isinstance(headers, dict):
                self._headers = dict(headers)
            else:
                raise TypeError(
                    f"Proxy() headers must be dict or None, got {type(headers).__name__}"
                )

        self._auth = auth

    @property
    def url(self):
        return self._url

    @property
    def headers(self) -> dict:
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
        return None

    def __repr__(self) -> str:
        url_str = str(self._url)
        if self._url.password:
            url_str = url_str.replace(f":{self._url.password}@", ":***@")
        parts = [f"url={url_str!r}"]
        if self._headers:
            parts.append(f"headers={self._headers!r}")
        if self._auth is not None:
            parts.append("auth=***")
        return f"Proxy({', '.join(parts)})"
