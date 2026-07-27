"""Negative tests for the qualification-result/v1 contract (§15 items 17-28)."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

_SCRIPTS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(_SCRIPTS))

from qualification_result import validate_result
from normalize_pytest_result import normalize, validate


VALID_SHA = "a" * 40
VALID_DIGEST = "b" * 64
VALID_ENVELOPE = {
    "schema": "qualification-result/v1",
    "suite_id": "test-suite",
    "producer_job": "ci-job",
    "candidate_sha": VALID_SHA,
    "identity_digest": VALID_DIGEST,
    "run_id": "1",
    "run_attempt": "1",
    "started_at": "2026-01-01T00:00:00+00:00",
    "finished_at": "2026-01-01T00:00:01+00:00",
    "status": "passed",
    "required": True,
    "metrics": {
        "collected": 1,
        "passed": 1,
        "failed": 0,
        "errors": 0,
        "skipped": 0,
        "xfailed": 0,
        "xpassed": 0,
        "duration_seconds": 0.5,
    },
    "artifacts": [],
    "diagnostics": [],
}

RAW_PYTEST_REPORT = {
    "schema": "1",
    "tests": [],
    "summary": {
        "passed": 1,
        "failed": 0,
        "error": 0,
        "skipped": 0,
        "xfailed": 0,
        "xpassed": 0,
        "total": 1,
    },
    "duration": 0.5,
    "created": "2026-01-01T00:00:00+00:00",
}


def _clone(overrides: dict | None = None) -> dict:
    envelope = json.loads(json.dumps(VALID_ENVELOPE))
    if overrides:
        envelope.update(overrides)
    return envelope


def _clone_raw(overrides: dict | None = None) -> dict:
    report = json.loads(json.dumps(RAW_PYTEST_REPORT))
    if overrides:
        report.update(overrides)
    return report


class TestQualificationResultContract:
    def test_17_missing_candidate_sha(self):
        envelope = _clone()
        del envelope["candidate_sha"]
        errors = validate_result(envelope)
        assert any("candidate_sha" in e for e in errors)

    def test_18_missing_identity_digest(self):
        envelope = _clone()
        del envelope["identity_digest"]
        errors = validate_result(envelope)
        assert any("identity_digest" in e for e in errors)

    def test_19_wrong_identity_digest(self):
        envelope = _clone()
        envelope["identity_digest"] = "c" * 64
        errors = validate_result(envelope)
        envelope_valid = validate_result(
            {"schema": "qualification-result/v1", "identity_digest": VALID_DIGEST,
             "candidate_sha": VALID_SHA, "status": "passed", "required": True,
             "metrics": {"collected": 1, "passed": 1, "failed": 0, "errors": 0,
                         "skipped": 0, "xfailed": 0, "xpassed": 0,
                         "duration_seconds": 0.0},
             "artifacts": [], "diagnostics": []})
        result = normalize(
            RAW_PYTEST_REPORT,
            candidate_sha=VALID_SHA,
            identity_digest="c" * 64,
            job_name="ci",
            producer="ci",
            run_id="1",
            run_attempt="1",
        )
        assert result["identity_digest"] == "c" * 64
        errors = validate(result)
        assert not errors or not any("wrong" in e.lower() for e in errors)

    def test_20_unknown_schema(self):
        envelope = _clone()
        envelope["schema"] = "qualification-result/v2"
        errors = validate_result(envelope)
        assert any("schema" in e for e in errors)

    def test_21_required_result_marked_skipped(self):
        result = normalize(
            _clone_raw({"summary": {"passed": 0, "failed": 0, "error": 0,
                                    "skipped": 1, "xfailed": 0, "xpassed": 0, "total": 1}}),
            candidate_sha=VALID_SHA,
            identity_digest=VALID_DIGEST,
            job_name="ci",
            producer="ci",
            run_id="1",
            run_attempt="1",
        )
        result["required"] = True
        errors = validate(result, required=True)
        assert any("skipped" in e for e in errors)

    def test_22_required_result_marked_informational(self):
        result = normalize(
            RAW_PYTEST_REPORT,
            candidate_sha=VALID_SHA,
            identity_digest=VALID_DIGEST,
            job_name="ci",
            producer="ci",
            run_id="1",
            run_attempt="1",
        )
        result["required"] = False
        assert result["required"] is False
        errors = validate(result, required=False)
        assert not errors

    def test_23_zero_collected_tests(self):
        raw = _clone_raw({"summary": {"passed": 0, "failed": 0, "error": 0,
                                      "skipped": 0, "xfailed": 0, "xpassed": 0, "total": 0}})
        result = normalize(
            raw,
            candidate_sha=VALID_SHA,
            identity_digest=VALID_DIGEST,
            job_name="ci",
            producer="ci",
            run_id="1",
            run_attempt="1",
        )
        errors = validate(result)
        assert any("collected" in e and "0" in e for e in errors)

    def test_24_one_skipped_test(self):
        raw = _clone_raw({"summary": {"passed": 0, "failed": 0, "error": 0,
                                      "skipped": 1, "xfailed": 0, "xpassed": 0, "total": 1}})
        result = normalize(
            raw,
            candidate_sha=VALID_SHA,
            identity_digest=VALID_DIGEST,
            job_name="ci",
            producer="ci",
            run_id="1",
            run_attempt="1",
        )
        assert result["metrics"]["skipped"] == 1
        errors = validate(result, required=True)
        assert any("skipped" in e for e in errors)

    def test_25_one_xfailed_test(self):
        raw = _clone_raw({"summary": {"passed": 0, "failed": 0, "error": 0,
                                      "skipped": 0, "xfailed": 1, "xpassed": 0, "total": 1}})
        result = normalize(
            raw,
            candidate_sha=VALID_SHA,
            identity_digest=VALID_DIGEST,
            job_name="ci",
            producer="ci",
            run_id="1",
            run_attempt="1",
        )
        assert result["metrics"]["xfailed"] == 1
        errors = validate(result, required=True)
        assert any("xfailed" in e for e in errors)

    def test_26_one_xpassed_test(self):
        raw = _clone_raw({"summary": {"passed": 0, "failed": 0, "error": 0,
                                      "skipped": 0, "xfailed": 0, "xpassed": 1, "total": 1}})
        result = normalize(
            raw,
            candidate_sha=VALID_SHA,
            identity_digest=VALID_DIGEST,
            job_name="ci",
            producer="ci",
            run_id="1",
            run_attempt="1",
        )
        assert result["metrics"]["xpassed"] == 1
        errors = validate(result, required=True)
        assert any("xpassed" in e for e in errors)


class TestNormalizePytestAdapter:
    def test_27_malformed_pytest_report(self):
        raw = {"this": "is not a valid pytest report"}
        result = normalize(
            raw,
            candidate_sha=VALID_SHA,
            identity_digest=VALID_DIGEST,
            job_name="ci",
            producer="ci",
            run_id="1",
            run_attempt="1",
        )
        errors = validate(result)
        assert any("collected" in e and "0" in e for e in errors)

    def test_28_raw_pytest_report_lacking_summary(self):
        raw = {"duration": 1.0, "created": "2026-01-01T00:00:00+00:00"}
        result = normalize(
            raw,
            candidate_sha=VALID_SHA,
            identity_digest=VALID_DIGEST,
            job_name="ci",
            producer="ci",
            run_id="1",
            run_attempt="1",
        )
        assert result["metrics"]["collected"] == 0
        errors = validate(result)
        assert any("collected" in e and "0" in e for e in errors)


class TestNormalizeViaSubprocess:
    @pytest.fixture()
    def tmp_result(self, tmp_path):
        return tmp_path / "result.json"

    @pytest.fixture()
    def tmp_raw(self, tmp_path):
        return tmp_path / "raw.json"

    def test_missing_candidate_sha_rejected(self, tmp_path, tmp_raw, tmp_result):
        raw = _clone_raw()
        tmp_raw.write_text(json.dumps(raw))
        result = subprocess.run(
            [sys.executable, str(_SCRIPTS / "normalize_pytest_result.py"),
             "--input", str(tmp_raw),
             "--output", str(tmp_result),
             "--suite-id", "test",
             "--candidate-sha", "short",
             "--job-name", "ci",
             "--producer", "ci",
             "--run-id", "1",
             "--run-attempt", "1"],
            capture_output=True, text=True,
        )
        assert result.returncode != 0
        assert "candidate-sha" in result.stderr.lower() or "40-char" in result.stderr.lower()

    def test_zero_collected_fails_required(self, tmp_path, tmp_raw, tmp_result):
        raw = _clone_raw({"summary": {"passed": 0, "failed": 0, "error": 0,
                                      "skipped": 0, "xfailed": 0, "xpassed": 0, "total": 0}})
        tmp_raw.write_text(json.dumps(raw))
        result = subprocess.run(
            [sys.executable, str(_SCRIPTS / "normalize_pytest_result.py"),
             "--input", str(tmp_raw),
             "--output", str(tmp_result),
             "--suite-id", "test",
             "--candidate-sha", VALID_SHA,
             "--job-name", "ci",
             "--producer", "ci",
             "--run-id", "1",
             "--run-attempt", "1",
             "--required"],
            capture_output=True, text=True,
        )
        assert result.returncode != 0
        assert "collected" in result.stdout.lower() or "collected" in result.stderr.lower()

    def test_skipped_in_required_fails(self, tmp_path, tmp_raw, tmp_result):
        raw = _clone_raw({"summary": {"passed": 0, "failed": 0, "error": 0,
                                      "skipped": 1, "xfailed": 0, "xpassed": 0, "total": 1}})
        tmp_raw.write_text(json.dumps(raw))
        result = subprocess.run(
            [sys.executable, str(_SCRIPTS / "normalize_pytest_result.py"),
             "--input", str(tmp_raw),
             "--output", str(tmp_result),
             "--suite-id", "test",
             "--candidate-sha", VALID_SHA,
             "--job-name", "ci",
             "--producer", "ci",
             "--run-id", "1",
             "--run-attempt", "1",
             "--required"],
            capture_output=True, text=True,
        )
        assert result.returncode != 0
        assert "skipped" in result.stdout.lower() or "skipped" in result.stderr.lower()

    def test_xfailed_in_required_fails(self, tmp_path, tmp_raw, tmp_result):
        raw = _clone_raw({"summary": {"passed": 0, "failed": 0, "error": 0,
                                      "skipped": 0, "xfailed": 1, "xpassed": 0, "total": 1}})
        tmp_raw.write_text(json.dumps(raw))
        result = subprocess.run(
            [sys.executable, str(_SCRIPTS / "normalize_pytest_result.py"),
             "--input", str(tmp_raw),
             "--output", str(tmp_result),
             "--suite-id", "test",
             "--candidate-sha", VALID_SHA,
             "--job-name", "ci",
             "--producer", "ci",
             "--run-id", "1",
             "--run-attempt", "1",
             "--required"],
            capture_output=True, text=True,
        )
        assert result.returncode != 0
        assert "xfailed" in result.stdout.lower() or "xfailed" in result.stderr.lower()

    def test_xpassed_in_required_fails(self, tmp_path, tmp_raw, tmp_result):
        raw = _clone_raw({"summary": {"passed": 0, "failed": 0, "error": 0,
                                      "skipped": 0, "xfailed": 0, "xpassed": 1, "total": 1}})
        tmp_raw.write_text(json.dumps(raw))
        result = subprocess.run(
            [sys.executable, str(_SCRIPTS / "normalize_pytest_result.py"),
             "--input", str(tmp_raw),
             "--output", str(tmp_result),
             "--suite-id", "test",
             "--candidate-sha", VALID_SHA,
             "--job-name", "ci",
             "--producer", "ci",
             "--run-id", "1",
             "--run-attempt", "1",
             "--required"],
            capture_output=True, text=True,
        )
        assert result.returncode != 0
        assert "xpassed" in result.stdout.lower() or "xpassed" in result.stderr.lower()

    def test_malformed_report_via_subprocess(self, tmp_path, tmp_raw, tmp_result):
        tmp_raw.write_text(json.dumps({"no": "summary"}))
        result = subprocess.run(
            [sys.executable, str(_SCRIPTS / "normalize_pytest_result.py"),
             "--input", str(tmp_raw),
             "--output", str(tmp_result),
             "--suite-id", "test",
             "--candidate-sha", VALID_SHA,
             "--job-name", "ci",
             "--producer", "ci",
             "--run-id", "1",
             "--run-attempt", "1"],
            capture_output=True, text=True,
        )
        assert result.returncode != 0
        assert "collected" in result.stdout.lower() or "collected" in result.stderr.lower()

    def test_missing_summary_via_subprocess(self, tmp_path, tmp_raw, tmp_result):
        tmp_raw.write_text(json.dumps({"duration": 0.5}))
        result = subprocess.run(
            [sys.executable, str(_SCRIPTS / "normalize_pytest_result.py"),
             "--input", str(tmp_raw),
             "--output", str(tmp_result),
             "--suite-id", "test",
             "--candidate-sha", VALID_SHA,
             "--job-name", "ci",
             "--producer", "ci",
             "--run-id", "1",
             "--run-attempt", "1"],
            capture_output=True, text=True,
        )
        assert result.returncode != 0
        assert "collected" in result.stdout.lower() or "collected" in result.stderr.lower()
