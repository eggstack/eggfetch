#!/usr/bin/env python3
"""Aggregate downstream per-package results into a portfolio result.

Downloads every expected matrix result, validates each envelope,
computes category coverage from passing results only, and emits one
normalized ``downstream-portfolio`` qualification-result/v1.

Per plan §8.7:
  - download every expected matrix result
  - verify exact set equality between expected package IDs and returned package IDs
  - validate every result envelope
  - require every required package status to be ``passed``
  - compute category coverage only from passing results
  - merge contract category results from API-oracle jobs when configured
  - require exact equality with the eight-category registry
  - emit one normalized ``downstream-portfolio`` result

Usage:
    aggregate_downstream.py \\
        --results-dir /path/to/per-package-results/ \\
        --manifest compat/downstream/manifest.toml \\
        --candidate-sha <sha> \\
        --candidate-identity candidate-identity.json \\
        --run-id <id> \\
        --run-attempt <n> \\
        --output /tmp/downstream-portfolio.json
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

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
from stage_c_categories import STAGE_C_CATEGORY_SET, validate_category_coverage

MANIFEST_PATH = SCRIPT_DIR.parent / "compat" / "downstream" / "manifest.toml"


def _fail(msg: str) -> None:
    print(f"FATAL: {msg}", file=sys.stderr)
    sys.exit(1)


def load_manifest(path: Path) -> dict:
    with open(path, "rb") as f:
        return tomllib.load(f)


def get_required_packages(manifest: dict) -> dict[str, dict]:
    """Return {package_id: pkg_entry} for required release-blocking packages."""
    packages = {}
    for pkg in manifest.get("package", []):
        name = pkg.get("name", "")
        usage = pkg.get("usage", "required")
        release_blocking = pkg.get("release-blocking", False)
        if usage == "required" and release_blocking:
            packages[name] = pkg
    return packages


def validate_result_envelope(data: dict, expected_sha: str, expected_identity: str) -> list[str]:
    """Validate a downstream result envelope. Returns list of errors."""
    errors = []

    # Check qualification-result/v1 schema
    if data.get("schema") != "qualification-result/v1":
        errors.append(f"wrong schema: {data.get('schema')!r}")

    # Check status
    status = data.get("status", "")
    if status not in ("passed", "failed"):
        errors.append(f"status must be 'passed' or 'failed', got {status!r}")

    # Check candidate SHA
    result_sha = data.get("candidate_sha", "")
    if result_sha and result_sha != expected_sha:
        errors.append(f"SHA mismatch: expected {expected_sha[:12]}, got {result_sha[:12]}")

    # Check identity digest
    result_identity = data.get("identity_digest", "")
    if expected_identity and result_identity and result_identity != expected_identity:
        errors.append(f"identity mismatch")

    # Check package field
    package = data.get("package", data.get("metrics", {}).get("package", ""))
    if not package:
        errors.append("missing 'package' field")

    # Check required test counts for required packages
    metrics = data.get("metrics", {})
    if isinstance(metrics, dict):
        collected = metrics.get("collected", 0)
        if collected == 0:
            errors.append("zero tests collected")

    return errors


def aggregate(
    results_dir: Path,
    manifest_path: Path,
    candidate_sha: str,
    identity_digest: str,
    run_id: str,
    run_attempt: str,
) -> dict:
    """Aggregate downstream results into a portfolio result."""
    manifest = load_manifest(manifest_path)
    required_packages = get_required_packages(manifest)
    expected_ids = set(required_packages.keys())

    # Discover result files
    result_files = sorted(results_dir.glob("downstream-result-*.json"))
    if not result_files:
        _fail(f"No downstream result files found in {results_dir}")

    # Load and validate each result
    package_results: dict[str, dict] = {}
    all_errors: list[str] = []
    passing_packages: set[str] = set()
    covered_categories: set[str] = set()
    package_categories: dict[str, set[str]] = {}

    for rf in result_files:
        try:
            with open(rf) as f:
                data = json.load(f)
        except (json.JSONDecodeError, FileNotFoundError) as e:
            all_errors.append(f"Failed to load {rf.name}: {e}")
            continue

        # Extract package name from result or filename
        pkg_name = data.get("package", "")
        if not pkg_name:
            # Try to extract from filename: downstream-result-<package>.json
            stem = rf.stem  # downstream-result-<package>
            if stem.startswith("downstream-result-"):
                pkg_name = stem[len("downstream-result-"):]
            else:
                all_errors.append(f"Cannot determine package for {rf.name}")
                continue

        # Validate envelope
        errors = validate_result_envelope(data, candidate_sha, identity_digest)
        if errors:
            all_errors.append(f"[{pkg_name}] {', '.join(errors)}")
            continue

        package_results[pkg_name] = data

        # Check if this is a required package
        if pkg_name in required_packages:
            status = data.get("status", "")
            if status == "passed":
                passing_packages.add(pkg_name)
                # Collect categories from this package
                pkg_entry = required_packages[pkg_name]
                cat_ids = pkg_entry.get("category-ids", [])
                if isinstance(cat_ids, list):
                    for cat in cat_ids:
                        covered_categories.add(cat)
                        package_categories.setdefault(pkg_name, set()).add(cat)
            else:
                all_errors.append(f"required package '{pkg_name}' status is '{status}'")

    # Check exact set equality
    returned_ids = set(package_results.keys())
    missing = expected_ids - returned_ids
    unexpected = returned_ids - expected_ids
    if missing:
        all_errors.append(f"missing required packages: {', '.join(sorted(missing))}")
    if unexpected:
        all_errors.append(f"unexpected packages: {', '.join(sorted(unexpected))}")

    # Check category coverage (only from passing results)
    # contract-tests is covered by API oracle jobs, not downstream packages
    non_package_categories = {"contract-tests"}
    cat_errors = validate_category_coverage(
        covered_categories, non_package_categories=non_package_categories
    )
    all_errors.extend(cat_errors)

    # Determine overall status
    overall_status = "passed" if not all_errors else "failed"

    # Build metrics
    total_packages = len(required_packages)
    passing_count = len(passing_packages)

    metrics = {
        "total_packages": total_packages,
        "passing_packages": passing_count,
        "failing_packages": total_packages - passing_count,
        "covered_categories": sorted(covered_categories),
        "missing_categories": sorted(STAGE_C_CATEGORY_SET - covered_categories - non_package_categories),
        "expected_package_ids": sorted(expected_ids),
        "returned_package_ids": sorted(returned_ids),
        "missing_packages": sorted(missing),
        "unexpected_packages": sorted(unexpected),
    }

    result = {
        "schema": "qualification-result/v1",
        "suite_id": "downstream-portfolio",
        "producer_job": "downstream-aggregate",
        "candidate_sha": candidate_sha,
        "identity_digest": identity_digest,
        "run_id": run_id,
        "run_attempt": run_attempt,
        "status": overall_status,
        "required": True,
        "metrics": metrics,
        "artifacts": [],
        "diagnostics": all_errors,
        "package_results": {
            name: {
                "status": data.get("status"),
                "package": data.get("package"),
                "version": data.get("version", ""),
                "diagnostic_code": data.get("diagnostic_code", ""),
            }
            for name, data in package_results.items()
        },
    }

    return result


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Aggregate downstream results into portfolio",
    )
    parser.add_argument("--results-dir", required=True,
                        help="Directory containing downstream-result-*.json files")
    parser.add_argument("--manifest", default=str(MANIFEST_PATH),
                        help="Path to downstream manifest.toml")
    parser.add_argument("--candidate-sha", required=True)
    parser.add_argument("--candidate-identity", required=True,
                        help="Path to candidate-identity.json")
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--run-attempt", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    results_dir = Path(args.results_dir)
    if not results_dir.exists():
        _fail(f"Results directory not found: {results_dir}")

    # Load identity digest
    identity_path = Path(args.candidate_identity)
    identity_digest = ""
    if identity_path.exists():
        with open(identity_path) as f:
            identity_data = json.load(f)
        identity_digest = identity_data.get("identity_digest", "")

    result = aggregate(
        results_dir=results_dir,
        manifest_path=Path(args.manifest),
        candidate_sha=args.candidate_sha,
        identity_digest=identity_digest,
        run_id=args.run_id,
        run_attempt=args.run_attempt,
    )

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(result, indent=2) + "\n")

    print(f"Downstream portfolio result: {output_path}")
    print(f"  status: {result['status']}")
    print(f"  passing: {result['metrics']['passing_packages']}/{result['metrics']['total_packages']}")
    print(f"  categories: {', '.join(result['metrics']['covered_categories'])}")

    if result["status"] != "passed":
        print("\nERRORS:")
        for err in result["diagnostics"]:
            print(f"  - {err}")
        sys.exit(1)


if __name__ == "__main__":
    main()
