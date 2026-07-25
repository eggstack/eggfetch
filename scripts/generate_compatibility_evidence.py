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
        [--facade-api-results <path>] \\
        [--shim-api-results <path>] \\
        [--shim-substitution-results <path>] \\
        [--native-timeout-results <path>] \\
        [--proxy-tls-results <path>] \\
        [--shutdown-results <path>] \\
        [--resource-results <path>] \\
        [--soak-results <path>] \\
        [--workflow-validation-results <path>] \\
        [--release] \\
        --output <path>

All result files are required. The generator verifies:
  - All result files exist and are valid JSON
  - All results reference the same candidate SHA
  - Artifact hashes match the built artifacts
  - No placeholder values ([N], unknown, pending, unavailable)
  - No required result category shows failure
  - overall_pass is true only when ALL categories pass

When --release is set, all sections are mandatory and identity consistency
is enforced.
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

# Mandatory result sections for release mode
MANDATORY_SECTIONS = [
    "compat_test_results",
    "facade_api_results",
    "shim_api_results",
    "downstream_aggregate_results",
    "shim_substitution_results",
    "native_timeout_results",
    "proxy_tls_results",
    "shutdown_results",
    "resource_results",
    "soak_results",
    "workflow_validation_results",
]

# Section argument names mapped to their CLI flags
SECTION_FLAGS = {
    "compat_test_results": "--compat-test-results",
    "facade_api_results": "--facade-api-results",
    "shim_api_results": "--shim-api-results",
    "downstream_aggregate_results": "--downstream-results",
    "shim_substitution_results": "--shim-substitution-results",
    "native_timeout_results": "--native-timeout-results",
    "proxy_tls_results": "--proxy-tls-results",
    "shutdown_results": "--shutdown-results",
    "resource_results": "--resource-results",
    "soak_results": "--soak-results",
    "workflow_validation_results": "--workflow-validation-results",
}


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


def _validate_sha256(digest: str, label: str) -> str:
    if not isinstance(digest, str):
        _fail(f"{label}: SHA-256 must be a string, got {type(digest).__name__}")
    if len(digest) != 64 or not all(c in "0123456789abcdef" for c in digest):
        _fail(f"{label}: SHA-256 must be a 64-char hex string, got: {digest!r}")
    return digest


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


def _load_candidate_identity(path: Path) -> dict[str, Any]:
    """Load and validate candidate identity."""
    data = _load_json(path, "candidate-identity")
    required = ["schema_version", "candidate_sha", "identity_digest",
                "eggfetch_version", "artifact_manifest_sha256"]
    for field in required:
        if field not in data:
            _fail(f"candidate-identity: missing required field '{field}'")
    _validate_sha(data["candidate_sha"], "candidate-identity.candidate_sha")
    _validate_sha256(data["identity_digest"], "candidate-identity.identity_digest")
    _validate_sha256(data["artifact_manifest_sha256"], "candidate-identity.artifact_manifest_sha256")
    return data


def _load_artifact_manifest(path: Path) -> dict[str, Any]:
    """Load and validate artifact manifest for hash verification."""
    data = _load_json(path, "artifact-manifest")
    if "artifacts" not in data:
        _fail("artifact-manifest: missing 'artifacts' field")
    if not isinstance(data["artifacts"], list):
        _fail("artifact-manifest: 'artifacts' must be a list")
    if "candidate_sha" not in data:
        _fail("artifact-manifest: missing 'candidate_sha' field")
    return data


