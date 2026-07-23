#!/usr/bin/env python3
"""Test pip dependency resolution and uninstall/reinstall cycles for eggfetch.

Validates that:
1. eggfetch installs cleanly alongside httpx
2. eggfetch does not shadow httpx
3. httpx-eggfetch-shim installs and provides import httpx
4. Uninstalling the shim restores httpx
5. Dependency constraints work correctly

Exit codes:
    0 — all tests passed
    1 — one or more tests failed
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


def _run(cmd: list[str], timeout: int = 120) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)


def _create_venv() -> Path:
    tmpdir = Path(tempfile.mkdtemp(prefix="eggfetch-pip-test-"))
    venv = tmpdir / "venv"
    _run([sys.executable, "-m", "venv", str(venv)])
    return venv


def _pip(venv: Path, *args: str) -> subprocess.CompletedProcess:
    pip = venv / "bin" / "pip"
    return _run([str(pip), "--disable-pip-version-check", *args])


def _python(venv: Path, code: str) -> subprocess.CompletedProcess:
    python = venv / "bin" / "python"
    return _run([str(python), "-c", code])


def test_eggfetch_installs_cleanly() -> bool:
    """eggfetch installs without errors."""
    venv = _create_venv()
    try:
        result = _pip(venv, "install", str(Path(__file__).resolve().parent.parent / "crates" / "eggfetch-python"))
        if result.returncode != 0:
            print(f"  FAIL: eggfetch install failed: {result.stderr[-200:]}")
            return False
        r = _python(venv, "import eggfetch; print(eggfetch.__version__)")
        if r.returncode != 0:
            print(f"  FAIL: import eggfetch failed: {r.stderr[-200:]}")
            return False
        print(f"  PASS: eggfetch {r.stdout.strip()} installed and importable")
        return True
    finally:
        import shutil
        shutil.rmtree(venv.parent, ignore_errors=True)


def test_eggfetch_does_not_shadow_httpx() -> bool:
    """Installing eggfetch alongside httpx doesn't shadow httpx."""
    venv = _create_venv()
    try:
        _pip(venv, "install", "httpx==0.28.1")
        _pip(venv, "install", str(Path(__file__).resolve().parent.parent / "crates" / "eggfetch-python"))
        r = _python(venv, """
import httpx
import eggfetch.compat.httpx as ef
print(f"httpx={httpx.__file__}")
print(f"eggfetch={ef.__file__}")
assert httpx.__file__ != ef.__file__, "eggfetch shadows httpx!"
assert httpx.__version__ == "0.28.1", f"wrong version: {httpx.__version__}"
print("PASS")
""")
        if r.returncode != 0:
            print(f"  FAIL: {r.stdout} {r.stderr[-200:]}")
            return False
        print(f"  PASS: httpx not shadowed by eggfetch")
        return True
    finally:
        import shutil
        shutil.rmtree(venv.parent, ignore_errors=True)


def test_shim_provides_import_httpx() -> bool:
    """httpx-eggfetch-shim provides import httpx backed by eggfetch."""
    venv = _create_venv()
    try:
        _pip(venv, "install", str(Path(__file__).resolve().parent.parent / "crates" / "eggfetch-python"))
        result = _pip(venv, "install", str(Path(__file__).resolve().parent.parent / "compat" / "httpx-shim"))
        if result.returncode != 0:
            print(f"  FAIL: shim install failed: {result.stderr[-200:]}")
            return False
        r = _python(venv, """
import httpx
print(f"httpx.__version__ = {httpx.__version__}")
print(f"httpx.__file__ = {httpx.__file__}")
assert httpx.__version__ == "0.28.1"
assert "eggfetch" in httpx.__file__ or "httpx-shim" in httpx.__file__
c = httpx.Client()
print(f"Client type: {type(c)}")
assert "eggfetch" in str(type(c))
print("PASS")
""")
        if r.returncode != 0:
            print(f"  FAIL: {r.stderr[-300:]}")
            return False
        print(f"  PASS: shim provides import httpx -> eggfetch")
        return True
    finally:
        import shutil
        shutil.rmtree(venv.parent, ignore_errors=True)


def test_uninstall_shim_restores_httpx() -> bool:
    """Uninstalling the shim and reinstalling httpx restores functionality.

    KNOWN LIMITATION: The shim overwrites httpx/__init__.py. pip cannot
    distinguish file ownership between the shim and real httpx. The
    correct workflow is: uninstall shim → force-reinstall httpx.
    """
    venv = _create_venv()
    try:
        _pip(venv, "install", "httpx==0.28.1")
        _pip(venv, "install", str(Path(__file__).resolve().parent.parent / "crates" / "eggfetch-python"))
        _pip(venv, "install", str(Path(__file__).resolve().parent.parent / "compat" / "httpx-shim"))
        # Uninstall shim then force-reinstall httpx
        _pip(venv, "uninstall", "-y", "httpx-eggfetch-shim")
        _pip(venv, "install", "--force-reinstall", "--no-deps", "httpx==0.28.1")
        r = _python(venv, "import httpx; print(httpx.__version__)")
        if r.returncode == 0 and "0.28.1" in r.stdout:
            print(f"  PASS: shim uninstalled, httpx force-reinstalled: {r.stdout.strip()}")
            return True
        print(f"  FAIL: httpx not restored after force-reinstall: {r.stderr[-200:]}")
        return False
    finally:
        import shutil
        shutil.rmtree(venv.parent, ignore_errors=True)


def test_dependency_resolution() -> bool:
    """pip resolves eggfetch alongside httpx>=0.27,<0.29."""
    venv = _create_venv()
    try:
        result = _pip(venv, "install",
                       str(Path(__file__).resolve().parent.parent / "crates" / "eggfetch-python"),
                       "httpx>=0.27,<0.29")
        if result.returncode != 0:
            print(f"  FAIL: resolution failed: {result.stderr[-300:]}")
            return False
        r = _python(venv, "import httpx; import eggfetch; print(f'httpx={httpx.__version__} eggfetch={eggfetch.__version__}')")
        if r.returncode != 0:
            print(f"  FAIL: import failed: {r.stderr[-200:]}")
            return False
        print(f"  PASS: {r.stdout.strip()}")
        return True
    finally:
        import shutil
        shutil.rmtree(venv.parent, ignore_errors=True)


def main() -> int:
    tests = [
        ("eggfetch installs cleanly", test_eggfetch_installs_cleanly),
        ("eggfetch does not shadow httpx", test_eggfetch_does_not_shadow_httpx),
        ("shim provides import httpx", test_shim_provides_import_httpx),
        ("uninstall shim restores httpx", test_uninstall_shim_restores_httpx),
        ("dependency resolution with httpx>=0.27,<0.29", test_dependency_resolution),
    ]
    results = []
    for name, test_fn in tests:
        print(f"Running: {name}")
        try:
            passed = test_fn()
        except Exception as exc:
            print(f"  ERROR: {exc}")
            passed = False
        results.append((name, passed))

    print("\n" + "=" * 60)
    all_passed = all(p for _, p in results)
    for name, passed in results:
        print(f"  {'PASS' if passed else 'FAIL'}: {name}")
    print(f"\nOverall: {'ALL PASSED' if all_passed else 'SOME FAILED'}")
    return 0 if all_passed else 1


if __name__ == "__main__":
    sys.exit(main())
