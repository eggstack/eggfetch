#!/usr/bin/env python3
"""Validate version coherence across Cargo crates, pyproject.toml, and Git tags.

Uses `cargo metadata` and stdlib only. No third-party dependencies.

Usage:
    python validate_release_versions.py                    # local coherence check
    python validate_release_versions.py --tag v0.1.0       # validate against a tag
    python validate_release_versions.py --tag v0.1.0 --publish  # publishing mode
"""

import argparse
import json
import re
import subprocess
import sys

PUBLISHABLE_CRATES = [
    "eggfetch-core",
    "eggfetch-cli",
    "eggfetch-ffi",
    "eggfetch-python",
    "eggfetch-node",
]

PYPROJECT_PATH = "crates/eggfetch-python/pyproject.toml"
EGGFETCH_PYTHON_CARGO_TOML = "crates/eggfetch-python/Cargo.toml"

PLACEHOLDER_RE = re.compile(r"(alpha|beta|rc|dev|pre|SNAPSHOT|UNRELEASED)", re.IGNORECASE)


def cargo_metadata() -> dict:
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(result.stdout)


def parse_pyproject_version(path: str) -> str | None:
    """Extract version from pyproject.toml using regex (stdlib only)."""
    try:
        with open(path) as f:
            content = f.read()
    except OSError:
        return None
    # Match [project] section's version = "X.Y.Z" or version = 'X.Y.Z'
    in_project = False
    for line in content.splitlines():
        stripped = line.strip()
        if stripped == "[project]":
            in_project = True
            continue
        if stripped.startswith("[") and in_project:
            break
        if in_project:
            m = re.match(r'^version\s*=\s*["\']([^"\']+)["\']', stripped)
            if m:
                return m.group(1)
    return None


def parse_cargo_toml_version(path: str) -> str | None:
    """Extract version from a Cargo.toml using regex (stdlib only)."""
    try:
        with open(path) as f:
            content = f.read()
    except OSError:
        return None
    for line in content.splitlines():
        stripped = line.strip()
        m = re.match(r'^version\s*=\s*["\']([^"\']+)["\']', stripped)
        if m:
            return m.group(1)
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate release version coherence")
    parser.add_argument("--tag", help="Git tag to validate (e.g. v0.1.0)")
    parser.add_argument(
        "--publish",
        action="store_true",
        help="Enable publishing mode (stricter tag validation)",
    )
    args = parser.parse_args()

    errors: list[str] = []

    # 1. Get Cargo versions for all publishable crates.
    try:
        metadata = cargo_metadata()
    except (subprocess.CalledProcessError, FileNotFoundError) as exc:
        print(f"FAIL: cargo metadata failed: {exc}", file=sys.stderr)
        return 1

    packages = {p["name"]: p for p in metadata.get("packages", [])}

    cargo_versions: dict[str, str] = {}
    for crate in PUBLISHABLE_CRATES:
        if crate not in packages:
            errors.append(f"Crate {crate!r} not found in workspace")
            continue
        cargo_versions[crate] = packages[crate]["version"]

    if errors:
        for e in errors:
            print(f"FAIL: {e}", file=sys.stderr)
        return 1

    # 2. Check all Cargo crate versions are identical.
    unique_cargo_versions = set(cargo_versions.values())
    if len(unique_cargo_versions) != 1:
        errors.append(
            f"Crate versions differ: "
            + ", ".join(f"{k}={v}" for k, v in sorted(cargo_versions.items()))
        )

    cargo_version = next(iter(unique_cargo_versions))

    # 3. Check for unreleasable placeholders.
    if PLACEHOLDER_RE.search(cargo_version):
        errors.append(f"Cargo version contains unreleasable placeholder: {cargo_version!r}")

    # 4. Check pyproject.toml version matches.
    pyproject_version = parse_pyproject_version(PYPROJECT_PATH)
    if pyproject_version is None:
        errors.append(f"Could not read version from {PYPROJECT_PATH}")
    elif pyproject_version != cargo_version:
        errors.append(
            f"pyproject.toml version {pyproject_version!r} != "
            f"Cargo version {cargo_version!r}"
        )

    # 5. Check eggfetch-python Cargo.toml version matches.
    py_cargo_version = parse_cargo_toml_version(EGGFETCH_PYTHON_CARGO_TOML)
    if py_cargo_version is None:
        errors.append(f"Could not read version from {EGGFETCH_PYTHON_CARGO_TOML}")
    elif py_cargo_version != cargo_version:
        errors.append(
            f"eggfetch-python Cargo version {py_cargo_version!r} != "
            f"workspace Cargo version {cargo_version!r}"
        )

    # 6. Tag validation (if --tag provided).
    if args.tag:
        tag = args.tag
        tag_match = re.match(r"^v(\d+\.\d+\.\d+(?:[+-].+)?)$", tag)
        if not tag_match:
            errors.append(f"Tag {tag!r} does not match v<SEMVER> pattern")
        else:
            tag_version = tag_match.group(1)
            if tag_version != cargo_version:
                errors.append(
                    f"Tag version {tag_version!r} != "
                    f"Cargo version {cargo_version!r}"
                )

        if args.publish:
            # Publishing mode: tag is required and must be exact.
            if not tag_match:
                errors.append("Publishing mode requires a valid v<SEMVER> tag")
            else:
                tag_version = tag_match.group(1)
                if tag_version != cargo_version:
                    errors.append(
                        f"Cannot publish: tag {tag!r} (version {tag_version!r}) "
                        f"does not match crate version {cargo_version!r}"
                    )
    else:
        # Non-publishing mode: branch/commit refs are allowed.
        # But if --publish is set without --tag, that's an error.
        if args.publish:
            errors.append("Publishing mode requires --tag")

    if errors:
        for e in errors:
            print(f"FAIL: {e}", file=sys.stderr)
        return 1

    # Print validated versions.
    for crate, ver in sorted(cargo_versions.items()):
        print(f"  {crate} version={ver}")
    if pyproject_version:
        print(f"  pyproject.toml version={pyproject_version}")
    if args.tag:
        print(f"  tag={args.tag}")

    print(f"\nAll versions consistent: {cargo_version}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
