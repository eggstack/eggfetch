#!/usr/bin/env python3
"""Validate downstream compatibility by running actual downstream test suites.

Reads compat/downstream/manifest.toml, validates each entry structure,
runs downstream package tests in isolated environments against the eggfetch
wheel, and reports results as JSON.

Usage:
    run_downstream_compat.py --artifact-manifest <manifest.json> [--packages pkg1,pkg2] [--timeout <seconds>]

Exit codes:
    0 — all required packages passed
    1 — one or more required packages failed
    2 — argument or manifest error
    3 — structured diagnostic error
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
    "min-tests",
    "test-command",
    "source-type",
    "source-locator",
    "source-hash",
    "python-versions",
    "public-httpx-api",
    "install-command",
    "test-working-dir",
    "test-result-format",
    "min-collected",
    "max-skipped",
    "max-xfailed",
    "timeout",
    "network-policy",
    "category-ids",
    "release-blocking",
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

VALID_USAGES = {"required", "informational", "private", "public"}

# Import shared Stage C category registry instead of duplicating.
from stage_c_categories import STAGE_C_CATEGORIES as _SHARED_STAGE_C_CATEGORIES

STAGE_C_CATEGORIES = _SHARED_STAGE_C_CATEGORIES


def _emit_result(result: dict, output_path: str | None = None) -> None:
    """Write structured result to file or stdout."""
    payload = json.dumps(result, indent=2)
    if output_path:
        Path(output_path).write_text(payload)
    else:
        print(payload)


def validate_manifest(path: Path) -> dict:
    """Load and validate the manifest structure. Returns a result dict."""
    import tomllib

    errors = []

    if not path.exists():
        return {"status": "error", "errors": [f"Manifest not found: {path}"], "packages": []}

    with open(path, "rb") as f:
        data = tomllib.load(f)

    portfolio = data.get("portfolio", {})
    schema_version = portfolio.get("schema-version", "1")
    if schema_version not in ("1", "2"):
        errors.append(f"portfolio.schema-version must be '1' or '2', got '{schema_version}'")
    if schema_version == "2" and portfolio.get("status") != "phase-6":
        errors.append(f"portfolio.status must be 'phase-6' for schema v2, got '{portfolio.get('status')}'")
    if schema_version == "1" and portfolio.get("status") != "phase-5":
        errors.append(f"portfolio.status must be 'phase-5' for schema v1, got '{portfolio.get('status')}'")

    ref_profile = portfolio.get("reference-profile", "")
    if ref_profile:
        ref_path = path.parent / ref_profile
        if not ref_path.exists():
            errors.append(f"reference-profile not found: {ref_path}")

    packages = data.get("package", [])
    if not packages:
        errors.append("No [[package]] entries found")

    # For schema v2, require behavioral test fixtures directory
    if schema_version == "2":
        fixtures_dir = path.parent / "behavioral_fixtures"
        if not fixtures_dir.exists():
            errors.append(f"Schema v2 requires behavioral_fixtures directory: {fixtures_dir}")

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

        usage = pkg.get("usage", "")
        if usage not in VALID_USAGES:
            errors.append(f"{name}: unknown usage '{usage}'")

        min_tests = pkg.get("min-tests")
        if min_tests is None:
            errors.append(f"{name}: missing min-tests field")
        elif not isinstance(min_tests, int) or min_tests < 0:
            errors.append(f"{name}: min-tests must be a non-negative integer, got '{min_tests}'")

        # Validate source-type
        source_type = pkg.get("source-type", "")
        if source_type and source_type not in ("pypi", "git", "local"):
            errors.append(f"{name}: invalid source-type '{source_type}'")

        # Validate test-result-format
        trf = pkg.get("test-result-format", "")
        if trf and trf not in ("exit-code", "junit-xml", "pytest-json"):
            errors.append(f"{name}: invalid test-result-format '{trf}'")

        # Validate category-ids is a list
        cat_ids = pkg.get("category-ids", [])
        if not isinstance(cat_ids, list):
            errors.append(f"{name}: category-ids must be a list")

        # Validate max-skipped and max-xfailed for required packages
        if usage == "required":
            max_skip = pkg.get("max-skipped", -1)
            if not isinstance(max_skip, int) or max_skip < 0:
                errors.append(f"{name}: required package must have non-negative max-skipped, got '{max_skip}'")
            max_xf = pkg.get("max-xfailed", -1)
            if not isinstance(max_xf, int) or max_xf < 0:
                errors.append(f"{name}: required package must have non-negative max-xfailed, got '{max_xf}'")

            # Required packages must have test-command
            if not pkg.get("test-command", ""):
                errors.append(f"{name}: required package missing test-command")

            # Required packages must have source-hash
            if not pkg.get("source-hash", ""):
                errors.append(f"{name}: required package missing source-hash")

    return {
        "status": "valid" if not errors else "invalid",
        "errors": errors,
        "packages": packages,
    }


def validate_wheel_dir(wheel_dir: Path) -> list[str]:
    """Validate that the wheel directory contains both required wheels."""
    errors = []
    if not wheel_dir.exists():
        errors.append(f"Wheel directory does not exist: {wheel_dir}")
        return errors

    wheels = list(wheel_dir.glob("*.whl"))
    if not wheels:
        errors.append(f"No .whl files found in {wheel_dir}")
        return errors

    names = [w.name.lower() for w in wheels]
    has_eggfetch = any(n.startswith("eggfetch-") for n in names)
    has_httpx = any(n.startswith("httpx-") for n in names)

    if not has_eggfetch:
        errors.append(f"No eggfetch wheel found in {wheel_dir}")
    if not has_httpx:
        errors.append(f"No httpx controlled replacement wheel found in {wheel_dir}")

    return errors


def run_downstream_test(package_name: str, artifact_manifest_path: Path, timeout: int,
                        output_dir: Path | None = None,
                        candidate_identity_path: Path | None = None) -> dict:
    """Run a single downstream package test via run_isolated_downstream.py."""
    cmd = [
        sys.executable,
        str(ISOLATED_RUNNER),
        "--package", package_name,
        "--artifact-manifest", str(artifact_manifest_path),
        "--timeout", str(timeout),
    ]
    if candidate_identity_path:
        cmd.extend(["--candidate-identity", str(candidate_identity_path)])
    if output_dir:
        output_dir.mkdir(parents=True, exist_ok=True)
        result_file = output_dir / f"{package_name}.json"
        cmd.extend(["--output", str(result_file)])

    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout + 60,  # Extra buffer for venv creation + install
        )

        stdout = result.stdout.strip()
        if stdout:
            try:
                parsed = json.loads(stdout)
                # Ensure package name is set
                if "package" not in parsed:
                    parsed["package"] = package_name
                return parsed
            except json.JSONDecodeError:
                return {
                    "package": package_name,
                    "status": "error",
                    "diagnostic_code": "E014",
                    "diagnostic_name": "malformed-result",
                    "error": f"Could not parse runner output: {stdout[:500]}",
                    "returncode": result.returncode,
                }

        return {
            "package": package_name,
            "status": "error",
            "diagnostic_code": "E014",
            "diagnostic_name": "malformed-result",
            "error": f"No output from runner. stderr: {result.stderr[:500]}",
            "returncode": result.returncode,
        }

    except subprocess.TimeoutExpired:
        return {
            "package": package_name,
            "status": "timeout",
            "error": f"Runner timed out after {timeout + 60}s",
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
        "--artifact-manifest", required=True,
        help="Path to artifact-manifest.json listing built wheels with hashes",
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
        help="Only test packages with usage=required",
    )
    parser.add_argument(
        "--candidate-identity", default=None,
        help="Path to candidate-identity.json for identity propagation",
    )
    parser.add_argument(
        "--output", default=None,
        help="Path to write aggregate result JSON (default: stdout)",
    )
    args = parser.parse_args()

    artifact_manifest_path = Path(args.artifact_manifest).resolve()
    if not artifact_manifest_path.exists():
        _emit_result({
            "status": "error",
            "errors": [f"Artifact manifest not found: {artifact_manifest_path}"],
        }, args.output)
        return 2

    with open(artifact_manifest_path) as f:
        artifact_manifest = json.load(f)

    # Validate artifact manifest has required wheels
    eggfetch_found = False
    httpx_found = False
    for art in artifact_manifest.get("artifacts", []):
        # Support schema v3 (role) and schema v2 (artifact_type)
        art_type = art.get("role", art.get("artifact_type", ""))
        # Support schema v3 (relative_path) and schema v2 (path)
        path = art.get("relative_path", art.get("path", ""))
        if art_type == "eggfetch" and path:
            eggfetch_found = True
        if art_type == "httpx-controlled-replacement" and path:
            httpx_found = True

    if not eggfetch_found or not httpx_found:
        missing = []
        if not eggfetch_found:
            missing.append("eggfetch wheel")
        if not httpx_found:
            missing.append("httpx-controlled-replacement wheel")
        _emit_result({
            "status": "error",
            "errors": [f"Artifact manifest missing required wheels: {', '.join(missing)}"],
        }, args.output)
        return 2

    # Load candidate identity if provided
    candidate_identity_path = None
    if args.candidate_identity:
        candidate_identity_path = Path(args.candidate_identity).resolve()
        if not candidate_identity_path.exists():
            _emit_result({
                "status": "error",
                "errors": [f"Candidate identity not found: {candidate_identity_path}"],
            }, args.output)
            return 2

    manifest_result = validate_manifest(MANIFEST_PATH)
    if manifest_result["status"] == "invalid":
        _emit_result({
            "status": "error",
            "errors": manifest_result["errors"],
        }, args.output)
        return 2

    packages = manifest_result["packages"]
    if args.packages:
        selected = set(args.packages.split(","))
        packages = [p for p in packages if p["name"] in selected]
        # Fail-closed: empty selection from explicit --packages is an error
        if not packages:
            _emit_result({
                "status": "error",
                "diagnostic_code": "E002",
                "diagnostic_name": "empty-selection",
                "errors": [f"No packages matched selection: {args.packages}"],
            }, args.output)
            return 3

    if args.required_only:
        packages = [p for p in packages if p.get("usage") == "required"]

    # Fail-closed: empty package list after filtering is an error
    if not packages:
        _emit_result({
            "status": "error",
            "diagnostic_code": "E002",
            "diagnostic_name": "empty-selection",
            "errors": ["No packages to test after filtering (fail-closed)"],
        }, args.output)
        return 3

    # Validate matrix equals manifest
    manifest_names = {p["name"] for p in manifest_result["packages"]}
    selected_names = {p["name"] for p in packages}
    if args.packages:
        requested = set(args.packages.split(","))
        missing_from_manifest = requested - manifest_names
        if missing_from_manifest:
            _emit_result({
                "status": "error",
                "diagnostic_code": "E001",
                "diagnostic_name": "unknown-package",
                "errors": [f"Packages not in manifest: {', '.join(sorted(missing_from_manifest))}"],
            }, args.output)
            return 3

    # Validate that required entries have real behavioral tests
    required_packages = [p for p in packages if p.get("usage") == "required"]
    validation_errors = []
    for pkg in required_packages:
        cmd = pkg.get("test-command", "")
        if not cmd:
            validation_errors.append(
                f"{pkg['name']}: required package missing test-command"
            )
        elif _is_import_only_command(cmd):
            # Import-only smoke test for a required package
            validation_errors.append(
                f"{pkg['name']}: required package has import-only test-command, "
                f"expected behavioral test"
            )
        # Validate max-skipped = 0 for required packages
        max_skip = pkg.get("max-skipped", -1)
        if max_skip != 0:
            validation_errors.append(
                f"{pkg['name']}: required package must have max-skipped = 0, got {max_skip}"
            )
        # Validate max-xfailed = 0 for required packages
        max_xf = pkg.get("max-xfailed", -1)
        if max_xf != 0:
            validation_errors.append(
                f"{pkg['name']}: required package must have max-xfailed = 0, got {max_xf}"
            )
    if validation_errors:
        _emit_result({
            "status": "error",
            "errors": validation_errors,
        }, args.output)
        return 2

    results = []
    results_dir = Path(args.output).parent / "results" if args.output else None
    for pkg in packages:
        print(f"Testing {pkg['name']}...", file=sys.stderr)
        result = run_downstream_test(
            pkg["name"], artifact_manifest_path, args.timeout, results_dir,
            candidate_identity_path,
        )
        results.append(result)

    # Build summary
    passed = [r for r in results if r.get("status") == "passed"]
    failed = [r for r in results if r.get("status") == "failed"]
    skipped = [r for r in results if r.get("status") in ("skipped", "skipped-no-tests")]
    errors = [r for r in results if r.get("status") in (
        "error", "timeout", "install-failed", "downstream-install-failed",
        "shim-identity-failure", "upstream-httpx-detected", "pip-check-failure",
        "below-min-tests", "below-min-count", "zero-tests", "zero-tests-expected",
        "zero-tests-required", "skipped-required", "xfailed-required",
        "source-hash-mismatch", "source-hash-missing",
        "identity-mismatch", "artifact-mismatch",
    )]

    # Determine overall pass/fail: required packages must pass
    required_packages = {p["name"] for p in packages if p.get("usage") == "required"}
    required_results = [r for r in results if r.get("package") in required_packages]

    # Required results must be exactly "passed" — skipped and skipped-no-tests are failures
    required_failed = [r for r in required_results if r.get("status") != "passed"]

    summary = {
        "manifest_status": manifest_result["status"],
        "schema_version": manifest_result["packages"][0].get("source-type", "") if manifest_result["packages"] else "",
        "artifact_manifest": str(artifact_manifest_path),
        "candidate_identity": str(candidate_identity_path) if candidate_identity_path else None,
        "total_packages": len(results),
        "passed": len(passed),
        "failed": len(failed),
        "skipped": len(skipped),
        "errors": len(errors),
        "required_total": len(required_packages),
        "required_passed": len(required_packages) - len(required_failed),
        "required_failed": len(required_failed),
        "required_missing_behavioral": len([
            r for r in required_failed if r.get("status") in ("zero-tests-required", "below-min-tests", "below-min-count")
        ]),
        "overall_pass": len(required_failed) == 0 and len(errors) == 0,
        "results": results,
    }

    _emit_result(summary, args.output)

    if required_failed or errors:
        return 1
    return 0


def _is_import_only_command(test_command: str) -> bool:
    """Return True if the test command is a trivial import-only smoke test."""
    if not test_command:
        return True
    if "-c" in test_command and "import" in test_command:
        if "assert" not in test_command:
            if "pytest" not in test_command and "unittest" not in test_command:
                return True
    return False


if __name__ == "__main__":
    sys.exit(main())
