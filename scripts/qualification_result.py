#!/usr/bin/env python3
"""Shared qualification-result/v1 contract module.

Provides construction, validation, and serialization for the unified
result envelope required by all release-blocking qualification jobs.

Schema:
    qualification-result/v1

Required top-level fields:
    schema, suite_id, producer_job, candidate_sha, identity_digest,
    run_id, run_attempt, started_at, finished_at, status, required,
    metrics, artifacts, diagnostics

Allowed statuses: "passed" and "failed" only.

Usage as library:
    from qualification_result import build_result, validate_result, write_result

Usage as CLI:
    python qualification_result.py validate --input result.json
    python qualification_result.py build --suite-id foo --candidate-sha ... --status passed --output result.json
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "qualification-result/v1"
_ALLOWED_STATUSES = frozenset({"passed", "failed"})


def _now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def _validate_sha40(sha: str, label: str) -> None:
    if not isinstance(sha, str) or len(sha) != 40:
        raise ValueError(f"{label}: must be a 40-char hex string, got {sha!r}")
    if not all(c in "0123456789abcdef" for c in sha):
        raise ValueError(f"{label}: contains non-hex characters: {sha}")


def _validate_digest64(digest: str, label: str) -> None:
    if not isinstance(digest, str) or len(digest) != 64:
        raise ValueError(f"{label}: must be a 64-char hex string, got {digest!r}")
    if not all(c in "0123456789abcdef" for c in digest):
        raise ValueError(f"{label}: contains non-hex characters: {digest}")


def build_result(
    *,
    suite_id: str,
    producer_job: str,
    candidate_sha: str,
    identity_digest: str,
    run_id: str,
    run_attempt: str,
    status: str,
    metrics: dict[str, Any] | None = None,
    artifacts: list[dict[str, Any]] | None = None,
    diagnostics: list[str] | None = None,
    required: bool = True,
    started_at: str | None = None,
    finished_at: str | None = None,
) -> dict[str, Any]:
    """Build a qualification-result/v1 envelope.

    Validates all required fields and returns a complete result dict.
    Raises ValueError on invalid inputs.
    """
    _validate_sha40(candidate_sha, "candidate_sha")
    _validate_digest64(identity_digest, "identity_digest")

    if status not in _ALLOWED_STATUSES:
        raise ValueError(
            f"status must be one of {sorted(_ALLOWED_STATUSES)}, got {status!r}"
        )

    if not suite_id:
        raise ValueError("suite_id is required")
    if not producer_job:
        raise ValueError("producer_job is required")

    return {
        "schema": SCHEMA_VERSION,
        "suite_id": suite_id,
        "producer_job": producer_job,
        "candidate_sha": candidate_sha,
        "identity_digest": identity_digest,
        "run_id": run_id,
        "run_attempt": run_attempt,
        "started_at": started_at or _now_iso(),
        "finished_at": finished_at or _now_iso(),
        "status": status,
        "required": required,
        "metrics": metrics or {},
        "artifacts": artifacts or [],
        "diagnostics": diagnostics or [],
    }


def validate_result(result: dict[str, Any], *, strict: bool = True) -> list[str]:
    """Validate a qualification-result/v1 envelope.

    Returns a list of error strings. Empty list means valid.
    When strict=True, enforces identity_digest and candidate_sha presence.
    """
    errors: list[str] = []

    if not isinstance(result, dict):
        return ["result must be a JSON object"]

    # Schema version
    if result.get("schema") != SCHEMA_VERSION:
        errors.append(
            f"schema must be '{SCHEMA_VERSION}', got {result.get('schema')!r}"
        )

    # Required top-level fields
    for field in (
        "suite_id", "producer_job", "candidate_sha", "identity_digest",
        "run_id", "run_attempt", "started_at", "finished_at",
        "status", "required", "metrics", "artifacts", "diagnostics",
    ):
        if field not in result:
            errors.append(f"missing required field: {field}")

    if errors:
        return errors

    # Status
    if result["status"] not in _ALLOWED_STATUSES:
        errors.append(
            f"status must be one of {sorted(_ALLOWED_STATUSES)}, got {result['status']!r}"
        )

    # SHA validation
    candidate_sha = result.get("candidate_sha", "")
    if strict and (not isinstance(candidate_sha, str) or len(candidate_sha) != 40):
        errors.append(f"candidate_sha must be a 40-char hex string")
    elif strict and not all(c in "0123456789abcdef" for c in candidate_sha):
        errors.append(f"candidate_sha contains non-hex characters")

    identity_digest = result.get("identity_digest", "")
    if strict and (not isinstance(identity_digest, str) or len(identity_digest) != 64):
        errors.append(f"identity_digest must be a 64-char hex string")
    elif strict and not all(c in "0123456789abcdef" for c in identity_digest):
        errors.append(f"identity_digest contains non-hex characters")

    # Type checks
    if not isinstance(result.get("metrics", {}), dict):
        errors.append("metrics must be a JSON object")
    if not isinstance(result.get("artifacts", []), list):
        errors.append("artifacts must be a JSON array")
    if not isinstance(result.get("diagnostics", []), list):
        errors.append("diagnostics must be a JSON array")
    if not isinstance(result.get("required", True), bool):
        errors.append("required must be a boolean")
    if not isinstance(result.get("suite_id", ""), str) or not result["suite_id"]:
        errors.append("suite_id must be a non-empty string")

    return errors


def write_result(result: dict[str, Any], path: Path) -> None:
    """Write a result envelope to a JSON file."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(result, indent=2, sort_keys=False) + "\n")


