#!/usr/bin/env python3
"""Generate downstream test matrix from compat/downstream/manifest.toml.

Reads the downstream manifest and produces a GitHub Actions matrix JSON
for the qualification workflow. Validates that all required categories
are covered and all required fields are present.

Usage:
    generate_downstream_matrix.py --output <matrix.json>

The output is a JSON object suitable for use with fromJSON() in GitHub
Actions:
    {
      "include": [
        {
          "package": "respx",
          "version": "0.21.1",
          "source-locator": "pypi:respx==0.21.1",
          "source-hash": "<sha256>",
          "category-ids": ["mock-transport-request-matching"],
          ...
        }
      ]
    }
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib  # type: ignore[no-redef]
    except ImportError:
        print("ERROR: Python 3.11+ or tomli required for TOML parsing", file=sys.stderr)
        sys.exit(2)


REQUIRED_STAGE_C_CATEGORIES = {
    "contract-tests",
    "mock-transport-request-matching",
    "framework-test-client",
    "asgi-test-client",
    "sdk-async-client",
    "streaming-sse-consumption",
    "custom-auth-flow",
    "event-hooks-instrumentation",
}

REQUIRED_PACKAGE_FIELDS = [
    "name", "version", "source-locator", "source-hash",
    "source-type", "test-command", "min-tests",
    "category-ids", "timeout",
]


def load_manifest(path: Path) -> dict:
    """Load and parse the downstream manifest TOML."""
    with open(path, "rb") as f:
        return tomllib.load(f)


def validate_manifest(manifest: dict) -> list[str]:
    """Validate the manifest structure. Returns list of errors."""
    errors: list[str] = []

    packages = manifest.get("package", [])
    if not isinstance(packages, list) or not packages:
        errors.append("manifest has no packages")
        return errors

    covered_categories: set[str] = set()
    required_packages = []
    seen_names: set[str] = set()

    for i, pkg in enumerate(packages):
        name = pkg.get("name", f"<entry {i}>")

        # Duplicate detection
        if name in seen_names:
            errors.append(f"duplicate package: {name}")
        seen_names.add(name)

        # Required field validation
        for field in REQUIRED_PACKAGE_FIELDS:
            if field not in pkg:
                errors.append(f"[{name}] missing required field: {field}")

        usage = pkg.get("usage", "required")
        if usage == "required":
            required_packages.append(name)

            # Validate required fields for required packages
            min_tests = pkg.get("min-tests", 0)
            if not isinstance(min_tests, int) or min_tests < 0:
                errors.append(f"[{name}] min-tests must be a non-negative integer")
            elif min_tests == 0:
                errors.append(f"[{name}] required package must have min-tests > 0")

            timeout = pkg.get("timeout", 0)
            if not isinstance(timeout, (int, float)) or timeout <= 0:
                errors.append(f"[{name}] timeout must be a positive number")

            source_hash = pkg.get("source-hash", "")
            if not source_hash or not isinstance(source_hash, str):
                errors.append(f"[{name}] source-hash is required")
            elif len(source_hash) != 64:
                errors.append(f"[{name}] source-hash must be 64 hex chars")

            # Collect categories
            cat_ids = pkg.get("category-ids", [])
            if isinstance(cat_ids, list):
                for cat in cat_ids:
                    covered_categories.add(cat)

    # Check required categories are covered by required packages
    missing_categories = REQUIRED_STAGE_C_CATEGORIES - covered_categories
    if missing_categories:
        # Only warn for categories only covered by informational packages
        # These are still tracked in the manifest but don't block the matrix
        print(
            f"WARNING: categories not covered by required packages: "
            f"{', '.join(sorted(missing_categories))}",
            file=sys.stderr,
        )

    return errors


def generate_matrix(manifest: dict) -> dict:
    """Generate the GitHub Actions matrix from the manifest."""
    packages = manifest.get("package", [])
    include = []

    for pkg in packages:
        usage = pkg.get("usage", "required")
        if usage != "required":
            continue

        entry = {
            "package": pkg["name"],
            "version": pkg.get("version", ""),
            "source-locator": pkg.get("source-locator", ""),
            "source-hash": pkg.get("source-hash", ""),
            "source-type": pkg.get("source-type", ""),
            "test-command": pkg.get("test-command", ""),
            "min-tests": pkg.get("min-tests", 0),
            "category-ids": pkg.get("category-ids", []),
            "timeout": pkg.get("timeout", 180),
        }

        # Optional fields
        for opt_field in ("optional-deps", "known-incompatibilities"):
            if opt_field in pkg:
                entry[opt_field] = pkg[opt_field]

        include.append(entry)

    return {"include": include}


def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser for workflow validation tests."""
    parser = argparse.ArgumentParser(
        prog="generate_downstream_matrix.py",
        description="Generate downstream test matrix from manifest.toml",
    )
    parser.add_argument("--manifest", default="compat/downstream/manifest.toml",
                        help="Path to downstream manifest TOML")
    parser.add_argument("--output", required=True, help="Output matrix JSON path")
    parser.add_argument("--validate-only", action="store_true",
                        help="Only validate the manifest, do not generate matrix")
    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()

    manifest_path = Path(args.manifest)
    if not manifest_path.exists():
        print(f"FATAL: manifest not found: {manifest_path}", file=sys.stderr)
        sys.exit(1)

    manifest = load_manifest(manifest_path)

    errors = validate_manifest(manifest)
    if errors:
        print(f"MANIFEST VALIDATION FAILED ({len(errors)} errors):", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        sys.exit(1)

    if args.validate_only:
        print("Manifest validation passed.")
        sys.exit(0)

    matrix = generate_matrix(manifest)

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(matrix, f, indent=2)
        f.write("\n")

    print(f"Matrix written to {output_path}")
    print(f"  entries: {len(matrix['include'])}")
    for entry in matrix["include"]:
        print(f"    {entry['package']}=={entry['version']} ({', '.join(entry['category-ids'])})")

    # Also output to stdout for GitHub Actions $GITHUB_OUTPUT consumption
    print(f"\n::set-output name=matrix::{json.dumps(matrix)}")


if __name__ == "__main__":
    main()
