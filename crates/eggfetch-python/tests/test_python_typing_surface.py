"""Regression tests for the reviewed native stub contract checker."""

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).parents[3]
CHECKER = ROOT / "scripts/check_python_typing_surface.py"


def _fixture(tmp_path: Path, native_body: str) -> tuple[Path, Path]:
    package = tmp_path / "eggfetch"
    package.mkdir()
    (package / "py.typed").touch()
    (package / "_native.pyi").write_text(native_body)
    (package / "__init__.pyi").write_text(
        "from ._native import Thing\n\n__all__ = [\"Thing\"]\n"
    )
    manifest = tmp_path / "manifest.json"
    manifest.write_text(json.dumps({
        "exports": ["Thing"],
        "symbol_kinds": {"class": ["Thing"], "exception": []},
        "exception_bases": {},
        "signatures": {},
        "members": {
            "Thing": {
                "properties": {"value": "bool"},
                "methods": {"ping": {"kind": "sync", "signature": "(self, /)"}},
            }
        },
        "semantic_contracts": {"Thing.value": "bool"},
    }))
    return package, manifest


def _run(package: Path, manifest: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(CHECKER), "--package", str(package), "--manifest", str(manifest)],
        capture_output=True,
        text=True,
    )


def test_typing_checker_accepts_reviewed_fixture(tmp_path: Path) -> None:
    package, manifest = _fixture(
        tmp_path,
        "class Thing:\n    value: bool\n    def ping(self) -> None: ...\n",
    )
    result = _run(package, manifest)
    assert result.returncode == 0, result.stdout + result.stderr


def test_typing_checker_rejects_member_and_semantic_drift(tmp_path: Path) -> None:
    package, manifest = _fixture(
        tmp_path,
        "class Thing:\n    value: str\n    async def ping(self, extra: int) -> None: ...\n    def extra(self) -> None: ...\n",
    )
    result = _run(package, manifest)
    assert result.returncode != 0
    assert "property annotation drift: Thing.value" in result.stdout
    assert "method kind drift: Thing.ping" in result.stdout
    assert "method signature shape drift: Thing.ping" in result.stdout
    assert "unreviewed public stub-only members on Thing" in result.stdout


def test_typing_checker_rejects_missing_members(tmp_path: Path) -> None:
    package, manifest = _fixture(tmp_path, "class Thing: ...\n")
    result = _run(package, manifest)
    assert result.returncode != 0
    assert "missing typed property: Thing.value" in result.stdout
    assert "missing typed method: Thing.ping" in result.stdout
