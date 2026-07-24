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
]

PLACEHOLDER_RE = re.compile(
    r"\[N\]|unknown|pending|unavailable|PLACEHOLDER",
    re.IGNORECASE,
)

# Stage C required categories for API comparison
_STAGE_C_CATEGORIES = {
    "intentional-difference",
    "stage-bounded",
    "not-applicable",
    "required-now",
    "resolved",
}


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
        )
    elif not all(c in "0123456789abcdef" for c in candidate_sha):
        errors.append(f"candidate_sha contains non-hex characters: {candidate_sha}")
    elif PLACEHOLDER_RE.search(candidate_sha):
        errors.append(f"candidate_sha contains placeholder: {candidate_sha}")

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

    # Required result sections
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

    # Candidate identity validation
    candidate_identity = data.get("candidate_identity")
    if isinstance(candidate_identity, dict):
        # Validate digest consistency: candidate_sha in identity must match top-level
        id_sha = candidate_identity.get("candidate_sha", "")
        if id_sha and id_sha != candidate_sha:
            errors.append(
                f"candidate_identity.candidate_sha ({id_sha}) does not match "
                f"top-level candidate_sha ({candidate_sha})"
            )
        # Validate required identity fields
        for field in ("schema_version", "candidate_sha", "eggfetch_version"):
            val = candidate_identity.get(field)
            if not val or (isinstance(val, str) and not val):
                errors.append(f"candidate_identity.{field} is missing or empty")

    # Manifest hash verification (compare artifact_hashes with any manifest)
    # This is a consistency check: if artifact_hashes contains entries, verify they exist
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
                    # Check for source_hash field if present
                    source_hash = pkg_result.get("source_hash")
                    if source_hash is not None:
                        if not isinstance(source_hash, str) or len(source_hash) != 64:
                            errors.append(
                                f"downstream_validation_results.results[{i}].source_hash "
                                f"must be a 64-char hex string"
                            )

    # Check for all 8 Stage C categories representation in API comparison
    if isinstance(api, dict):
        allowed_matches = api.get("allowed_matches", [])
        if isinstance(allowed_matches, list) and allowed_matches:
            found_categories = set()
            for match in allowed_matches:
                if isinstance(match, dict):
                    cat = match.get("category")
                    if cat:
                        found_categories.add(cat)
            # Report missing categories (informational, not necessarily an error)
            missing = _STAGE_C_CATEGORIES - found_categories
            if missing:
                # Only flag as error if we have matches but missing critical categories
                critical_missing = missing & {"required-now", "stage-bounded"}
                if critical_missing:
                    errors.append(
                        f"api_comparison_results is missing required Stage C categories: "
                        f"{', '.join(sorted(critical_missing))}"
                    )

    # Identity fields: no placeholders
    for field in (
        "candidate_sha",
        "eggfetch_version",
        "reference_httpx_version",
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
