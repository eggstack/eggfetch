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
    if hasattr(_native, "__all__"):
        errors.append("eggfetch._native must not define an independent __all__")
    for name in expected:
        if not hasattr(eggfetch, name):
            errors.append(f"missing root export: {name}")

    for kind, names in manifest["symbol_kinds"].items():
        for name in names:
            value = getattr(eggfetch, name, None)
            if kind == "value" and value is None:
                errors.append(f"missing value export: {name}")
            elif kind == "singleton" and not isinstance(value, eggfetch.NoAuth):
                errors.append(f"singleton kind drift: {name}")
            elif kind == "function" and not inspect.isroutine(value):
                errors.append(f"function kind drift: {name}")
            elif kind == "class" and not inspect.isclass(value):
                errors.append(f"class kind drift: {name}")
            elif kind == "exception" and not inspect.isclass(value):
                errors.append(f"exception kind drift: {name}")

    for name, base in manifest["exception_bases"].items():
        value = getattr(eggfetch, name, None)
        if value is None or value.__bases__[0].__name__ != base:
            errors.append(f"exception base drift: {name} != {base}")

    for name, expected_mro in manifest["exception_mro"].items():
        value = getattr(eggfetch, name, None)
        actual_mro = [item.__name__ for item in value.__mro__] if value else []
        if actual_mro != expected_mro:
            errors.append(f"exception MRO drift: {name} = {actual_mro!r}")

    for name, expected_signature in manifest["signatures"].items():
        try:
            owner, separator, member = name.rpartition(".")
            value = getattr(getattr(eggfetch, owner), member) if separator else getattr(eggfetch, name)
            actual_signature = str(inspect.signature(value))
            if actual_signature != expected_signature:
                errors.append(
                    f"signature drift: {name} = {actual_signature!r}, expected {expected_signature!r}"
                )
        except (AttributeError, TypeError, ValueError) as error:
            errors.append(f"signature unavailable for {name}: {error}")

    # The re-exported native types remain discoverable from the supported
    # package even though PyO3 reports their implementation module as builtins.
    for name in manifest["symbol_kinds"]["class"]:
        if not getattr(eggfetch, name).__module__:
            errors.append(f"class module metadata missing: {name}")

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
