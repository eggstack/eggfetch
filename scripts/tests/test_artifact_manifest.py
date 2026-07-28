#!/usr/bin/env python3
"""Negative tests for generate_artifact_manifest.py and candidate_identity.py.

Exercises items 1-16 from plan section 15.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
MANIFEST_SCRIPT = SCRIPTS_DIR / "generate_artifact_manifest.py"
IDENTITY_SCRIPT = SCRIPTS_DIR / "candidate_identity.py"

VALID_SHA = "a" * 40
OTHER_SHA = "b" * 40


def _wheel(path: Path, content: bytes = b"dummy") -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    return path


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, *args],
        capture_output=True,
        text=True,
        timeout=30,
    )


def _manifest(
    path: Path,
    artifacts: list[dict],
    candidate_sha: str = VALID_SHA,
) -> Path:
    path.write_text(json.dumps({
        "schema_version": "3",
        "candidate_sha": candidate_sha,
        "run_id": "run-1",
        "run_attempt": "1",
        "generated_at": "2026-01-01T00:00:00+00:00",
        "artifacts": artifacts,
    }, indent=2))
    return path


def _identity(
    path: Path,
    manifest_sha: str = "d" * 64,
    candidate_sha: str = VALID_SHA,
    **overrides: object,
) -> Path:
    base: dict = {
        "schema_version": "4",
        "candidate_sha": candidate_sha,
        "artifact_manifest_sha256": manifest_sha,
        "eggfetch_version": "0.1.0",
        "eggfetch_wheel": {
            "filename": "eggfetch-0.1.0-py3-none-any.whl",
            "sha256": "e" * 64,
        },
        "httpx_replacement_wheel": {
            "filename": "httpx-0.28.1-py3-none-any.whl",
            "sha256": "f" * 64,
        },
        "reference_httpx_version": "0.28.1",
        "producer": "test",
        "run_id": "run-1",
        "run_attempt": "1",
        "started_at": "2026-01-01T00:00:00+00:00",
        "finished_at": "2026-01-01T00:00:01+00:00",
    }
    base.update(overrides)
    if "identity_digest" not in overrides:
        subset = {k: v for k, v in base.items() if k != "identity_digest"}
        canonical = json.dumps(subset, sort_keys=True, separators=(",", ":"))
        base["identity_digest"] = hashlib.sha256(
            canonical.encode("utf-8")
        ).hexdigest()
    path.write_text(json.dumps(base, indent=2))
    return path


def _artifact(
    role: str = "eggfetch",
    filename: str = "eggfetch-0.1.0-py3-none-any.whl",
    sha256: str | None = None,
    size: int = 100,
    rel_path: str | None = None,
) -> dict:
    parts = filename.rsplit("-", 4)
    dist = parts[0] if len(parts) >= 5 else ""
    version = parts[1] if len(parts) >= 5 else ""
    return {
        "role": role,
        "distribution": dist,
        "version": version,
        "filename": filename,
        "relative_path": rel_path or f"wheels/{filename}",
        "sha256": sha256 or ("a" * 64),
        "size_bytes": size,
        "tags": "py3-none-any",
    }


def _generate_manifest(
    tmp: Path,
    eggfetch_name: str = "eggfetch-0.1.0-py3-none-any.whl",
    httpx_name: str = "httpx-0.28.1-py3-none-any.whl",
    candidate_sha: str = VALID_SHA,
    run_attempt: str = "1",
    bundle_dir: Path | None = None,
) -> tuple[Path, subprocess.CompletedProcess]:
    ef = _wheel(tmp / eggfetch_name)
    hx = _wheel(tmp / httpx_name)
    bd = bundle_dir or tmp / "bundle"
    out = bd / "artifact-manifest.json"
    r = _run(
        str(MANIFEST_SCRIPT), "generate",
        "--eggfetch-wheel", str(ef),
        "--httpx-replacement-wheel", str(hx),
        "--candidate-sha", candidate_sha,
        "--run-id", "1",
        "--run-attempt", run_attempt,
        "--output", str(out),
        "--bundle-dir", str(bd),
    )
    return out, r


class TestNoEggfetchWheel:
    def test_missing_eggfetch_wheel(self, tmp_path: Path) -> None:
        httpx = _wheel(tmp_path / "httpx-0.28.1-py3-none-any.whl")
        missing = tmp_path / "nonexistent.whl"
        r = _run(
            str(MANIFEST_SCRIPT), "generate",
            "--eggfetch-wheel", str(missing),
            "--httpx-replacement-wheel", str(httpx),
            "--candidate-sha", VALID_SHA,
            "--run-id", "1", "--run-attempt", "1",
            "--output", str(tmp_path / "out.json"),
        )
        assert r.returncode == 1
        assert "eggfetch wheel not found" in r.stderr


class TestNoReplacementWheel:
    def test_missing_replacement_wheel(self, tmp_path: Path) -> None:
        eggfetch = _wheel(tmp_path / "eggfetch-0.1.0-py3-none-any.whl")
        missing = tmp_path / "nonexistent.whl"
        r = _run(
            str(MANIFEST_SCRIPT), "generate",
            "--eggfetch-wheel", str(eggfetch),
            "--httpx-replacement-wheel", str(missing),
            "--candidate-sha", VALID_SHA,
            "--run-id", "1", "--run-attempt", "1",
            "--output", str(tmp_path / "out.json"),
        )
        assert r.returncode == 1
        assert "httpx replacement wheel not found" in r.stderr


class TestDuplicateEggfetchWheels:
    def test_two_eggfetch_roles(self, tmp_path: Path) -> None:
        arts = [
            _artifact(role="eggfetch", filename="eggfetch-0.1.0-py3-none-any.whl"),
            _artifact(role="eggfetch", filename="eggfetch-0.2.0-py3-none-any.whl"),
        ]
        m = _manifest(tmp_path / "manifest.json", arts)
        r = _run(str(MANIFEST_SCRIPT), "validate", "--manifest", str(m))
        assert r.returncode == 1
        assert "duplicate artifact role: eggfetch" in r.stderr


class TestDuplicateReplacementWheels:
    def test_two_replacement_roles(self, tmp_path: Path) -> None:
        arts = [
            _artifact(
                role="httpx-controlled-replacement",
                filename="httpx-0.28.1-py3-none-any.whl",
            ),
            _artifact(
                role="httpx-controlled-replacement",
                filename="httpx-0.28.1-py3-none-any.whl",
            ),
        ]
        m = _manifest(tmp_path / "manifest.json", arts)
        r = _run(str(MANIFEST_SCRIPT), "validate", "--manifest", str(m))
        assert r.returncode == 1
        assert "duplicate artifact role: httpx-controlled-replacement" in r.stderr


class TestExtraUnlistedWheel:
    def test_three_artifacts(self, tmp_path: Path) -> None:
        arts = [
            _artifact(role="eggfetch", filename="eggfetch-0.1.0-py3-none-any.whl"),
            _artifact(
                role="httpx-controlled-replacement",
                filename="httpx-0.28.1-py3-none-any.whl",
            ),
            _artifact(role="extra", filename="extra-1.0-py3-none-any.whl"),
        ]
        m = _manifest(tmp_path / "manifest.json", arts)
        r = _run(str(MANIFEST_SCRIPT), "validate", "--manifest", str(m))
        assert r.returncode == 1
        assert "must contain exactly 2 artifacts, got 3" in r.stderr


class TestReplacementWrongRole:
    def test_httpx_assigned_eggfetch_role(self, tmp_path: Path) -> None:
        arts = [
            _artifact(role="eggfetch", filename="eggfetch-0.1.0-py3-none-any.whl"),
            _artifact(role="eggfetch", filename="httpx-0.28.1-py3-none-any.whl"),
        ]
        m = _manifest(tmp_path / "manifest.json", arts)
        r = _run(str(MANIFEST_SCRIPT), "validate", "--manifest", str(m))
        assert r.returncode == 1
        assert (
            "must contain an artifact with role 'httpx-controlled-replacement'"
            in r.stderr
        )


class TestReplacementVersionNot0281:
    def test_wrong_reference_httpx_version(self, tmp_path: Path) -> None:
        ident = _identity(
            tmp_path / "identity.json",
            reference_httpx_version="0.29.0",
        )
        r = _run(str(IDENTITY_SCRIPT), "validate", str(ident))
        assert r.returncode == 1
        assert "reference_httpx_version must be '0.28.1'" in r.stdout


class TestWheelHashMismatch:
    def test_sha256_differs_from_disk(self, tmp_path: Path) -> None:
        content = b"real wheel content here"
        _wheel(tmp_path / "wheels" / "eggfetch-0.1.0-py3-none-any.whl", content)
        _wheel(tmp_path / "wheels" / "httpx-0.28.1-py3-none-any.whl", content)
        real_sha = _sha256(content)
        bad_sha = "0" * 64
        arts = [
            _artifact(
                role="eggfetch",
                filename="eggfetch-0.1.0-py3-none-any.whl",
                sha256=bad_sha,
                size=len(content),
            ),
            _artifact(
                role="httpx-controlled-replacement",
                filename="httpx-0.28.1-py3-none-any.whl",
                sha256=real_sha,
                size=len(content),
            ),
        ]
        m = _manifest(tmp_path / "manifest.json", arts)
        r = _run(
            str(MANIFEST_SCRIPT), "validate",
            "--manifest", str(m),
            "--bundle-root", str(tmp_path),
        )
        assert r.returncode == 1
        assert "sha256 mismatch" in r.stderr


class TestWheelSizeMismatch:
    def test_size_differs_from_disk(self, tmp_path: Path) -> None:
        content = b"real wheel content here"
        _wheel(tmp_path / "wheels" / "eggfetch-0.1.0-py3-none-any.whl", content)
        _wheel(tmp_path / "wheels" / "httpx-0.28.1-py3-none-any.whl", content)
        real_sha = _sha256(content)
        arts = [
            _artifact(
                role="eggfetch",
                filename="eggfetch-0.1.0-py3-none-any.whl",
                sha256=real_sha,
                size=9999,
            ),
            _artifact(
                role="httpx-controlled-replacement",
                filename="httpx-0.28.1-py3-none-any.whl",
                sha256=real_sha,
                size=len(content),
            ),
        ]
        m = _manifest(tmp_path / "manifest.json", arts)
        r = _run(
            str(MANIFEST_SCRIPT), "validate",
            "--manifest", str(m),
            "--bundle-root", str(tmp_path),
        )
        assert r.returncode == 1
        assert "size_bytes mismatch" in r.stderr


class TestAbsolutePath:
    def test_absolute_relative_path(self, tmp_path: Path) -> None:
        arts = [
            _artifact(
                role="eggfetch",
                filename="eggfetch-0.1.0-py3-none-any.whl",
                rel_path="/tmp/absolute/path/eggfetch-0.1.0-py3-none-any.whl",
            ),
            _artifact(
                role="httpx-controlled-replacement",
                filename="httpx-0.28.1-py3-none-any.whl",
            ),
        ]
        m = _manifest(tmp_path / "manifest.json", arts)
        r = _run(str(MANIFEST_SCRIPT), "validate", "--manifest", str(m))
        assert r.returncode == 1
        assert "contains path traversal" in r.stderr


class TestPathTraversal:
    def test_dot_dot_traversal(self, tmp_path: Path) -> None:
        arts = [
            _artifact(
                role="eggfetch",
                filename="eggfetch-0.1.0-py3-none-any.whl",
                rel_path="../../etc/passwd",
            ),
            _artifact(
                role="httpx-controlled-replacement",
                filename="httpx-0.28.1-py3-none-any.whl",
            ),
        ]
        m = _manifest(tmp_path / "manifest.json", arts)
        r = _run(str(MANIFEST_SCRIPT), "validate", "--manifest", str(m))
        assert r.returncode == 1
        assert "contains path traversal" in r.stderr


class TestCandidateShaMismatch:
    def test_expected_sha_differs(self, tmp_path: Path) -> None:
        manifest, r = _generate_manifest(tmp_path)
        assert r.returncode == 0
        r = _run(
            str(MANIFEST_SCRIPT), "validate",
            "--manifest", str(manifest),
            "--expected-sha", OTHER_SHA,
        )
        assert r.returncode == 1
        assert "candidate_sha mismatch" in r.stderr


class TestManifestDigestMismatch:
    def test_identity_bound_to_different_manifest(self, tmp_path: Path) -> None:
        bd1 = tmp_path / "bundle1"
        manifest1, r = _generate_manifest(tmp_path, run_attempt="1", bundle_dir=bd1)
        assert r.returncode == 0
        ident = tmp_path / "identity.json"
        r = _run(
            str(IDENTITY_SCRIPT), "generate",
            "--artifact-manifest", str(manifest1),
            "--candidate-sha", VALID_SHA,
            "--run-id", "1", "--run-attempt", "1",
            "--output", str(ident),
        )
        assert r.returncode == 0
        bd2 = tmp_path / "bundle2"
        manifest2, r2 = _generate_manifest(tmp_path, run_attempt="2", bundle_dir=bd2)
        assert r2.returncode == 0
        r = _run(
            str(IDENTITY_SCRIPT), "validate",
            str(ident),
            "--artifact-manifest", str(manifest2),
        )
        assert r.returncode == 1
        assert "artifact_manifest_sha256 mismatch" in r.stdout


class TestIdentityDigestMismatch:
    def test_tampered_identity_digest(self, tmp_path: Path) -> None:
        ident = _identity(tmp_path / "identity.json")
        data = json.loads(ident.read_text())
        data["identity_digest"] = "0" * 64
        ident.write_text(json.dumps(data, indent=2))
        r = _run(str(IDENTITY_SCRIPT), "validate", str(ident))
        assert r.returncode == 1
        assert "identity_digest mismatch" in r.stdout


class TestBundleIndexDigestMismatch:
    def test_manifest_digest_not_bound(self, tmp_path: Path) -> None:
        bd1 = tmp_path / "bundle1"
        manifest_a, r = _generate_manifest(tmp_path, bundle_dir=bd1)
        assert r.returncode == 0
        r = _run(
            str(IDENTITY_SCRIPT), "generate",
            "--artifact-manifest", str(manifest_a),
            "--candidate-sha", VALID_SHA,
            "--run-id", "1", "--run-attempt", "1",
            "--output", str(tmp_path / "identity.json"),
        )
        assert r.returncode == 0
        bd2 = tmp_path / "bundle2"
        manifest_b, r2 = _generate_manifest(tmp_path, bundle_dir=bd2)
        assert r2.returncode == 0
        r = _run(
            str(IDENTITY_SCRIPT), "validate",
            str(tmp_path / "identity.json"),
            "--artifact-manifest", str(manifest_b),
        )
        assert r.returncode == 1
        assert "artifact_manifest_sha256 mismatch" in r.stdout


class TestManifestRegeneratedWithoutRebinding:
    def test_identity_stale_after_manifest_regenerated(self, tmp_path: Path) -> None:
        bd = tmp_path / "bundle"
        manifest, r = _generate_manifest(tmp_path, run_attempt="1", bundle_dir=bd)
        assert r.returncode == 0
        ident = tmp_path / "identity.json"
        r = _run(
            str(IDENTITY_SCRIPT), "generate",
            "--artifact-manifest", str(manifest),
            "--candidate-sha", VALID_SHA,
            "--run-id", "1", "--run-attempt", "1",
            "--output", str(ident),
        )
        assert r.returncode == 0
        _, r2 = _generate_manifest(tmp_path, run_attempt="2", bundle_dir=bd)
        assert r2.returncode == 0
        r = _run(
            str(IDENTITY_SCRIPT), "validate",
            str(ident),
            "--artifact-manifest", str(manifest),
        )
        assert r.returncode == 1
        assert "artifact_manifest_sha256 mismatch" in r.stdout
