"""``import httpx`` backed by eggfetch — the single source for both replacement packages.

This module is the one place the eggfetch-backed ``httpx`` replacement code
exists. Two distributions package this file; they differ only in their
distribution metadata:

- ``httpx-eggfetch-shim`` (``compat/httpx-shim/``) — the convenience
  distribution for environments where downstream code imports ``httpx``
  directly and cannot be modified.
- ``httpx`` (``compat/httpx-controlled-replacement/``) — the controlled
  replacement artifact consumed by the Stage C downstream qualification path.

Neither distribution may be published to public PyPI as ``httpx``. The
controlled replacement exists solely for qualification; the shim exists so
that downstream consumers can be pointed at eggfetch's compatibility layer.

Both builds package this file through setuptools' ``package-dir`` mapping, so
the runtime code is not duplicated in either source tree.
"""

from eggfetch.compat.httpx import *  # noqa: F401,F403
from eggfetch.compat.httpx import __all__  # noqa: F401

__version__ = "0.28.1"  # Emulated HTTPX version

# Identity marker. The downstream qualification path
# (``scripts/run_isolated_downstream.py``) requires it to prove that ``httpx``
# resolved to the eggfetch-backed package rather than to the real httpx.
__eggfetch_shim__ = True
