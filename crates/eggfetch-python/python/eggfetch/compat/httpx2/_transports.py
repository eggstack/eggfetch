"""HTTPX2 re-export of httpx compat _transports (identical semantics)."""

from eggfetch.compat.httpx._transports import *  # noqa: F401,F403
try:
    from eggfetch.compat.httpx._transports import __all__  # noqa: F401
except ImportError:
    pass
