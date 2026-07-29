#!/usr/bin/env python3
"""Validate that publishable internal dependencies form the exact expected topology.

Uses `cargo metadata --format-version 1 --no-deps` and stdlib json.
No third-party dependencies.

Expected publishable internal crates and their dependency edges:
  eggfetch-cli  -> eggfetch-core
  eggfetch-ffi  -> eggfetch-core
  eggfetch-python -> eggfetch-core
  eggfetch-node -> eggfetch-ffi
"""

import json
import subprocess
import sys

EXPECTED_DEPENDENCIES: dict[str, set[str]] = {
    "eggfetch-cli": {"eggfetch-core"},
    "eggfetch-ffi": {"eggfetch-core"},
    "eggfetch-python": {"eggfetch-core"},
    "eggfetch-node": {"eggfetch-ffi"},
}


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
    if "*" in v:
        return f"{dep_name}: wildcard version requirement not publishable: {v!r}"
    return None


def main() -> int:
    try:
        metadata = cargo_metadata()
    except (subprocess.CalledProcessError, FileNotFoundError) as exc:
        print(f"FAIL: cargo metadata failed: {exc}", file=sys.stderr)
        return 1

    packages = {p["name"]: p for p in metadata.get("packages", [])}

    # Verify all expected dependent crates exist in the workspace.
    missing = set(EXPECTED_DEPENDENCIES.keys()) - set(packages.keys())
    if missing:
        print(
            f"FAIL: expected dependent crates not found in workspace: "
            f"{', '.join(sorted(missing))}",
            file=sys.stderr,
        )
        return 1

    errors: list[str] = []
    edges: list[str] = []

    for crate_name in sorted(EXPECTED_DEPENDENCIES):
        expected = EXPECTED_DEPENDENCIES[crate_name]
        pkg = packages[crate_name]
        deps = pkg.get("dependencies", [])

        # Build a map of actual package name -> dependency record.
        # Handle renamed deps by using the actual package name.
        actual_internal: dict[str, dict] = {}
        for dep in deps:
            # The "package" field is the real crate name for renamed deps.
            real_name = dep.get("package", dep["name"])
            if real_name in expected or dep["name"] in expected:
                actual_internal[real_name] = dep

        # Check every expected dependency is present.
        for exp_dep in sorted(expected):
            if exp_dep not in actual_internal:
                errors.append(
                    f"{crate_name}: expected internal dependency {exp_dep!r} not found"
                )
                continue

            dep = actual_internal[exp_dep]

            # Validate version requirement.
            version_req = dep.get("req")
            err = validate_version_req(exp_dep, version_req)
            if err:
                errors.append(f"{crate_name}: {err}")
                continue

            # Validate local path is present.
            path = dep.get("path")
            if not path:
                errors.append(
                    f"{crate_name}: {exp_dep} missing local path "
                    f"(version-only dependency not allowed)"
                )
                continue

            # Record the validated edge.
            alias = dep.get("name", exp_dep)
            alias_note = f" (aliased as {alias!r})" if alias != exp_dep else ""
            edges.append(
                f"  {crate_name} -> {exp_dep} req={version_req} "
                f"path={path}{alias_note}"
            )

        # Check for unexpected internal dependencies.
        for real_name, dep in actual_internal.items():
            if real_name not in expected:
                errors.append(
                    f"{crate_name}: unexpected internal dependency {real_name!r}"
                )

    if errors:
        for e in errors:
            print(f"FAIL: {e}", file=sys.stderr)
        return 1

    # Print validated edges.
    for edge in edges:
        print(edge)

    return 0


if __name__ == "__main__":
    sys.exit(main())
