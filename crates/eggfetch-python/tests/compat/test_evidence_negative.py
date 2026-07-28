"""Negative evidence fixture tests.

Track 9.3: Prove the evidence generator exits nonzero on specific defective inputs.
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parent.parent.parent.parent.parent / "scripts" / "generate_compatibility_evidence.py"
REPO_ROOT = SCRIPT.parent.parent


def _make_valid_compat_results(sha="a" * 40):
    return {
        "candidate_sha": sha,
        "total": 100,
        "passed": 100,
        "failures": 0,
        "errors": 0,
    }


def _make_valid_downstream_results(sha="a" * 40):
    return {
        "candidate_sha": sha,
        "overall_pass": True,
        "results": [{"package": "respx", "status": "passed", "tests": 10}],
    }


def _make_valid_api_results(sha="a" * 40):
    return {
        "candidate_sha": sha,
        "unexplained": [],
        "stale_allowed": [],
    }


def _make_valid_manifest(sha="a" * 40):
    """Create a valid artifact manifest with dummy wheel hashes."""
    return {
        "schema_version": "3",
        "candidate_sha": sha,
        "artifacts": [
            {
                "role": "eggfetch",
                "distribution": "eggfetch",
                "version": "0.1.0",
                "filename": "eggfetch-0.1.0-py3-none-any.whl",
                "relative_path": "wheels/eggfetch-0.1.0-py3-none-any.whl",
                "sha256": "a" * 64,
                "size_bytes": 100,
                "tags": "py3-none-any",
            },
            {
                "role": "httpx-controlled-replacement",
                "distribution": "httpx",
                "version": "0.28.1",
                "filename": "httpx-0.28.1-py3-none-any.whl",
                "relative_path": "wheels/httpx-0.28.1-py3-none-any.whl",
                "sha256": "b" * 64,
                "size_bytes": 200,
                "tags": "py3-none-any",
            },
        ],
    }


def _make_valid_identity(sha="a" * 40):
    """Create a valid candidate identity."""
    return {
        "schema_version": "4",
        "candidate_sha": sha,
        "artifact_manifest_sha256": "c" * 64,
        "identity_digest": "d" * 64,
        "eggfetch_version": "0.1.0",
        "reference_httpx_version": "0.28.1",
    }


def _run_evidence(compat, downstream, api, sha="a" * 40, extra_args=None):
    """Run evidence generator as subprocess. Returns (exit_code, stdout, stderr)."""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
        json.dump(compat, f)
        compat_path = f.name
    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
        json.dump(downstream, f)
        downstream_path = f.name
    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
        json.dump(api, f)
        api_path = f.name
    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
        json.dump(_make_valid_manifest(sha), f)
        manifest_path = f.name
    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
        json.dump(_make_valid_identity(sha), f)
        identity_path = f.name
    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
        output_path = f.name

    cmd = [
        sys.executable, str(SCRIPT),
        "--compat-test-results", compat_path,
        "--downstream-results", downstream_path,
        "--api-comparison-results", api_path,
        "--candidate-sha", sha,
        "--artifact-manifest", manifest_path,
        "--candidate-identity", identity_path,
        "--native-timeout-results", manifest_path,  # reuse as placeholder
        "--proxy-tls-results", manifest_path,
        "--shutdown-results", manifest_path,
        "--resource-results", manifest_path,
        "--soak-results", manifest_path,
        "--workflow-validation-results", manifest_path,
        "--output", output_path,
    ]
    if extra_args:
        cmd.extend(extra_args)

    result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
    for p in [compat_path, downstream_path, api_path, manifest_path, identity_path, output_path]:
        Path(p).unlink(missing_ok=True)
    return result.returncode, result.stdout, result.stderr


class TestNegativeEvidenceFixtures:
    """Each test proves the evidence generator fails on a specific defect."""

    def test_mismatched_sha_fails(self):
        """Evidence generator fails when result files have different SHAs."""
        compat = _make_valid_compat_results(sha="a" * 40)
        downstream = _make_valid_downstream_results(sha="b" * 40)
        api = _make_valid_api_results(sha="a" * 40)
        rc, stdout, stderr = _run_evidence(compat, downstream, api, sha="a" * 40)
        assert rc != 0, f"Expected nonzero exit for SHA mismatch.\nstdout: {stdout}\nstderr: {stderr}"
        assert "mismatch" in stderr.lower() or "mismatch" in stdout.lower(), \
            f"Expected mismatch error message.\nstdout: {stdout}\nstderr: {stderr}"

    def test_missing_downstream_result_fails(self):
        """Evidence generator fails when downstream results file is missing."""
        cmd = [
            sys.executable, str(SCRIPT),
            "--compat-test-results", "/nonexistent/compat.json",
            "--downstream-results", "/nonexistent/downstream.json",
            "--api-comparison-results", "/nonexistent/api.json",
            "--candidate-sha", "a" * 40,
            "--artifact-manifest", "/nonexistent/manifest.json",
            "--candidate-identity", "/nonexistent/identity.json",
            "--native-timeout-results", "/nonexistent/native.json",
            "--proxy-tls-results", "/nonexistent/proxy.json",
            "--shutdown-results", "/nonexistent/shutdown.json",
            "--resource-results", "/nonexistent/resource.json",
            "--soak-results", "/nonexistent/soak.json",
            "--workflow-validation-results", "/nonexistent/workflow.json",
            "--output", "/tmp/test-evidence-output.json",
        ]
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        assert result.returncode != 0, \
            f"Expected nonzero exit for missing files.\nstdout: {result.stdout}\nstderr: {result.stderr}"

    def test_no_tests_collected_fails(self):
        """Evidence generator fails when total tests is 0."""
        compat = _make_valid_compat_results()
        compat["total"] = 0
        compat["passed"] = 0
        downstream = _make_valid_downstream_results()
        api = _make_valid_api_results()
        rc, stdout, stderr = _run_evidence(compat, downstream, api)
        assert rc != 0, f"Expected nonzero exit for zero tests.\nstdout: {stdout}\nstderr: {stderr}"

    def test_test_failure_fails(self):
        """Evidence generator fails when compat tests have failures."""
        compat = _make_valid_compat_results()
        compat["failures"] = 5
        downstream = _make_valid_downstream_results()
        api = _make_valid_api_results()
        rc, stdout, stderr = _run_evidence(compat, downstream, api)
        assert rc != 0, f"Expected nonzero exit for test failures.\nstdout: {stdout}\nstderr: {stderr}"

    def test_downstream_overall_pass_false_fails(self):
        """Evidence generator fails when downstream overall_pass is False."""
        compat = _make_valid_compat_results()
        downstream = _make_valid_downstream_results()
        downstream["overall_pass"] = False
        api = _make_valid_api_results()
        rc, stdout, stderr = _run_evidence(compat, downstream, api)
        assert rc != 0, f"Expected nonzero exit for downstream failure.\nstdout: {stdout}\nstderr: {stderr}"

    def test_unexplained_api_differences_fails(self):
        """Evidence generator fails when API comparison has unexplained differences."""
        compat = _make_valid_compat_results()
        downstream = _make_valid_downstream_results()
        api = _make_valid_api_results()
        api["unexplained"] = [{"symbol": "MissingSymbol", "type": "missing"}]
        rc, stdout, stderr = _run_evidence(compat, downstream, api)
        assert rc != 0, f"Expected nonzero exit for unexplained API differences.\nstdout: {stdout}\nstderr: {stderr}"

    def test_placeholder_in_result_fails(self):
        """Evidence generator fails when result contains placeholder values."""
        compat = _make_valid_compat_results()
        compat["total"] = "[N]"
        downstream = _make_valid_downstream_results()
        api = _make_valid_api_results()
        rc, stdout, stderr = _run_evidence(compat, downstream, api)
        assert rc != 0, f"Expected nonzero exit for placeholder value.\nstdout: {stdout}\nstderr: {stderr}"

    def test_stale_allowed_differences_fails(self):
        """Evidence generator fails when API comparison has stale allowed entries."""
        compat = _make_valid_compat_results()
        downstream = _make_valid_downstream_results()
        api = _make_valid_api_results()
        api["stale_allowed"] = [{"id": "STALE-001", "symbol": "Foo"}]
        rc, stdout, stderr = _run_evidence(compat, downstream, api)
        assert rc != 0, f"Expected nonzero exit for stale allowed differences.\nstdout: {stdout}\nstderr: {stderr}"

    def test_malformed_json_fails(self):
        """Evidence generator fails on malformed JSON input."""
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            f.write("not valid json {{{")
            bad_path = f.name
        cmd = [
            sys.executable, str(SCRIPT),
            "--compat-test-results", bad_path,
            "--downstream-results", bad_path,
            "--api-comparison-results", bad_path,
            "--candidate-sha", "a" * 40,
            "--artifact-manifest", bad_path,
            "--candidate-identity", bad_path,
            "--native-timeout-results", bad_path,
            "--proxy-tls-results", bad_path,
            "--shutdown-results", bad_path,
            "--resource-results", bad_path,
            "--soak-results", bad_path,
            "--workflow-validation-results", bad_path,
            "--output", "/tmp/test-evidence-malformed.json",
        ]
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        Path(bad_path).unlink(missing_ok=True)
        assert result.returncode != 0, \
            f"Expected nonzero exit for malformed JSON.\nstdout: {result.stdout}\nstderr: {result.stderr}"
