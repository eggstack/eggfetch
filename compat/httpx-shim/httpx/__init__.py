"""httpx compatibility shim backed by eggfetch.

This package provides ``import httpx`` that delegates to
``eggfetch.compat.httpx``. It is intended for environments where
downstream code imports ``httpx`` directly and cannot be modified.

Install this package INSTEAD of (or after removing) the real ``httpx``
package. It declares a conflict with ``httpx`` to prevent co-installation.
"""

from eggfetch.compat.httpx import *  # noqa: F401,F403
from eggfetch.compat.httpx import __all__  # noqa: F401

__version__ = "0.28.1"  # Emulated HTTPX version
