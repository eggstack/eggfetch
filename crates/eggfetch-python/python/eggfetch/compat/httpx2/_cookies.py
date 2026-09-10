"""HTTPX2 re-export of httpx compat _cookies (identical semantics)."""

from eggfetch.compat.httpx._cookies import *  # noqa: F401,F403
try:
    from eggfetch.compat.httpx._cookies import __all__  # noqa: F401
except ImportError:
    pass
