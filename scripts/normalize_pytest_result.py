#!/usr/bin/env python3
"""Normalize raw pytest JSON reports into versioned result contracts.

Converts ``pytest --json-report`` output into the schema-1 normalized format
required by the qualification evidence pipeline. Every normalized result
includes candidate identity, producer metadata, and terminal test counts.

Usage:
    normalize_pytest_result.py --input <pytest.json> --output <normalized.json> \\
        --candidate-sha <40-char-sha> --job-name <name> \\
        --producer <producer> --run-id <id> --run-attempt <n> \\
        [--required]

The normalizer enforces:
  - ``collected`` equals the sum of terminal outcomes
  - Required suites have ``collected > 0`` (error, not warning)
  - Required suites have zero skipped, xfailed, xpassed, failed, errors
  - Missing fields fail
  - Unknown schema versions fail

When ``--required`` is set, zero collected / zero terminal outcomes for
required suites is a hard failure.
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

SCHEMA_VERSION = "1"

# Terminal outcome keys in pytest JSON
_OUTCOME_KEYS = ("passed", "failed", "error", "skipped", "xfailed", "xpassed")


def _now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def normalize(raw: dict, candidate_sha: str, job_name: str,
              producer: str, run_id: str, run_attempt: str) -> dict:
    """Normalize a raw pytest JSON report into schema-1 contract.

    Accepts either the standard ``pytest-json-report`` schema (with
    ``tests``, ``summary``, ``duration``) or a simplified flat schema
    (with ``total``, ``passed``, ``failures``, ``errors``).
    """
    errors: list[str] = []

    # --- Extract counts from raw pytest JSON ---
    summary = raw.get("summary", {})
    if not summary and "total" in raw:
        # Simplified flat schema
        summary = {
            "passed": raw.get("passed", 0),
            "failed": raw.get("failures", 0),
            "error": raw.get("errors", 0),
            "skipped": raw.get("skipped", 0),
            "xfailed": raw.get("xfailed", 0),
            "xpassed": raw.get("xpassed", 0),
            "total": raw.get("total", 0),
        }

    counts = {}
    total = 0
    for key in _OUTCOME_KEYS:
        val = summary.get(key, 0)
        if not isinstance(val, int) or val < 0:
            errors.append(f"summary.{key} must be a non-negative integer, got {val!r}")
            val = 0
        counts[key] = val
        total += val

    # collected: prefer explicit collected, fall back to sum of outcomes
    collected = summary.get("total", total)
    if not isinstance(collected, int) or collected < 0:
        errors.append(f"summary.total must be a non-negative integer, got {collected!r}")
        collected = total

    # Validate collected == sum of terminal outcomes
    if collected != total:
        errors.append(
            f"collected ({collected}) != sum of terminal outcomes ({total})"
        )

    duration = raw.get("duration", 0.0)
    if not isinstance(duration, (int, float)) or duration < 0:
        errors.append(f"duration must be a non-negative number, got {duration!r}")
        duration = 0.0

    # Determine status
    has_failures = counts["failed"] > 0 or counts["error"] > 0
    status = "failed" if has_failures else "passed"

    # Ensure started_at is always a UTC timestamp
    raw_started = raw.get("created")
    if raw_started and isinstance(raw_started, str):
        try:
            dt = datetime.fromisoformat(raw_started)
            if dt.tzinfo is None:
                dt = dt.replace(tzinfo=timezone.utc)
            started_at = dt.astimezone(timezone.utc).isoformat()
        except ValueError:
            started_at = _now_iso()
    else:
        started_at = _now_iso()

    result: dict = {
        "schema_version": SCHEMA_VERSION,
        "candidate_identity": {
            "candidate_sha": candidate_sha,
        },
        "producer": producer,
        "run_id": run_id,
        "run_attempt": run_attempt,
        "job_name": job_name,
        "started_at": started_at,
        "finished_at": _now_iso(),
        "status": status,
        "errors": errors,
        "metrics": {
            "collected": collected,
            "passed": counts["passed"],
            "failed": counts["failed"],
            "errors": counts["error"],
            "skipped": counts["skipped"],
            "xfailed": counts["xfailed"],
            "xpassed": counts["xpassed"],
            "duration_seconds": round(float(duration), 3),
        },
    }

    return result


def validate(result: dict, required: bool = False) -> list[str]:
    """Validate a normalized result contract. Returns list of errors.

    When *required* is ``True``, stricter checks apply:
    - ``collected > 0`` is enforced (error, not warning).
    - Zero skip/xfail/xpassed/failed/errors is enforced for required suites.
    """
    errors: list[str] = []

    if not isinstance(result, dict):
        return ["result must be a JSON object"]

    required_fields = [
        "schema_version", "candidate_identity", "producer", "run_id",
        "run_attempt", "job_name", "started_at", "finished_at",
        "status", "errors", "metrics",
    ]
    for field in required_fields:
        if field not in result:
            errors.append(f"missing required field: {field}")

    if errors:
        return errors

    if result["schema_version"] != SCHEMA_VERSION:
        errors.append(
            f"schema_version must be '{SCHEMA_VERSION}', got '{result['schema_version']}'"
        )

    if not isinstance(result["errors"], list):
        errors.append("errors must be a list")

    if not isinstance(result["metrics"], dict):
        errors.append("metrics must be a JSON object")
    else:
        metrics = result["metrics"]
        for key in ("collected", "passed", "failed", "errors", "skipped", "xfailed", "xpassed"):
            if key not in metrics:
                errors.append(f"metrics.{key} is missing")
            elif not isinstance(metrics[key], int) or metrics[key] < 0:
                errors.append(f"metrics.{key} must be a non-negative integer, got {metrics[key]!r}")
        if "duration_seconds" not in metrics:
            errors.append("metrics.duration_seconds is missing")
        elif not isinstance(metrics["duration_seconds"], (int, float)) or metrics["duration_seconds"] < 0:
            errors.append(f"metrics.duration_seconds must be non-negative, got {metrics['duration_seconds']!r}")

        # Validate collected == sum(terminal outcomes)
        terminal_sum = sum(
            metrics.get(k, 0) for k in ("passed", "failed", "errors", "skipped", "xfailed", "xpassed")
        )
        collected = metrics.get("collected", 0)
        if collected != terminal_sum:
            errors.append(
                f"collected ({collected}) != sum of terminal outcomes ({terminal_sum})"
            )

        # collected > 0 is always an error (not a warning)
        if collected == 0:
            errors.append("collected is 0 — no tests were collected")

        # Required suites: zero skip/xfail/xpassed/failed/errors
        if required:
            for key in ("skipped", "xfailed", "xpassed", "failed", "errors"):
                if metrics.get(key, 0) > 0:
                    errors.append(
                        f"required suite has non-zero {key}: {metrics[key]}"
                    )

    # Status must be passed or failed
    if result["status"] not in ("passed", "failed"):
        errors.append(f"status must be 'passed' or 'failed', got '{result['status']}'")

    # If status is passed, no failures/errors/skips
    if result["status"] == "passed":
        m = result.get("metrics", {})
        for key in ("failed", "errors", "skipped", "xfailed", "xpassed"):
            if m.get(key, 0) > 0:
                errors.append(
                    f"status is 'passed' but metrics.{key} is {m[key]} > 0"
                )
        if m.get("collected", 0) == 0:
            errors.append("status is 'passed' but collected is 0")

    # Result-level errors list must be empty for a valid result
    if result.get("errors"):
        for e in result["errors"]:
            errors.append(f"result error: {e}")

    return errors


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Normalize raw pytest JSON into schema-1 result contract",
    )
    parser.add_argument("--input", required=True, help="Path to raw pytest JSON report")
    parser.add_argument("--output", required=True, help="Output normalized JSON path")
    parser.add_argument("--candidate-sha", help="40-char hex SHA")
    parser.add_argument("--job-name", help="Job name")
    parser.add_argument("--producer", help="Producer identifier")
    parser.add_argument("--run-id", help="GitHub run ID")
    parser.add_argument("--run-attempt", help="Run attempt number")
    parser.add_argument("--validate-only", action="store_true",
                        help="Only validate an existing normalized result")
    parser.add_argument("--required", action="store_true",
                        help="Enforce stricter validation for required suites "
                             "(zero skip/xfail/xpassed/failed/errors)")
    args = parser.parse_args()

    if args.validate_only:
        with open(args.input) as f:
            result = json.load(f)
        errors = validate(result, required=args.required)
        if errors:
            print(f"VALIDATION FAILED ({len(errors)} errors):")
            for e in errors:
                print(f"  - {e}")
            sys.exit(1)
        else:
            print("Result validation passed.")
            sys.exit(0)

    for field in ("candidate_sha", "job_name", "producer", "run_id", "run_attempt"):
        if not getattr(args, field):
            parser.error(f"--{field.replace('_', '-')} is required unless --validate-only is used")

    # Validate SHA format
    if len(args.candidate_sha) != 40 or not all(c in "0123456789abcdef" for c in args.candidate_sha):
        parser.error(f"--candidate-sha must be a 40-char hex string, got: {args.candidate_sha!r}")

    with open(args.input) as f:
        raw = json.load(f)

    result = normalize(
        raw,
        candidate_sha=args.candidate_sha,
        job_name=args.job_name,
        producer=args.producer,
        run_id=args.run_id,
        run_attempt=args.run_attempt,
    )

    # Self-validate
    errors = validate(result, required=args.required)
    if errors:
        print(f"FATAL: normalized result has validation errors:")
        for e in errors:
            print(f"  - {e}")
        sys.exit(1)

    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, "w") as f:
        json.dump(result, f, indent=2)
        f.write("\n")

    print(f"Normalized result written to {args.output}")
    print(f"  status: {result['status']}")
    print(f"  collected: {result['metrics']['collected']}")
    print(f"  passed: {result['metrics']['passed']}")
    print(f"  failed: {result['metrics']['failed']}")


if __name__ == "__main__":
    main()
