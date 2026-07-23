#!/usr/bin/env python3
"""Verify that the installed httpx wheel is the controlled replacement.

Exits 0 on success, nonzero on failure.
"""

import importlib.metadata
import sys


def main() -> int:
    errors: list[str] = []

    # 1. Import httpx and check __eggfetch_shim__
    try:
        import httpx
    except ImportError as exc:
        print(f"FAIL: cannot import httpx: {exc}", file=sys.stderr)
        return 1

    if not getattr(httpx, "__eggfetch_shim__", False):
        errors.append("httpx.__eggfetch_shim__ is not True")

    # 2. Check __version__
    if getattr(httpx, "__version__", None) != "0.28.1":
        errors.append(
            f"httpx.__version__ is {httpx.__version__!r}, expected '0.28.1'"
        )

    # 3. Check Client/AsyncClient module paths
    for cls_name in ("Client", "AsyncClient"):
        cls = getattr(httpx, cls_name, None)
        if cls is None:
            errors.append(f"httpx.{cls_name} not found")
            continue
        mod = getattr(cls, "__module__", "")
        if "eggfetch" not in mod:
            errors.append(
                f"httpx.{cls_name}.__module__ is {mod!r}, expected 'eggfetch'"
            )

    # 4. Distribution metadata
    try:
        dist = importlib.metadata.distribution("httpx")
    except importlib.metadata.PackageNotFoundError as exc:
        print(f"FAIL: distribution 'httpx' not found: {exc}", file=sys.stderr)
        return 1

    meta_name = dist.metadata["Name"]
    if meta_name != "httpx":
        errors.append(f"distribution name is {meta_name!r}, expected 'httpx'")

    meta_version = dist.metadata["Version"]
    if meta_version != "0.28.1":
        errors.append(
            f"distribution version is {meta_version!r}, expected '0.28.1'"
        )

    reqs = dist.requires or []
    has_eggfetch_dep = any("eggfetch" in r for r in reqs)
    if not has_eggfetch_dep:
        errors.append(f"distribution missing eggfetch dependency, got: {reqs}")

    # Report
    if errors:
        for e in errors:
            print(f"FAIL: {e}", file=sys.stderr)
        return 1

    print("PASS: all identity checks passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
