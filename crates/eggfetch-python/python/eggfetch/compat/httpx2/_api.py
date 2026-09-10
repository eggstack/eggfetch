"""Top-level HTTPX2-compatible helpers for eggfetch.

Mirrors httpx2==2.12.0 signatures plus an additive ``extensions`` kwarg
for transport hints (classified in allowed-differences). Includes the new
``query`` and ``websocket`` entry points.
"""

from __future__ import annotations

import typing
from contextlib import contextmanager

from eggfetch.compat.httpx2._client import AsyncClient, Client
from eggfetch.compat.httpx._response import Response
from eggfetch.compat.httpx2._config import (
    DEFAULT_KEEPALIVE_PING_INTERVAL_SECONDS,
    DEFAULT_KEEPALIVE_PING_TIMEOUT_SECONDS,
    DEFAULT_MAX_MESSAGE_SIZE_BYTES,
    DEFAULT_QUEUE_SIZE,
    DEFAULT_TIMEOUT_CONFIG,
)

if typing.TYPE_CHECKING:
    from eggfetch.compat.httpx2.websockets._api import WebSocketSession


def request(
    method,
    url,
    *,
    params=None,
    content=None,
    data=None,
    files=None,
    json=None,
    headers=None,
    cookies=None,
    auth=None,
    proxy=None,
    timeout=DEFAULT_TIMEOUT_CONFIG,
    follow_redirects=False,
    verify=True,
    trust_env=True,
    extensions=None,
) -> Response:
    with Client(
        cookies=cookies, proxy=proxy, verify=verify, timeout=timeout, trust_env=trust_env
    ) as client:
        return client.request(
            method, url, params=params, content=content, data=data, files=files,
            json=json, headers=headers, auth=auth,
            follow_redirects=follow_redirects, extensions=extensions,
        )


def get(
    url, *, params=None, headers=None, cookies=None, auth=None, proxy=None,
    follow_redirects=False, verify=True, timeout=DEFAULT_TIMEOUT_CONFIG,
    trust_env=True, extensions=None,
) -> Response:
    with Client(
        cookies=cookies, proxy=proxy, verify=verify, timeout=timeout, trust_env=trust_env
    ) as client:
        return client.get(
            url, params=params, headers=headers, auth=auth,
            follow_redirects=follow_redirects, extensions=extensions,
        )


def options(
    url, *, params=None, headers=None, cookies=None, auth=None, proxy=None,
    follow_redirects=False, verify=True, timeout=DEFAULT_TIMEOUT_CONFIG,
    trust_env=True, extensions=None,
) -> Response:
    with Client(
        cookies=cookies, proxy=proxy, verify=verify, timeout=timeout, trust_env=trust_env
    ) as client:
        return client.options(
            url, params=params, headers=headers, auth=auth,
            follow_redirects=follow_redirects, extensions=extensions,
        )


def head(
    url, *, params=None, headers=None, cookies=None, auth=None, proxy=None,
    follow_redirects=False, verify=True, timeout=DEFAULT_TIMEOUT_CONFIG,
    trust_env=True, extensions=None,
) -> Response:
    with Client(
        cookies=cookies, proxy=proxy, verify=verify, timeout=timeout, trust_env=trust_env
    ) as client:
        return client.head(
            url, params=params, headers=headers, auth=auth,
            follow_redirects=follow_redirects, extensions=extensions,
        )


def post(
    url, *, content=None, data=None, files=None, json=None, params=None,
    headers=None, cookies=None, auth=None, proxy=None, follow_redirects=False,
    verify=True, timeout=DEFAULT_TIMEOUT_CONFIG, trust_env=True, extensions=None,
) -> Response:
    with Client(
        cookies=cookies, proxy=proxy, verify=verify, timeout=timeout, trust_env=trust_env
    ) as client:
        return client.post(
            url, content=content, data=data, files=files, json=json, params=params,
            headers=headers, auth=auth, follow_redirects=follow_redirects,
            extensions=extensions,
        )


def put(
    url, *, content=None, data=None, files=None, json=None, params=None,
    headers=None, cookies=None, auth=None, proxy=None, follow_redirects=False,
    verify=True, timeout=DEFAULT_TIMEOUT_CONFIG, trust_env=True, extensions=None,
) -> Response:
    with Client(
        cookies=cookies, proxy=proxy, verify=verify, timeout=timeout, trust_env=trust_env
    ) as client:
        return client.put(
            url, content=content, data=data, files=files, json=json, params=params,
            headers=headers, auth=auth, follow_redirects=follow_redirects,
            extensions=extensions,
        )


