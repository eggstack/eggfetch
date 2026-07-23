#!/usr/bin/env python3
"""Validate downstream compatibility by running actual downstream test suites.

Reads compat/downstream/manifest.toml, validates each entry structure,
runs downstream package tests in isolated environments against the eggfetch
wheel, and reports results as JSON.

Usage:
    run_downstream_compat.py --wheel-dir <dir> [--packages pkg1,pkg2] [--timeout <seconds>]

Exit codes:
    0 — all required packages passed
    1 — one or more required packages failed
    2 — argument or manifest error
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
MANIFEST_PATH = SCRIPT_DIR.parent / "compat" / "downstream" / "manifest.toml"
ISOLATED_RUNNER = SCRIPT_DIR / "run_isolated_downstream.py"

REQUIRED_FIELDS = {
    "name",
    "version",
    "license",
    "category",
    "rationale",
    "usage",
    "test-subset",
    "expected-network-isolation",
    "optional-dependencies",
    "known-incompatibilities",
    "update-owner",
    "review-cadence",
}

VALID_CATEGORIES = {
    "contract-tests",
    "mock-transport-user",
    "framework-test-client",
    "framework-asgi-transport",
    "sdk-async-client",
    "sdk-sync-client",
    "streaming-upload-download",
    "custom-transport-subclass",
    "async-testing-support",
    "custom-auth-flow",
    "event-hook-instrumentation",
    "heavy-config-user",
}


def validate_manifest(path: Path) -> dict:
    """Load and validate the manifest structure. Returns a result dict."""
    import tomllib

    errors = []

    if not path.exists():
        return {"status": "error", "errors": [f"Manifest not found: {path}"], "packages": []}

    with open(path, "rb") as f:
        data = tomllib.load(f)

    portfolio = data.get("portfolio", {})
    if portfolio.get("schema-version") != "1":
        errors.append(f"portfolio.schema-version must be '1', got '{portfolio.get('schema-version')}'")
    if portfolio.get("status") != "phase-5":
        errors.append(f"portfolio.status must be 'phase-5', got '{portfolio.get('status')}'")

    ref_profile = portfolio.get("reference-profile", "")
    if ref_profile:
        ref_path = path.parent / ref_profile
        if not ref_path.exists():
            errors.append(f"reference-profile not found: {ref_path}")

    packages = data.get("package", [])
    if not packages:
        errors.append("No [[package]] entries found")

    seen_names = set()
    for pkg in packages:
        name = pkg.get("name", "<unnamed>")
        missing = REQUIRED_FIELDS - set(pkg.keys())
        if missing:
            errors.append(f"{name}: missing fields {missing}")
        if name in seen_names:
            errors.append(f"{name}: duplicate package name")
        seen_names.add(name)

        version = pkg.get("version", "")
        if not version or not any(c.isdigit() for c in str(version)):
            errors.append(f"{name}: version not pinned or invalid: '{version}'")

        cat = pkg.get("category", "")
        if cat not in VALID_CATEGORIES:
            errors.append(f"{name}: unknown category '{cat}'")

    return {
        "status": "valid" if not errors else "invalid",
        "errors": errors,
        "packages": packages,
    }


def run_downstream_test(package_name: str, wheel_dir: Path, timeout: int) -> dict:
    """Run a single downstream package test via run_isolated_downstream.py."""
    cmd = [
        sys.executable,
        str(ISOLATED_RUNNER),
        "--package", package_name,
        "--wheel-dir", str(wheel_dir),
        "--timeout", str(timeout),
    ]

    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout + 30,  # Extra buffer for venv creation
        )

        # Parse the JSON output from the runner
        stdout = result.stdout.strip()
        if stdout:
            try:
                return json.loads(stdout)
            except json.JSONDecodeError:
                return {
                    "package": package_name,
                    "status": "error",
                    "error": f"Could not parse runner output: {stdout[:500]}",
                    "returncode": result.returncode,
                }

        return {
            "package": package_name,
            "status": "error",
            "error": f"No output from runner. stderr: {result.stderr[:500]}",
            "returncode": result.returncode,
        }

    except subprocess.TimeoutExpired:
        return {
            "package": package_name,
            "status": "timeout",
            "error": f"Runner timed out after {timeout + 30}s",
        }
    except Exception as exc:
        return {
            "package": package_name,
            "status": "error",
            "error": str(exc),
        }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run downstream compatibility tests against eggfetch wheel",
    )
    parser.add_argument(
        "--wheel-dir", required=True,
        help="Directory containing eggfetch .whl files",
    )
    parser.add_argument(
        "--packages", default=None,
        help="Comma-separated list of packages to test (default: all in manifest)",
    )
    parser.add_argument(
        "--timeout", type=int, default=120,
        help="Per-package timeout in seconds (default: 120)",
    )
    parser.add_argument(
        "--required-only", action="store_true",
        help="Only test packages with usage=public (required for Stage C)",
    )
    args = parser.parse_args()

    wheel_dir = Path(args.wheel_dir).resolve()
    if not wheel_dir.exists():
        print(json.dumps({"status": "error", "message": f"Wheel directory not found: {wheel_dir}"}))
        return 2

    manifest_result = validate_manifest(MANIFEST_PATH)
    if manifest_result["status"] == "invalid":
        print(json.dumps({
            "status": "error",
            "errors": manifest_result["errors"],
        }, indent=2))
        return 2

    packages = manifest_result["packages"]
    if args.packages:
        selected = set(args.packages.split(","))
        packages = [p for p in packages if p["name"] in selected]

    if args.required_only:
        packages = [p for p in packages if p.get("usage") == "public"]

    results = []
    for pkg in packages:
        print(f"Testing {pkg['name']}...", file=sys.stderr)
        result = run_downstream_test(pkg["name"], wheel_dir, args.timeout)
        results.append(result)

    # Build summary
    passed = [r for r in results if r.get("status") == "passed"]
    failed = [r for r in results if r.get("status") == "failed"]
    skipped = [r for r in results if r.get("status") in ("skipped", "skipped-no-tests")]
    errors = [r for r in results if r.get("status") in ("error", "timeout", "install-failed", "downstream-install-failed")]

    summary = {
        "manifest_status": manifest_result["status"],
        "total_packages": len(results),
        "passed": len(passed),
        "failed": len(failed),
        "skipped": len(skipped),
        "errors": len(errors),
        "overall_pass": len(failed) == 0 and len(errors) == 0,
        "results": results,
    }

    print(json.dumps(summary, indent=2))

    if failed or errors:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
