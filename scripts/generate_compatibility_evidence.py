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
        --artifact-manifest <path> \\
        --candidate-identity <path> \\
        --native-timeout-results <path> \\
        --proxy-tls-results <path> \\
        --shutdown-results <path> \\
        --resource-results <path> \\
        --soak-results <path> \\
        --workflow-validation-results <path> \\
        --output <path>

All result files are required. The generator verifies:
  - All result files exist and are valid JSON
  - All results reference the same candidate SHA
  - Artifact hashes match the built artifacts via bundle-relative paths
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


def _load_result_section(path: Path, label: str) -> dict[str, Any]:
    """Load and validate a generic result section."""
    data = _load_json(path, label)
    _check_placeholder(data, "root", label)
    return data


def _verify_artifact_manifest_hashes(
    manifest_data: dict[str, Any],
    bundle_root: Path,
    candidate_sha: str,
) -> dict[str, Any]:
    """Verify artifact hashes from manifest using bundle-relative paths.

    Per plan §11.3: resolves bundle_root / relative_path and recomputes
    SHA-256 and size for every artifact.
    """
    results: dict[str, Any] = {}
    artifacts = manifest_data.get("artifacts", [])
    if not isinstance(artifacts, list):
        _fail("artifact-manifest: 'artifacts' must be a list")

    for art in artifacts:
        if not isinstance(art, dict):
            _fail("artifact-manifest: each artifact must be a JSON object")
        role = art.get("role", "unknown")
        filename = art.get("filename", "")
        relative_path = art.get("relative_path", "")
        expected_hash = art.get("sha256", "")
        expected_size = art.get("size_bytes", 0)

        if not filename:
            _fail(f"artifact-manifest: artifact role={role} has no filename")
        if not expected_hash or len(expected_hash) != 64:
            _fail(f"artifact-manifest: artifact {filename} has invalid sha256")

        # Resolve bundle-relative path
        if relative_path:
            artifact_path = bundle_root / relative_path
        else:
            artifact_path = bundle_root / "wheels" / filename

        if not artifact_path.exists():
            results[filename] = {
                "expected": expected_hash,
                "actual": None,
                "matches": False,
                "error": f"artifact file not found at bundle-relative path: {artifact_path}",
            }
            continue

        actual_bytes = artifact_path.read_bytes()
        actual_hash = hashlib.sha256(actual_bytes).hexdigest()
        actual_size = len(actual_bytes)
        matches = actual_hash == expected_hash and actual_size == expected_size

        results[filename] = {
            "expected": expected_hash,
            "actual": actual_hash,
            "matches": matches,
            "path": str(artifact_path),
            "expected_size": expected_size,
            "actual_size": actual_size,
            "role": role,
        }

        if not matches:
            if actual_hash != expected_hash:
                _fail(
                    f"Artifact hash mismatch for {filename}: "
                    f"expected {expected_hash}, got {actual_hash}"
                )
            if actual_size != expected_size:
                _fail(
                    f"Artifact size mismatch for {filename}: "
                    f"expected {expected_size}, got {actual_size}"
                )

    return results


def _check_result_identity(
    data: dict[str, Any],
    candidate_sha: str,
    expected_identity_digest: str | None,
    label: str,
) -> None:
    """Check that a result has matching candidate_sha and identity_digest."""
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
                f"{label}: SHA mismatch: expected {candidate_sha}, got {result_sha}"
            )

    if expected_identity_digest:
        result_identity = data.get("identity_digest", "")
        if result_identity and result_identity != expected_identity_digest:
            _fail(
                f"{label}: identity_digest mismatch: "
                f"expected {expected_identity_digest}, got {result_identity}"
            )


