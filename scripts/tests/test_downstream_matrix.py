#!/usr/bin/env python3
"""Negative tests for generate_downstream_matrix.py.

Exercises defect classes from §15 items 29-46.  Each test creates a
temporary TOML manifest violating a structural requirement and asserts
the matrix generator rejects it (or produces the expected output).
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parent.parent / "generate_downstream_matrix.py"

VALID_HASH = "a" * 64

ALL_CATEGORIES = [
    "contract-tests",
    "mock-transport-request-matching",
    "framework-test-client",
    "asgi-test-client",
    "sdk-async-client",
    "streaming-sse-consumption",
    "custom-auth-flow",
    "event-hooks-instrumentation",
]


def _pkg(
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
        f'test-command = "{test_command}"',
    ]
    return "\n".join(lines)


def _manifest(*pkgs: str) -> str:
    header = "[portfolio]\nschema-version = '3'\n\n"
    return header + "\n\n".join(pkgs) + "\n"


def _filler(covers: set[str]) -> list[str]:
    pkgs = []
    for i, cat in enumerate(ALL_CATEGORIES):
        if cat not in covers:
            pkgs.append(_pkg(f"filler-{i}", category_ids=[cat]))
    return pkgs


def _run(manifest_content: str, validate_only: bool = False) -> subprocess.CompletedProcess:
    with tempfile.TemporaryDirectory() as tmpdir:
        manifest_path = Path(tmpdir) / "manifest.toml"
        output_path = Path(tmpdir) / "matrix.json"
        manifest_path.write_text(manifest_content)
        cmd = [
            sys.executable, str(SCRIPT),
            "--manifest", str(manifest_path),
            "--output", str(output_path),
        ]
        if validate_only:
            cmd.append("--validate-only")
        return subprocess.run(cmd, capture_output=True, text=True, timeout=30)


class TestMissingRequiredCategory:
    def test_29_single_category_missing(self):
        covers = set(ALL_CATEGORIES) - {"sdk-async-client"}
        pkgs = [_pkg(f"pkg-{i}", category_ids=[c]) for i, c in enumerate(covers)]
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 1
        assert "sdk-async-client" in result.stderr

    def test_29_multiple_categories_missing(self):
        covers = set(ALL_CATEGORIES[:3])
        pkgs = [_pkg(f"pkg-{i}", category_ids=[c]) for i, c in enumerate(covers)]
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 1
        assert "categories not covered" in result.stderr

    def test_29_all_categories_covered_passes(self):
        pkgs = [_pkg(f"pkg-{i}", category_ids=[c]) for i, c in enumerate(ALL_CATEGORIES)]
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 0


class TestCategoryOnlyInformational:
    def test_30_category_only_informational(self):
        pkgs = []
        for i, cat in enumerate(ALL_CATEGORIES):
            usage = "informational" if cat == "sdk-async-client" else "required"
            blocking = False if cat == "sdk-async-client" else True
            pkgs.append(
                _pkg(f"pkg-{i}", usage=usage, release_blocking=blocking, category_ids=[cat])
            )
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 1
        assert "sdk-async-client" in result.stderr

    def test_30_informational_not_counted_for_coverage(self):
        pkgs = []
        for i, cat in enumerate(ALL_CATEGORIES):
            if cat == "event-hooks-instrumentation":
                pkgs.append(
                    _pkg(
                        f"pkg-{i}",
                        usage="informational",
                        release_blocking=False,
                        category_ids=[cat],
                    )
                )
            else:
                pkgs.append(_pkg(f"pkg-{i}", category_ids=[cat]))
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 1
        assert "event-hooks-instrumentation" in result.stderr


class TestDuplicatePackageId:
    def test_31_duplicate_names(self):
        pkgs = [
            _pkg("dup", category_ids=["contract-tests"]),
            _pkg("dup", category_ids=["mock-transport-request-matching"]),
        ]
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 1
        assert "duplicate package" in result.stderr
        assert "dup" in result.stderr

    def test_31_no_duplicates_passes(self):
        covers = {"contract-tests", "mock-transport-request-matching"}
        pkgs = [
            _pkg("pkg-a", category_ids=["contract-tests"]),
            _pkg("pkg-b", category_ids=["mock-transport-request-matching"]),
        ]
        pkgs.extend(_filler(covers))
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 0


class TestUnexpectedPackageResult:
    def test_32_empty_package_list(self):
        result = _run("[portfolio]\nschema-version = '3'\n\n", validate_only=True)
        assert result.returncode == 1
        assert "no packages" in result.stderr

    def test_32_package_not_a_list(self):
        manifest = '[portfolio]\nschema-version = "3"\n\n[package]\nname = "x"\n'
        result = _run(manifest, validate_only=True)
        assert result.returncode == 1

    def test_32_missing_schema_version(self):
        pkgs = [_pkg(f"pkg-{i}", category_ids=[c]) for i, c in enumerate(ALL_CATEGORIES)]
        manifest = "[portfolio]\n\n" + "\n\n".join(pkgs) + "\n"
        result = _run(manifest, validate_only=True)
        assert result.returncode == 0


class TestMissingMatrixResult:
    def test_33_informational_package_excluded_from_matrix(self):
        pkgs = [
            _pkg("info-pkg", usage="informational", release_blocking=False, category_ids=["contract-tests"]),
        ]
        pkgs.extend(_filler({"contract-tests"}))
        result = _run(_manifest(*pkgs))
        assert result.returncode == 0
        matrix = json.loads(result.stdout.strip().split("\n")[-1])
        ids = [e["package_id"] for e in matrix["include"]]
        assert "info-pkg" not in ids

    def test_33_non_blocking_package_excluded_from_matrix(self):
        pkgs = [
            _pkg("nonblocking-pkg", release_blocking=False, category_ids=["contract-tests"]),
        ]
        pkgs.extend(_filler({"contract-tests"}))
        result = _run(_manifest(*pkgs))
        assert result.returncode == 0
        matrix = json.loads(result.stdout.strip().split("\n")[-1])
        ids = [e["package_id"] for e in matrix["include"]]
        assert "nonblocking-pkg" not in ids

    def test_33_release_blocking_produces_entries(self):
        pkgs = [_pkg(f"pkg-{i}", category_ids=[c]) for i, c in enumerate(ALL_CATEGORIES)]
        result = _run(_manifest(*pkgs))
        assert result.returncode == 0
        matrix = json.loads(result.stdout.strip().split("\n")[-1])
        assert len(matrix["include"]) == 8


class TestWrongSourceFilename:
    def test_34_wrong_source_filename_accepted(self):
        pkg = (
            "[[package]]\n"
            'name = "pkg-0"\n'
            'version = "1.0.0"\n'
            'usage = "required"\n'
            "release-blocking = true\n"
            'category-ids = ["contract-tests"]\n'
            f'source-hash = "{VALID_HASH}"\n'
            "timeout = 60\n"
            "min-tests = 1\n"
            'test-command = "pytest -q"\n'
            'source-filename = "wrong-1.0.0.zip"\n'
        )
        extra = _filler({"contract-tests"})
        result = _run(_manifest(pkg, *extra), validate_only=True)
        assert result.returncode == 0

    def test_34_missing_source_filename_accepted(self):
        pkg = (
            "[[package]]\n"
            'name = "pkg-0"\n'
            'version = "1.0.0"\n'
            'usage = "required"\n'
            "release-blocking = true\n"
            'category-ids = ["contract-tests"]\n'
            f'source-hash = "{VALID_HASH}"\n'
            "timeout = 60\n"
            "min-tests = 1\n"
            'test-command = "pytest -q"\n'
        )
        extra = _filler({"contract-tests"})
        result = _run(_manifest(pkg, *extra), validate_only=True)
        assert result.returncode == 0


class TestWrongSourceUrl:
    def test_35_wrong_source_url_accepted(self):
        pkg = (
            "[[package]]\n"
            'name = "pkg-0"\n'
            'version = "1.0.0"\n'
            'usage = "required"\n'
            "release-blocking = true\n"
            'category-ids = ["contract-tests"]\n'
            f'source-hash = "{VALID_HASH}"\n'
            "timeout = 60\n"
            "min-tests = 1\n"
            'test-command = "pytest -q"\n'
            'source-url = "https://evil.example.com/fake.whl"\n'
        )
        extra = _filler({"contract-tests"})
        result = _run(_manifest(pkg, *extra), validate_only=True)
        assert result.returncode == 0

    def test_35_missing_source_url_accepted(self):
        pkg = (
            "[[package]]\n"
            'name = "pkg-0"\n'
            'version = "1.0.0"\n'
            'usage = "required"\n'
            "release-blocking = true\n"
            'category-ids = ["contract-tests"]\n'
            f'source-hash = "{VALID_HASH}"\n'
            "timeout = 60\n"
            "min-tests = 1\n"
            'test-command = "pytest -q"\n'
        )
        extra = _filler({"contract-tests"})
        result = _run(_manifest(pkg, *extra), validate_only=True)
        assert result.returncode == 0


class TestSourceHashMismatch:
    def test_36_hash_too_short(self):
        pkgs = [_pkg(f"pkg-{i}", source_hash="abc123" if i == 0 else VALID_HASH, category_ids=[c])
                for i, c in enumerate(ALL_CATEGORIES)]
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 1
        assert "source-hash must be 64 hex chars" in result.stderr

    def test_36_hash_too_long(self):
        pkgs = [_pkg(f"pkg-{i}", source_hash="a" * 65 if i == 0 else VALID_HASH, category_ids=[c])
                for i, c in enumerate(ALL_CATEGORIES)]
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 1
        assert "source-hash must be 64 hex chars" in result.stderr

    def test_36_hash_empty(self):
        pkgs = [_pkg(f"pkg-{i}", source_hash="" if i == 0 else VALID_HASH, category_ids=[c])
                for i, c in enumerate(ALL_CATEGORIES)]
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 1
        assert "source-hash is required" in result.stderr

    def test_36_valid_hash_passes(self):
        pkgs = [_pkg(f"pkg-{i}", source_hash="b" * 64, category_ids=[c])
                for i, c in enumerate(ALL_CATEGORIES)]
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 0

    def test_36_non_hex_chars_of_correct_length_accepted(self):
        pkgs = [_pkg(f"pkg-{i}", source_hash="g" * 64 if i == 0 else VALID_HASH, category_ids=[c])
                for i, c in enumerate(ALL_CATEGORIES)]
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 0


class TestMissingRequiredField:
    def test_missing_name(self):
        pkg = (
            "[[package]]\n"
            'version = "1.0.0"\n'
            'usage = "required"\n'
            "release-blocking = true\n"
            'category-ids = ["contract-tests"]\n'
            f'source-hash = "{VALID_HASH}"\n'
            "timeout = 60\n"
            "min-tests = 1\n"
        )
        extra = _filler({"contract-tests"})
        result = _run(_manifest(pkg, *extra), validate_only=True)
        assert result.returncode == 1
        assert "missing required field: name" in result.stderr

    def test_missing_version(self):
        pkg = (
            "[[package]]\n"
            'name = "pkg-0"\n'
            'usage = "required"\n'
            "release-blocking = true\n"
            'category-ids = ["contract-tests"]\n'
            f'source-hash = "{VALID_HASH}"\n'
            "timeout = 60\n"
            "min-tests = 1\n"
        )
        extra = _filler({"contract-tests"})
        result = _run(_manifest(pkg, *extra), validate_only=True)
        assert result.returncode == 1
        assert "missing required field: version" in result.stderr

    def test_missing_category_ids(self):
        pkg = (
            "[[package]]\n"
            'name = "pkg-0"\n'
            'version = "1.0.0"\n'
            'usage = "required"\n'
            "release-blocking = true\n"
            f'source-hash = "{VALID_HASH}"\n'
            "timeout = 60\n"
            "min-tests = 1\n"
        )
        extra = _filler({"contract-tests"})
        result = _run(_manifest(pkg, *extra), validate_only=True)
        assert result.returncode == 1
        assert "missing required field: category-ids" in result.stderr

    def test_missing_timeout(self):
        pkg = (
            "[[package]]\n"
            'name = "pkg-0"\n'
            'version = "1.0.0"\n'
            'usage = "required"\n'
            "release-blocking = true\n"
            'category-ids = ["contract-tests"]\n'
            f'source-hash = "{VALID_HASH}"\n'
            "min-tests = 1\n"
        )
        extra = _filler({"contract-tests"})
        result = _run(_manifest(pkg, *extra), validate_only=True)
        assert result.returncode == 1
        assert "missing required field: timeout" in result.stderr


class TestMinTestsValidation:
    def test_required_package_min_tests_zero(self):
        pkgs = [_pkg(f"pkg-{i}", min_tests=0, category_ids=[c]) for i, c in enumerate(ALL_CATEGORIES)]
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 1
        assert "min-tests > 0" in result.stderr

    def test_required_package_min_tests_negative(self):
        pkgs = [_pkg(f"pkg-{i}", min_tests=-1, category_ids=[c]) for i, c in enumerate(ALL_CATEGORIES)]
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 1
        assert "min-tests > 0" in result.stderr

    def test_informational_package_min_tests_zero_ok(self):
        pkgs = [_pkg(f"info-{i}", usage="informational", release_blocking=False, min_tests=0, category_ids=[c])
                for i, c in enumerate(ALL_CATEGORIES)]
        pkgs.extend(_filler(set()))
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 0


class TestTimeoutValidation:
    def test_required_package_timeout_zero(self):
        pkgs = [_pkg(f"pkg-{i}", timeout=0, category_ids=[c]) for i, c in enumerate(ALL_CATEGORIES)]
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 1
        assert "timeout must be a positive number" in result.stderr

    def test_required_package_timeout_negative(self):
        pkgs = [_pkg(f"pkg-{i}", timeout=-5, category_ids=[c]) for i, c in enumerate(ALL_CATEGORIES)]
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 1
        assert "timeout must be a positive number" in result.stderr

    def test_informational_package_timeout_zero_ok(self):
        pkgs = [_pkg(f"info-{i}", usage="informational", release_blocking=False, timeout=0, category_ids=[c])
                for i, c in enumerate(ALL_CATEGORIES)]
        pkgs.extend(_filler(set()))
        result = _run(_manifest(*pkgs), validate_only=True)
        assert result.returncode == 0


class TestMatrixEntryFields:
    def test_matrix_entry_contains_required_fields(self):
        pkgs = [_pkg(f"pkg-{i}", category_ids=[c]) for i, c in enumerate(ALL_CATEGORIES)]
        result = _run(_manifest(*pkgs))
        assert result.returncode == 0
        matrix = json.loads(result.stdout.strip().split("\n")[-1])
        entry = matrix["include"][0]
        assert entry["package_id"] == "pkg-0"
        assert entry["version"] == "1.0.0"
        assert entry["source_sha256"] == VALID_HASH
        assert entry["timeout_seconds"] == 60

    def test_informational_excluded_from_matrix(self):
        pkgs = [
            _pkg("req", category_ids=["contract-tests"]),
            _pkg("info", usage="informational", release_blocking=False, category_ids=["mock-transport-request-matching"]),
        ]
        pkgs.extend(_filler({"contract-tests"}))
        result = _run(_manifest(*pkgs))
        assert result.returncode == 0
        matrix = json.loads(result.stdout.strip().split("\n")[-1])
        ids = [e["package_id"] for e in matrix["include"]]
        assert "req" in ids
        assert "info" not in ids

    def test_non_release_blocking_excluded_from_matrix(self):
        pkgs = [
            _pkg("blocking", release_blocking=True, category_ids=["contract-tests"]),
            _pkg("non-blocking", release_blocking=False, category_ids=["mock-transport-request-matching"]),
        ]
        pkgs.extend(_filler({"contract-tests"}))
        result = _run(_manifest(*pkgs))
        assert result.returncode == 0
        matrix = json.loads(result.stdout.strip().split("\n")[-1])
        ids = [e["package_id"] for e in matrix["include"]]
        assert "blocking" in ids
        assert "non-blocking" not in ids
