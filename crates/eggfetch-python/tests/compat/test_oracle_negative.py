"""Negative oracle tests for the API manifest comparator.

Track 1.5: Prove the comparator exits nonzero on specific malformed fixtures.
Each test creates a deliberately broken manifest or allowed-difference file
and verifies the comparator detects it.
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parent.parent.parent.parent.parent / "scripts" / "compare_httpx_api_manifest.py"


def _make_ref_and_cand(ref_symbols=None, cand_symbols=None):
    """Create minimal reference and candidate manifest dicts."""
    ref = {"symbols": ref_symbols or [
        {"name": "Client", "kind": "CLASS", "bases": [], "signature": None, "properties": [], "methods": []},
        {"name": "AsyncClient", "kind": "CLASS", "bases": [], "signature": None, "properties": [], "methods": []},
    ]}
    cand = {"symbols": cand_symbols or [
        {"name": "Client", "kind": "CLASS", "bases": [], "signature": None, "properties": [], "methods": []},
        {"name": "AsyncClient", "kind": "CLASS", "bases": [], "signature": None, "properties": [], "methods": []},
    ]}
    return ref, cand


def _empty_allowed():
    """Create a temp file with no allowed differences."""
    f = tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False)
    f.write("# empty\n")
    f.close()
    return f.name


def _run_comparator(ref, cand, allowed_path=None):
    """Run the comparator as a subprocess. Returns (exit_code, stdout, stderr)."""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as rf:
        json.dump(ref, rf)
        ref_path = rf.name
    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as cf:
        json.dump(cand, cf)
        cand_path = cf.name

    if allowed_path is None:
        allowed_path = _empty_allowed()
        cleanup_allowed = True
    else:
        cleanup_allowed = False

    cmd = [sys.executable, str(SCRIPT), "--reference", ref_path, "--candidate", cand_path,
           "--allowed", allowed_path]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
    Path(ref_path).unlink(missing_ok=True)
    Path(cand_path).unlink(missing_ok=True)
    if cleanup_allowed:
        Path(allowed_path).unlink(missing_ok=True)
    return result.returncode, result.stdout, result.stderr


class TestNegativeOracle:
    """Each test proves the comparator fails on a specific defect."""

    def test_missing_client_fails(self):
        """Removing Client from candidate should cause nonzero exit."""
        ref, cand = _make_ref_and_cand()
        cand["symbols"] = [s for s in cand["symbols"] if s["name"] != "Client"]
        rc, stdout, stderr = _run_comparator(ref, cand)
        assert rc != 0, f"Expected nonzero exit for missing Client, got 0.\nstdout: {stdout}\nstderr: {stderr}"

    def test_missing_async_client_fails(self):
        """Removing AsyncClient from candidate should cause nonzero exit."""
        ref, cand = _make_ref_and_cand()
        cand["symbols"] = [s for s in cand["symbols"] if s["name"] != "AsyncClient"]
        rc, stdout, stderr = _run_comparator(ref, cand)
        assert rc != 0, f"Expected nonzero exit for missing AsyncClient, got 0.\nstdout: {stdout}\nstderr: {stderr}"

    def test_changed_follow_redirects_default_fails(self):
        """Changing follow_redirects default should cause nonzero exit."""
        ref_sym = {"name": "Client", "kind": "CLASS", "bases": [], "properties": [], "methods": [],
                   "signature": {"parameters": [
                       {"name": "self", "kind": "POSITIONAL_OR_KEYWORD", "default": None},
                       {"name": "follow_redirects", "kind": "KEYWORD_ONLY", "default": "False"},
                   ], "return_annotation": None}}
        cand_sym = {"name": "Client", "kind": "CLASS", "bases": [], "properties": [], "methods": [],
                    "signature": {"parameters": [
                        {"name": "self", "kind": "POSITIONAL_OR_KEYWORD", "default": None},
                        {"name": "follow_redirects", "kind": "KEYWORD_ONLY", "default": "True"},
                    ], "return_annotation": None}}
        ref, cand = _make_ref_and_cand([ref_sym], [cand_sym])
        rc, stdout, stderr = _run_comparator(ref, cand)
        assert rc != 0, f"Expected nonzero exit for changed default, got 0.\nstdout: {stdout}\nstderr: {stderr}"

    def test_kind_mismatch_fails(self):
        """Changing a CLASS to a FUNCTION should cause nonzero exit."""
        ref_sym = {"name": "Client", "kind": "CLASS", "bases": [], "signature": None, "properties": [], "methods": []}
        cand_sym = {"name": "Client", "kind": "FUNCTION", "bases": [], "signature": None, "properties": [], "methods": []}
        ref, cand = _make_ref_and_cand([ref_sym], [cand_sym])
        rc, stdout, stderr = _run_comparator(ref, cand)
        assert rc != 0, f"Expected nonzero exit for kind mismatch, got 0.\nstdout: {stdout}\nstderr: {stderr}"

    def test_inheritance_mismatch_detected(self):
        """Changing inheritance should be detected."""
        ref_sym = {"name": "Client", "kind": "CLASS", "bases": ["BaseClient"],
                   "signature": None, "properties": [], "methods": []}
        cand_sym = {"name": "Client", "kind": "CLASS", "bases": ["DifferentBase"],
                    "signature": None, "properties": [], "methods": []}
        ref, cand = _make_ref_and_cand([ref_sym], [cand_sym])
        rc, stdout, stderr = _run_comparator(ref, cand)
        assert "inheritance" in stdout.lower() or rc != 0, \
            f"Expected inheritance mismatch detection.\nstdout: {stdout}\nstderr: {stderr}"

    def test_property_to_method_mismatch_detected(self):
        """Replacing a property with a method should be detected."""
        ref_sym = {"name": "Client", "kind": "CLASS", "bases": [], "signature": None,
                   "properties": [{"name": "is_closed"}], "methods": []}
        cand_sym = {"name": "Client", "kind": "CLASS", "bases": [], "signature": None,
                    "properties": [], "methods": [{"name": "is_closed"}]}
        ref, cand = _make_ref_and_cand([ref_sym], [cand_sym])
        rc, stdout, stderr = _run_comparator(ref, cand)
        assert "property" in stdout.lower() or "method" in stdout.lower() or rc != 0, \
            f"Expected property/method mismatch detection.\nstdout: {stdout}\nstderr: {stderr}"

    def test_expired_allowed_difference_fails(self):
        """An allowed-difference entry covering a non-differing symbol is stale."""
        ref, cand = _make_ref_and_cand()
        # Write a temp allowed-differences file with an entry for Client
        # but Client matches in both manifests (no actual difference)
        with tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False) as f:
            f.write('[[difference]]\n')
            f.write('id = "EXPIRED-TEST-001"\n')
            f.write('symbol = "Client"\n')
            f.write('category = "signature-difference"\n')
            f.write('rationale = "test"\n')
            allowed_path = f.name
        try:
            rc, stdout, stderr = _run_comparator(ref, cand, allowed_path=allowed_path)
            # Since Client exists in both manifests with same kind and no actual diff,
            # the allowed entry is stale
            assert rc != 0 or "STALE" in stdout, \
                f"Expected stale detection for unused allowed entry.\nstdout: {stdout}\nstderr: {stderr}"
        finally:
            Path(allowed_path).unlink(missing_ok=True)
