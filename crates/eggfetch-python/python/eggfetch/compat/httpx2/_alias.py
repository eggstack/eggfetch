"""Explicit opt-in alias for the HTTPX2 facade.

Calling :func:`alias_httpx` makes ``import httpx`` resolve to
``eggfetch.compat.httpx2`` process-wide (for apps migrating from httpx).
Ordinary ``import eggfetch.compat.httpx2`` never mutates ``sys.modules``
for ``httpx`` or for ``eggfetch.compat.httpx``; the two facades coexist.
"""

from __future__ import annotations

import importlib
import importlib.abc
import importlib.machinery
import importlib.util
import sys
from collections.abc import Sequence
from types import ModuleType

__all__ = ["alias_httpx"]


class _AliasLoader(importlib.abc.Loader):
    _original_spec: importlib.machinery.ModuleSpec | None

    def __init__(self, real_name: str) -> None:
        self._real_name = real_name

    def create_module(self, spec: importlib.machinery.ModuleSpec) -> ModuleType:
        module = importlib.import_module(self._real_name)
        self._original_spec = module.__spec__
        return module

    def exec_module(self, module: ModuleType) -> None:
        module.__spec__ = self._original_spec


class _AliasFinder(importlib.abc.MetaPathFinder):
    def __init__(self, alias: str, real: str) -> None:
        self._alias = alias
        self._real = real

    def find_spec(
        self,
        fullname: str,
        path: Sequence[str] | None = None,
        target: ModuleType | None = None,
    ) -> ModuleType | None:
        if fullname != self._alias and not fullname.startswith(self._alias + "."):
            return None
        real_name = self._real + fullname.removeprefix(self._alias)
        if real_name not in sys.modules and importlib.util.find_spec(real_name) is None:
            return None
        return importlib.machinery.ModuleSpec(fullname, _AliasLoader(real_name))


def _alias(alias: str, module: ModuleType) -> None:
    existing = sys.modules.get(alias)
    if existing is not None and existing is not module:
        raise RuntimeError(
            f"{alias} was already imported; call `alias_httpx()` before any `import {alias}`."
        )
    if not any(
        isinstance(finder, _AliasFinder) and finder._alias == alias
        for finder in sys.meta_path
    ):
        sys.meta_path.insert(0, _AliasFinder(alias, module.__name__))
    sys.modules[alias] = module


def alias_httpx() -> None:
    """Make ``import httpx`` resolve to ``eggfetch.compat.httpx2``.

    Intended for applications migrating from httpx. Libraries should never
    call this. Must be called before anything imports ``httpx``; calling
    again is a no-op. Never invoked implicitly by eggfetch imports.
    """
    import eggfetch.compat.httpx2 as httpx2_facade

    _alias("httpx", httpx2_facade)
