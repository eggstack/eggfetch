"""HTTPX2 re-export of httpx compat _wsgi (identical semantics)."""

from eggfetch.compat.httpx._wsgi import *  # noqa: F401,F403
try:
    from eggfetch.compat.httpx._wsgi import __all__  # noqa: F401
except ImportError:
    pass
