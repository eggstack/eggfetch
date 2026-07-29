#!/usr/bin/env python3
"""Validate that publishable internal dependencies have concrete version requirements.

Uses `cargo metadata --format-version 1 --no-deps` and stdlib json.
No third-party dependencies.

Expected publishable internal crates:
  eggfetch-core, eggfetch-ffi

Each dependent crate (eggfetch-cli, eggfetch-ffi, eggfetch-python, eggfetch-node)
must have a concrete (non-wildcard, non-empty) version requirement for every
internal dependency it uses.
"""

import json
import subprocess
import sys

EXPECTED_PACKAGES = {"eggfetch-cli", "eggfetch-ffi", "eggfetch-python", "eggfetch-node"}
INTERNAL_DEPS = {"eggfetch-core", "eggfetch-ffi"}


def cargo_metadata() -> dict:
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(result.stdout)


def validate_version_req(dep_name: str, version_req: str | None) -> str | None:
    """Return an error message if the version requirement is not publishable."""
    if version_req is None:
        return f"{dep_name}: missing version requirement (path-only dependency)"
    v = version_req.strip()
    if not v:
        return f"{dep_name}: empty version requirement"
    if v == "*":
        return f"{dep_name}: wildcard version requirement not publishable"
    return None


def main() -> int:
    try:
        metadata = cargo_metadata()
    except (subprocess.CalledProcessError, FileNotFoundError) as exc:
        print(f"FAIL: cargo metadata failed: {exc}", file=sys.stderr)
        return 1

    packages = {p["name"]: p for p in metadata.get("packages", [])}

    missing_packages = EXPECTED_PACKAGES - set(packages.keys())
    if missing_packages:
        print(f"FAIL: expected packages not found in workspace: {', '.join(sorted(missing_packages))}", file=sys.stderr)
        return 1

    errors: list[str] = []

    for crate_name in sorted(EXPECTED_PACKAGES):
        pkg = packages[crate_name]
        deps = pkg.get("dependencies", [])
        internal_deps_found = [d for d in deps if d["name"] in INTERNAL_DEPS]

        if not internal_deps_found:
            # eggfetch-node depends on eggfetch-ffi, not eggfetch-core directly.
            # eggfetch-ffi depends on eggfetch-core, not on itself.
            # Only fail if the crate SHOULD have internal deps but doesn't.
            if crate_name in ("eggfetch-cli", "eggfetch-python"):
                errors.append(f"{crate_name}: no internal publishable dependency found (expected eggfetch-core)")
            continue

        for dep in internal_deps_found:
            # cargo metadata puts the version from the resolved dependency.
            # We need the version from the requirement, not the resolved version.
            # In cargo metadata, `req` field has the version requirement string.
            version_req = dep.get("req")
            err = validate_version_req(dep["name"], version_req)
            if err:
                errors.append(f"{crate_name}: {err}")

    if errors:
        for e in errors:
            print(f"FAIL: {e}", file=sys.stderr)
        return 1

    # Print what was checked
    for crate_name in sorted(EXPECTED_PACKAGES):
        pkg = packages[crate_name]
        deps = pkg.get("dependencies", [])
        internal = [d for d in deps if d["name"] in INTERNAL_DEPS]
        if internal:
            for dep in internal:
                print(f"  {crate_name} -> {dep['name']} req={dep.get('req', '?')}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
