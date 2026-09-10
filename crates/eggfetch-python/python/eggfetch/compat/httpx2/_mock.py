"""HTTPX2 re-export of httpx compat _mock (identical semantics)."""

from eggfetch.compat.httpx._mock import *  # noqa: F401,F403
try:
    from eggfetch.compat.httpx._mock import __all__  # noqa: F401
except ImportError:
    pass
