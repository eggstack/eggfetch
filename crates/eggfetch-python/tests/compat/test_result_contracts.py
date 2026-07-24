"""Tests for the pytest result normalizer and result contract validation.

Track B: Versioned result contracts — every result must include
schema_version, candidate_identity, producer, run_id, run_attempt,
job_name, started_at, finished_at, status, errors, and metrics.
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parent.parent.parent.parent.parent / "scripts" / "normalize_pytest_result.py"

VALID_SHA = "a" * 40


def _make_raw_pytest_json(passed=10, failed=0, errors=0, skipped=0, xfailed=0, xpassed=0, duration=1.5):
    """Create a raw pytest-json-report compatible dict."""
    return {
        "summary": {
            "passed": passed,
            "failed": failed,
            "error": errors,
            "skipped": skipped,
            "xfailed": xfailed,
            "xpassed": xpassed,
            "total": passed + failed + errors + skipped + xfailed + xpassed,
        },
        "duration": duration,
        "created": "2026-01-01T00:00:00+00:00",
    }


def _run_normalize(raw, **kwargs):
    """Run the normalizer as a subprocess. Returns (exit_code, stdout, stderr)."""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
        json.dump(raw, f)
        input_path = f.name
    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
        output_path = f.name

    defaults = {
        "candidate_sha": VALID_SHA,
        "job_name": "test-job",
        "producer": "test-producer",
        "run_id": "12345",
        "run_attempt": "1",
    }
    defaults.update(kwargs)

    cmd = [
        sys.executable, str(SCRIPT),
        "--input", input_path,
        "--output", output_path,
        "--candidate-sha", defaults["candidate_sha"],
        "--job-name", defaults["job_name"],
        "--producer", defaults["producer"],
        "--run-id", defaults["run_id"],
        "--run-attempt", defaults["run_attempt"],
    ]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
    output = None
    if Path(output_path).exists():
        try:
            with open(output_path) as f:
                output = json.load(f)
        except json.JSONDecodeError:
            pass
    Path(input_path).unlink(missing_ok=True)
    Path(output_path).unlink(missing_ok=True)
    return result.returncode, result.stdout, result.stderr, output


class TestNormalization:
    """Tests for the pytest result normalizer."""

    def test_normalize_passed_suite(self):
        """A passing suite normalizes to status='passed'."""
        raw = _make_raw_pytest_json(passed=100, failed=0, errors=0, skipped=0)
        rc, stdout, stderr, output = _run_normalize(raw)
        assert rc == 0, f"Expected exit 0.\nstdout: {stdout}\nstderr: {stderr}"
        assert output is not None
        assert output["status"] == "passed"
        assert output["schema_version"] == "1"
        assert output["metrics"]["collected"] == 100
        assert output["metrics"]["passed"] == 100
        assert output["metrics"]["failed"] == 0
        assert output["metrics"]["skipped"] == 0

    def test_normalize_failed_suite(self):
        """A failing suite normalizes to status='failed'."""
        raw = _make_raw_pytest_json(passed=90, failed=10, errors=0, skipped=0)
        rc, stdout, stderr, output = _run_normalize(raw)
        assert rc == 0, f"Expected exit 0.\nstdout: {stdout}\nstderr: {stderr}"
        assert output["status"] == "failed"
        assert output["metrics"]["failed"] == 10

    def test_normalize_includes_required_fields(self):
        """Normalized result includes all required contract fields."""
        raw = _make_raw_pytest_json(passed=5)
        rc, stdout, stderr, output = _run_normalize(raw)
        assert rc == 0
        required = ["schema_version", "candidate_identity", "producer", "run_id",
                    "run_attempt", "job_name", "started_at", "finished_at",
                    "status", "errors", "metrics"]
        for field in required:
            assert field in output, f"Missing required field: {field}"

    def test_normalize_candidate_identity(self):
        """Candidate identity is embedded in the result."""
        raw = _make_raw_pytest_json(passed=5)
        rc, stdout, stderr, output = _run_normalize(raw, candidate_sha="b" * 40)
        assert rc == 0
        assert output["candidate_identity"]["candidate_sha"] == "b" * 40

    def test_normalize_skipped_suite_fails(self):
        """A suite with skipped tests fails normalization (fail-closed)."""
        raw = _make_raw_pytest_json(passed=95, failed=0, errors=0, skipped=5)
        rc, stdout, stderr, output = _run_normalize(raw)
        assert rc != 0, f"Expected nonzero exit for skipped tests.\nstdout: {stdout}"
        assert "skipped" in stdout.lower() or "skipped" in stderr.lower()

    def test_normalize_xfailed_suite_fails(self):
        """A suite with xfailed tests fails normalization."""
        raw = _make_raw_pytest_json(passed=95, failed=0, errors=0, skipped=0, xfailed=5)
        rc, stdout, stderr, output = _run_normalize(raw)
        assert rc != 0, f"Expected nonzero exit for xfailed tests.\nstdout: {stdout}"

    def test_normalize_zero_collected_fails(self):
        """A suite with zero collected tests fails normalization."""
        raw = _make_raw_pytest_json(passed=0, failed=0, errors=0, skipped=0)
        rc, stdout, stderr, output = _run_normalize(raw)
        assert rc != 0, f"Expected nonzero exit for zero collected.\nstdout: {stdout}"

    def test_normalize_collected_mismatch_fails(self):
        """collected != sum of outcomes fails normalization."""
        raw = {
            "summary": {"passed": 5, "failed": 0, "total": 10},
            "duration": 1.0,
            "created": "2026-01-01T00:00:00+00:00",
        }
        rc, stdout, stderr, output = _run_normalize(raw)
        assert rc != 0, f"Expected nonzero exit for collected mismatch.\nstdout: {stdout}"
        assert "collected" in stdout.lower() or "collected" in stderr.lower()

    def test_normalize_invalid_sha_fails(self):
        """Invalid candidate SHA fails normalization."""
        raw = _make_raw_pytest_json(passed=5)
        rc, stdout, stderr, output = _run_normalize(raw, candidate_sha="not-a-sha")
        assert rc != 0, f"Expected nonzero exit for invalid SHA.\nstdout: {stdout}"

    def test_validate_only_mode(self):
        """--validate-only validates an existing normalized result."""
        raw = _make_raw_pytest_json(passed=5)
        rc, stdout, stderr, output = _run_normalize(raw)
        assert rc == 0

        # Now validate the output
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump(output, f)
            norm_path = f.name

        cmd = [sys.executable, str(SCRIPT), "--input", norm_path,
               "--output", "/tmp/test-validate.json", "--validate-only"]
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        Path(norm_path).unlink(missing_ok=True)
        assert result.returncode == 0, f"Validation failed.\nstdout: {result.stdout}"
