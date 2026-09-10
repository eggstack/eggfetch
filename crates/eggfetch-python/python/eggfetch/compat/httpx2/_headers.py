"""HTTPX2 2.12.0-compatible Headers for eggfetch.

Subclasses the 0.28.1-compatible Headers with ``|``, ``|=`` merge
operators matching httpx2==2.12.0 (case-insensitive replacement,
duplicate handling, operand-type validation) while preserving redaction.
"""

from __future__ import annotations

import typing

from eggfetch.compat.httpx._headers import Headers as _BaseHeaders

if typing.TYPE_CHECKING:
    from collections.abc import Mapping


class Headers(_BaseHeaders):
    """Headers with httpx2 merge operators."""

    def __or__(self, other: Mapping[str, str]) -> Headers:
        if not isinstance(other, typing.Mapping):
            try:
                from collections.abc import Mapping as _Mapping

                if not isinstance(other, _Mapping):
                    return NotImplemented
            except Exception:
                return NotImplemented
        merged = self.copy()
        # copy() returns base-class instance; re-wrap as httpx2 Headers.
        wrapped = Headers(encoding=merged.encoding)
        wrapped._items = list(merged._items)
        wrapped.update(other)
        return wrapped

    def __ror__(self, other: Mapping[str, str]) -> Headers:
        from collections.abc import Mapping as _Mapping

        if not isinstance(other, _Mapping):
            return NotImplemented
        merged = Headers(other)
        merged.update(self)
        return merged

    def __ior__(self, other) -> Headers:
        self.update(other)
        return self


__all__ = ["Headers"]