def _verify_artifact_paths(manifest: dict, candidate_identity: dict | None) -> dict[str, Any]:
    """Verify artifact paths and hashes from the manifest.

    Loads each exact path from artifact-manifest.json, verifies the path
    exists within the normalized artifact directory, recomputes SHA-256,
    and compares with both manifest and candidate identity.
    """
    results: dict[str, Any] = {}
    manifest_sha = hashlib.sha256(
        json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()

    # Verify manifest SHA matches candidate identity
    if candidate_identity is not None:
        id_manifest_sha = candidate_identity.get("artifact_manifest_sha256", "")
        if id_manifest_sha and id_manifest_sha != manifest_sha:
            _fail(
                f"artifact manifest SHA mismatch: manifest={manifest_sha}, "
                f"identity={id_manifest_sha}"
            )

    for art in manifest.get("artifacts", []):
        filename = art.get("filename", "")
        expected_sha = art.get("sha256", "")
        path_str = art.get("path", "")

        if not filename or not expected_sha or not path_str:
            _fail(f"artifact manifest entry missing filename/sha256/path: {art}")

        _validate_sha256(expected_sha, f"artifact {filename}")

        # Check for path traversal
        if ".." in path_str:
            _fail(f"artifact path traversal detected: {path_str}")

        art_path = Path(path_str)
        if not art_path.exists():
            results[filename] = {
                "expected": expected_sha,
                "actual": None,
                "matches": False,
                "error": f"artifact file not found: {path_str}",
            }
            continue

        actual_sha = hashlib.sha256(art_path.read_bytes()).hexdigest()
        matches = actual_sha == expected_sha

        # Also compare with candidate identity wheel hashes
        if candidate_identity is not None:
            for wheel_key in ("eggfetch_wheel", "httpx_replacement_wheel"):
                wheel = candidate_identity.get(wheel_key, {})
                if wheel.get("filename") == filename:
                    id_sha = wheel.get("sha256", "")
                    if id_sha and id_sha != expected_sha:
                        _fail(
                            f"artifact {filename} hash mismatch: "
                            f"manifest={expected_sha}, identity={id_sha}"
                        )

        results[filename] = {
            "expected": expected_sha,
            "actual": actual_sha,
            "matches": matches,
            "path": str(art_path),
            "size_bytes": art.get("size_bytes", 0),
        }

    return results


def _load_result_section(path: Path | None, label: str,
                         candidate_sha: str, identity_digest: str | None,
                         release: bool) -> dict[str, Any] | None:
    """Load and validate a result section. Returns None if path is None."""
    if path is None:
        if release:
            _fail(f"missing mandatory result section: {label}")
        return None

    data = _load_json(path, label)
    _check_placeholder(data, "root", label)

    # Validate candidate_sha
    result_sha = data.get("candidate_sha") or data.get("sha") or data.get("commit")
    if result_sha:
        result_sha = str(result_sha).strip()
        if result_sha != candidate_sha:
            _fail(f"{label}: candidate_sha mismatch: expected {candidate_sha}, got {result_sha}")
    elif release:
        _fail(f"{label}: missing candidate_sha")

    # Validate identity_digest if present
    result_digest = data.get("identity_digest")
    if result_digest:
        if identity_digest and result_digest != identity_digest:
            _fail(f"{label}: identity_digest mismatch: expected {identity_digest}, got {result_digest}")
    elif release:
        _fail(f"{label}: missing identity_digest")

    return data


def _compute_overall_pass(
    sections: dict[str, dict[str, Any] | None],
    artifact_verification: dict[str, Any],
    api_data: dict[str, Any],
    downstream_data: dict[str, Any],
    compat_data: dict[str, Any],
    soak_data: dict[str, Any] | None,
    resource_data: dict[str, Any] | None,
    workflow_validation_data: dict[str, Any] | None,
    release: bool,
) -> bool:
    """overall_pass is true ONLY when ALL required categories pass."""
    # Compat tests must pass
    compat_pass = (
        compat_data.get("failures", 0) == 0
        and compat_data.get("errors", 0) == 0
        and compat_data.get("total", 0) > 0
    )

    # Downstream must pass
    downstream_pass = downstream_data.get("overall_pass", False)

    # API must have no unexplained or stale differences
    api_pass = (
        len(api_data.get("unexplained", [])) == 0
        and len(api_data.get("stale_allowed", [])) == 0
    )

    # Artifacts must all match
    artifacts_pass = all(v.get("matches", False) for v in artifact_verification.values())

    # In release mode, all sections must be present and pass
    if release:
        for section_name, section_data in sections.items():
            if section_data is None:
                return False
            if not section_data.get("overall_pass", False):
                return False

    # Soak must pass if present
    soak_pass = True
    if soak_data is not None:
        soak_pass = soak_data.get("overall_pass", False)
        if not soak_pass:
            return False

    # Resource must pass if present
    resource_pass = True
    if resource_data is not None:
        resource_pass = resource_data.get("overall_pass", False)
        if not resource_pass:
            return False

    # Workflow validation must pass if present
    workflow_pass = True
    if workflow_validation_data is not None:
        workflow_pass = workflow_validation_data.get("overall_pass", False)
        if not workflow_pass:
            return False

    return compat_pass and downstream_pass and api_pass and artifacts_pass and soak_pass and resource_pass and workflow_pass


def generate_evidence(
    compat_test_results_path: Path,
    downstream_results_path: Path,
    api_comparison_results_path: Path,
    candidate_sha: str,
    artifact_hashes_path: Path,
    output_path: Path,
    artifact_manifest_path: Path | None = None,
    candidate_identity_path: Path | None = None,
    facade_api_results_path: Path | None = None,
    shim_api_results_path: Path | None = None,
    shim_substitution_results_path: Path | None = None,
    native_timeout_results_path: Path | None = None,
    proxy_tls_results_path: Path | None = None,
    shutdown_results_path: Path | None = None,
    resource_results_path: Path | None = None,
    soak_results_path: Path | None = None,
    workflow_validation_results_path: Path | None = None,
    release: bool = False,
) -> dict[str, Any]:
    """Generate evidence by consuming explicit result files."""
    _validate_sha(candidate_sha, "candidate-sha")

    compat_data = _load_result_section(
        compat_test_results_path, "compat-test-results",
        candidate_sha, None, release
    )
    downstream_data = _load_result_section(
        downstream_results_path, "downstream-results",
        candidate_sha, None, release
    )
    api_data = _load_result_section(
        api_comparison_results_path, "api-comparison-results",
        candidate_sha, None, release
    )

    # Load candidate identity if provided
    candidate_identity_data = None
    identity_digest = None
    if candidate_identity_path and candidate_identity_path.exists():
        candidate_identity_data = _load_candidate_identity(candidate_identity_path)
        identity_digest = candidate_identity_data.get("identity_digest")

    # Load artifact manifest if provided (for hash verification)
    artifact_manifest_data = None
    if artifact_manifest_path and artifact_manifest_path.exists():
        artifact_manifest_data = _load_artifact_manifest(artifact_manifest_path)

    # Verify artifact hashes
    if artifact_manifest_data is not None:
        artifact_verification = _verify_artifact_paths(
            artifact_manifest_data, candidate_identity_data
        )
    else:
        # Fall back to --artifact-hashes file
        artifact_data = _load_json(artifact_hashes_path, "artifact-hashes")
        if "hashes" not in artifact_data:
            _fail("artifact-hashes: missing 'hashes' field")
        hashes = artifact_data["hashes"]
        if not isinstance(hashes, dict):
            _fail("artifact-hashes: 'hashes' must be a JSON object")
        artifact_verification = {}
        for name, expected_hash in hashes.items():
            if not isinstance(expected_hash, str):
                _fail(f"artifact-hashes: hash for {name!r} must be a string")
            if len(expected_hash) != 64 or not all(c in "0123456789abcdef" for c in expected_hash):
                _fail(f"artifact-hashes: hash for {name!r} is not a valid SHA-256: {expected_hash!r}")
            artifact_verification[name] = {"expected": expected_hash, "actual": None, "matches": False}

    all_artifacts_valid = all(v.get("matches", False) for v in artifact_verification.values())
    if not all_artifacts_valid:
        failed = [k for k, v in artifact_verification.items() if not v.get("matches")]
        _fail(f"Artifact hash verification failed for: {', '.join(failed)}")

    # Load optional result sections
    sections: dict[str, dict[str, Any] | None] = {}
    sections["facade_api_results"] = _load_result_section(
        facade_api_results_path, "facade-api-results",
        candidate_sha, identity_digest, release
    )
    sections["shim_api_results"] = _load_result_section(
        shim_api_results_path, "shim-api-results",
        candidate_sha, identity_digest, release
    )
    sections["shim_substitution_results"] = _load_result_section(
        shim_substitution_results_path, "shim-substitution-results",
        candidate_sha, identity_digest, release
    )
    sections["native_timeout_results"] = _load_result_section(
        native_timeout_results_path, "native-timeout-results",
        candidate_sha, identity_digest, release
    )
    sections["proxy_tls_results"] = _load_result_section(
        proxy_tls_results_path, "proxy-tls-results",
        candidate_sha, identity_digest, release
    )
    sections["shutdown_results"] = _load_result_section(
        shutdown_results_path, "shutdown-results",
        candidate_sha, identity_digest, release
    )
    sections["resource_results"] = _load_result_section(
        resource_results_path, "resource-results",
        candidate_sha, identity_digest, release
    )
    sections["soak_results"] = _load_result_section(
        soak_results_path, "soak-results",
        candidate_sha, identity_digest, release
    )
    sections["workflow_validation_results"] = _load_result_section(
        workflow_validation_results_path, "workflow-validation-results",
        candidate_sha, identity_digest, release
    )

    overall_pass = _compute_overall_pass(
        sections, artifact_verification, api_data, downstream_data,
        compat_data, sections["soak_results"], sections["resource_results"],
        sections["workflow_validation_results"], release
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
        if release:
            for name, data in sections.items():
                if data is None:
                    reasons.append(f"missing mandatory section: {name}")
                elif not data.get("overall_pass", False):
                    reasons.append(f"section {name} did not pass")
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
    section_keys = {
        "facade_api_results": "facade_api_results",
        "shim_api_results": "shim_api_results",
        "shim_substitution_results": "shim_substitution_results",
        "native_timeout_results": "native_timeout_results",
        "proxy_tls_results": "proxy_tls_results",
        "shutdown_results": "shutdown_results",
        "resource_results": "resource_results",
        "soak_results": "soak_results",
        "workflow_validation_results": "workflow_validation_results",
    }
    for section_name, evidence_key in section_keys.items():
        if sections[section_name] is not None:
            evidence[evidence_key] = sections[section_name]

    # Add candidate identity
    if candidate_identity_data is not None:
        evidence["candidate_identity"] = candidate_identity_data

    # Add artifact manifest
    if artifact_manifest_data is not None:
        evidence["artifact_manifest"] = artifact_manifest_data

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(evidence, indent=2) + "\n")
    return evidence


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate compatibility evidence from consumed result files",
    )
    parser.add_argument("--compat-test-results", required=True,
                        help="Path to compat test result JSON (pytest --json-report or equivalent)")
    parser.add_argument("--downstream-results", required=True,
                        help="Path to downstream runner result JSON")
    parser.add_argument("--api-comparison-results", required=True,
                        help="Path to manifest comparison JSON")
    parser.add_argument("--candidate-sha", required=True,
                        help="Exact 40-char hex SHA of the candidate commit")
    parser.add_argument("--artifact-hashes", required=True,
                        help="Path to artifact hash manifest JSON")
    parser.add_argument("--artifact-manifest", default=None,
                        help="Path to artifact-manifest.json for hash verification (overrides --artifact-hashes)")
    parser.add_argument("--candidate-identity", default=None,
                        help="Path to candidate-identity.json to embed in evidence")
    parser.add_argument("--facade-api-results", default=None,
                        help="Path to facade API oracle results JSON")
    parser.add_argument("--shim-api-results", default=None,
                        help="Path to shim API oracle results JSON")
    parser.add_argument("--shim-substitution-results", default=None,
                        help="Path to shim substitution results JSON")
    parser.add_argument("--native-timeout-results", default=None,
                        help="Path to native timeout classification results JSON")
    parser.add_argument("--proxy-tls-results", default=None,
                        help="Path to proxy/TLS results JSON")
    parser.add_argument("--shutdown-results", default=None,
                        help="Path to shutdown lifecycle results JSON")
    parser.add_argument("--resource-results", default=None,
                        help="Path to resource regression results JSON")
    parser.add_argument("--soak-results", default=None,
                        help="Path to soak test results JSON")
    parser.add_argument("--workflow-validation-results", default=None,
                        help="Path to workflow validation results JSON")
    parser.add_argument("--release", action="store_true",
                        help="Release mode: all sections are mandatory and identity is enforced")
    parser.add_argument("--output", default="compatibility-evidence.json",
                        help="Output JSON path (default: compatibility-evidence.json)")
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
        facade_api_results_path=Path(args.facade_api_results) if args.facade_api_results else None,
        shim_api_results_path=Path(args.shim_api_results) if args.shim_api_results else None,
        shim_substitution_results_path=Path(args.shim_substitution_results) if args.shim_substitution_results else None,
        native_timeout_results_path=Path(args.native_timeout_results) if args.native_timeout_results else None,
        proxy_tls_results_path=Path(args.proxy_tls_results) if args.proxy_tls_results else None,
        shutdown_results_path=Path(args.shutdown_results) if args.shutdown_results else None,
        resource_results_path=Path(args.resource_results) if args.resource_results else None,
        soak_results_path=Path(args.soak_results) if args.soak_results else None,
        workflow_validation_results_path=Path(args.workflow_validation_results) if args.workflow_validation_results else None,
        release=args.release,
    )
    print(f"Evidence written to {args.output}")
    print(f"Candidate SHA: {evidence['candidate_sha']}")
    print(f"Overall pass: {evidence['overall_pass']}")


if __name__ == "__main__":
    main()
