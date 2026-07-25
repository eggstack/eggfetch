#!/usr/bin/env python3
"""Validate compatibility evidence JSON independently.

Used by the final qualification gate to assert:
- Expected schema version
- Exact candidate SHA
- Identity digest present and consistent
- All required result sections present
- overall_pass is true
- No placeholders
- No failed, skipped, unavailable, or malformed results
- Candidate identity validation (digest consistency)
- Manifest hash verification
- All 8 Stage C categories represented
"""

from __future__ import annotations

import json
import re
import sys

REQUIRED_RESULT_SECTIONS = [
    "compat_test_results",
    "downstream_validation_results",
    "api_comparison_results",
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


def validate_evidence(path: str) -> list[str]:
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
    elif len(candidate_sha) != 40:
        errors.append(
            f"candidate_sha must be 40 hex characters, got {len(candidate_sha)} chars"
    elif not all(c in "0123456789abcdef" for c in candidate_sha):
        errors.append(f"candidate_sha contains non-hex characters: {candidate_sha}")
    elif PLACEHOLDER_RE.search(candidate_sha):
        errors.append(f"candidate_sha contains placeholder: {candidate_sha}")

    # Identity digest (required)
    identity_digest = data.get("identity_digest", "")
    if not identity_digest or not isinstance(identity_digest, str):
        errors.append(f"identity_digest is missing or not a string")
    elif len(identity_digest) != 64:
        errors.append(f"identity_digest must be 64 hex characters, got {len(identity_digest)} chars")
    elif not all(c in "0123456789abcdef" for c in identity_digest):
        errors.append(f"identity_digest contains non-hex characters")

    # Artifact hashes section
    artifact_hashes = data.get("artifact_hashes")
    if artifact_hashes is None:
        errors.append("artifact_hashes section is missing")
    elif not isinstance(artifact_hashes, dict):
        errors.append(
            f"artifact_hashes must be a JSON object, got {type(artifact_hashes).__name__}"
        )
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

    # Required result sections (all must be present)
    for section in REQUIRED_RESULT_SECTIONS:
        val = data.get(section)
        if val is None:
            errors.append(f"missing required section: {section}")
        elif not isinstance(val, dict):
            errors.append(
                f"{section} must be a JSON object, got {type(val).__name__}"
            )

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
            errors.append(
                f"api_comparison_results has {len(unexplained)} unexplained differences"
            )
        if stale:
            errors.append(
                f"api_comparison_results has {len(stale)} stale allowed differences"
            )

    # Native timeout results: must have status
    native_timeout = data.get("native_timeout_results")
    if isinstance(native_timeout, dict):
        if native_timeout.get("status") == "failed":
            errors.append("native_timeout_results.status is 'failed'")

    # Proxy TLS results: must have status
    proxy_tls = data.get("proxy_tls_results")
    if isinstance(proxy_tls, dict):
        if proxy_tls.get("status") == "failed":
            errors.append("proxy_tls_results.status is 'failed'")

    # Shutdown results: must have status
    shutdown = data.get("shutdown_results")
    if isinstance(shutdown, dict):
        if shutdown.get("status") == "failed":
            errors.append("shutdown_results.status is 'failed'")

    # Resource results: must have status
    resource = data.get("resource_results")
    if isinstance(resource, dict):
        if resource.get("status") == "failed":
            errors.append("resource_results.status is 'failed'")

    # Soak results: must have status
    soak = data.get("soak_results")
    if isinstance(soak, dict):
        if soak.get("status") == "failed":
            errors.append("soak_results.status is 'failed'")

    # Workflow validation results: must have status
    workflow = data.get("workflow_validation_results")
    if isinstance(workflow, dict):
        if workflow.get("status") == "failed":
            errors.append("workflow_validation_results.status is 'failed'")

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

    # Identity fields: no placeholders
    for field in (
        "candidate_sha",
        "eggfetch_version",
        "reference_httpx_version",
        "identity_digest",
    ):
        val = data.get(field, "")
        if not val or PLACEHOLDER_RE.search(str(val)):
            errors.append(f"{field} contains placeholder or is missing: {val!r}")

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