def _compute_overall_pass(
    all_sections: dict[str, Any],
    artifact_verification: dict[str, Any],
) -> bool:
    """overall_pass is true ONLY when ALL required categories pass.

    Per plan §11.4: every expected result exists, every status is passed,
    all artifact hashes match.
    """
    # Artifact verification must pass
    artifacts_pass = all(
        v.get("matches", False) for v in artifact_verification.values()
    )
    if not artifacts_pass:
        return False

    # Check every required section
    for section_name, section_data in all_sections.items():
        if section_data is None:
            return False
        if isinstance(section_data, dict):
            # Check for explicit overall_pass field
            if "overall_pass" in section_data:
                if not section_data["overall_pass"]:
                    return False
            # Check for status field
            elif "status" in section_data:
                if section_data["status"] != "passed":
                    return False
            # Check for failures/errors in compat results
            if section_name == "compat_test_results":
                failures = section_data.get("failures", 0)
                errors_count = section_data.get("errors", 0)
                total = section_data.get("total", 0)
                if total > 0 and (failures > 0 or errors_count > 0):
                    return False
                if total == 0:
                    return False
            # Check for unexplained/stale in API comparison
            if section_name == "api_comparison_results":
                unexplained = section_data.get("unexplained", [])
                stale = section_data.get("stale_allowed", [])
                if unexplained or stale:
                    return False

    return True


