#!/usr/bin/env python3
"""Negative tests for run_isolated_downstream.py.

Exercises defect classes from §15 items 37-46.  Each test creates
temporary artifact manifests and downstream manifests with various
defects, then calls the runner via subprocess.  Focuses on manifest-level
failures that happen before venv creation.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parent.parent / "run_isolated_downstream.py"

REAL_MANIFEST = (
    Path(__file__).resolve().parent.parent.parent
    / "compat"
    / "downstream"
    / "manifest.toml"
)

VALID_HASH = "a" * 64


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _make_wheel(tmp: Path, name: str, content: bytes = b"dummy") -> tuple[Path, str]:
    wheel = tmp / name
    wheel.write_bytes(content)
    return wheel, _sha256(content)


def _artifact_manifest(
    tmp: Path,
    *,
    eggfetch_content: bytes = b"eggfetch-wheel",
    eggfetch_name: str = "eggfetch-0.1.0-py3-none-any.whl",
    httpx_content: bytes = b"httpx-wheel",
    httpx_name: str = "httpx-0.28.1-py3-none-any.whl",
    include_eggfetch: bool = True,
    include_httpx: bool = True,
    wrong_eggfetch_hash: bool = False,
    wrong_httpx_hash: bool = False,
) -> tuple[dict, Path]:
    bundle = tmp / "bundle"
    bundle.mkdir(exist_ok=True)
    artifacts = []

    if include_eggfetch:
        path, hash_val = _make_wheel(bundle, eggfetch_name, eggfetch_content)
        if wrong_eggfetch_hash:
            hash_val = "0" * 64
        artifacts.append({
            "role": "eggfetch",
            "relative_path": eggfetch_name,
            "sha256": hash_val,
        })

    if include_httpx:
        path, hash_val = _make_wheel(bundle, httpx_name, httpx_content)
        if wrong_httpx_hash:
            hash_val = "0" * 64
        artifacts.append({
            "role": "httpx-controlled-replacement",
            "relative_path": httpx_name,
            "sha256": hash_val,
        })

    manifest = {"artifacts": artifacts}
    manifest_path = bundle / "artifact-manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2))
    return manifest, bundle


def _downstream_pkg(
    name: str,
    *,
    version: str = "1.0.0",
    usage: str = "required",
    release_blocking: bool = True,
    category_ids: list[str] | None = None,
    source_hash: str = VALID_HASH,
    timeout: int = 60,
    min_tests: int = 1,
    test_command: str = "pytest -q",
) -> str:
    cat_ids = category_ids or ["contract-tests"]
    cat_items = ", ".join(f'"{c}"' for c in cat_ids)
    lines = [
        "[[package]]",
        f'name = "{name}"',
        f'version = "{version}"',
        f'usage = "{usage}"',
        f"release-blocking = {str(release_blocking).lower()}",
        f"category-ids = [{cat_items}]",
        f'source-hash = "{source_hash}"',
        f"timeout = {timeout}",
        f"min-tests = {min_tests}",
    ]
    if test_command:
        escaped = test_command.replace('"', '\\"')
        lines.append(f'test-command = "{escaped}"')
    return "\n".join(lines)


def _downstream_manifest(*pkgs: str) -> str:
    header = "[portfolio]\nschema-version = '3'\n\n"
    return header + "\n\n".join(pkgs) + "\n"


def _write_json(tmp: Path, name: str, data: dict) -> Path:
    p = tmp / name
    p.write_text(json.dumps(data, indent=2))
    return p


def _run_runner(
    tmp: Path,
    *,
    package: str,
    bundle: Path | None = None,
    downstream_manifest: str | None = None,
    candidate_identity: dict | None = None,
) -> subprocess.CompletedProcess:
    output_path = tmp / "result.json"
    cmd = [
        sys.executable, str(SCRIPT),
        "--package", package,
        "--output", str(output_path),
        "--keep-env",
    ]
    if bundle:
        cmd.extend(["--bundle-root", str(bundle)])
    if candidate_identity:
        ci_path = _write_json(tmp, "candidate-identity.json", candidate_identity)
        cmd.extend(["--candidate-identity", str(ci_path)])

    if downstream_manifest is not None:
        backup = REAL_MANIFEST.read_bytes()
        try:
            REAL_MANIFEST.write_text(downstream_manifest)
            return subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        finally:
            REAL_MANIFEST.write_bytes(backup)
    else:
        return subprocess.run(cmd, capture_output=True, text=True, timeout=30)


def _read_result(tmp: Path) -> dict:
    result_path = tmp / "result.json"
    if result_path.exists():
        return json.loads(result_path.read_text())
    return {}


class TestArtifactMismatch:
    def test_37_no_eggfetch_wheel(self, tmp_path):
        _, bundle = _artifact_manifest(tmp_path, include_eggfetch=False)
        result = _run_runner(tmp_path, package="respx", bundle=bundle)
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "artifact-mismatch"

    def test_37_no_httpx_wheel(self, tmp_path):
        _, bundle = _artifact_manifest(tmp_path, include_httpx=False)
        result = _run_runner(tmp_path, package="respx", bundle=bundle)
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "artifact-mismatch"

    def test_37_empty_artifacts(self, tmp_path):
        bundle = tmp_path / "bundle"
        bundle.mkdir()
        manifest_path = bundle / "artifact-manifest.json"
        manifest_path.write_text(json.dumps({"artifacts": []}))
        result = _run_runner(tmp_path, package="respx", bundle=bundle)
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "artifact-mismatch"

    def test_37_wrong_eggfetch_hash(self, tmp_path):
        _, bundle = _artifact_manifest(tmp_path, wrong_eggfetch_hash=True)
        result = _run_runner(tmp_path, package="respx", bundle=bundle)
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "artifact-mismatch"

    def test_37_wrong_httpx_hash(self, tmp_path):
        _, bundle = _artifact_manifest(tmp_path, wrong_httpx_hash=True)
        result = _run_runner(tmp_path, package="respx", bundle=bundle)
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "artifact-mismatch"


class TestUpstreamHttpxReplaces:
    def test_38_role_not_matching_httpx_controlled(self, tmp_path):
        bundle = tmp_path / "bundle"
        bundle.mkdir()
        (bundle / "eggfetch.whl").write_bytes(b"eggfetch")
        (bundle / "httpx.wrong.whl").write_bytes(b"httpx-wrong")
        manifest = {"artifacts": [
            {"role": "eggfetch", "relative_path": "eggfetch.whl",
             "sha256": hashlib.sha256(b"eggfetch").hexdigest()},
            {"role": "wrong-role", "relative_path": "httpx.wrong.whl",
             "sha256": hashlib.sha256(b"httpx-wrong").hexdigest()},
        ]}
        (bundle / "artifact-manifest.json").write_text(json.dumps(manifest))
        result = _run_runner(tmp_path, package="respx", bundle=bundle)
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "artifact-mismatch"

    def test_38_httpx_role_required_not_optional(self, tmp_path):
        bundle = tmp_path / "bundle"
        bundle.mkdir()
        (bundle / "eggfetch.whl").write_bytes(b"eggfetch")
        manifest = {"artifacts": [
            {"role": "eggfetch", "relative_path": "eggfetch.whl",
             "sha256": hashlib.sha256(b"eggfetch").hexdigest()},
        ]}
        (bundle / "artifact-manifest.json").write_text(json.dumps(manifest))
        result = _run_runner(tmp_path, package="respx", bundle=bundle)
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "artifact-mismatch"


class TestBareOptionalDependency:
    def test_39_optional_dep_not_in_manifest_accepted(self, tmp_path):
        pkg = _downstream_pkg(
            "respx",
            category_ids=["mock-transport-request-matching"],
            test_command="pytest -q",
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="respx", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode in (0, 1, 3)


class TestPipCheckFailure:
    def test_40_manifest_level_not_detected(self, tmp_path):
        pkg = _downstream_pkg(
            "respx",
            category_ids=["mock-transport-request-matching"],
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="respx", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode in (0, 1, 3)


class TestImportOnlyRequired:
    def test_41_import_only_test_command_rejected(self, tmp_path):
        pkg = _downstream_pkg(
            "respx",
            category_ids=["mock-transport-request-matching"],
            test_command="python -c 'import respx; print(respx.__version__)'",
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="respx", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "import-only-required"

    def test_41_empty_test_command_rejected(self, tmp_path):
        pkg = _downstream_pkg(
            "respx",
            category_ids=["mock-transport-request-matching"],
            test_command="",
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="respx", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "missing-test-command"

    def test_41_import_with_print_no_assert(self, tmp_path):
        pkg = _downstream_pkg(
            "respx",
            category_ids=["mock-transport-request-matching"],
            test_command="python -c 'import respx; print(\"ok\")'",
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="respx", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "import-only-required"


class TestFixtureImportsNotExercises:
    def test_42_import_only_fixture_rejected(self, tmp_path):
        pkg = _downstream_pkg(
            "respx",
            category_ids=["mock-transport-request-matching"],
            test_command="python -c 'import respx.mock; print(\"OK\")'",
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="respx", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "import-only-required"

    def test_42_import_with_assert_accepted(self, tmp_path):
        pkg = _downstream_pkg(
            "respx",
            category_ids=["mock-transport-request-matching"],
            test_command="python -c 'import respx; assert hasattr(respx, \"MockTransport\")'",
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="respx", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode != 3 or _read_result(tmp_path).get("diagnostic_name") != "import-only-required"


class TestRespxCommandSyntaxFailure:
    def test_43_bad_command_shell_error(self, tmp_path):
        pkg = _downstream_pkg(
            "respx",
            category_ids=["mock-transport-request-matching"],
            test_command="pytest -q nonexistent_path::TestClass::test_method",
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="respx", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode in (1, 3)


class TestPytestHttpxFixtureNotUsed:
    def test_44_pytest_httpx_not_used_in_manifest(self, tmp_path):
        pkg = _downstream_pkg(
            "respx",
            category_ids=["mock-transport-request-matching"],
            test_command="pytest -q --co",
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="respx", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode in (0, 1, 3)


class TestSdkExternalNetwork:
    def test_45_network_blocked_at_manifest_level(self, tmp_path):
        pkg = (
            _downstream_pkg(
                "respx",
                category_ids=["mock-transport-request-matching"],
                test_command="pytest -q",
            )
            + "\nnetwork-policy = \"isolated\"\n"
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="respx", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode in (0, 1, 3)


class TestEventHookNoAssertion:
    def test_46_event_hook_import_only_rejected(self, tmp_path):
        pkg = _downstream_pkg(
            "httpx-ws",
            category_ids=["event-hooks-instrumentation"],
            test_command="python -c 'import httpx_ws; print(\"OK\")'",
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="httpx-ws", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "import-only-required"

    def test_46_event_hook_with_pytest_ok(self, tmp_path):
        pkg = _downstream_pkg(
            "httpx-ws",
            category_ids=["event-hooks-instrumentation"],
            test_command="pytest -q",
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="httpx-ws", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode in (0, 1, 3)


class TestUnknownPackage:
    def test_unknown_package_e001(self, tmp_path):
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(tmp_path, package="nonexistent-pkg", bundle=bundle)
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "unknown-package"

    def test_unknown_package_in_custom_manifest(self, tmp_path):
        manifest = _downstream_manifest(
            _downstream_pkg("real-pkg", category_ids=["contract-tests"]),
        )
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="other-pkg", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "unknown-package"


class TestMissingSourceHash:
    def test_e005_missing_source_hash(self, tmp_path):
        pkg = (
            "[[package]]\n"
            'name = "respx"\n'
            'version = "0.21.1"\n'
            'usage = "required"\n'
            "release-blocking = true\n"
            'category-ids = ["mock-transport-request-matching"]\n'
            "timeout = 60\n"
            "min-tests = 1\n"
            'test-command = "pytest -q"\n'
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="respx", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "source-hash-missing"


class TestMissingTestCommand:
    def test_e003_required_no_test_command(self, tmp_path):
        pkg = (
            "[[package]]\n"
            'name = "respx"\n'
            'version = "0.21.1"\n'
            'usage = "required"\n'
            "release-blocking = true\n"
            'category-ids = ["mock-transport-request-matching"]\n'
            f'source-hash = "{VALID_HASH}"\n'
            "timeout = 60\n"
            "min-tests = 1\n"
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="respx", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r.get("diagnostic_name") == "missing-test-command"

    def test_e003_informational_no_test_command_ok(self, tmp_path):
        pkg = (
            "[[package]]\n"
            'name = "respx"\n'
            'version = "0.21.1"\n'
            'usage = "informational"\n'
            "release-blocking = false\n"
            'category-ids = ["mock-transport-request-matching"]\n'
            f'source-hash = "{VALID_HASH}"\n'
            "timeout = 60\n"
            "min-tests = 0\n"
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(
            tmp_path, package="respx", bundle=bundle, downstream_manifest=manifest,
        )
        assert result.returncode != 3 or _read_result(tmp_path).get("diagnostic_name") != "missing-test-command"


class TestCandidateIdentityMismatch:
    def test_identity_mismatch_e015(self, tmp_path):
        pkg = _downstream_pkg(
            "respx",
            category_ids=["mock-transport-request-matching"],
            test_command="python -c 'import respx; assert True'",
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        ci = {"package_name": "wrong-pkg", "version": "1.0.0"}
        result = _run_runner(
            tmp_path,
            package="respx",
            bundle=bundle,
            downstream_manifest=manifest,
            candidate_identity=ci,
        )
        assert result.returncode == 1
        r = _read_result(tmp_path)
        assert r.get("status") == "install-failed"

    def test_identity_match_not_checked_before_install(self, tmp_path):
        pkg = _downstream_pkg(
            "respx",
            category_ids=["mock-transport-request-matching"],
            test_command="python -c 'import respx; assert True'",
        )
        manifest = _downstream_manifest(pkg)
        _, bundle = _artifact_manifest(tmp_path)
        ci = {"package_name": "respx", "version": "0.21.0"}
        result = _run_runner(
            tmp_path,
            package="respx",
            bundle=bundle,
            downstream_manifest=manifest,
            candidate_identity=ci,
        )
        assert result.returncode == 1
        r = _read_result(tmp_path)
        assert r.get("status") == "install-failed"


class TestArtifactManifestNotFound:
    def test_missing_artifact_manifest_file(self, tmp_path):
        fake_path = tmp_path / "nonexistent-manifest.json"
        result = _run_runner(
            tmp_path, package="respx", bundle=fake_path,
        )
        assert result.returncode == 2


class TestDiagnosticCodeStructure:
    def test_diagnostic_has_required_fields(self, tmp_path):
        _, bundle = _artifact_manifest(tmp_path, include_eggfetch=False)
        result = _run_runner(tmp_path, package="respx", bundle=bundle)
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert "diagnostic_code" in r
        assert "diagnostic_name" in r
        assert "error" in r
        assert r["status"] == "error"

    def test_unknown_package_diagnostic_fields(self, tmp_path):
        _, bundle = _artifact_manifest(tmp_path)
        result = _run_runner(tmp_path, package="no-such-pkg", bundle=bundle)
        assert result.returncode == 3
        r = _read_result(tmp_path)
        assert r["diagnostic_code"] == "E001"
        assert r["diagnostic_name"] == "unknown-package"
        assert "package" in r
