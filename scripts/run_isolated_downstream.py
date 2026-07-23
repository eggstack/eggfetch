#!/usr/bin/env python3
"""Run a downstream package's tests in an isolated virtual environment.

Creates a temporary venv, installs httpx==0.28.1 and the target package,
runs the package's tests with network disabled, and reports results as JSON.

Usage:
    run_isolated_downstream.py --package <name> [--timeout <seconds>] [--keep-env]

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


def run_tests(venv_dir: Path, package_name: str, timeout: int) -> subprocess.CompletedProcess:
    pytest_bin = venv_dir / "bin" / "pytest"
    python_bin = venv_dir / "bin" / "python"
    test_runner = pytest_bin if pytest_bin.exists() else python_bin
    test_args = [str(test_runner)]
    if pytest_bin.exists():
        test_args.extend(["-m", "pytest", "-v", "--tb=short"])

    env = os.environ.copy()
    env["http_proxy"] = ""
    env["https_proxy"] = ""
    env["HTTP_PROXY"] = ""
    env["HTTPS_PROXY"] = ""
    env["NO_PROXY"] = "*"
    env["NOPROXY"] = "*"

    site_packages = list((venv_dir / "lib").rglob("site-packages"))
    if site_packages:
        env["PYTHONPATH"] = str(site_packages[0])

    return subprocess.run(
        test_args,
        capture_output=True,
        text=True,
        timeout=timeout,
        env=env,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Run downstream tests in isolation")
    parser.add_argument("--package", required=True, help="Package name from manifest")
    parser.add_argument("--timeout", type=int, default=120, help="Timeout in seconds (default: 120)")
    parser.add_argument("--keep-env", action="store_true", help="Keep virtual environment after tests")
    args = parser.parse_args()

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
        "status": "error",
    }

    try:
        venv_dir = create_venv(Path(tmpdir))
        install_pkgs = [f"httpx==0.28.1", pkg["name"]]
        if pkg.get("optional-dependencies"):
            install_pkgs.extend(pkg["optional-dependencies"])

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

        test_result = run_tests(venv_dir, pkg["name"], args.timeout)
        result["tests"] = {
            "success": test_result.returncode == 0,
            "returncode": test_result.returncode,
            "stdout": test_result.stdout,
            "stderr": test_result.stderr,
        }
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
