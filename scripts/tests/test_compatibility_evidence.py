#!/usr/bin/env python3
"""Negative tests for generate_compatibility_evidence.py and validate_compatibility_evidence.py.

Exercises defect classes from §15 items 72-82 related to evidence handling.
Each test creates temporary JSON files representing malformed evidence and
asserts the scripts reject them.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

VALIDATE_SCRIPT = Path(__file__).resolve().parent.parent / "validate_compatibility_evidence.py"
GENERATE_SCRIPT = Path(__file__).resolve().parent.parent / "generate_compatibility_evidence.py"

SHA_40 = "a" * 40
SHA_64 = "b" * 64
GOOD_SHAS = {
    "compat": "c" * 40,
    "downstream": "d" * 40,
    "api": "e" * 40,
    "native_timeout": "f" * 40,
    "proxy_tls": "1" * 40,
    "shutdown": "2" * 40,
    "resource": "3" * 40,
    "soak": "4" * 40,
    "workflow": "5" * 40,
}


def _run_validate(evidence_path: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(VALIDATE_SCRIPT), str(evidence_path)],
        capture_output=True,
        text=True,
        timeout=30,
    )


def _run_generate(args: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(GENERATE_SCRIPT)] + args,
        capture_output=True,
        text=True,
        timeout=30,
    )


def _make_result_file(tmp: Path, name: str, data: dict) -> Path:
    p = tmp / name
    p.write_text(json.dumps(data))
    return p


def _good_result(tmp: Path, name: str, sha: str = SHA_40) -> Path:
    return _make_result_file(tmp, name, {
        "candidate_sha": sha,
        "identity_digest": SHA_64,
        "status": "passed",
        "total": 10,
        "passed": 10,
        "failures": 0,
        "errors": 0,
    })


def _good_identity(tmp: Path, sha: str = SHA_40, manifest_sha: str = SHA_64) -> Path:
    return _make_result_file(tmp, "identity.json", {
        "schema_version": "4",
        "candidate_sha": sha,
        "eggfetch_version": "1.0.0",
        "identity_digest": SHA_64,
        "artifact_manifest_sha256": manifest_sha,
    })


def _good_manifest(tmp: Path, manifest_sha: str = SHA_64) -> Path:
    data = {
        "artifacts": [
            {
                "role": "eggfetch",
                "filename": "eggfetch-1.0.0-py3-none-any.whl",
                "relative_path": "wheels/eggfetch-1.0.0-py3-none-any.whl",
                "sha256": manifest_sha,
                "size_bytes": 100,
            }
        ]
    }
    return _make_result_file(tmp, "manifest.json", data)


class TestValidateMalformedJSON:
    def test_invalid_json_syntax(self, tmp_path):
        p = tmp_path / "bad.json"
        p.write_text("{not valid json!!!")
        result = _run_validate(p)
        assert result.returncode == 1
        assert "failed to load evidence" in result.stdout

    def test_json_array_not_object(self, tmp_path):
        p = tmp_path / "array.json"
        p.write_text("[1, 2, 3]")
        result = _run_validate(p)
        assert result.returncode == 1
        assert "must be a JSON object" in result.stdout

    def test_json_string_not_object(self, tmp_path):
        p = tmp_path / "string.json"
        p.write_text('"just a string"')
        result = _run_validate(p)
        assert result.returncode == 1

    def test_nonexistent_file(self, tmp_path):
        result = _run_validate(tmp_path / "nope.json")
        assert result.returncode == 1
        assert "failed to load evidence" in result.stdout


class TestValidateSchemaVersion:
    def test_wrong_schema_version(self, tmp_path):
        evidence = {
            "schema_version": "2",
            "candidate_sha": SHA_40,
            "identity_digest": SHA_64,
            "eggfetch_version": "1.0.0",
            "reference_httpx_version": "0.28.1",
            "overall_pass": True,
            "artifact_hashes": {"wheel.whl": {"matches": True, "actual": SHA_64, "expected": SHA_64}},
            "compat_test_results": {"total": 10, "passed": 10, "failures": 0, "errors": 0},
            "downstream_validation_results": {"overall_pass": True},
            "api_comparison_results": {"unexplained": [], "stale_allowed": []},
            "native_timeout_results": {"status": "passed"},
            "proxy_tls_results": {"status": "passed"},
            "shutdown_results": {"status": "passed"},
            "resource_results": {"status": "passed"},
            "soak_results": {"status": "passed"},
            "workflow_validation_results": {"status": "passed"},
        }
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "schema_version" in result.stdout

    def test_missing_schema_version(self, tmp_path):
        evidence = {
            "candidate_sha": SHA_40,
            "identity_digest": SHA_64,
            "eggfetch_version": "1.0.0",
            "reference_httpx_version": "0.28.1",
            "overall_pass": True,
            "artifact_hashes": {},
            "compat_test_results": {"total": 10, "passed": 10, "failures": 0, "errors": 0},
            "downstream_validation_results": {"overall_pass": True},
            "api_comparison_results": {"unexplained": [], "stale_allowed": []},
            "native_timeout_results": {"status": "passed"},
            "proxy_tls_results": {"status": "passed"},
            "shutdown_results": {"status": "passed"},
            "resource_results": {"status": "passed"},
            "soak_results": {"status": "passed"},
            "workflow_validation_results": {"status": "passed"},
        }
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "schema_version" in result.stdout


class TestValidateCandidateSHA:
    def test_short_sha(self, tmp_path):
        evidence = _base_evidence()
        evidence["candidate_sha"] = "abc123"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "40 hex characters" in result.stdout

    def test_non_hex_sha(self, tmp_path):
        evidence = _base_evidence()
        evidence["candidate_sha"] = "z" * 40
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "non-hex" in result.stdout

    def test_placeholder_sha(self, tmp_path):
        evidence = _base_evidence()
        evidence["candidate_sha"] = "unknown"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "placeholder" in result.stdout

    def test_missing_sha(self, tmp_path):
        evidence = _base_evidence()
        del evidence["candidate_sha"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "candidate_sha" in result.stdout


class TestValidateIdentityDigest:
    def test_short_identity_digest(self, tmp_path):
        evidence = _base_evidence()
        evidence["identity_digest"] = "abc"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "64 hex characters" in result.stdout

    def test_missing_identity_digest(self, tmp_path):
        evidence = _base_evidence()
        del evidence["identity_digest"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "identity_digest" in result.stdout


class TestValidateOverallPass:
    def test_overall_pass_false(self, tmp_path):
        evidence = _base_evidence()
        evidence["overall_pass"] = False
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "overall_pass is not true" in result.stdout

    def test_overall_pass_missing(self, tmp_path):
        evidence = _base_evidence()
        del evidence["overall_pass"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1


class TestValidateMissingSections:
    def test_missing_compat_test_results(self, tmp_path):
        evidence = _base_evidence()
        del evidence["compat_test_results"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "compat_test_results" in result.stdout

    def test_missing_downstream_validation_results(self, tmp_path):
        evidence = _base_evidence()
        del evidence["downstream_validation_results"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "downstream_validation_results" in result.stdout

    def test_missing_api_comparison_results(self, tmp_path):
        evidence = _base_evidence()
        del evidence["api_comparison_results"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "api_comparison_results" in result.stdout

    def test_missing_native_timeout_results(self, tmp_path):
        evidence = _base_evidence()
        del evidence["native_timeout_results"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "native_timeout_results" in result.stdout

    def test_missing_proxy_tls_results(self, tmp_path):
        evidence = _base_evidence()
        del evidence["proxy_tls_results"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "proxy_tls_results" in result.stdout

    def test_missing_shutdown_results(self, tmp_path):
        evidence = _base_evidence()
        del evidence["shutdown_results"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "shutdown_results" in result.stdout

    def test_missing_resource_results(self, tmp_path):
        evidence = _base_evidence()
        del evidence["resource_results"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "resource_results" in result.stdout

    def test_missing_soak_results(self, tmp_path):
        evidence = _base_evidence()
        del evidence["soak_results"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "soak_results" in result.stdout

    def test_missing_workflow_validation_results(self, tmp_path):
        evidence = _base_evidence()
        del evidence["workflow_validation_results"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "workflow_validation_results" in result.stdout


class TestValidatePlaceholders:
    def test_placeholder_in_eggfetch_version(self, tmp_path):
        evidence = _base_evidence()
        evidence["eggfetch_version"] = "unknown"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "eggfetch_version" in result.stdout
        assert "placeholder" in result.stdout

    def test_placeholder_in_reference_version(self, tmp_path):
        evidence = _base_evidence()
        evidence["reference_httpx_version"] = "[N]"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "reference_httpx_version" in result.stdout

    def test_placeholder_in_identity_digest(self, tmp_path):
        evidence = _base_evidence()
        evidence["identity_digest"] = "pending"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1


class TestValidateFailedResults:
    def test_compat_test_failures(self, tmp_path):
        evidence = _base_evidence()
        evidence["compat_test_results"]["failures"] = 3
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "3 failures" in result.stdout

    def test_compat_test_zero_total(self, tmp_path):
        evidence = _base_evidence()
        evidence["compat_test_results"]["total"] = 0
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "total is 0" in result.stdout

    def test_downstream_overall_pass_false(self, tmp_path):
        evidence = _base_evidence()
        evidence["downstream_validation_results"]["overall_pass"] = False
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "downstream_validation_results.overall_pass is not true" in result.stdout

    def test_native_timeout_failed(self, tmp_path):
        evidence = _base_evidence()
        evidence["native_timeout_results"]["status"] = "failed"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "native_timeout_results.status is 'failed'" in result.stdout

    def test_proxy_tls_failed(self, tmp_path):
        evidence = _base_evidence()
        evidence["proxy_tls_results"]["status"] = "failed"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "proxy_tls_results.status is 'failed'" in result.stdout

    def test_shutdown_failed(self, tmp_path):
        evidence = _base_evidence()
        evidence["shutdown_results"]["status"] = "failed"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "shutdown_results.status is 'failed'" in result.stdout

    def test_resource_failed(self, tmp_path):
        evidence = _base_evidence()
        evidence["resource_results"]["status"] = "failed"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "resource_results.status is 'failed'" in result.stdout

    def test_soak_failed(self, tmp_path):
        evidence = _base_evidence()
        evidence["soak_results"]["status"] = "failed"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "soak_results.status is 'failed'" in result.stdout

    def test_workflow_validation_failed(self, tmp_path):
        evidence = _base_evidence()
        evidence["workflow_validation_results"]["status"] = "failed"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "workflow_validation_results.status is 'failed'" in result.stdout


class TestValidateApiComparison:
    def test_unexplained_differences(self, tmp_path):
        evidence = _base_evidence()
        evidence["api_comparison_results"]["unexplained"] = ["func_a", "func_b"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "2 unexplained" in result.stdout

    def test_stale_allowed_differences(self, tmp_path):
        evidence = _base_evidence()
        evidence["api_comparison_results"]["stale_allowed"] = ["old_func"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "1 stale" in result.stdout


class TestValidateCandidateIdentity:
    def test_identity_sha_mismatch(self, tmp_path):
        evidence = _base_evidence()
        evidence["candidate_identity"] = {
            "schema_version": "4",
            "candidate_sha": "f" * 40,
            "eggfetch_version": "1.0.0",
        }
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "does not match" in result.stdout

    def test_identity_missing_eggfetch_version(self, tmp_path):
        evidence = _base_evidence()
        evidence["candidate_identity"] = {
            "schema_version": "4",
            "candidate_sha": SHA_40,
        }
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "eggfetch_version" in result.stdout


class TestValidateArtifactHashes:
    def test_missing_artifact_hashes(self, tmp_path):
        evidence = _base_evidence()
        del evidence["artifact_hashes"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "artifact_hashes" in result.stdout

    def test_empty_artifact_hashes(self, tmp_path):
        evidence = _base_evidence()
        evidence["artifact_hashes"] = {}
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "empty" in result.stdout

    def test_hash_mismatch(self, tmp_path):
        evidence = _base_evidence()
        evidence["artifact_hashes"] = {
            "wheel.whl": {
                "matches": False,
                "actual": SHA_64,
                "expected": "c" * 64,
            }
        }
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "does not match" in result.stdout


class TestGenerateMissingArgs:
    def test_missing_required_arg(self, tmp_path):
        result = _run_generate(["--candidate-sha", SHA_40])
        assert result.returncode != 0

    def test_missing_candidate_sha(self, tmp_path):
        result = _run_generate([
            "--compat-test-results", str(tmp_path / "x.json"),
            "--downstream-results", str(tmp_path / "x.json"),
            "--api-comparison-results", str(tmp_path / "x.json"),
            "--artifact-manifest", str(tmp_path / "x.json"),
            "--candidate-identity", str(tmp_path / "x.json"),
            "--native-timeout-results", str(tmp_path / "x.json"),
            "--proxy-tls-results", str(tmp_path / "x.json"),
            "--shutdown-results", str(tmp_path / "x.json"),
            "--resource-results", str(tmp_path / "x.json"),
            "--soak-results", str(tmp_path / "x.json"),
            "--workflow-validation-results", str(tmp_path / "x.json"),
            "--output", str(tmp_path / "out.json"),
        ])
        assert result.returncode != 0


class TestGenerateMalformedInputs:
    def test_invalid_json_input(self, tmp_path):
        bad = tmp_path / "bad.json"
        bad.write_text("{invalid")
        result = _run_generate([
            "--compat-test-results", str(bad),
            "--downstream-results", str(bad),
            "--api-comparison-results", str(bad),
            "--artifact-manifest", str(bad),
            "--candidate-identity", str(bad),
            "--native-timeout-results", str(bad),
            "--proxy-tls-results", str(bad),
            "--shutdown-results", str(bad),
            "--resource-results", str(bad),
            "--soak-results", str(bad),
            "--workflow-validation-results", str(bad),
            "--candidate-sha", SHA_40,
            "--output", str(tmp_path / "out.json"),
        ])
        assert result.returncode == 1
        assert "FATAL" in result.stderr

    def test_wrong_sha_length(self, tmp_path):
        good = _make_result_file(tmp_path, "ok.json", {"status": "passed"})
        result = _run_generate([
            "--compat-test-results", str(good),
            "--downstream-results", str(good),
            "--api-comparison-results", str(good),
            "--artifact-manifest", str(good),
            "--candidate-identity", str(good),
            "--native-timeout-results", str(good),
            "--proxy-tls-results", str(good),
            "--shutdown-results", str(good),
            "--resource-results", str(good),
            "--soak-results", str(good),
            "--workflow-validation-results", str(good),
            "--candidate-sha", "tooshort",
            "--output", str(tmp_path / "out.json"),
        ])
        assert result.returncode == 1
        assert "FATAL" in result.stderr


class TestEvidenceConsumesTwoDifferentIdentityDigests:
    def test_digest_length_mismatch(self, tmp_path):
        evidence = _base_evidence()
        evidence["identity_digest"] = "a" * 32
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "64 hex characters" in result.stdout


class TestEvidenceOmitsResourceResult:
    def test_resource_results_not_dict(self, tmp_path):
        evidence = _base_evidence()
        evidence["resource_results"] = "passed"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "resource_results must be a JSON object" in result.stdout


class TestEvidenceOmitsOneAPIOracleResult:
    def test_api_results_not_dict(self, tmp_path):
        evidence = _base_evidence()
        evidence["api_comparison_results"] = []
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "api_comparison_results must be a JSON object" in result.stdout


class TestEvidencePlaceholderValues:
    def test_pending_in_identity(self, tmp_path):
        evidence = _base_evidence()
        evidence["identity_digest"] = "pending"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1

    def test_unavailable_in_version(self, tmp_path):
        evidence = _base_evidence()
        evidence["eggfetch_version"] = "unavailable"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1

    def test_unavailable_in_identity_digest(self, tmp_path):
        evidence = _base_evidence()
        evidence["identity_digest"] = "unavailable"
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1


class TestEvidenceGuessedWheelPath:
    def test_artifact_hash_missing_actual(self, tmp_path):
        evidence = _base_evidence()
        evidence["artifact_hashes"] = {
            "wheel.whl": {
                "matches": False,
                "actual": None,
                "expected": SHA_64,
            }
        }
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        assert "missing or invalid actual hash" in result.stdout


class TestSkippedJobAcceptance:
    def test_skipped_not_flagged_by_validator(self, tmp_path):
        evidence = _base_evidence()
        evidence["native_timeout_results"] = {"status": "skipped"}
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 0


class TestMultipleEvidenceDefects:
    def test_cascading_failures(self, tmp_path):
        evidence = _base_evidence()
        evidence["schema_version"] = "1"
        evidence["candidate_sha"] = "short"
        evidence["identity_digest"] = "wrong"
        evidence["overall_pass"] = False
        evidence["compat_test_results"]["failures"] = 5
        del evidence["resource_results"]
        p = tmp_path / "evidence.json"
        p.write_text(json.dumps(evidence))
        result = _run_validate(p)
        assert result.returncode == 1
        output = result.stdout
        assert "schema_version" in output
        assert "40 hex characters" in output
        assert "64 hex characters" in output
        assert "overall_pass is not true" in output
        assert "5 failures" in output
        assert "resource_results" in output


def _base_evidence() -> dict:
    return {
        "schema_version": "3",
        "candidate_sha": SHA_40,
        "identity_digest": SHA_64,
        "eggfetch_version": "1.0.0",
        "reference_httpx_version": "0.28.1",
        "overall_pass": True,
        "artifact_hashes": {
            "wheel.whl": {
                "matches": True,
                "actual": SHA_64,
                "expected": SHA_64,
            }
        },
        "compat_test_results": {
            "total": 100,
            "passed": 100,
            "failures": 0,
            "errors": 0,
        },
        "downstream_validation_results": {"overall_pass": True},
        "api_comparison_results": {"unexplained": [], "stale_allowed": []},
        "native_timeout_results": {"status": "passed"},
        "proxy_tls_results": {"status": "passed"},
        "shutdown_results": {"status": "passed"},
        "resource_results": {"status": "passed"},
        "soak_results": {"status": "passed"},
        "workflow_validation_results": {"status": "passed"},
        "candidate_identity": {
            "schema_version": "4",
            "candidate_sha": SHA_40,
            "eggfetch_version": "1.0.0",
        },
    }
