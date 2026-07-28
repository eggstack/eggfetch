#!/usr/bin/env python3
"""Generate downstream test matrix from compat/downstream/manifest.toml.

Reads the downstream manifest and produces a GitHub Actions matrix JSON
for the qualification workflow. Validates that all required categories
are covered by release-blocking packages and all required fields are present.

Usage:
    generate_downstream_matrix.py --output <matrix.json>
    generate_downstream_matrix.py --validate-only

The output matrix contains only identifiers and normalized scalar fields.
Test commands, dependency lists, and other runtime details are resolved
by the downstream runner from the manifest, not embedded in the matrix.

Matrix schema:
    {
      "include": [
        {
          "package_id": "respx",
          "version": "0.21.1",
          "source_sha256": "<hash>",
          "timeout_seconds": 60
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


from stage_c_categories import STAGE_C_CATEGORY_SET, validate_category_coverage


def load_manifest(path: Path) -> dict:
    """Load and parse the downstream manifest TOML."""
    with open(path, "rb") as f:
        return tomllib.load(f)


def validate_manifest(manifest: dict) -> list[str]:
    """Validate the manifest structure. Returns list of errors.

    Missing required categories are errors, not warnings.
    """
    errors: list[str] = []

    packages = manifest.get("package", [])
    if not isinstance(packages, list) or not packages:
        errors.append("manifest has no packages")
        return errors

    covered_categories: set[str] = set()
    seen_names: set[str] = set()

    for i, pkg in enumerate(packages):
        name = pkg.get("name", f"<entry {i}>")

        if name in seen_names:
            errors.append(f"duplicate package: {name}")
        seen_names.add(name)

        # Required field validation
        for field in ("name", "version", "source-hash", "category-ids", "timeout"):
            if field not in pkg:
                errors.append(f"[{name}] missing required field: {field}")

        usage = pkg.get("usage", "required")
        if usage == "required":
            min_tests = pkg.get("min-tests", 0)
            if not isinstance(min_tests, int) or min_tests <= 0:
                errors.append(f"[{name}] required package must have min-tests > 0")

            timeout = pkg.get("timeout", 0)
            if not isinstance(timeout, (int, float)) or timeout <= 0:
                errors.append(f"[{name}] timeout must be a positive number")

            source_hash = pkg.get("source-hash", "")
            if not source_hash or not isinstance(source_hash, str):
                errors.append(f"[{name}] source-hash is required")
            elif len(source_hash) != 64:
                errors.append(f"[{name}] source-hash must be 64 hex chars")

            # Collect categories from release-blocking packages only
            release_blocking = pkg.get("release-blocking", False)
            if release_blocking:
                cat_ids = pkg.get("category-ids", [])
                if isinstance(cat_ids, list):
                    for cat in cat_ids:
                        covered_categories.add(cat)

    # Categories covered by non-package jobs (API oracles, etc.)
    _NON_PACKAGE_CATEGORIES = {"contract-tests"}

    # Check required categories are covered by release-blocking packages
    # or by known non-package producers
    cat_errors = validate_category_coverage(
        covered_categories, non_package_categories=_NON_PACKAGE_CATEGORIES
    )
    errors.extend(cat_errors)

    return errors


def generate_matrix(manifest: dict) -> dict:
    """Generate the GitHub Actions matrix from the manifest.

    Output contains only identifiers and scalar fields.
    Test commands and dependency details are resolved by the runner.
    """
    packages = manifest.get("package", [])
    include = []

    for pkg in packages:
        usage = pkg.get("usage", "required")
        if usage != "required":
            continue

        release_blocking = pkg.get("release-blocking", False)
        if not release_blocking:
            continue

        entry = {
            "package_id": pkg["name"],
            "version": pkg.get("version", ""),
            "source_sha256": pkg.get("source-hash", ""),
            "timeout_seconds": pkg.get("timeout", 180),
        }

        include.append(entry)

    return {"include": include}


def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser."""
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
        print(f"    {entry['package_id']}=={entry['version']}")

    # Compact single-line JSON for $GITHUB_OUTPUT
    print(json.dumps(matrix, separators=(',', ':')))


if __name__ == "__main__":
    main()
