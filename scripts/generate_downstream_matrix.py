#!/usr/bin/env python3
"""Generate a GitHub Actions matrix from the downstream compatibility manifest.

Reads ``compat/downstream/manifest.toml``, selects ``release-blocking=true``
packages, validates eight-category coverage, and emits stable sorted matrix
JSON suitable for ``fromJSON(needs.prepare-downstream-matrix.outputs.matrix)``.

Usage:
    generate_downstream_matrix.py --manifest <path> --output <path.json>

Exit codes:
    0 — matrix generated successfully
    1 — validation error (missing categories, duplicates, etc.)
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib  # type: ignore[no-redef]

SCRIPT_DIR = Path(__file__).resolve().parent
DEFAULT_MANIFEST = SCRIPT_DIR.parent / "compat" / "downstream" / "manifest.toml"

# The eight declared Stage C consumer categories
REQUIRED_CATEGORIES = [
    "contract-tests",
    "mock-transport-request-matching",
    "framework-test-client",
    "asgi-test-client",
    "sdk-async-client",
    "streaming-sse-consumption",
    "custom-auth-flow",
    "event-hooks-instrumentation",
]


def load_manifest(path: Path) -> dict:
    """Load and parse the manifest TOML file."""
    if not path.exists():
        print(f"FATAL: manifest not found: {path}", file=sys.stderr)
        sys.exit(1)
    with open(path, "rb") as f:
        return tomllib.load(f)


def validate_and_select(data: dict) -> list[dict]:
    """Validate the manifest and return the list of release-blocking packages.

    Raises ``ValueError`` on any validation failure.
    """
    errors: list[str] = []

    portfolio = data.get("portfolio", {})
    schema_version = portfolio.get("schema-version", "1")
    if schema_version not in ("1", "2"):
        errors.append(f"portfolio.schema-version must be '1' or '2', got '{schema_version}'")

    packages = data.get("package", [])
    if not packages:
        errors.append("No [[package]] entries found")
        raise ValueError("; ".join(errors))

    # Check for duplicate names
    names = [p.get("name", "<unnamed>") for p in packages]
    seen: set[str] = set()
    duplicates: set[str] = set()
    for name in names:
        if name in seen:
            duplicates.add(name)
        seen.add(name)
    if duplicates:
        errors.append(f"Duplicate package names: {sorted(duplicates)}")

    # Select release-blocking packages
    blocking = [p for p in packages if p.get("release-blocking") is True]
    if not blocking:
        errors.append("No release-blocking packages found")
        raise ValueError("; ".join(errors))

    # Validate each release-blocking package has required fields
    required_fields = {
        "name", "version", "source-locator", "source-hash",
        "category-ids", "test-command", "install-command",
        "timeout", "min-collected", "min-passed",
        "max-skipped", "max-xfailed",
    }
    for pkg in blocking:
        name = pkg.get("name", "<unnamed>")
        missing = required_fields - set(pkg.keys())
        if missing:
            errors.append(f"{name}: missing required fields: {sorted(missing)}")

    # Validate category coverage
    covered_categories: set[str] = set()
    for pkg in blocking:
        for cat in pkg.get("category-ids", []):
            covered_categories.add(cat)

    missing_categories = set(REQUIRED_CATEGORIES) - covered_categories
    if missing_categories:
        errors.append(
            f"Missing required Stage C categories: {sorted(missing_categories)}"
        )

    # Check for unknown categories
    all_categories = covered_categories | {
        cat for pkg in packages for cat in pkg.get("category-ids", [])
    }
    known_categories = set(REQUIRED_CATEGORIES) | {
        "sdk-sync-client", "custom-transport-subclass",
        "async-testing-support", "heavy-config-user",
    }
    unknown = all_categories - known_categories
    if unknown:
        errors.append(f"Unknown categories in manifest: {sorted(unknown)}")

    if errors:
        raise ValueError("; ".join(errors))

    return blocking


def generate_matrix(packages: list[dict]) -> dict:
    """Generate a stable sorted GitHub Actions matrix from packages."""
    # Sort by package name for deterministic output
    sorted_packages = sorted(packages, key=lambda p: p["name"])

    matrix = {
        "include": [
            {
                "package": pkg["name"],
                "version": pkg["version"],
                "category-ids": pkg.get("category-ids", []),
                "source-locator": pkg.get("source-locator", ""),
                "source-hash": pkg.get("source-hash", ""),
                "test-command": pkg.get("test-command", ""),
                "install-command": pkg.get("install-command", ""),
                "timeout": pkg.get("timeout", 120),
                "min-collected": pkg.get("min-collected", 1),
                "min-passed": pkg.get("min-passed", 1),
                "max-skipped": pkg.get("max-skipped", 0),
                "max-xfailed": pkg.get("max-xfailed", 0),
            }
            for pkg in sorted_packages
        ]
    }
    return matrix


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate GitHub Actions downstream matrix from manifest",
    )
    parser.add_argument(
        "--manifest", default=str(DEFAULT_MANIFEST),
        help="Path to downstream manifest.toml",
    )
    parser.add_argument(
        "--output", required=True,
        help="Output matrix JSON path",
    )
    args = parser.parse_args()

    data = load_manifest(Path(args.manifest))
    packages = validate_and_select(data)
    matrix = generate_matrix(packages)

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(matrix, f, indent=2)
        f.write("\n")

    print(f"Matrix written to {output_path}")
    print(f"  packages: {len(matrix['include'])}")
    for entry in matrix["include"]:
        print(f"    {entry['package']}: {entry['version']}")


if __name__ == "__main__":
    main()
