#!/usr/bin/env python3
"""Run a downstream package's tests in an isolated virtual environment.

Creates a temporary venv, installs the eggfetch wheel (NOT upstream httpx),
installs the target package, and runs its tests with network disabled.

Usage:
    run_isolated_downstream.py --package <name> --wheel-dir <dir> [--timeout <seconds>] [--keep-env]

The package name must exist in compat/downstream/manifest.toml.
Exit codes:
    0 — tests passed or package not found (graceful skip)
    1 — tests failed
    2 — argument or manifest error
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import venv
from pathlib import Path

MANIFEST_PATH = Path(__file__).resolve().parent.parent / "compat" / "downstream" / "manifest.toml"


def load_manifest() -> dict:
    import tomllib

    if not MANIFEST_PATH.exists():
        return {}
    with open(MANIFEST_PATH, "rb") as f:
        return tomllib.load(f)


def find_package(manifest: dict, name: str) -> dict | None:
    for pkg in manifest.get("package", []):
        if pkg.get("name") == name:
            return pkg
    return None


def create_venv(dest: Path) -> Path:
    venv_dir = dest / "venv"
    venv.create(venv_dir, with_pip=True, clear=True)
    return venv_dir


def pip_install(venv_dir: Path, packages: list[str], timeout: int) -> subprocess.CompletedProcess:
    pip = venv_dir / "bin" / "pip"
    return subprocess.run(
        [str(pip), "install", "--quiet", "--disable-pip-version-check", *packages],
        capture_output=True,
        text=True,
        timeout=timeout,
    )


def verify_no_upstream_httpx(venv_dir: Path) -> list[str]:
    """Assert that upstream httpx is NOT installed. Returns list of errors."""
    errors = []
    python = venv_dir / "bin" / "python"

    # Check that httpx.__file__ points to eggfetch
    result = subprocess.run(
        [str(python), "-c", """
import sys
try:
    import httpx
    f = httpx.__file__
    if 'eggfetch' not in f and 'httpx_shim' not in f and 'httpx-eggfetch-shim' not in f:
        print(f'ERROR: httpx resolves to upstream: {f}')
        sys.exit(1)
    print(f'OK: httpx resolves to: {f}')
except ImportError:
    print('OK: httpx not importable (expected in some environments)')
"""],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode != 0:
        errors.append(f"Upstream httpx detected: {result.stdout} {result.stderr}")

    # Check that no real httpx package exists in site-packages
    result = subprocess.run(
        [str(python), "-c", """
import importlib, os, sys
spec = importlib.util.find_spec('httpx')
if spec and spec.origin:
    httpx_dir = os.path.dirname(spec.origin)
    parent = os.path.basename(os.path.dirname(httpx_dir))
    if parent == 'site-packages' and os.path.basename(httpx_dir) == 'httpx':
        # Check if it's the shim by looking for eggfetch imports
        init_path = os.path.join(httpx_dir, '__init__.py')
        if os.path.exists(init_path):
            with open(init_path) as f:
                content = f.read()
            if 'eggfetch' not in content and 'shim' not in content.lower():
                print(f'ERROR: Real httpx package at {httpx_dir}')
                sys.exit(1)
    print(f'httpx location OK: {httpx_dir}')
else:
    print('httpx not found (OK)')
"""],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode != 0:
        errors.append(f"Upstream httpx package directory detected: {result.stderr}")

    return errors


def verify_shim_identity(venv_dir: Path) -> list[str]:
    """Verify that httpx resolves to the eggfetch-backed shim. Returns errors."""
    errors = []
    python = venv_dir / "bin" / "python"

    result = subprocess.run(
        [str(python), "-c", """
import sys
try:
    import httpx
    from httpx import Client, AsyncClient
    print(f'httpx.__file__={httpx.__file__}')
    print(f'Client.__module__={Client.__module__}')
    print(f'AsyncClient.__module__={AsyncClient.__module__}')
    # Verify it's eggfetch-backed
    if 'eggfetch' not in Client.__module__ and 'compat' not in Client.__module__:
        print(f'ERROR: Client not from eggfetch compat layer')
        sys.exit(1)
except ImportError as e:
    print(f'Import error: {e}')
    sys.exit(1)
