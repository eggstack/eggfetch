"""Backward-compatible import shim for the neutral SSL interop helpers."""

from eggfetch._ssl_context import (  # noqa: F401
    Classification,
    _EggfetchSSLRegistry,
    _SSLContextSnapshot,
    _classify_context,
    _eggfetch_ssl_registry,
    context_to_eggfetch_kwargs,
    snapshot_context,
)

__all__ = [
    "Classification",
    "context_to_eggfetch_kwargs",
    "snapshot_context",
]
