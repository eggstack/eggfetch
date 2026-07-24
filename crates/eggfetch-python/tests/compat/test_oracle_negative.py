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


class TestExactTupleMatching:
    """Negative tests proving the comparator matches by exact tuple, not symbol only."""

    def _make_typed_allowed(self, symbol, diff_type, member, reference, candidate, entry_id="TUPLE-001"):
        """Create a temp allowed-differences file with a typed tuple entry."""
        f = tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False)
        f.write('[[difference]]\n')
        f.write(f'id = "{entry_id}"\n')
        f.write(f'category = "stage-bounded"\n')
        f.write(f'symbol = "{symbol}"\n')
        f.write(f'difference-type = "{diff_type}"\n')
        f.write(f'member = "{member}"\n')
        f.write(f'reference = "{reference}"\n')
        f.write(f'candidate = "{candidate}"\n')
        f.write('rationale = "test"\n')
        f.write('owner = "test"\n')
        f.write('review-milestone = "test"\n')
        f.close()
        return f.name

    def _make_diff_manifest(self, symbol="Client", diff_type="parameter-default",
                            member="timeout", reference="False", candidate="True"):
        """Create reference and candidate manifests with a specific difference."""
        ref_sym = {"name": symbol, "kind": "CLASS", "bases": [], "properties": [], "methods": [],
                   "signature": {"parameters": [
                       {"name": "self", "kind": "POSITIONAL_OR_KEYWORD", "default": None},
                       {"name": member, "kind": "KEYWORD_ONLY", "default": reference},
                   ], "return_annotation": None}}
        cand_sym = {"name": symbol, "kind": "CLASS", "bases": [], "properties": [], "methods": [],
                    "signature": {"parameters": [
                        {"name": "self", "kind": "POSITIONAL_OR_KEYWORD", "default": None},
                        {"name": member, "kind": "KEYWORD_ONLY", "default": candidate},
                    ], "return_annotation": None}}
        ref = {"symbols": [ref_sym]}
        cand = {"symbols": [cand_sym]}
        return ref, cand

    def test_wrong_difference_type_does_not_match(self):
        """Same symbol, wrong difference type does not match."""
        ref, cand = self._make_diff_manifest(diff_type="parameter-default")
        allowed = self._make_typed_allowed("Client", "parameter-name", "timeout", "False", "True")
        try:
            rc, stdout, stderr = _run_comparator(ref, cand, allowed_path=allowed)
            assert rc != 0, f"Expected nonzero exit for wrong difference type.\nstdout: {stdout}"
            assert "UNEXPLAINED" in stdout, f"Expected unexplained difference.\nstdout: {stdout}"
        finally:
            Path(allowed).unlink(missing_ok=True)

    def test_wrong_member_does_not_match(self):
        """Same symbol and type, wrong member does not match."""
        ref, cand = self._make_diff_manifest(member="timeout")
        allowed = self._make_typed_allowed("Client", "parameter-default", "follow_redirects", "False", "True")
        try:
            rc, stdout, stderr = _run_comparator(ref, cand, allowed_path=allowed)
            assert rc != 0, f"Expected nonzero exit for wrong member.\nstdout: {stdout}"
            assert "UNEXPLAINED" in stdout, f"Expected unexplained difference.\nstdout: {stdout}"
        finally:
            Path(allowed).unlink(missing_ok=True)

    def test_changed_reference_value_does_not_match(self):
        """Same tuple, changed reference value does not match."""
        ref, cand = self._make_diff_manifest(reference="False", candidate="True")
        allowed = self._make_typed_allowed("Client", "parameter-default", "timeout", "True", "True")
        try:
            rc, stdout, stderr = _run_comparator(ref, cand, allowed_path=allowed)
            assert rc != 0, f"Expected nonzero exit for changed reference.\nstdout: {stdout}"
        finally:
            Path(allowed).unlink(missing_ok=True)

    def test_changed_candidate_value_does_not_match(self):
        """Same tuple, changed candidate value does not match."""
        ref, cand = self._make_diff_manifest(reference="False", candidate="True")
        allowed = self._make_typed_allowed("Client", "parameter-default", "timeout", "False", "False")
        try:
            rc, stdout, stderr = _run_comparator(ref, cand, allowed_path=allowed)
            assert rc != 0, f"Expected nonzero exit for changed candidate.\nstdout: {stdout}"
        finally:
            Path(allowed).unlink(missing_ok=True)

    def test_exact_tuple_matches(self):
        """Exact tuple match succeeds (exit 0)."""
        ref, cand = self._make_diff_manifest(reference="False", candidate="True")
        allowed = self._make_typed_allowed("Client", "parameter-default", "timeout", "False", "True")
        try:
            rc, stdout, stderr = _run_comparator(ref, cand, allowed_path=allowed)
            assert rc == 0, f"Expected exit 0 for exact tuple match.\nstdout: {stdout}\nstderr: {stderr}"
        finally:
            Path(allowed).unlink(missing_ok=True)

    def test_one_entry_cannot_satisfy_two_differences(self):
        """One allowed entry cannot suppress two different differences on same symbol."""
        ref_sym = {"name": "Client", "kind": "CLASS", "bases": [], "properties": [], "methods": [],
                   "signature": {"parameters": [
                       {"name": "self", "kind": "POSITIONAL_OR_KEYWORD", "default": None},
                       {"name": "timeout", "kind": "KEYWORD_ONLY", "default": "False"},
                       {"name": "follow_redirects", "kind": "KEYWORD_ONLY", "default": "False"},
                   ], "return_annotation": None}}
        cand_sym = {"name": "Client", "kind": "CLASS", "bases": [], "properties": [], "methods": [],
                    "signature": {"parameters": [
                        {"name": "self", "kind": "POSITIONAL_OR_KEYWORD", "default": None},
                        {"name": "timeout", "kind": "KEYWORD_ONLY", "default": "True"},
                        {"name": "follow_redirects", "kind": "KEYWORD_ONLY", "default": "True"},
                    ], "return_annotation": None}}
        ref = {"symbols": [ref_sym]}
        cand = {"symbols": [cand_sym]}
        # Only one allowed entry for timeout — follow_redirects should be unexplained
        allowed = self._make_typed_allowed("Client", "parameter-default", "timeout", "False", "True")
        try:
            rc, stdout, stderr = _run_comparator(ref, cand, allowed_path=allowed)
            assert rc != 0, f"Expected nonzero exit for unmatched difference.\nstdout: {stdout}"
            assert "UNEXPLAINED" in stdout, f"Expected unexplained for follow_redirects.\nstdout: {stdout}"
        finally:
            Path(allowed).unlink(missing_ok=True)

    def test_resolved_entry_in_active_file_fails(self):
        """A resolved entry in the active allowed file must fail as stale."""
        ref, cand = _make_ref_and_cand()
        with tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False) as f:
            f.write('[[difference]]\n')
            f.write('id = "RESOLVED-TEST-001"\n')
            f.write('symbol = "Client"\n')
            f.write('category = "resolved"\n')
            f.write('rationale = "test"\n')
            f.write('owner = "test"\n')
            f.write('review-milestone = "test"\n')
            allowed_path = f.name
        try:
            rc, stdout, stderr = _run_comparator(ref, cand, allowed_path=allowed_path)
            # Resolved entry for a non-differing symbol should be stale
            assert rc != 0 or "STALE" in stdout, \
                f"Expected stale for resolved entry.\nstdout: {stdout}\nstderr: {stderr}"
        finally:
            Path(allowed_path).unlink(missing_ok=True)

    def test_duplicate_id_fails_validation(self):
        """Duplicate IDs in allowed-differences.toml fail validation."""
        with tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False) as f:
            f.write('[[difference]]\n')
            f.write('id = "DUP-001"\n')
            f.write('symbol = "Client"\n')
            f.write('category = "stage-bounded"\n')
            f.write('rationale = "test"\n')
            f.write('owner = "test"\n')
            f.write('review-milestone = "test"\n')
            f.write('[[difference]]\n')
            f.write('id = "DUP-001"\n')
            f.write('symbol = "AsyncClient"\n')
            f.write('category = "stage-bounded"\n')
            f.write('rationale = "test"\n')
            f.write('owner = "test"\n')
            f.write('review-milestone = "test"\n')
            allowed_path = f.name
        try:
            rc, stdout, stderr = _run_comparator(
                *_make_ref_and_cand(), allowed_path=allowed_path,
            )
            # Use --validate flag
            cmd = [sys.executable, str(SCRIPT), "--validate", "--allowed", allowed_path]
            result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
            assert result.returncode != 0, \
                f"Expected validation failure for duplicate ID.\nstdout: {result.stdout}"
            assert "duplicate" in result.stdout.lower(), \
                f"Expected duplicate error.\nstdout: {result.stdout}"
        finally:
            Path(allowed_path).unlink(missing_ok=True)

    def test_wildcard_symbol_fails_validation(self):
        """Wildcard in symbol fails validation."""
        with tempfile.NamedTemporaryFile(mode="w", suffix=".toml", delete=False) as f:
            f.write('[[difference]]\n')
            f.write('id = "WILD-001"\n')
            f.write('symbol = "*"\n')
            f.write('category = "stage-bounded"\n')
            f.write('rationale = "test"\n')
            f.write('owner = "test"\n')
            f.write('review-milestone = "test"\n')
            allowed_path = f.name
        try:
            cmd = [sys.executable, str(SCRIPT), "--validate", "--allowed", allowed_path]
            result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
            assert result.returncode != 0, \
                f"Expected validation failure for wildcard.\nstdout: {result.stdout}"
            assert "wildcard" in result.stdout.lower(), \
                f"Expected wildcard error.\nstdout: {result.stdout}"
        finally:
            Path(allowed_path).unlink(missing_ok=True)
