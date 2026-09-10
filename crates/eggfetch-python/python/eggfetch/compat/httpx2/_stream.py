"""HTTPX2 re-export of httpx compat _stream (identical semantics)."""

from eggfetch.compat.httpx._stream import *  # noqa: F401,F403
try:
    from eggfetch.compat.httpx._stream import __all__  # noqa: F401
except ImportError:
    pass
