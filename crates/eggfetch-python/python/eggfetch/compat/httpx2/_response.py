"""HTTPX2 re-export of httpx compat _response (identical semantics)."""

from eggfetch.compat.httpx._response import *  # noqa: F401,F403
try:
    from eggfetch.compat.httpx._response import __all__  # noqa: F401
except ImportError:
    pass