def compute_identity_digest(result: dict[str, Any]) -> str:
    """Compute SHA-256 of the result envelope for identity purposes.

    Strips identity-digest-like fields before hashing to avoid circularity.
    """
    stripped = {k: v for k, v in result.items() if k != "identity_digest"}
    canonical = json.dumps(stripped, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(
        prog="qualification_result.py",
        description="Build or validate qualification-result/v1 envelopes",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    # validate subcommand
    val_p = sub.add_parser("validate", help="Validate a result JSON file")
    val_p.add_argument("--input", required=True, help="Path to result JSON")
    val_p.add_argument(
        "--strict", action="store_true", default=True,
        help="Enforce strict identity field validation (default: true)",
    )

    # build subcommand
    build_p = sub.add_parser("build", help="Build a result envelope")
    build_p.add_argument("--suite-id", required=True)
    build_p.add_argument("--producer-job", required=True)
    build_p.add_argument("--candidate-sha", required=True)
    build_p.add_argument("--identity-digest", required=True)
    build_p.add_argument("--run-id", required=True)
    build_p.add_argument("--run-attempt", required=True)
    build_p.add_argument("--status", required=True, choices=["passed", "failed"])
    build_p.add_argument("--required", action="store_true", default=True)
    build_p.add_argument("--output", required=True)

    args = parser.parse_args()

    if args.command == "validate":
        with open(args.input) as f:
            data = json.load(f)
        errors = validate_result(data, strict=args.strict)
        if errors:
            print(f"RESULT VALIDATION FAILED ({len(errors)} errors):")
            for e in errors:
                print(f"  - {e}")
            sys.exit(1)
        print("Result validation passed.")
        sys.exit(0)

    elif args.command == "build":
        result = build_result(
            suite_id=args.suite_id,
            producer_job=args.producer_job,
            candidate_sha=args.candidate_sha,
            identity_digest=args.identity_digest,
            run_id=args.run_id,
            run_attempt=args.run_attempt,
            status=args.status,
            required=args.required,
        )
        errors = validate_result(result)
        if errors:
            print("FATAL: built result has validation errors:")
            for e in errors:
                print(f"  - {e}")
            sys.exit(1)
        write_result(result, Path(args.output))
        print(f"Result written to {args.output}")


if __name__ == "__main__":
    main()