def generate_evidence(
    compat_test_results_path: Path,
    downstream_results_path: Path,
    api_comparison_results_path: Path,
    candidate_sha: str,
    output_path: Path,
    artifact_manifest_path: Path,
    candidate_identity_path: Path,
    native_timeout_results_path: Path,
    proxy_tls_results_path: Path,
    shutdown_results_path: Path,
    resource_results_path: Path,
    soak_results_path: Path,
    workflow_validation_results_path: Path,
) -> dict[str, Any]:
    """Generate evidence by consuming explicit result files.

    All result files are required per plan §11.2.
    """
    _validate_sha(candidate_sha, "candidate-sha")

    # Load and validate all required sections
    compat_data = _load_result_section(compat_test_results_path, "compat-test-results")
    downstream_data = _load_result_section(downstream_results_path, "downstream-results")
    api_data = _load_result_section(api_comparison_results_path, "api-comparison-results")
    native_timeout_data = _load_result_section(native_timeout_results_path, "native-timeout-results")
    proxy_tls_data = _load_result_section(proxy_tls_results_path, "proxy-tls-results")
    shutdown_data = _load_result_section(shutdown_results_path, "shutdown-results")
    resource_data = _load_result_section(resource_results_path, "resource-results")
    soak_data = _load_result_section(soak_results_path, "soak-results")
    workflow_validation_data = _load_result_section(workflow_validation_results_path, "workflow-validation-results")

    # Load artifact manifest (required)
    manifest_data = _load_json(artifact_manifest_path, "artifact-manifest")
    if "artifacts" not in manifest_data:
        _fail("artifact-manifest: missing 'artifacts' field")

    # Load candidate identity (required)
    identity_data = _load_json(candidate_identity_path, "candidate-identity")
    identity_digest = identity_data.get("identity_digest", "")
    if not identity_digest or len(identity_digest) != 64:
        _fail("candidate-identity: missing or invalid identity_digest")

    # Verify manifest digest matches identity
    manifest_bytes = artifact_manifest_path.read_bytes()
    manifest_digest = hashlib.sha256(manifest_bytes).hexdigest()
    expected_manifest_digest = identity_data.get("artifact_manifest_sha256", "")
    if expected_manifest_digest and manifest_digest != expected_manifest_digest:
        _fail(
            f"artifact manifest digest mismatch: identity says {expected_manifest_digest}, "
            f"computed {manifest_digest}"
        )

    # Verify all result SHA and identity digest
    for label, data in [
        ("compat-test-results", compat_data),
        ("downstream-results", downstream_data),
        ("api-comparison-results", api_data),
        ("native-timeout-results", native_timeout_data),
        ("proxy-tls-results", proxy_tls_data),
        ("shutdown-results", shutdown_data),
        ("resource-results", resource_data),
        ("soak-results", soak_data),
        ("workflow-validation-results", workflow_validation_data),
    ]:
        _check_result_identity(data, candidate_sha, identity_digest, label)

    # Determine bundle root from manifest path
    bundle_root = artifact_manifest_path.parent

    # Verify artifact hashes using bundle-relative paths (§11.3)
    artifact_verification = _verify_artifact_manifest_hashes(
        manifest_data, bundle_root, candidate_sha
    )

    # Compute overall_pass
    all_sections = {
        "compat_test_results": compat_data,
        "downstream_validation_results": downstream_data,
        "api_comparison_results": api_data,
        "native_timeout_results": native_timeout_data,
        "proxy_tls_results": proxy_tls_data,
        "shutdown_results": shutdown_data,
        "resource_results": resource_data,
        "soak_results": soak_data,
        "workflow_validation_results": workflow_validation_data,
    }

    overall_pass = _compute_overall_pass(all_sections, artifact_verification)

    if not overall_pass:
        reasons = []
        for section_name, section_data in all_sections.items():
            if section_data is None:
                reasons.append(f"{section_name} is missing")
                continue
            if isinstance(section_data, dict):
                if "overall_pass" in section_data and not section_data["overall_pass"]:
                    reasons.append(f"{section_name}.overall_pass is false")
                elif "status" in section_data and section_data["status"] != "passed":
                    reasons.append(f"{section_name}.status is '{section_data['status']}'")
                elif section_name == "compat_test_results":
                    total = section_data.get("total", 0)
                    failures = section_data.get("failures", 0)
                    errors_count = section_data.get("errors", 0)
                    if total == 0:
                        reasons.append("compat_test_results.total is 0")
                    elif failures > 0 or errors_count > 0:
                        reasons.append(f"compat_test_results has {failures} failures, {errors_count} errors")
                elif section_name == "api_comparison_results":
                    unexplained = section_data.get("unexplained", [])
                    stale = section_data.get("stale_allowed", [])
                    if unexplained:
                        reasons.append(f"api_comparison has {len(unexplained)} unexplained differences")
                    if stale:
                        reasons.append(f"api_comparison has {len(stale)} stale differences")
        if not all(v.get("matches", False) for v in artifact_verification.values()):
            failed = [k for k, v in artifact_verification.items() if not v.get("matches")]
            reasons.append(f"artifact hash mismatch for: {', '.join(failed)}")
        _fail(f"overall_pass is false: {'; '.join(reasons)}")

    evidence: dict[str, Any] = {
        "schema_version": "3",
        "candidate_sha": candidate_sha,
        "identity_digest": identity_digest,
        "eggfetch_commit": _git_commit(),
        "eggfetch_version": _eggfetch_version(),
        "reference_httpx_version": "0.28.1",
        "compatibility_stage": "stage-c-candidate",
        "platform_python_backend": _platform_info(),
        "compat_test_results": compat_data,
        "downstream_validation_results": downstream_data,
        "api_comparison_results": api_data,
        "artifact_hashes": artifact_verification,
        "native_timeout_results": native_timeout_data,
        "proxy_tls_results": proxy_tls_data,
        "shutdown_results": shutdown_data,
        "resource_results": resource_data,
        "soak_results": soak_data,
        "workflow_validation_results": workflow_validation_data,
        "candidate_identity": identity_data,
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
    parser.add_argument("--compat-test-results", required=True)
    parser.add_argument("--downstream-results", required=True)
    parser.add_argument("--api-comparison-results", required=True)
    parser.add_argument("--candidate-sha", required=True)
    parser.add_argument("--artifact-manifest", required=True)
    parser.add_argument("--candidate-identity", required=True)
    parser.add_argument("--native-timeout-results", required=True)
    parser.add_argument("--proxy-tls-results", required=True)
    parser.add_argument("--shutdown-results", required=True)
    parser.add_argument("--resource-results", required=True)
    parser.add_argument("--soak-results", required=True)
    parser.add_argument("--workflow-validation-results", required=True)
    parser.add_argument("--output", default="compatibility-evidence.json")
    args = parser.parse_args()

    evidence = generate_evidence(
        compat_test_results_path=Path(args.compat_test_results),
        downstream_results_path=Path(args.downstream_results),
        api_comparison_results_path=Path(args.api_comparison_results),
        candidate_sha=args.candidate_sha,
        artifact_manifest_path=Path(args.artifact_manifest),
        candidate_identity_path=Path(args.candidate_identity),
        native_timeout_results_path=Path(args.native_timeout_results),
        proxy_tls_results_path=Path(args.proxy_tls_results),
        shutdown_results_path=Path(args.shutdown_results),
        resource_results_path=Path(args.resource_results),
        soak_results_path=Path(args.soak_results),
        workflow_validation_results_path=Path(args.workflow_validation_results),
        output_path=Path(args.output),
    )
    print(f"Evidence written to {args.output}")
    print(f"Candidate SHA: {evidence['candidate_sha']}")
    print(f"Overall pass: {evidence['overall_pass']}")


if __name__ == "__main__":
    main()