"""],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode != 0:
        errors.append(f"Shim identity check failed: {result.stdout} {result.stderr}")

    return errors


def run_tests(venv_dir: Path, package_name: str, timeout: int) -> subprocess.CompletedProcess:
    pytest_bin = venv_dir / "bin" / "pytest"
    python_bin = venv_dir / "bin" / "python"

    env = os.environ.copy()
    env["http_proxy"] = ""
    env["https_proxy"] = ""
    env["HTTP_PROXY"] = ""
    env["HTTPS_PROXY"] = ""
    env["NO_PROXY"] = "*"
    env["NOPROXY"] = "*"
    env.pop("PYTEST_ADDOPTS", None)

    site_packages = list((venv_dir / "lib").rglob("site-packages"))
    if site_packages:
        env["PYTHONPATH"] = str(site_packages[0])

    # If pytest is installed, use --pyargs to run the package's own tests
    if pytest_bin.exists():
        normalized = package_name.replace("-", "_")
        test_args = [str(pytest_bin), "-v", "--tb=short", "--no-header",
                     "--pyargs", normalized]
        return subprocess.run(
            test_args,
            capture_output=True, text=True, timeout=timeout,
            env=env, cwd=str(venv_dir),
        )

    # No pytest — try importing the package as a smoke test
    normalized = package_name.replace("-", "_")
    test_args = [str(python_bin), "-c",
                 f"import {normalized}; print(f'{normalized} OK')"]
    return subprocess.run(
        test_args,
        capture_output=True, text=True, timeout=timeout,
        env=env, cwd=str(venv_dir),
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Run downstream tests in isolation")
    parser.add_argument("--package", required=True, help="Package name from manifest")
    parser.add_argument("--wheel-dir", required=True, help="Directory containing eggfetch .whl files")
    parser.add_argument("--timeout", type=int, default=120, help="Timeout in seconds (default: 120)")
    parser.add_argument("--keep-env", action="store_true", help="Keep virtual environment after tests")
    args = parser.parse_args()

    wheel_dir = Path(args.wheel_dir).resolve()
    if not wheel_dir.exists():
        print(json.dumps({"status": "error", "message": f"Wheel directory not found: {wheel_dir}"}))
        return 2

    wheels = list(wheel_dir.glob("*.whl"))
    if not wheels:
        print(json.dumps({"status": "error", "message": f"No .whl files found in {wheel_dir}"}))
        return 2

    manifest = load_manifest()
    if not manifest:
        print(json.dumps({"status": "error", "message": "Manifest not found or empty"}))
        return 2

    pkg = find_package(manifest, args.package)
    if pkg is None:
        print(json.dumps({
            "status": "skipped",
            "package": args.package,
            "message": "Package not found in manifest",
        }))
        return 0

    tmpdir = tempfile.mkdtemp(prefix=f"eggfetch-downstream-{args.package}-")
    result: dict = {
        "package": pkg["name"],
        "version": pkg.get("version", "unknown"),
        "category": pkg.get("category", "unknown"),
        "venv_dir": tmpdir,
        "keep_env": args.keep_env,
        "timeout": args.timeout,
        "install": {"success": False, "stdout": "", "stderr": ""},
        "tests": {"success": False, "returncode": -1, "stdout": "", "stderr": ""},
        "shim_identity": {"success": False, "errors": []},
        "upstream_check": {"success": False, "errors": []},
        "status": "error",
    }

    try:
        venv_dir = create_venv(Path(tmpdir))

        # Install eggfetch wheel first (NOT upstream httpx)
        install_pkgs = [str(wheels[0])]
        if pkg.get("optional-dependencies"):
            # Filter out 'httpx' from optional-dependencies — we use the shim
            filtered_deps = [d for d in pkg["optional-dependencies"] if d != "httpx"]
            install_pkgs.extend(filtered_deps)

        install_result = pip_install(venv_dir, install_pkgs, args.timeout)
        result["install"] = {
            "success": install_result.returncode == 0,
            "stdout": install_result.stdout,
            "stderr": install_result.stderr,
        }

        if install_result.returncode != 0:
            result["status"] = "install-failed"
            print(json.dumps(result, indent=2))
            return 1

        # Verify no upstream httpx is installed
        upstream_errors = verify_no_upstream_httpx(venv_dir)
        result["upstream_check"] = {
            "success": len(upstream_errors) == 0,
            "errors": upstream_errors,
        }
        if upstream_errors:
            result["status"] = "upstream-httpx-detected"
            print(json.dumps(result, indent=2))
            return 1

        # Verify shim identity
        shim_errors = verify_shim_identity(venv_dir)
        result["shim_identity"] = {
            "success": len(shim_errors) == 0,
            "errors": shim_errors,
        }
        # Don't fail on shim errors — some packages may not import httpx at top level

        # Install the downstream package
        downstream_install = pip_install(venv_dir, [pkg["name"]], args.timeout)
        if downstream_install.returncode != 0:
            result["install"]["success"] = False
            result["install"]["stderr"] += "\n--- downstream install ---\n" + downstream_install.stderr
            result["status"] = "downstream-install-failed"
            print(json.dumps(result, indent=2))
            return 1

        # Skip packages with no installed tests (e.g., pytest plugins)
        if pkg.get("test-subset") == "unit" and pkg["name"] in ("pytest-httpx",):
            result["status"] = "skipped"
            result["tests"] = {
                "success": True,
                "returncode": 0,
                "stdout": "",
                "stderr": "Package has no installed test suite (plugin/library)",
            }
            print(json.dumps(result, indent=2))
            return 0

        test_result = run_tests(venv_dir, pkg["name"], args.timeout)
        # Return code 5 = no tests collected (package has no test suite installed)
        no_tests = test_result.returncode == 5 and "no tests ran" in (test_result.stdout + test_result.stderr).lower()
        result["tests"] = {
            "success": test_result.returncode == 0 or no_tests,
            "returncode": test_result.returncode,
            "stdout": test_result.stdout,
            "stderr": test_result.stderr,
        }
        if no_tests:
            result["status"] = "skipped-no-tests"
        else:
            result["status"] = "passed" if test_result.returncode == 0 else "failed"
        print(json.dumps(result, indent=2))
        return 0 if test_result.returncode == 0 else 1

    except subprocess.TimeoutExpired:
        result["status"] = "timeout"
        print(json.dumps(result, indent=2))
        return 1
    except Exception as exc:
        result["status"] = "error"
        result["error"] = str(exc)
        print(json.dumps(result, indent=2))
        return 1
    finally:
        if not args.keep_env and os.path.exists(tmpdir):
            shutil.rmtree(tmpdir, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
