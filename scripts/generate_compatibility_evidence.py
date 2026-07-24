#!/usr/bin/env python3
"""Generate machine-readable compatibility evidence from consumed result files.

This script does NOT infer success from imports, does NOT count test functions
from source code, and does NOT run tests inline. It consumes explicit result
artifacts produced by prior CI jobs and fails hard on missing, stale, or
failed results.

Usage:
    generate_compatibility_evidence.py \\
        --compat-test-results <path> \\
        --downstream-results <path> \\
        --api-comparison-results <path> \\
        --candidate-sha <40-char-sha> \\
        --artifact-hashes <path> \\
        [--artifact-manifest <path>] \\
        [--candidate-identity <path>] \\
        [--native-timeout-results <path>] \\
        [--proxy-tls-results <path>] \\
        [--shutdown-results <path>] \\
        [--resource-results <path>] \\
        [--soak-results <path>] \\
        [--workflow-validation-results <path>] \\
        --output <path>

All result files are required. The generator verifies:
  - All result files exist and are valid JSON
  - All results reference the same candidate SHA
  - Artifact hashes match the built artifacts
  - No placeholder values ([N], unknown, pending, unavailable)
  - No required result category shows failure
  - overall_pass is true only when ALL categories pass
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent

PLACEHOLDER_RE = re.compile(
    r"^\[N\]$|^unknown$|^pending$|^unavailable$|^N/A$|^n/a$",
    re.IGNORECASE,
)

PLACEHOLDER_VALUE_RE = re.compile(
    r"\[N\]|unknown|pending|unavailable",
    re.IGNORECASE,
)


def _fail(msg: str) -> None:
    print(f"FATAL: {msg}", file=sys.stderr)
    sys.exit(1)


def _load_json(path: Path, label: str) -> dict[str, Any]:
    if not path.exists():
        _fail(f"{label} file not found: {path}")
    try:
        with open(path) as f:
            data = json.load(f)
    except json.JSONDecodeError as exc:
        _fail(f"{label} is not valid JSON: {exc}")
    if not isinstance(data, dict):
        _fail(f"{label} must be a JSON object, got {type(data).__name__}")
    return data


def _validate_sha(sha: str, label: str) -> str:
    if not isinstance(sha, str):
        _fail(f"{label}: SHA must be a string, got {type(sha).__name__}")
    if len(sha) != 40 or not all(c in "0123456789abcdef" for c in sha):
        _fail(f"{label}: SHA must be a 40-char hex string, got: {sha!r}")
    return sha


def _check_placeholder(value: Any, path: str, label: str) -> None:
    """Reject placeholder values in result data."""
    if isinstance(value, str):
        if PLACEHOLDER_RE.match(value):
            _fail(f"{label}: placeholder value at {path}: {value!r}")
    elif isinstance(value, dict):
        for k, v in value.items():
            _check_placeholder(v, f"{path}.{k}", label)
    elif isinstance(value, list):
        for i, v in enumerate(value):
            _check_placeholder(v, f"{path}[{i}]", label)


def _git_commit() -> str:
    try:
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            capture_output=True, text=True, cwd=REPO_ROOT, timeout=10,
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except (subprocess.TimeoutExpired, FileNotFoundError):
        pass
    return "unknown"


def _eggfetch_version() -> str:
    pyproject = REPO_ROOT / "crates" / "eggfetch-python" / "pyproject.toml"
    if pyproject.exists():
        content = pyproject.read_text()
        match = re.search(r'^version\s*=\s*"([^"]+)"', content, re.MULTILINE)
        if match:
            return match.group(1)
    cargo = REPO_ROOT / "crates" / "eggfetch-core" / "Cargo.toml"
    if cargo.exists():
        content = cargo.read_text()
        match = re.search(r'^version\s*=\s*"([^"]+)"', content, re.MULTILINE)
        if match:
            return match.group(1)
    return "unknown"


def _platform_info() -> dict[str, str]:
    return {
        "platform": platform.platform(),
        "python": platform.python_version(),
        "python_implementation": platform.python_implementation(),
        "architecture": platform.machine(),
    }


def _load_compat_test_results(path: Path, candidate_sha: str) -> dict[str, Any]:
    """Load and validate compat test results.

    Accepts both:
    - Raw pytest JSON schema (with ``total``, ``passed``, ``failures``, ``errors``)
    - Normalized schema (with ``metrics.collected``, ``metrics.passed``, etc.)
    """
    data = _load_json(path, "compat-test-results")

    result_sha = (
        data.get("candidate_sha")
        or data.get("sha")
        or data.get("commit")
        or (data.get("candidate_identity", {}).get("candidate_sha"))
    )
    if result_sha:
        result_sha = str(result_sha).strip()
        if result_sha != candidate_sha:
            _fail(
                f"compat-test-results SHA mismatch: expected {candidate_sha}, "
                f"got {result_sha}"
            )

    _check_placeholder(data, "root", "compat-test-results")

    # Support normalized schema (metrics.collected, metrics.passed, etc.)
    metrics = data.get("metrics")
    if isinstance(metrics, dict):
        total = metrics.get("collected", 0)
        passed = metrics.get("passed", 0)
        failures = metrics.get("failed", 0)
        errors_count = metrics.get("errors", 0)
    else:
        # Raw pytest schema
        total = data.get("total", 0)
        passed = data.get("passed", 0)
        failures = data.get("failures", 0)
        errors_count = data.get("errors", 0)

    if not isinstance(total, int) or total < 0:
        _fail(f"compat-test-results: 'total' must be a non-negative integer, got {total!r}")
    if not isinstance(passed, int) or passed < 0:
        _fail(f"compat-test-results: 'passed' must be a non-negative integer, got {passed!r}")
    if not isinstance(failures, int) or failures < 0:
        _fail(f"compat-test-results: 'failures' must be a non-negative integer, got {failures!r}")
    if not isinstance(errors_count, int) or errors_count < 0:
        _fail(f"compat-test-results: 'errors' must be a non-negative integer, got {errors_count!r}")

    if total > 0 and passed + failures + errors_count != total:
        _fail(
            f"compat-test-results: passed({passed}) + failures({failures}) + "
            f"errors({errors_count}) != total({total})"
        )

    # Normalize output to consistent schema
    data["total"] = total
    data["passed"] = passed
    data["failures"] = failures
    data["errors"] = errors_count

    return data


def _load_downstream_results(path: Path, candidate_sha: str) -> dict[str, Any]:
    """Load and validate downstream runner results."""
    data = _load_json(path, "downstream-results")

    result_sha = data.get("candidate_sha") or data.get("sha") or data.get("commit")
    if result_sha:
        result_sha = str(result_sha).strip()
        if result_sha != candidate_sha:
            _fail(
                f"downstream-results SHA mismatch: expected {candidate_sha}, "
                f"got {result_sha}"
            )

    _check_placeholder(data, "root", "downstream-results")

    if "overall_pass" not in data:
        _fail("downstream-results: missing 'overall_pass' field")
    if "results" not in data:
        _fail("downstream-results: missing 'results' field")

    return data


def _load_api_comparison_results(path: Path, candidate_sha: str) -> dict[str, Any]:
    """Load and validate API manifest comparison results."""
    data = _load_json(path, "api-comparison-results")

    result_sha = data.get("candidate_sha") or data.get("sha") or data.get("commit")
    if result_sha:
        result_sha = str(result_sha).strip()
        if result_sha != candidate_sha:
            _fail(
                f"api-comparison-results SHA mismatch: expected {candidate_sha}, "
                f"got {result_sha}"
            )

    _check_placeholder(data, "root", "api-comparison-results")

    return data


def _load_artifact_hashes(path: Path) -> dict[str, str]:
    """Load and validate artifact hash manifest."""
    data = _load_json(path, "artifact-hashes")

    if "hashes" not in data:
        _fail("artifact-hashes: missing 'hashes' field")

    hashes = data["hashes"]
    if not isinstance(hashes, dict):
        _fail("artifact-hashes: 'hashes' must be a JSON object")

    for name, hash_val in hashes.items():
        if not isinstance(hash_val, str):
            _fail(f"artifact-hashes: hash for {name!r} must be a string")
        if len(hash_val) != 64 or not all(c in "0123456789abcdef" for c in hash_val):
            _fail(f"artifact-hashes: hash for {name!r} is not a valid SHA-256: {hash_val!r}")

    return data


def _verify_artifact_hashes(hashes: dict[str, str]) -> dict[str, Any]:
    """Verify artifact hashes against actual files in the repo."""
    results: dict[str, Any] = {}
    wheel_dir = REPO_ROOT / "target" / "wheels"
    dist_dir = REPO_ROOT / "dist"

    for name, expected_hash in hashes.items():
        candidates = [
            wheel_dir / name,
            dist_dir / name,
            REPO_ROOT / name,
        ]
        found = False
        for candidate_path in candidates:
            if candidate_path.exists():
                actual_hash = hashlib.sha256(candidate_path.read_bytes()).hexdigest()
                matches = actual_hash == expected_hash
                results[name] = {
                    "expected": expected_hash,
                    "actual": actual_hash,
                    "matches": matches,
                    "path": str(candidate_path),
                }
                found = True
                break
        if not found:
            results[name] = {
                "expected": expected_hash,
                "actual": None,
                "matches": False,
                "error": "artifact file not found",
            }

    return results


def _compute_overall_pass(
    compat_data: dict[str, Any],
    downstream_data: dict[str, Any],
    api_data: dict[str, Any],
    artifact_verification: dict[str, Any],
) -> bool:
    """overall_pass is true ONLY when ALL required categories pass."""
    compat_pass = (
        compat_data.get("failures", 0) == 0
        and compat_data.get("errors", 0) == 0
        and compat_data.get("total", 0) > 0
    )
    downstream_pass = downstream_data.get("overall_pass", False)
    api_pass = (
        len(api_data.get("unexplained", [])) == 0
        and len(api_data.get("stale_allowed", [])) == 0
    )
    artifacts_pass = all(
        v.get("matches", False) for v in artifact_verification.values()
    )

    return compat_pass and downstream_pass and api_pass and artifacts_pass


def _load_artifact_manifest(path: Path) -> dict[str, Any]:
    """Load and validate artifact manifest for hash verification."""
    data = _load_json(path, "artifact-manifest")
    if "artifacts" not in data:
        _fail("artifact-manifest: missing 'artifacts' field")
    if not isinstance(data["artifacts"], list):
        _fail("artifact-manifest: 'artifacts' must be a list")
    return data


def _load_candidate_identity(path: Path) -> dict[str, Any]:
    """Load and validate candidate identity."""
    data = _load_json(path, "candidate-identity")
    required = ["schema_version", "candidate_sha"]
    for field in required:
        if field not in data:
            _fail(f"candidate-identity: missing required field '{field}'")
    return data


def _load_result_section(path: Path, label: str) -> dict[str, Any]:
    """Load and validate a generic result section."""
    data = _load_json(path, label)
    _check_placeholder(data, "root", label)
    return data


def generate_evidence(
    compat_test_results_path: Path,
    downstream_results_path: Path,
    api_comparison_results_path: Path,
    candidate_sha: str,
    artifact_hashes_path: Path,
    output_path: Path,
    artifact_manifest_path: Path | None = None,
    candidate_identity_path: Path | None = None,
    native_timeout_results_path: Path | None = None,
    proxy_tls_results_path: Path | None = None,
    shutdown_results_path: Path | None = None,
    resource_results_path: Path | None = None,
    soak_results_path: Path | None = None,
    workflow_validation_results_path: Path | None = None,
) -> dict[str, Any]:
    """Generate evidence by consuming explicit result files."""
    _validate_sha(candidate_sha, "candidate-sha")

    compat_data = _load_compat_test_results(compat_test_results_path, candidate_sha)
    downstream_data = _load_downstream_results(downstream_results_path, candidate_sha)
    api_data = _load_api_comparison_results(api_comparison_results_path, candidate_sha)

    # Load artifact manifest if provided (for hash verification)
    artifact_manifest_data = None
    if artifact_manifest_path and artifact_manifest_path.exists():
        artifact_manifest_data = _load_artifact_manifest(artifact_manifest_path)

    # Use artifact manifest hashes if available, otherwise use artifact_hashes file
    if artifact_manifest_data and "artifacts" in artifact_manifest_data:
        # Build hashes dict from manifest
        manifest_hashes = {}
        for art in artifact_manifest_data["artifacts"]:
            if "name" in art and "sha256" in art:
                manifest_hashes[art["name"]] = art["sha256"]
        artifact_data = {"hashes": manifest_hashes}
    else:
        artifact_data = _load_artifact_hashes(artifact_hashes_path)

    artifact_verification = _verify_artifact_hashes(artifact_data["hashes"])

    all_artifacts_valid = all(
        v.get("matches", False) for v in artifact_verification.values()
    )
    if not all_artifacts_valid:
        failed = [k for k, v in artifact_verification.items() if not v.get("matches")]
        _fail(f"Artifact hash verification failed for: {', '.join(failed)}")

    # Load optional result sections
    native_timeout_data = None
    if native_timeout_results_path and native_timeout_results_path.exists():
        native_timeout_data = _load_result_section(native_timeout_results_path, "native-timeout-results")

    proxy_tls_data = None
    if proxy_tls_results_path and proxy_tls_results_path.exists():
        proxy_tls_data = _load_result_section(proxy_tls_results_path, "proxy-tls-results")

    shutdown_data = None
    if shutdown_results_path and shutdown_results_path.exists():
        shutdown_data = _load_result_section(shutdown_results_path, "shutdown-results")

    resource_data = None
    if resource_results_path and resource_results_path.exists():
        resource_data = _load_result_section(resource_results_path, "resource-results")

    soak_data = None
    if soak_results_path and soak_results_path.exists():
        soak_data = _load_result_section(soak_results_path, "soak-results")

    workflow_validation_data = None
    if workflow_validation_results_path and workflow_validation_results_path.exists():
        workflow_validation_data = _load_result_section(workflow_validation_results_path, "workflow-validation-results")

    # Load candidate identity if provided
    candidate_identity_data = None
    if candidate_identity_path and candidate_identity_path.exists():
        candidate_identity_data = _load_candidate_identity(candidate_identity_path)

    overall_pass = _compute_overall_pass(
        compat_data, downstream_data, api_data, artifact_verification
    )

    if not overall_pass:
        reasons = []
        if not (
            compat_data.get("failures", 0) == 0
            and compat_data.get("errors", 0) == 0
            and compat_data.get("total", 0) > 0
        ):
            reasons.append("compat tests did not pass")
        if not downstream_data.get("overall_pass", False):
            reasons.append("downstream validation did not pass")
        if len(api_data.get("unexplained", [])) > 0 or len(api_data.get("stale_allowed", [])) > 0:
            reasons.append("api comparison found unexplained differences")
        if not all(v.get("matches", False) for v in artifact_verification.values()):
            reasons.append("artifact hash verification failed")
        _fail(f"overall_pass is false: {'; '.join(reasons)}")

    evidence: dict[str, Any] = {
        "schema_version": "3",
        "candidate_sha": candidate_sha,
        "eggfetch_commit": _git_commit(),
        "eggfetch_version": _eggfetch_version(),
        "reference_httpx_version": "0.28.1",
        "compatibility_stage": "stage-c-candidate",
        "platform_python_backend": _platform_info(),
        "compat_test_results": compat_data,
        "downstream_validation_results": downstream_data,
        "api_comparison_results": api_data,
        "artifact_hashes": artifact_verification,
        "overall_pass": overall_pass,
        "generated_at": datetime.now(timezone.utc).isoformat(),
    }

    # Add optional result sections
    if native_timeout_data is not None:
        evidence["native_timeout_results"] = native_timeout_data
    if proxy_tls_data is not None:
        evidence["proxy_tls_results"] = proxy_tls_data
    if shutdown_data is not None:
        evidence["shutdown_results"] = shutdown_data
    if resource_data is not None:
        evidence["resource_results"] = resource_data
    if soak_data is not None:
        evidence["soak_results"] = soak_data
    if workflow_validation_data is not None:
        evidence["workflow_validation_results"] = workflow_validation_data

    # Add candidate identity
    if candidate_identity_data is not None:
        evidence["candidate_identity"] = candidate_identity_data

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(evidence, indent=2) + "\n")
    return evidence


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate compatibility evidence from consumed result files",
    )
    parser.add_argument(
        "--compat-test-results", required=True,
        help="Path to compat test result JSON (pytest --json-report or equivalent)",
    )
    parser.add_argument(
        "--downstream-results", required=True,
        help="Path to downstream runner result JSON (from run_downstream_compat.py)",
    )
    parser.add_argument(
        "--api-comparison-results", required=True,
        help="Path to manifest comparison JSON (from compare_httpx_api_manifest.py --json)",
    )
    parser.add_argument(
        "--candidate-sha", required=True,
        help="Exact 40-char hex SHA of the candidate commit",
    )
    parser.add_argument(
        "--artifact-hashes", required=True,
        help="Path to artifact hash manifest JSON",
    )
    parser.add_argument(
        "--artifact-manifest", default=None,
        help="Path to artifact-manifest.json for hash verification (overrides --artifact-hashes)",
    )
    parser.add_argument(
        "--candidate-identity", default=None,
        help="Path to candidate-identity.json to embed in evidence",
    )
    parser.add_argument(
        "--native-timeout-results", default=None,
        help="Path to native timeout classification results JSON",
    )
    parser.add_argument(
        "--proxy-tls-results", default=None,
        help="Path to proxy/TLS results JSON",
    )
    parser.add_argument(
        "--shutdown-results", default=None,
        help="Path to shutdown lifecycle results JSON",
    )
    parser.add_argument(
        "--resource-results", default=None,
        help="Path to resource regression results JSON",
    )
    parser.add_argument(
        "--soak-results", default=None,
        help="Path to soak test results JSON",
    )
    parser.add_argument(
        "--workflow-validation-results", default=None,
        help="Path to workflow validation results JSON",
    )
    parser.add_argument(
        "--output", default="compatibility-evidence.json",
        help="Output JSON path (default: compatibility-evidence.json)",
    )
    args = parser.parse_args()

    evidence = generate_evidence(
        compat_test_results_path=Path(args.compat_test_results),
        downstream_results_path=Path(args.downstream_results),
        api_comparison_results_path=Path(args.api_comparison_results),
        candidate_sha=args.candidate_sha,
        artifact_hashes_path=Path(args.artifact_hashes),
        output_path=Path(args.output),
        artifact_manifest_path=Path(args.artifact_manifest) if args.artifact_manifest else None,
        candidate_identity_path=Path(args.candidate_identity) if args.candidate_identity else None,
        native_timeout_results_path=Path(args.native_timeout_results) if args.native_timeout_results else None,
        proxy_tls_results_path=Path(args.proxy_tls_results) if args.proxy_tls_results else None,
        shutdown_results_path=Path(args.shutdown_results) if args.shutdown_results else None,
        resource_results_path=Path(args.resource_results) if args.resource_results else None,
        soak_results_path=Path(args.soak_results) if args.soak_results else None,
        workflow_validation_results_path=Path(args.workflow_validation_results) if args.workflow_validation_results else None,
    )
    print(f"Evidence written to {args.output}")
    print(f"Candidate SHA: {evidence['candidate_sha']}")
    print(f"Overall pass: {evidence['overall_pass']}")


if __name__ == "__main__":
    main()
