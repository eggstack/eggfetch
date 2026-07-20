"""Tests for evaluate_ci_gate.py.

Each test invokes the script as a subprocess and asserts on the exit code.
Tests use temporary JSON files to supply input and policy data.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

SCRIPT = str(Path(__file__).resolve().parent / "evaluate_ci_gate.py")

DEFAULT_POLICY: dict = {
    "required_jobs": ["job-a", "job-b", "job-c"],
    "conditional_jobs": {
        "job-d": {
            "condition": "runs on ubuntu only",
            "skip_when": "platform is not ubuntu",
        }
    },
    "allowed_skip_results": ["success", "skipped"],
    "fail_results": ["failure", "cancelled"],
}


def _write_json(tmpdir: Path, name: str, data: dict) -> str:
    """Write *data* as JSON and return the file path."""
    path = tmpdir / name
    path.write_text(json.dumps(data), encoding="utf-8")
    return str(path)


def _run(script_args: list[str]) -> subprocess.CompletedProcess[str]:
    """Run the evaluator and return the CompletedProcess."""
    return subprocess.run(
        [sys.executable, SCRIPT, *script_args],
        capture_output=True,
        text=True,
    )


def _write_input(
    tmpdir: Path,
    results: dict[str, str],
    policy: dict | None = None,
    policy_file: str | None = None,
) -> str:
    """Write an input JSON file. If *policy* is given it is written as a
    sibling file and referenced via policy_file."""
    policy_path: str | None = policy_file
    if policy is not None:
        policy_path = _write_json(tmpdir, "policy.json", policy)
    payload: dict = {"results": results}
    if policy_path is not None:
        payload["policy_file"] = os.path.basename(policy_path)
    return _write_json(tmpdir, "input.json", payload)


# ── Case 1: All required jobs succeed → exit 0 ───────────────────────

def test_all_required_succeed(tmp_path: Path) -> None:
    results = {"job-a": "success", "job-b": "success", "job-c": "success"}
    inp = _write_input(tmp_path, results, DEFAULT_POLICY)
    proc = _run([inp])
    assert proc.returncode == 0, proc.stderr


# ── Case 2: One required job fails → exit non-zero ───────────────────

def test_one_required_fails(tmp_path: Path) -> None:
    results = {"job-a": "success", "job-b": "failure", "job-c": "success"}
    inp = _write_input(tmp_path, results, DEFAULT_POLICY)
    proc = _run([inp])
    assert proc.returncode != 0
    assert "job-b" in proc.stderr


# ── Case 3: One required job is cancelled → exit non-zero ────────────

def test_one_required_cancelled(tmp_path: Path) -> None:
    results = {"job-a": "success", "job-b": "cancelled", "job-c": "success"}
    inp = _write_input(tmp_path, results, DEFAULT_POLICY)
    proc = _run([inp])
    assert proc.returncode != 0
    assert "cancelled" in proc.stderr


# ── Case 4: One required job is missing from results → exit non-zero ─

def test_one_required_missing(tmp_path: Path) -> None:
    results = {"job-a": "success", "job-c": "success"}
    inp = _write_input(tmp_path, results, DEFAULT_POLICY)
    proc = _run([inp])
    assert proc.returncode != 0
    assert "missing" in proc.stderr


# ── Case 5: One required job is unexpectedly skipped → exit non-zero ─

def test_one_required_skipped(tmp_path: Path) -> None:
    results = {"job-a": "success", "job-b": "skipped", "job-c": "success"}
    inp = _write_input(tmp_path, results, DEFAULT_POLICY)
    proc = _run([inp])
    assert proc.returncode != 0
    assert "unexpected-skip" in proc.stderr


# ── Case 6: Conditional job skipped (condition false) → exit 0 ───────

def test_conditional_job_skipped(tmp_path: Path) -> None:
    results = {
        "job-a": "success",
        "job-b": "success",
        "job-c": "success",
        "job-d": "skipped",
    }
    inp = _write_input(tmp_path, results, DEFAULT_POLICY)
    proc = _run([inp])
    assert proc.returncode == 0, proc.stderr


# ── Case 7: Unknown result value → exit non-zero ─────────────────────

def test_unknown_result_value(tmp_path: Path) -> None:
    results = {"job-a": "success", "job-b": "bogus", "job-c": "success"}
    inp = _write_input(tmp_path, results, DEFAULT_POLICY)
    proc = _run([inp])
    assert proc.returncode != 0
    assert "unknown" in proc.stderr


# ── Case 8: Malformed input (not valid JSON) → exit non-zero ────────

def test_malformed_json(tmp_path: Path) -> None:
    bad = tmp_path / "bad.json"
    bad.write_text("{not valid json!!!", encoding="utf-8")
    proc = _run([str(bad)])
    assert proc.returncode != 0


# ── Case 9: Multiple simultaneous failures are all reported ──────────

def test_multiple_failures_reported(tmp_path: Path) -> None:
    results = {
        "job-a": "failure",
        "job-b": "cancelled",
        "job-c": "success",
    }
    inp = _write_input(tmp_path, results, DEFAULT_POLICY)
    proc = _run([inp])
    assert proc.returncode != 0
    assert "job-a" in proc.stderr
    assert "job-b" in proc.stderr


# ── Case 10: Evaluator cannot find its configuration file → exit non-zero

def test_missing_policy_file(tmp_path: Path) -> None:
    results = {"job-a": "success"}
    payload = {
        "results": results,
        "policy_file": "does-not-exist.json",
    }
    inp = _write_json(tmp_path, "input.json", payload)
    proc = _run([inp])
    assert proc.returncode != 0
    assert "not found" in proc.stderr.lower() or "error" in proc.stderr.lower()


# ── Edge case: no arguments → exit 2 ─────────────────────────────────

def test_no_arguments() -> None:
    proc = subprocess.run(
        [sys.executable, SCRIPT],
        capture_output=True,
        text=True,
    )
    assert proc.returncode == 2


# ── Edge case: conditional job fails → still non-zero ────────────────

def test_conditional_job_fails(tmp_path: Path) -> None:
    results = {
        "job-a": "success",
        "job-b": "success",
        "job-c": "success",
        "job-d": "failure",
    }
    inp = _write_input(tmp_path, results, DEFAULT_POLICY)
    proc = _run([inp])
    assert proc.returncode != 0
    assert "job-d" in proc.stderr


# ── Edge case: conditional job succeeds → exit 0 ─────────────────────

def test_conditional_job_succeeds(tmp_path: Path) -> None:
    results = {
        "job-a": "success",
        "job-b": "success",
        "job-c": "success",
        "job-d": "success",
    }
    inp = _write_input(tmp_path, results, DEFAULT_POLICY)
    proc = _run([inp])
    assert proc.returncode == 0, proc.stderr
