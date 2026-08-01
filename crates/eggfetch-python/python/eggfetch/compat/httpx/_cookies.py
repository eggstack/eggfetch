"""HTTPX-compatible Cookies class for eggfetch.

Architecture: Preferred A — stdlib ``CookieJar`` owned by the compatibility
client, matching HTTPX 0.28.1.  The facade generates ``Cookie`` headers
before each one-hop dispatch and extracts ``Set-Cookie`` headers after
each response.  Native automatic cookies are disabled for compatibility
dispatch to prevent two jars.
"""

from __future__ import annotations

import email.message
import typing
import urllib.request
from http.cookiejar import Cookie, CookieJar

from eggfetch.compat.httpx._exceptions import CookieConflict

if typing.TYPE_CHECKING:
    from eggfetch.compat.httpx._request import Request
    from eggfetch.compat.httpx._response import Response


class Cookies(typing.MutableMapping[str, str]):
    """HTTPX-compatible Cookies backed by ``http.cookiejar.CookieJar``.

    Supports domain/path/secure/expiry scoping, multiple ``Set-Cookie``
    headers, and duplicate-name conflict detection via ``.get(name)``.
    """

    __slots__ = ("jar",)

    def __init__(self, cookies: typing.Any = None) -> None:
        if cookies is None or isinstance(cookies, dict):
            self.jar: CookieJar = CookieJar()
            if isinstance(cookies, dict):
                for key, value in cookies.items():
                    self.set(key, str(value))
        elif isinstance(cookies, (list, tuple)):
            self.jar = CookieJar()
            for key, value in cookies:
                self.set(str(key), str(value))
        elif isinstance(cookies, Cookies):
            self.jar = CookieJar()
            for cookie in cookies.jar:
                self.jar.set_cookie(cookie)
        elif isinstance(cookies, CookieJar):
            self.jar = cookies
        else:
            raise TypeError(
                f"Cookies() argument must be dict, list, Cookies, or CookieJar, "
                f"not {type(cookies).__name__}"
            )

    # ── HTTPX-compatible API ──────────────────────────────────────────

    def extract_cookies(self, response: Response) -> None:
        """Load cookies from a response's ``Set-Cookie`` headers."""
        if response is None:
            return
        try:
            req = response.request
        except RuntimeError:
            req = None
        urllib_response = self._CookieCompatResponse(response)
        urllib_request = self._CookieCompatRequest(req)
        self.jar.extract_cookies(urllib_response, urllib_request)  # type: ignore[arg-type]

    def set_cookie_header(self, request: Request) -> None:
        """Set the ``Cookie`` header on a request from the jar."""
        urllib_request = self._CookieCompatRequest(request)
        self.jar.add_cookie_header(urllib_request)

    def set(
        self,
        name: str,
        value: str,
        *,
        domain: str = "",
        path: str = "/",
        secure: bool = False,
        expires: int | None = None,
        samesite: str | None = None,
    ) -> None:
        """Set a cookie value by name, with optional domain and path."""
        kwargs: dict[str, typing.Any] = {
            "version": 0,
            "name": name,
            "value": value,
            "port": None,
            "port_specified": False,
            "domain": domain,
            "domain_specified": bool(domain),
            "domain_initial_dot": domain.startswith(".") if domain else False,
            "path": path,
            "path_specified": bool(path),
            "secure": secure,
            "expires": expires,
            "discard": expires is None,
            "comment": None,
            "comment_url": None,
            "rest": {"HttpOnly": None},
            "rfc2109": False,
        }
        cookie = Cookie(**kwargs)  # type: ignore[arg-type]
        self.jar.set_cookie(cookie)

    def get(  # type: ignore[override]
        self,
        name: str,
        default: str | None = None,
        domain: str | None = None,
        path: str | None = None,
    ) -> str | None:
        """Get a cookie by name.  Raises ``CookieConflict`` if ambiguous."""
        value: str | None = None
        for cookie in self.jar:
            if cookie.name == name:
                if domain is None or cookie.domain == domain:
                    if path is None or cookie.path == path:
                        if value is not None:
                            raise CookieConflict(
                                f"Multiple cookies exist with name={name}"
                            )
                        value = cookie.value
        if value is None:
            return default
        return value

    def delete(
        self,
        name: str,
        *,
        domain: str | None = None,
        path: str | None = None,
    ) -> None:
        """Delete a cookie by name."""
        if domain is not None and path is not None:
            try:
                self.jar.clear(domain, path, name)
            except KeyError:
                pass
            return

        remove = [
            cookie
            for cookie in self.jar
            if cookie.name == name
            and (domain is None or cookie.domain == domain)
            and (path is None or cookie.path == path)
        ]
        for cookie in remove:
            try:
                self.jar.clear(cookie.domain, cookie.path, cookie.name)
            except KeyError:
                pass

    def clear(self, domain: str | None = None, path: str | None = None) -> None:
        """Delete all cookies, or a subset by domain/path."""
        args: list[str] = []
        if domain is not None:
            args.append(domain)
        if path is not None:
            assert domain is not None
            args.append(path)
        try:
            self.jar.clear(*args)
        except KeyError:
            pass

    def update(self, cookies: typing.Any = None) -> None:  # type: ignore[override]
        if isinstance(cookies, Cookies):
            for cookie in cookies.jar:
                self.jar.set_cookie(cookie)
        else:
            tmp = Cookies(cookies)
            for cookie in tmp.jar:
                self.jar.set_cookie(cookie)

    def setdefault(self, name: str, default: str | None = None) -> str:
        value = self.get(name)
        if value is None:
            value = str(default) if default is not None else ""
            self.set(name, value)
        return value

    def items(self) -> typing.Iterator[tuple[str, str]]:
        return ((c.name, c.value) for c in self.jar)

    def keys(self) -> typing.Iterator[str]:
        return (c.name for c in self.jar)

    def values(self) -> typing.Iterator[str]:
        return (c.value for c in self.jar)

    # ── MutableMapping protocol ───────────────────────────────────────

    def __setitem__(self, name: str, value: str) -> None:
        self.set(name, value)

    def __getitem__(self, name: str) -> str:
        value = self.get(name)
        if value is None:
            raise KeyError(name)
        return value

    def __delitem__(self, name: str) -> None:
        self.delete(name)

    def __contains__(self, key: object) -> bool:
        if not isinstance(key, str):
            return False
        for cookie in self.jar:
            if cookie.name == key:
                return True
        return False

    def __len__(self) -> int:
        return len(list(self.jar))

    def __iter__(self) -> typing.Iterator[str]:
        return (cookie.name for cookie in self.jar)

    def __bool__(self) -> bool:
        for _ in self.jar:
            return True
        return False

    def __repr__(self) -> str:
        items = ", ".join(
            f"<Cookie {c.name}={c.value} for {c.domain} />"
            for c in self.jar
        )
        return f"<Cookies[{items}]>" if items else "Cookies([])"

    # ── Compatibility adapters for stdlib CookieJar ───────────────────

    class _CookieCompatRequest(urllib.request.Request):
        """Wraps a compat Request for stdlib CookieJar operations."""

        def __init__(self, request: Request | None) -> None:
            super().__init__(
                url=str(request.url) if request is not None and hasattr(request, "url") else "http://localhost/",
                headers=dict(request.headers) if request is not None and hasattr(request, "headers") else {},
                method=getattr(request, "method", "GET") if request is not None else "GET",
            )
            self._compat_request = request

        def add_unredirected_header(self, key: str, value: str) -> None:
            super().add_unredirected_header(key, value)
            if self._compat_request is not None and hasattr(self._compat_request, "headers"):
                self._compat_request.headers[key] = value

    class _CookieCompatResponse:
        """Wraps a compat Response for stdlib CookieJar operations."""

        def __init__(self, response: Response) -> None:
            self._response = response

        def info(self) -> email.message.Message:
            info = email.message.Message()
            if self._response is not None and hasattr(self._response, "headers"):
                for key, value in self._response.headers.multi_items():
                    info[key] = value
            return info
