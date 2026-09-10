"""HTTPX2 re-export of httpx compat _asgi (identical semantics)."""

from eggfetch.compat.httpx._asgi import *  # noqa: F401,F403
try:
    from eggfetch.compat.httpx._asgi import __all__  # noqa: F401
except ImportError:
    pass
