"""HTTPX2 response class for eggfetch.

Subclasses the 0.28.1-compatible response (single shared Rust engine) and
keeps its ``url`` an httpx2 ``URL``, so the httpx2 ``origin`` delta is
reachable from every response a user observes.  The 0.28.1 facade is not
modified.
"""

from __future__ import annotations

import typing
from typing import Any

from eggfetch.compat.httpx._request import Request as _BaseRequest
from eggfetch.compat.httpx._response import Response as _BaseResponse
from eggfetch.compat.httpx2._request import _as_httpx2_request
from eggfetch.compat.httpx2._urls import URL, _as_httpx2_url, _upgrade_httpx2_url

__all__ = ["Response"]


class Response(_BaseResponse):
    """0.28.1-compatible response whose ``url`` is an httpx2 ``URL``.

    A 0.28.1 request handed to the constructor is upgraded to an httpx2
    request, so ``response.request.url`` exposes ``origin`` as well.
    """

    def __init__(
        self,
        status_code: int,
        *,
        headers=None,
        content: bytes | None = None,
        text: str | None = None,
        html: str | None = None,
        json: Any = None,
        stream=None,
        request=None,
        extensions: dict | None = None,
        history: list | None = None,
        default_encoding: str | typing.Callable[[bytes], str] = "utf-8",
    ) -> None:
        super().__init__(
            status_code,
            headers=headers,
            content=content,
            text=text,
            html=html,
            json=json,
            stream=stream,
            request=request,
            extensions=extensions,
            history=history,
            default_encoding=default_encoding,
        )
        if isinstance(self._request, _BaseRequest):
            self._request = _as_httpx2_request(self._request)
        self._url = _upgrade_httpx2_url(self._url)

    @_BaseResponse.url.setter
    def url(self, value: URL | str) -> None:
        self._url = _as_httpx2_url(value)

    @_BaseResponse.request.setter
    def request(self, value) -> None:
        if isinstance(value, _BaseRequest):
            value = _as_httpx2_request(value)
        _BaseResponse.request.fset(self, value)
        self._url = _upgrade_httpx2_url(self._url)

    @_BaseResponse.next_request.setter
    def next_request(self, value) -> None:
        if isinstance(value, _BaseRequest):
            value = _as_httpx2_request(value)
        _BaseResponse.next_request.fset(self, value)


def _as_httpx2_response(response) -> Response:
    """Return *response* as an httpx2 :class:`Response`.

    The 0.28.1 client builds responses with its own class, so responses
    that reach the httpx2 facade from there are re-typed in place (their
    state, body and history are preserved) and their URL is upgraded to an
    httpx2 ``URL``.
    """
    if not isinstance(response, Response):
        response.__class__ = Response
    response._url = _upgrade_httpx2_url(response._url)
    return response
