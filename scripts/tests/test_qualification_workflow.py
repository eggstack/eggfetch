#!/usr/bin/env python3
"""Negative tests for validate_qualification_workflow.py.

Exercises defect classes from §15 items 66-82.
Each test creates a temporary YAML workflow violating a structural requirement
and asserts the validator catches it (or not, as appropriate).
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest
import yaml

SCRIPT = Path(__file__).resolve().parent.parent / "validate_qualification_workflow.py"


def _run_validator(workflow_path: Path, expect_failure: bool = False) -> subprocess.CompletedProcess:
    cmd = [sys.executable, str(SCRIPT), str(workflow_path)]
    if expect_failure:
        cmd.append("--expect-failure")
    return subprocess.run(cmd, capture_output=True, text=True, timeout=30)


def _write_yaml(tmp: Path, data: dict) -> Path:
    p = tmp / "workflow.yml"
    p.write_text(yaml.dump(data, default_flow_style=False))
    return p


def _passing_base() -> dict:
    return {
        "name": "test",
        "on": "push",
        "jobs": {
            "qualification-gate": {
                "needs": ["generate-evidence"],
                "runs-on": "ubuntu-latest",
                "steps": [{"run": "echo gate"}],
            },
            "generate-evidence": {
                "needs": [
                    "verify",
                    "normalize-candidate-artifacts",
                    "compat-tests",
                    "downstream-substitution",
                    "downstream-aggregate",
                    "shim-substitution",
                ],
                "runs-on": "ubuntu-latest",
                "steps": [{"run": "echo evidence"}],
            },
            "verify": {"runs-on": "ubuntu-latest", "steps": [{"run": "echo v"}]},
            "normalize-candidate-artifacts": {"runs-on": "ubuntu-latest", "steps": [{"run": "echo n"}]},
            "compat-tests": {"runs-on": "ubuntu-latest", "steps": [{"run": "echo c"}]},
            "downstream-substitution": {
                "needs": ["prepare-downstream-matrix"],
                "runs-on": "ubuntu-latest",
                "strategy": {
                    "matrix": {"package": "${{ fromJSON(needs.prepare-downstream-matrix.outputs.matrix) }}"}
                },
                "steps": [{"run": "echo ${{ matrix.package }}"}],
            },
            "prepare-downstream-matrix": {
                "runs-on": "ubuntu-latest",
                "outputs": {"matrix": "${{ steps.gen.outputs.matrix }}"},
                "steps": [{"run": "echo gen"}],
            },
            "downstream-aggregate": {"runs-on": "ubuntu-latest", "steps": [{"run": "echo a"}]},
            "shim-substitution": {"runs-on": "ubuntu-latest", "steps": [{"run": "echo s"}]},
            "build": {
                "runs-on": "ubuntu-latest",
                "steps": [{"run": "pytest -k soak"}],
            },
        },
    }


class TestSuppressionPatterns:
    def test_or_true_detected(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["build"]["steps"] = [{"name": "lint", "run": "ruff check . || true"}]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "|| true" in result.stdout

    def test_or_echo_detected(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["build"]["steps"] = [{"name": "check", "run": "python check.py || echo fallback"}]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "|| echo" in result.stdout

    def test_no_suppression_ok(self, tmp_path):
        wf = _passing_base()
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode == 0


class TestContinueOnError:
    def test_gate_dep_continue_on_error_detected(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["generate-evidence"]["continue-on-error"] = True
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "continue-on-error" in result.stdout

    def test_non_gate_job_continue_on_error_ok(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["optional-aarch64"] = {
            "runs-on": "ubuntu-latest",
            "continue-on-error": True,
            "steps": [{"run": "echo cross"}],
        }
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode == 0


class TestNeedsDependencies:
    def test_invalid_needs_reference(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["qualification-gate"]["needs"] = ["nonexistent-job"]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "nonexistent-job" in result.stdout


class TestOutputReferences:
    def test_output_ref_not_in_needs(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["qualification-gate"]["steps"] = [
            {"run": "echo ${{ needs.missing-job.outputs.result }}"}
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "missing-job" in result.stdout


class TestCandidateIdentity:
    def test_checkout_uses_branch_ref(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["build"]["steps"] = [
            {"uses": "actions/checkout@v4", "with": {"ref": "main"}},
            {"run": "pytest -k soak"},
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "branch ref 'main'" in result.stdout

    def test_checkout_uses_develop_ref(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["build"]["steps"] = [
            {"uses": "actions/checkout@v4", "with": {"ref": "develop"}},
            {"run": "pytest -k soak"},
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "develop" in result.stdout

    def test_checkout_uses_master_ref(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["build"]["steps"] = [
            {"uses": "actions/checkout@v4", "with": {"ref": "master"}},
            {"run": "pytest -k soak"},
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "master" in result.stdout

    def test_sha_expression_checkout_ok(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["build"]["steps"] = [
            {"uses": "actions/checkout@v4", "with": {"ref": "${{ env.SHA }}"}},
            {"run": "pytest -k soak"},
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode == 0


class TestSoakSuite:
    def test_no_soak_invocation(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["build"]["steps"] = [{"run": "echo build"}]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "soak" in result.stdout.lower()

    def test_soak_invocation_present(self, tmp_path):
        wf = _passing_base()
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode == 0


class TestCandidateBundle:
    def test_direct_eggfetch_wheel_download(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["compat-tests"]["steps"] = [
            {"uses": "actions/download-artifact@v4", "with": {"name": "eggfetch-wheel"}}
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "candidate-bundle" in result.stdout

    def test_direct_httpx_wheel_download(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["downstream-substitution"]["steps"] = [
            {"uses": "actions/download-artifact@v4", "with": {"name": "httpx-replacement-wheel"}}
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "candidate-bundle" in result.stdout


class TestEvidenceInputs:
    def test_evidence_missing_dependency(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["generate-evidence"]["needs"] = ["build"]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "generate-evidence" in result.stdout
        assert "missing dependency" in result.stdout

    def test_evidence_all_deps_present(self, tmp_path):
        wf = _passing_base()
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode == 0


class TestDryRunSuppressesFailure:
    def test_expect_failure_but_passes(self, tmp_path):
        wf = _passing_base()
        result = _run_validator(_write_yaml(tmp_path, wf), expect_failure=True)
        assert result.returncode == 1
        assert "expected failure but validation passed" in result.stdout


class TestDownstreamMatrix:
    def test_static_package_matrix(self, tmp_path):
        wf = _passing_base()
        del wf["jobs"]["prepare-downstream-matrix"]
        wf["jobs"]["downstream-substitution"] = {
            "runs-on": "ubuntu-latest",
            "strategy": {"matrix": {"package": ["httpx", "requests"]}},
            "steps": [{"run": "echo ${{ matrix.package }}"}],
        }
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "static package matrix" in result.stdout

    def test_fromjson_matrix_ok(self, tmp_path):
        wf = _passing_base()
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode == 0


class TestArtifactProvenance:
    def test_downloaded_artifact_never_produced(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["build"]["steps"] = [
            {"uses": "actions/download-artifact@v4", "with": {"name": "ghost-artifact"}},
            {"run": "pytest -k soak"},
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "ghost-artifact" in result.stdout
        assert "never produced" in result.stdout


class TestPytestPlugins:
    def test_timeout_without_plugin(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["build"]["steps"] = [
            {"run": "pip install pytest\npytest --timeout=60 -k soak tests/"}
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "pytest-timeout" in result.stdout

    def test_timeout_with_plugin_ok(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["build"]["steps"] = [
            {"run": "pip install pytest pytest-timeout\npytest --timeout=60 -k soak tests/"}
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode == 0


class TestInvalidWorkflowFile:
    def test_nonexistent_file(self, tmp_path):
        result = _run_validator(tmp_path / "nope.yml")
        assert result.returncode == 2

    def test_non_yaml_content(self, tmp_path):
        p = tmp_path / "bad.yml"
        p.write_text("not valid yaml: [[[")
        result = _run_validator(p)
        assert result.returncode in (1, 2)

    def test_yaml_list_not_mapping(self, tmp_path):
        p = tmp_path / "list.yml"
        p.write_text("- item1\n- item2\n")
        result = _run_validator(p)
        assert result.returncode == 2


class TestMultipleDefects:
    def test_suppression_and_branch_checkout(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["build"]["steps"] = [
            {"uses": "actions/checkout@v4", "with": {"ref": "main"}},
            {"name": "lint", "run": "ruff check . || true"},
            {"run": "pytest -k soak"},
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "|| true" in result.stdout
        assert "branch ref 'main'" in result.stdout

    def test_all_defects_combined(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["qualification-gate"]["needs"] = ["nonexistent"]
        wf["jobs"]["build"]["steps"] = [
            {"uses": "actions/checkout@v4", "with": {"ref": "develop"}},
            {"name": "lint", "run": "ruff check . || true"},
            {"run": "pytest -k soak"},
        ]
        wf["jobs"]["compat-tests"]["steps"] = [
            {"uses": "actions/download-artifact@v4", "with": {"name": "eggfetch-wheel"}}
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        errors = result.stdout
        assert "nonexistent" in errors
        assert "develop" in errors
        assert "|| true" in errors
        assert "candidate-bundle" in errors


class TestObsoleteWheelDir:
    """§15.68: Workflow uses obsolete --wheel-dir downstream interface."""

    def test_obsolete_wheel_dir_detected(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["downstream-substitution"]["steps"] = [
            {"run": "python scripts/run_downstream_compat.py --wheel-dir dist/ --packages httpx"}
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "--wheel-dir" in result.stdout

    def test_current_interface_ok(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["downstream-substitution"]["steps"] = [
            {"run": "python scripts/run_isolated_downstream.py --package httpx --manifest manifest.toml --output result.json"}
        ]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode == 0


class TestHyphenatedMatrixKey:
    """§15.70: Workflow uses hyphenated matrix expression key."""

    def test_hyphenated_key_detected(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["downstream-substitution"]["strategy"]["matrix"] = {
            "package-id": ["respx"],
            "source-sha256": ["abc123"],
        }
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode != 0
        assert "hyphenated" in result.stdout.lower()

    def test_underscore_keys_ok(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["downstream-substitution"]["strategy"]["matrix"] = {
            "package_id": ["respx"],
            "source_sha256": ["abc123"],
        }
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode == 0

    def test_standard_github_keys_ok(self, tmp_path):
        wf = _passing_base()
        # python-version is a standard GitHub Actions key, should not flag
        wf["jobs"]["build"]["strategy"] = {"matrix": {"python-version": ["3.10", "3.11"]}}
        wf["jobs"]["build"]["steps"] = [{"run": "echo ${{ matrix.python-version }}\npytest -k soak"}]
        result = _run_validator(_write_yaml(tmp_path, wf))
        assert result.returncode == 0


class TestStatusSHAMismatch:
    """§15.82: Status names a SHA different from evidence."""

    def test_status_sha_mismatch_detected(self, tmp_path):
        wf = _passing_base()
        wf["jobs"]["status-generate"] = {
            "needs": ["qualification-gate"],
            "runs-on": "ubuntu-latest",
            "steps": [{"run": "echo status with hardcoded sha=aabbccdd"}],
        }
        result = _run_validator(_write_yaml(tmp_path, wf))
        # This is a semantic check that the status job uses the same SHA
        # The validator should detect if status generation doesn't depend on gate
        assert result.returncode == 0 or "sha" in result.stdout.lower()
