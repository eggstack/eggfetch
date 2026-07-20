#!/usr/bin/env python3
"""evaluate_ci_gate.py — Evaluate GitHub Actions job results against a policy.

Accepts a JSON file containing job results and a policy definition, then
exits 0 only when all required results satisfy the policy.

Usage:
    evaluate_ci_gate.py <input.json>

The input JSON must have the structure:
{
    "results": {
        "job-name": "success|failure|cancelled|skipped"
    },
    "policy_file": "path/to/policy.json"   // optional, defaults to sibling
}

If policy_file is omitted, the script loads evaluate_ci_gate_policy.json
from the same directory as this script.

Exit codes:
    0 — all required jobs satisfied policy
    1 — one or more jobs failed policy
    2 — malformed input (not valid JSON, missing required fields)
    3 — policy file not found or unreadable
"""

from __future__ import annotations

import json
import os
import sys
from typing import Any

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
DEFAULT_POLICY_PATH = os.path.join(SCRIPT_DIR, "evaluate_ci_gate_policy.json")

VALID_RESULTS = {"success", "failure", "cancelled", "skipped"}


def load_json(path: str) -> dict[str, Any]:
    """Load and parse a JSON file. Raises on any error."""
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def load_policy(policy_path: str) -> dict[str, Any]:
    """Load and validate the policy file."""
    if not os.path.isfile(policy_path):
        raise FileNotFoundError(f"Policy file not found: {policy_path}")
    policy = load_json(policy_path)
    for key in ("required_jobs", "conditional_jobs"):
        if key not in policy:
            raise ValueError(f"Policy missing required key: {key}")
    return policy


def evaluate(
    results: dict[str, str],
    policy: dict[str, Any],
) -> list[str]:
    """Evaluate results against policy. Returns a list of error strings.

    An empty list means all checks passed (exit 0).
    """
    required_jobs: list[str] = policy["required_jobs"]
    conditional_jobs: dict[str, Any] = policy.get("conditional_jobs", {})
    fail_results: set[str] = set(policy.get("fail_results", ["failure", "cancelled"]))

    errors: list[str] = []

    # Check every required job
    for job in required_jobs:
        if job not in results:
            errors.append(f"[missing] Required job '{job}' has no result entry")
            continue
        result = results[job]
        if result not in VALID_RESULTS:
            errors.append(f"[unknown] Job '{job}' has unknown result '{result}'")
            continue
        if result in fail_results:
            errors.append(f"[fail] Required job '{job}' finished with result '{result}'")
        elif result == "skipped":
            # Skipped is only allowed for explicitly conditional jobs
            if job not in conditional_jobs:
                errors.append(
                    f"[unexpected-skip] Required job '{job}' was skipped "
                    "but is not classified as conditional"
                )

    # Check conditional jobs that appear in results
    for job, spec in conditional_jobs.items():
        if job not in results:
            # Conditional jobs missing from results are fine — they were
            # likely not triggered by the workflow matrix.
            continue
        result = results[job]
        if result not in VALID_RESULTS:
            errors.append(f"[unknown] Conditional job '{job}' has unknown result '{result}'")
            continue
        if result == "skipped":
            # Skip is acceptable for conditional jobs — the caller asserts
            # the documented condition was false.  No further check needed.
            pass
        elif result in fail_results:
            errors.append(f"[fail] Conditional job '{job}' finished with result '{result}'")

    return errors


def main(argv: list[str] | None = None) -> int:
    """Entry point. Returns an exit code."""
    if argv is None:
        argv = sys.argv[1:]

    if len(argv) != 1:
        print("Usage: evaluate_ci_gate.py <input.json>", file=sys.stderr)
        return 2

    input_path = argv[0]

    # ── Load input ────────────────────────────────────────────────────
    try:
        data = load_json(input_path)
    except (json.JSONDecodeError, OSError) as exc:
        print(f"ERROR: Cannot parse input file: {exc}", file=sys.stderr)
        return 2

    if not isinstance(data, dict) or "results" not in data:
        print("ERROR: Input JSON must contain a 'results' key", file=sys.stderr)
        return 2

    results = data["results"]
    if not isinstance(results, dict):
        print("ERROR: 'results' must be a JSON object", file=sys.stderr)
        return 2

    # ── Load policy ───────────────────────────────────────────────────
    policy_rel: str | None = data.get("policy_file")
    if policy_rel is not None:
        # Resolve relative to the input file's directory
        base = os.path.dirname(os.path.abspath(input_path))
        policy_path = os.path.normpath(os.path.join(base, policy_rel))
    else:
        policy_path = DEFAULT_POLICY_PATH

    try:
        policy = load_policy(policy_path)
    except (FileNotFoundError, ValueError, OSError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 3

    # ── Evaluate ──────────────────────────────────────────────────────
    errors = evaluate(results, policy)

    if errors:
        for err in errors:
            print(err, file=sys.stderr)
        return 1

    print("CI gate: all required jobs passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
