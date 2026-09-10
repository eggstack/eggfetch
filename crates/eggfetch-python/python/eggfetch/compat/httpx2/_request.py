"""HTTPX2 re-export of httpx compat _request (identical semantics)."""

from eggfetch.compat.httpx._request import *  # noqa: F401,F403
try:
    from eggfetch.compat.httpx._request import __all__  # noqa: F401
except ImportError:
    pass
