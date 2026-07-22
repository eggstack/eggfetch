"""HTTPX-compatible Cookies class for eggfetch."""

from __future__ import annotations


class Cookies:
    """HTTPX-compatible Cookies class.

    Simplified dict-based approach matching HTTPX behavior.
    """

    __slots__ = ("_cookies",)

    def __init__(self, cookies=None):
        self._cookies: dict[str, str] = {}
        if cookies is not None:
            if isinstance(cookies, Cookies):
                self._cookies = dict(cookies._cookies)
            elif isinstance(cookies, dict):
                self._cookies = {k: str(v) for k, v in cookies.items()}
            elif isinstance(cookies, (list, tuple)):
                for k, v in cookies:
                    self._cookies[str(k)] = str(v)
            else:
                raise TypeError(
                    f"Cookies() argument must be dict, list, or Cookies, "
                    f"not {type(cookies).__name__}"
                )

    def set(
        self,
        name: str,
        value,
        *,
        domain=None,
        path=None,
        secure=None,
        expires=None,
        samesite=None,
    ) -> None:
        self._cookies[name] = str(value)

    def get(self, name: str, default=None) -> str | None:
        return self._cookies.get(name, default)

    def delete(self, name: str, *, domain=None, path=None) -> None:
        self._cookies.pop(name, None)

    def clear(self) -> None:
        self._cookies.clear()

    def update(self, cookies=None) -> None:
        if isinstance(cookies, Cookies):
            self._cookies.update(cookies._cookies)
        elif isinstance(cookies, dict):
            self._cookies.update({k: str(v) for k, v in cookies.items()})
        elif isinstance(cookies, (list, tuple)):
            for k, v in cookies:
                self._cookies[str(k)] = str(v)

    def setdefault(self, name: str, default=None) -> str:
        if name not in self._cookies:
            self._cookies[name] = str(default) if default is not None else ""
        return self._cookies[name]

    def items(self):
        return self._cookies.items()

    def keys(self):
        return self._cookies.keys()

    def values(self):
        return self._cookies.values()

    def extract_cookies(self, request) -> None:
        """Populate cookies from a request's Cookie header."""
        cookie_header = None
        if hasattr(request, "headers"):
            cookie_header = request.headers.get("Cookie") or request.headers.get("cookie")
        if cookie_header:
            for item in cookie_header.split(";"):
                item = item.strip()
                if "=" in item:
                    name, _, value = item.partition("=")
                    self._cookies[name.strip()] = value.strip()

    def set_cookie_header(self, response) -> None:
        """Process Set-Cookie headers from a response."""
        if hasattr(response, "headers"):
            set_cookie = response.headers.get("set-cookie") or response.headers.get("Set-Cookie")
            if set_cookie:
                parts = set_cookie.split(";")
                cookie_part = parts[0].strip()
                if "=" in cookie_part:
                    name, _, value = cookie_part.partition("=")
                    self._cookies[name.strip()] = value.strip()

    def __setitem__(self, name: str, value) -> None:
        self._cookies[name] = str(value)

    def __getitem__(self, name: str) -> str:
        return self._cookies[name]

    def __delitem__(self, name: str) -> None:
        del self._cookies[name]

    def __contains__(self, key: object) -> bool:
        return key in self._cookies

    def __len__(self) -> int:
        return len(self._cookies)

    def __iter__(self):
        return iter(self._cookies)

    def __bool__(self) -> bool:
        return bool(self._cookies)

    def __repr__(self) -> str:
        items = ", ".join(f"{k!r}: {v!r}" for k, v in self._cookies.items())
        return f"Cookies({{{items}}})"
