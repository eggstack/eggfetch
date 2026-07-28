#!/usr/bin/env python3
"""Dedicated test suite for candidate_identity.py.

Covers schema v4 identity generation, validation, digest computation,
and cross-validation against artifact manifests.

Per plan §6.3, §16 (line 1175).
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
IDENTITY_SCRIPT = SCRIPTS_DIR / "candidate_identity.py"
MANIFEST_SCRIPT = SCRIPTS_DIR / "generate_artifact_manifest.py"

VALID_SHA = "a" * 40
VALID_MANIFEST_SHA = "b" * 64
VALID_IDENTITY_DIGEST = "c" * 64


def _run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, *args],
        capture_output=True,
        text=True,
        timeout=30,
    )


def _make_identity(**overrides) -> dict:
    """Build a minimal valid identity dict."""
    identity = {
        "schema_version": "4",
        "candidate_sha": VALID_SHA,
        "artifact_manifest_sha256": VALID_MANIFEST_SHA,
        "eggfetch_version": "1.0.0",
        "eggfetch_wheel": {"filename": "eggfetch-1.0.0-py3-none-any.whl", "sha256": "d" * 64},
        "httpx_replacement_wheel": {"filename": "httpx-0.28.1-py3-none-any.whl", "sha256": "e" * 64},
        "reference_httpx_version": "0.28.1",
        "producer": "test",
        "run_id": "12345",
        "run_attempt": "1",
        "started_at": "2026-01-01T00:00:00+00:00",
        "finished_at": "2026-01-01T00:00:01+00:00",
        "identity_digest": VALID_IDENTITY_DIGEST,
    }
    identity.update(overrides)
    return identity


def _compute_digest(identity: dict) -> str:
    """Compute the expected identity digest."""
    subset = {k: v for k, v in identity.items() if k != "identity_digest"}
    canonical = json.dumps(subset, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


class TestIdentitySchema:
    """Schema version and required field validation."""

    def test_schema_version_is_4(self):
        identity = _make_identity()
        assert identity["schema_version"] == "4"

    def test_missing_required_field_fails(self):
        identity = _make_identity()
        del identity["candidate_sha"]
        identity["identity_digest"] = _compute_digest(identity)
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump(identity, f)
            f.flush()
            result = _run(IDENTITY_SCRIPT, "validate", f.name)
        assert result.returncode != 0
        assert "missing required field" in result.stdout.lower() or "missing required field" in result.stderr.lower()

    def test_wrong_schema_version_fails(self):
        identity = _make_identity(schema_version="3")
        identity["identity_digest"] = _compute_digest(identity)
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump(identity, f)
            f.flush()
            result = _run(IDENTITY_SCRIPT, "validate", f.name)
        assert result.returncode != 0
        assert "schema_version" in result.stdout.lower() or "schema_version" in result.stderr.lower()


class TestIdentityDigest:
    """Digest computation and verification."""

    def test_digest_computation_excludes_identity_digest(self):
        identity = _make_identity()
        computed = _compute_digest(identity)
        assert computed != VALID_IDENTITY_DIGEST

    def test_digest_mismatch_detected(self):
        identity = _make_identity()
        identity["identity_digest"] = "f" * 64  # wrong digest
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump(identity, f)
            f.flush()
            result = _run(IDENTITY_SCRIPT, "validate", f.name)
        assert result.returncode != 0
        assert "identity_digest mismatch" in result.stdout.lower() or "identity_digest mismatch" in result.stderr.lower()

    def test_valid_identity_passes(self):
        identity = _make_identity()
        identity["identity_digest"] = _compute_digest(identity)
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump(identity, f)
            f.flush()
            result = _run(IDENTITY_SCRIPT, "validate", f.name)
        assert result.returncode == 0
        assert "OK" in result.stdout


class TestWheelRecords:
    """Wheel record validation."""

    def test_missing_eggfetch_wheel_fails(self):
        identity = _make_identity(eggfetch_wheel={})
        identity["identity_digest"] = _compute_digest(identity)
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump(identity, f)
            f.flush()
            result = _run(IDENTITY_SCRIPT, "validate", f.name)
        assert result.returncode != 0

    def test_short_sha256_in_wheel_fails(self):
        identity = _make_identity(
            eggfetch_wheel={"filename": "x.whl", "sha256": "abc123"}
        )
        identity["identity_digest"] = _compute_digest(identity)
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump(identity, f)
            f.flush()
            result = _run(IDENTITY_SCRIPT, "validate", f.name)
        assert result.returncode != 0
        assert "sha256" in result.stdout.lower() or "sha256" in result.stderr.lower()


class TestCrossValidation:
    """Cross-validation between identity and manifest."""

    def test_manifest_digest_mismatch_detected(self):
        identity = _make_identity(artifact_manifest_sha256="0" * 64)
        identity["identity_digest"] = _compute_digest(identity)
        manifest_sha = hashlib.sha256(b"{}").hexdigest()
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as mf:
            json.dump({"schema_version": "3", "candidate_sha": VALID_SHA,
                        "run_id": "1", "run_attempt": "1",
                        "generated_at": "2026-01-01T00:00:00Z",
                        "artifacts": []}, mf)
            mf.flush()
            with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as idf:
                json.dump(identity, idf)
                idf.flush()
                result = _run(
                    IDENTITY_SCRIPT, "validate", idf.name,
                    "--artifact-manifest", mf.name,
                )
        assert result.returncode != 0
        assert "artifact_manifest_sha256 mismatch" in result.stdout.lower() or "artifact_manifest_sha256 mismatch" in result.stderr.lower()

    def test_candidate_sha_mismatch_detected(self):
        identity = _make_identity(candidate_sha="1" * 40)
        identity["identity_digest"] = _compute_digest(identity)
        manifest = {
            "schema_version": "3",
            "candidate_sha": "2" * 40,
            "run_id": "1",
            "run_attempt": "1",
            "generated_at": "2026-01-01T00:00:00Z",
            "artifacts": [],
        }
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as mf:
            json.dump(manifest, mf)
            mf.flush()
            with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as idf:
                json.dump(identity, idf)
                idf.flush()
                result = _run(
                    IDENTITY_SCRIPT, "validate", idf.name,
                    "--artifact-manifest", mf.name,
                    "--expected-sha", "2" * 40,
                )
        assert result.returncode != 0
        assert "candidate_sha mismatch" in result.stdout.lower() or "candidate_sha mismatch" in result.stderr.lower()


class TestCLIGenerate:
    """CLI generate subcommand."""

    def test_generate_from_manifest(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create a dummy wheel
            wheel_path = Path(tmpdir) / "eggfetch-1.0.0-py3-none-any.whl"
            wheel_path.write_bytes(b"dummy wheel")
            httpx_path = Path(tmpdir) / "httpx-0.28.1-py3-none-any.whl"
            httpx_path.write_bytes(b"dummy httpx wheel")

            # Generate manifest with bundle-dir to use relative paths
            bundle_dir = Path(tmpdir) / "bundle"
            bundle_dir.mkdir()
            manifest_path = bundle_dir / "artifact-manifest.json"
            result = _run(
                MANIFEST_SCRIPT, "generate",
                "--eggfetch-wheel", str(wheel_path),
                "--httpx-replacement-wheel", str(httpx_path),
                "--candidate-sha", VALID_SHA,
                "--run-id", "12345",
                "--run-attempt", "1",
                "--bundle-dir", str(bundle_dir),
                "--output", str(manifest_path),
            )
            assert result.returncode == 0, f"Manifest generation failed: {result.stderr}"

            # Generate identity
            identity_path = bundle_dir / "identity.json"
            result = _run(
                IDENTITY_SCRIPT, "generate",
                "--artifact-manifest", str(manifest_path),
                "--candidate-sha", VALID_SHA,
                "--run-id", "12345",
                "--run-attempt", "1",
                "--output", str(identity_path),
            )
            assert result.returncode == 0, f"Identity generation failed: {result.stderr}"
            assert identity_path.exists()

            # Validate the generated identity
            result = _run(IDENTITY_SCRIPT, "validate", str(identity_path))
            assert result.returncode == 0, f"Identity validation failed: {result.stdout} {result.stderr}"

    def test_generate_rejects_invalid_sha(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            manifest_path = Path(tmpdir) / "manifest.json"
            manifest_path.write_text("{}")
            identity_path = Path(tmpdir) / "identity.json"
            result = _run(
                IDENTITY_SCRIPT, "generate",
                "--artifact-manifest", str(manifest_path),
                "--candidate-sha", "not-a-sha",
                "--run-id", "12345",
                "--run-attempt", "1",
                "--output", str(identity_path),
            )
            assert result.returncode != 0
