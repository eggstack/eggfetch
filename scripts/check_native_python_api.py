#!/usr/bin/env python3
"""Check the supported native Python API against its reviewed manifest."""
from __future__ import annotations

import importlib.metadata
import inspect
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "crates/eggfetch-python/tests/native_api_manifest.json"


def main() -> int:
    import eggfetch
    from eggfetch import _native

    manifest = json.loads(MANIFEST.read_text())
    expected = manifest["exports"]
    errors: list[str] = []
    if list(eggfetch.__all__) != expected:
        errors.append("eggfetch.__all__ differs from the reviewed native manifest")
    if set(_native.__all__) != set(expected) - {"__version__"}:
        errors.append("eggfetch._native exports differ from the root contract")
    for name in expected:
        if not hasattr(eggfetch, name):
            errors.append(f"missing root export: {name}")
    for name, base in manifest["exception_bases"].items():
        value = getattr(eggfetch, name, None)
        if value is None or value.__bases__[0].__name__ != base:
            errors.append(f"exception base drift: {name} != {base}")
    for name in manifest["signature_names"]:
        try:
            inspect.signature(getattr(eggfetch, name))
        except (AttributeError, TypeError, ValueError) as error:
            errors.append(f"signature unavailable for {name}: {error}")
    try:
        installed = importlib.metadata.version("eggfetch")
    except importlib.metadata.PackageNotFoundError:
        installed = None
    if installed is not None and eggfetch.__version__ != installed:
        errors.append(f"runtime version {eggfetch.__version__!r} != installed {installed!r}")
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"Native API manifest passed ({len(expected)} exports)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
