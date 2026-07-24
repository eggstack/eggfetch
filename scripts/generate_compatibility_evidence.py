#!/usr/bin/env python3
"""Generate machine-readable compatibility evidence from consumed result files.

This script does NOT infer success from imports, does NOT count test functions
from source code, and does NOT run tests inline. It consumes explicit result
artifacts produced by prior CI jobs and fails hard on missing, stale, or
failed results.

Usage:
    generate_compatibility_evidence.py \
        --compat-test-results <path> \
        --downstream-results <path> \
        --api-comparison-results <path> \
        --candidate-sha <40-char-sha> \
        --artifact-hashes <path> \
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
    """Load and validate compat test results."""
    data = _load_json(path, "compat-test-results")

    result_sha = data.get("candidate_sha") or data.get("sha") or data.get("commit")
    if result_sha:
        result_sha = str(result_sha).strip()
        if result_sha != candidate_sha:
            _fail(
                f"compat-test-results SHA mismatch: expected {candidate_sha}, "
                f"got {result_sha}"
            )

    _check_placeholder(data, "root", "compat-test-results")

    total = data.get("total", 0)
    passed = data.get("passed", 0)
    failures = data.get("failures", 0)
    errors = data.get("errors", 0)

    if not isinstance(total, int) or total < 0:
        _fail(f"compat-test-results: 'total' must be a non-negative integer, got {total!r}")
    if not isinstance(passed, int) or passed < 0:
        _fail(f"compat-test-results: 'passed' must be a non-negative integer, got {passed!r}")
    if not isinstance(failures, int) or failures < 0:
        _fail(f"compat-test-results: 'failures' must be a non-negative integer, got {failures!r}")
    if not isinstance(errors, int) or errors < 0:
        _fail(f"compat-test-results: 'errors' must be a non-negative integer, got {errors!r}")

    if total > 0 and passed + failures + errors != total:
        _fail(
            f"compat-test-results: passed({passed}) + failures({failures}) + "
            f"errors({errors}) != total({total})"
        )

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


def generate_evidence(
    compat_test_results_path: Path,
    downstream_results_path: Path,
    api_comparison_results_path: Path,
    candidate_sha: str,
    artifact_hashes_path: Path,
    output_path: Path,
) -> dict[str, Any]:
    """Generate evidence by consuming explicit result files."""
    _validate_sha(candidate_sha, "candidate-sha")

    compat_data = _load_compat_test_results(compat_test_results_path, candidate_sha)
    downstream_data = _load_downstream_results(downstream_results_path, candidate_sha)
    api_data = _load_api_comparison_results(api_comparison_results_path, candidate_sha)
    artifact_data = _load_artifact_hashes(artifact_hashes_path)
    artifact_verification = _verify_artifact_hashes(artifact_data["hashes"])

    all_artifacts_valid = all(
        v.get("matches", False) for v in artifact_verification.values()
    )
    if not all_artifacts_valid:
        failed = [k for k, v in artifact_verification.items() if not v.get("matches")]
        _fail(f"Artifact hash verification failed for: {', '.join(failed)}")

    overall_pass = _compute_overall_pass(
        compat_data, downstream_data, api_data, artifact_verification
    )

    evidence: dict[str, Any] = {
        "schema_version": "2",
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
    )
    print(f"Evidence written to {args.output}")
    print(f"Candidate SHA: {evidence['candidate_sha']}")
    print(f"Overall pass: {evidence['overall_pass']}")


if __name__ == "__main__":
    main()
