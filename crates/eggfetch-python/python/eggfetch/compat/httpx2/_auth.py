"""HTTPX2 2.12.0-compatible authentication classes for eggfetch.

Re-exports the shared Auth/Basic/Digest/NetRC implementations (identical
semantics) and adds the public ``FunctionAuth`` adapter.
"""

from __future__ import annotations

import typing

from eggfetch.compat.httpx._auth import Auth, BasicAuth, DigestAuth, NetRCAuth
from eggfetch.compat.httpx._request import Request

if typing.TYPE_CHECKING:
    from collections.abc import Generator

    from eggfetch.compat.httpx._response import Response


class FunctionAuth(Auth):
    """Allows the ``auth`` argument to be passed as a simple callable.

    The callable takes the request and returns a new, modified request,
    matching httpx2==2.12.0. Redirect/retry flows drive the same
    ``auth_flow`` state machine as the other auth classes; the function is
    called once per logical auth attempt, never with leaked credentials
    across origins beyond the client's existing redirect policy.
    """

    def __init__(self, func: typing.Callable[[Request], Request]) -> None:
        self._func = func

    def auth_flow(self, request: Request) -> Generator[Request, Response, None]:
        yield self._func(request)

    def __repr__(self) -> str:
        return f"FunctionAuth({self._func!r})"


__all__ = ["Auth", "BasicAuth", "DigestAuth", "FunctionAuth", "NetRCAuth"]
