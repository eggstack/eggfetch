"""Controlled replacement artifact: ``import httpx`` backed by eggfetch.

This package declares distribution name ``httpx`` (version 0.28.1) so that
downstream ``Requires-Dist: httpx`` metadata is satisfied without installing
the real PyPI httpx package.  It is intended solely for Stage C qualification
testing and must never be published to public PyPI.
"""

from eggfetch.compat.httpx import *  # noqa: F401,F403
from eggfetch.compat.httpx import __all__  # noqa: F401

__version__ = "0.28.1"
__eggfetch_shim__ = True
