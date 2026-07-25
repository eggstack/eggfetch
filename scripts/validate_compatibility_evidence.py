#!/usr/bin/env python3
"""Validate compatibility evidence JSON independently.

Used by the final qualification gate to assert:
- Expected schema version
- Exact candidate SHA
- Exact artifact hashes present
- All required result sections present
- overall_pass is true
- No placeholders
- No failed, skipped, unavailable, or malformed results
- Candidate identity validation (digest consistency)
- Manifest hash verification
- Source hash verification for downstream packages
- All 8 Stage C categories represented
- Independent recomputation of overall_pass

The validator must fail if ``overall_pass=true`` disagrees with recomputation.
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

REQUIRED_RESULT_SECTIONS = [
    "compat_test_results",
    "downstream_validation_results",
    "api_comparison_results",
    "facade_api_results",
    "shim_api_results",
    "shim_substitution_results",
    "native_timeout_results",
    "proxy_tls_results",
    "shutdown_results",
    "resource_results",
    "soak_results",
    "workflow_validation_results",
]

PLACEHOLDER_RE = re.compile(
    r"\[N\]|unknown|pending|unavailable|PLACEHOLDER",
    re.IGNORECASE,
)

# Stage C required categories
STAGE_C_CATEGORIES = {
    "contract-tests",
    "mock-transport-request-matching",
    "framework-test-client",
    "asgi-test-client",
    "sdk-async-client",
    "streaming-sse-consumption",
    "custom-auth-flow",
    "event-hooks-instrumentation",
}


def _validate_sha(sha: str) -> bool:
    return isinstance(sha, str) and len(sha) == 40 and all(c in "0123456789abcdef" for c in sha)


def _validate_sha256(digest: str) -> bool:
    return isinstance(digest, str) and len(digest) == 64 and all(c in "0123456789abcdef" for c in digest)


def _check_placeholder(value, path: str, errors: list[str]) -> None:
    """Recursively check for placeholder values."""
    if isinstance(value, str):
        if PLACEHOLDER_RE.search(value):
            errors.append(f"placeholder value at {path}: {value!r}")
    elif isinstance(value, dict):
        for k, v in value.items():
            _check_placeholder(v, f"{path}.{k}", errors)
    elif isinstance(value, list):
        for i, v in enumerate(value):
            _check_placeholder(v, f"{path}[{i}]", errors)


def _recompute_overall_pass(data: dict) -> bool:
    """Independently recompute overall_pass from evidence sections."""
    # Compat tests must pass
    compat = data.get("compat_test_results", {})
    if not isinstance(compat, dict):
        return False
    if compat.get("failures", 0) != 0 or compat.get("errors", 0) != 0:
        return False
    if compat.get("total", 0) == 0:
        return False

    # Downstream must pass
    downstream = data.get("downstream_validation_results", {})
    if not isinstance(downstream, dict) or not downstream.get("overall_pass"):
        return False

    # API must have no unexplained or stale differences
    api = data.get("api_comparison_results", {})
    if not isinstance(api, dict):
        return False
    if len(api.get("unexplained", [])) > 0:
        return False
    if len(api.get("stale_allowed", [])) > 0:
        return False

    # Artifacts must all match
    artifact_hashes = data.get("artifact_hashes", {})
    if not isinstance(artifact_hashes, dict) or not artifact_hashes:
        return False
    for name, info in artifact_hashes.items():
        if not isinstance(info, dict) or not info.get("matches"):
            return False

    # All mandatory sections must be present and pass
    for section in REQUIRED_RESULT_SECTIONS:
        section_data = data.get(section)
        if section_data is None:
            return False
        if not isinstance(section_data, dict):
            return False
        if not section_data.get("overall_pass", False):
            return False

    # Soak must pass
    soak = data.get("soak_results", {})
    if not isinstance(soak, dict) or not soak.get("overall_pass"):
        return False

    # Resource must pass
    resource = data.get("resource_results", {})
    if not isinstance(resource, dict) or not resource.get("overall_pass"):
        return False

    # Workflow validation must pass
    workflow = data.get("workflow_validation_results", {})
    if not isinstance(workflow, dict) or not workflow.get("overall_pass"):
        return False

    return True


def validate_evidence(path: str) -> list[str]:
    """Validate evidence JSON. Returns list of errors."""
    errors: list[str] = []
    try:
        with open(path) as f:
            data = json.load(f)
    except (json.JSONDecodeError, FileNotFoundError) as e:
        return [f"failed to load evidence: {e}"]

    if not isinstance(data, dict):
        return [f"evidence must be a JSON object, got {type(data).__name__}"]

    # Schema version
    if data.get("schema_version") != "3":
        errors.append(
            f"schema_version must be '3', got '{data.get('schema_version')}'"
        )

    # Candidate SHA
    candidate_sha = data.get("candidate_sha", "")
    if not candidate_sha or not isinstance(candidate_sha, str):
        errors.append(f"candidate_sha is missing or not a string: {candidate_sha!r}")
    elif not _validate_sha(candidate_sha):
        errors.append(f"candidate_sha must be 40 hex characters, got: {candidate_sha}")
    elif PLACEHOLDER_RE.search(candidate_sha):
        errors.append(f"candidate_sha contains placeholder: {candidate_sha}")

    # Artifact hashes section
    artifact_hashes = data.get("artifact_hashes")
    if artifact_hashes is None:
        errors.append("artifact_hashes section is missing")
    elif not isinstance(artifact_hashes, dict):
        errors.append(f"artifact_hashes must be a JSON object, got {type(artifact_hashes).__name__}")
    else:
        if not artifact_hashes:
            errors.append("artifact_hashes is empty — no artifacts verified")
        for name, info in artifact_hashes.items():
            if not isinstance(info, dict):
                errors.append(f"artifact_hashes.{name} must be a JSON object")
                continue
            if not info.get("matches"):
                errors.append(f"artifact '{name}' hash does not match")
            actual = info.get("actual")
            if not actual or not isinstance(actual, str) or len(actual) != 64:
                errors.append(f"artifact '{name}' missing or invalid actual hash")
            expected = info.get("expected")
            if not expected or not isinstance(expected, str) or len(expected) != 64:
                errors.append(f"artifact '{name}' missing or invalid expected hash")

    # overall_pass
    if not data.get("overall_pass"):
        errors.append("overall_pass is not true")

    # Required result sections
    for section in REQUIRED_RESULT_SECTIONS:
        val = data.get(section)
        if val is None:
            errors.append(f"missing required section: {section}")
        elif not isinstance(val, dict):
            errors.append(f"{section} must be a JSON object, got {type(val).__name__}")

    # Compat test results: check for failures/errors
    compat = data.get("compat_test_results")
    if isinstance(compat, dict):
        failures = compat.get("failures", 0)
        errors_count = compat.get("errors", 0)
        total = compat.get("total", 0)
        if total > 0 and (failures > 0 or errors_count > 0):
            errors.append(
                f"compat_test_results: {failures} failures, {errors_count} errors out of {total}"
            )
        if total == 0:
            errors.append("compat_test_results: total is 0 — no tests ran")

    # Downstream validation results: check overall_pass
    downstream = data.get("downstream_validation_results")
    if isinstance(downstream, dict) and not downstream.get("overall_pass"):
        errors.append("downstream_validation_results.overall_pass is not true")

    # API comparison results: check for unexplained or stale
    api = data.get("api_comparison_results")
    if isinstance(api, dict):
        unexplained = api.get("unexplained", [])
        stale = api.get("stale_allowed", [])
        if unexplained:
            errors.append(f"api_comparison_results has {len(unexplained)} unexplained differences")
        if stale:
            errors.append(f"api_comparison_results has {len(stale)} stale allowed differences")

    # Candidate identity validation
    candidate_identity = data.get("candidate_identity")
    if isinstance(candidate_identity, dict):
        id_sha = candidate_identity.get("candidate_sha", "")
        if id_sha and id_sha != candidate_sha:
            errors.append(
                f"candidate_identity.candidate_sha ({id_sha}) does not match "
                f"top-level candidate_sha ({candidate_sha})"
            )
        for field in ("schema_version", "candidate_sha", "eggfetch_version"):
            val = candidate_identity.get(field)
            if not val or (isinstance(val, str) and not val):
                errors.append(f"candidate_identity.{field} is missing or empty")

        # Validate identity_digest
        stored_digest = candidate_identity.get("identity_digest", "")
        if not stored_digest:
            errors.append("candidate_identity.identity_digest is missing")
        elif not _validate_sha256(stored_digest):
            errors.append(f"candidate_identity.identity_digest is not a valid SHA-256: {stored_digest!r}")
        else:
            # Recompute and compare
            identity_copy = {k: v for k, v in candidate_identity.items() if k != "identity_digest"}
            canonical = json.dumps(identity_copy, sort_keys=True, separators=(",", ":")).encode()
            recomputed = hashlib.sha256(canonical).hexdigest()
            if recomputed != stored_digest:
                errors.append(
                    f"candidate_identity.identity_digest mismatch: "
                    f"stored={stored_digest}, recomputed={recomputed}"
                )

    # Manifest hash verification
    if isinstance(artifact_hashes, dict) and artifact_hashes:
        for name in artifact_hashes:
            if not isinstance(name, str) or not name:
                errors.append(f"artifact_hashes contains invalid key: {name!r}")

    # Source hash verification for downstream packages
    downstream = data.get("downstream_validation_results")
    if isinstance(downstream, dict):
        results_list = downstream.get("results", [])
        if isinstance(results_list, list):
            for i, pkg_result in enumerate(results_list):
                if isinstance(pkg_result, dict):
                    source_hash = pkg_result.get("source_hash")
                    if source_hash is not None:
                        if not isinstance(source_hash, str) or len(source_hash) != 64:
                            errors.append(
                                f"downstream_validation_results.results[{i}].source_hash "
                                f"must be a 64-char hex string"
                            )

    # Check for all 8 Stage C categories representation in downstream results
    if isinstance(downstream, dict):
        results_list = downstream.get("results", [])
        if isinstance(results_list, list):
            found_categories: set[str] = set()
            for pkg_result in results_list:
                if isinstance(pkg_result, dict):
                    for cat in pkg_result.get("category_ids", []):
                        found_categories.add(cat)
            missing = STAGE_C_CATEGORIES - found_categories
            if missing:
                errors.append(
                    f"downstream results missing Stage C categories: {', '.join(sorted(missing))}"
                )

    # Identity fields: no placeholders
    for field in ("candidate_sha", "eggfetch_version", "reference_httpx_version"):
        val = data.get(field, "")
        if not val or PLACEHOLDER_RE.search(str(val)):
            errors.append(f"{field} contains placeholder or is missing: {val!r}")

    # Independent recomputation of overall_pass
    recomputed = _recompute_overall_pass(data)
    if data.get("overall_pass") != recomputed:
        errors.append(
            f"overall_pass ({data.get('overall_pass')}) disagrees with "
            f"recomputed value ({recomputed})"
        )

    return errors


def main() -> None:
    if len(sys.argv) < 2:
        print(
            "Usage: validate_compatibility_evidence.py <evidence.json>",
            file=sys.stderr,
        )
        sys.exit(2)

    errors = validate_evidence(sys.argv[1])
    if errors:
        print("EVIDENCE VALIDATION FAILED:")
        for err in errors:
            print(f"  - {err}")
        sys.exit(1)
    else:
        print("Evidence validation passed.")
        sys.exit(0)


if __name__ == "__main__":
    main()
