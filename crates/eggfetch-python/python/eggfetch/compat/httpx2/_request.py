"""HTTPX2 request class for eggfetch.

Subclasses the 0.28.1-compatible request (single shared Rust engine) and
keeps its ``url`` an httpx2 ``URL``, so the httpx2 ``origin`` delta is
reachable from every request a user observes.  The 0.28.1 facade is not
modified.
"""

from __future__ import annotations

from eggfetch.compat.httpx._request import Request as _BaseRequest
from eggfetch.compat.httpx2._urls import URL, _as_httpx2_url

__all__ = ["Request"]


class Request(_BaseRequest):
    """0.28.1-compatible request whose ``url`` is an httpx2 ``URL``."""

    __slots__ = ()

    def __init__(
        self,
        method: str,
        url: URL | str,
        *,
        params=None,
        headers=None,
        cookies=None,
        content: bytes | None = None,
        data=None,
        files=None,
        json=None,
        stream=None,
        extensions: dict | None = None,
    ) -> None:
        super().__init__(
            method,
            url,
            params=params,
            headers=headers,
            cookies=cookies,
            content=content,
            data=data,
            files=files,
            json=json,
            stream=stream,
            extensions=extensions,
        )
        self._url = _as_httpx2_url(self._url)

    @_BaseRequest.url.setter
    def url(self, value: URL | str) -> None:
        self._url = _as_httpx2_url(value)


def _as_httpx2_request(request) -> Request:
    """Return *request* as an httpx2 :class:`Request`.

    The 0.28.1 client builds requests with its own class, so requests that
    reach the httpx2 facade from there are re-typed in place (their state
    is preserved) and their URL is upgraded to an httpx2 ``URL``.
    """
    if not isinstance(request, Request):
        request.__class__ = Request
    request._url = _as_httpx2_url(request._url)
    return request