def patch(
    url, *, content=None, data=None, files=None, json=None, params=None,
    headers=None, cookies=None, auth=None, proxy=None, follow_redirects=False,
    verify=True, timeout=DEFAULT_TIMEOUT_CONFIG, trust_env=True, extensions=None,
) -> Response:
    with Client(
        cookies=cookies, proxy=proxy, verify=verify, timeout=timeout, trust_env=trust_env
    ) as client:
        return client.patch(
            url, content=content, data=data, files=files, json=json, params=params,
            headers=headers, auth=auth, follow_redirects=follow_redirects,
            extensions=extensions,
        )


def delete(
    url, *, params=None, headers=None, cookies=None, auth=None, proxy=None,
    follow_redirects=False, timeout=DEFAULT_TIMEOUT_CONFIG, verify=True,
    trust_env=True, extensions=None,
) -> Response:
    with Client(
        cookies=cookies, proxy=proxy, verify=verify, timeout=timeout, trust_env=trust_env
    ) as client:
        return client.delete(
            url, params=params, headers=headers, auth=auth,
            follow_redirects=follow_redirects, extensions=extensions,
        )


def query(
    url, *, content=None, data=None, files=None, json=None, params=None,
    headers=None, cookies=None, auth=None, proxy=None, follow_redirects=False,
    verify=True, timeout=DEFAULT_TIMEOUT_CONFIG, trust_env=True, extensions=None,
) -> Response:
    """Send a ``QUERY`` request."""
    with Client(
        cookies=cookies, proxy=proxy, verify=verify, timeout=timeout, trust_env=trust_env
    ) as client:
        return client.query(
            url, content=content, data=data, files=files, json=json, params=params,
            headers=headers, auth=auth, follow_redirects=follow_redirects,
            extensions=extensions,
        )


@contextmanager
def stream(
    method, url, *, params=None, content=None, data=None, files=None, json=None,
    headers=None, cookies=None, auth=None, proxy=None, timeout=DEFAULT_TIMEOUT_CONFIG,
    follow_redirects=False, verify=True, trust_env=True, extensions=None,
) -> typing.Iterator:
    with Client(
        cookies=cookies, proxy=proxy, verify=verify, timeout=timeout, trust_env=trust_env
    ) as client:
        with client.stream(
            method, url, params=params, content=content, data=data, files=files,
            json=json, headers=headers, auth=auth,
            follow_redirects=follow_redirects, extensions=extensions,
        ) as response:
            yield response


@contextmanager
def websocket(
    url, *, params=None, headers=None, cookies=None, auth=None, proxy=None,
    follow_redirects=False, timeout=DEFAULT_TIMEOUT_CONFIG, verify=True,
    trust_env=True, subprotocols=None,
    max_message_size_bytes: int = DEFAULT_MAX_MESSAGE_SIZE_BYTES,
    queue_size: int = DEFAULT_QUEUE_SIZE,
    keepalive_ping_interval_seconds: float | None = DEFAULT_KEEPALIVE_PING_INTERVAL_SECONDS,
    keepalive_ping_timeout_seconds: float | None = DEFAULT_KEEPALIVE_PING_TIMEOUT_SECONDS,
    extensions=None,
) -> typing.Iterator:
    """Open a WebSocket session."""
    with Client(
        cookies=cookies, proxy=proxy, verify=verify, timeout=timeout, trust_env=trust_env
    ) as client:
        with client.websocket(
            url, params=params, headers=headers, cookies=cookies, auth=auth,
            follow_redirects=follow_redirects, timeout=timeout,
            subprotocols=subprotocols,
            max_message_size_bytes=max_message_size_bytes, queue_size=queue_size,
            keepalive_ping_interval_seconds=keepalive_ping_interval_seconds,
            keepalive_ping_timeout_seconds=keepalive_ping_timeout_seconds,
            extensions=extensions,
        ) as ws:
            yield ws


__all__ = [
    "delete", "get", "head", "options", "patch", "post", "put", "query",
    "request", "stream", "websocket",
]
