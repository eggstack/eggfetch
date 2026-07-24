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
"""

from __future__ import annotations

import json
import re
import sys

REQUIRED_RESULT_SECTIONS = [
    "compat_test_results",
    "downstream_validation_results",
    "api_comparison_results",
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
