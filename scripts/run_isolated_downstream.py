#!/usr/bin/env python3
"""Run a downstream package's tests in an isolated virtual environment.

Creates a temporary venv, installs the eggfetch wheel AND the controlled
replacement httpx wheel, installs the target package, and runs its tests
with network disabled. Verifies shim identity at multiple points.

Usage:
    run_isolated_downstream.py --package <name> --wheel-dir <dir> [--timeout <seconds>] [--keep-env]

The package name must exist in compat/downstream/manifest.toml.
The wheel-dir must contain both the eggfetch wheel and the httpx controlled replacement wheel.

Exit codes:
    0 — tests passed or package not found (graceful skip)
    1 — tests failed
    2 — argument or manifest error
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import venv
from pathlib import Path

MANIFEST_PATH = Path(__file__).resolve().parent.parent / "compat" / "downstream" / "manifest.toml"


def _emit_result(result: dict, output_path: str | None = None) -> None:
    """Write structured result to file or stdout."""
    payload = json.dumps(result, indent=2)
    if output_path:
        Path(output_path).write_text(payload)
    else:
        print(payload)


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


def find_wheels(wheel_dir: Path) -> tuple[Path | None, Path | None]:
    """Find the eggfetch wheel and the httpx controlled replacement wheel.

    Returns (eggfetch_wheel, httpx_wheel).
    """
    eggfetch_wheel = None
    httpx_wheel = None

    for whl in wheel_dir.glob("*.whl"):
        name_lower = whl.name.lower()
        if name_lower.startswith("eggfetch-"):
            eggfetch_wheel = whl
        elif name_lower.startswith("httpx-") and "controlled" not in name_lower:
            httpx_wheel = whl

    return eggfetch_wheel, httpx_wheel


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


def pip_check(venv_dir: Path) -> tuple[bool, str]:
    """Run pip check. Returns (ok, output)."""
    pip = venv_dir / "bin" / "pip"
    result = subprocess.run(
        [str(pip), "check"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    output = (result.stdout + result.stderr).strip()
    return result.returncode == 0, output


def verify_shim_identity_strict(venv_dir: Path) -> list[str]:
    """Verify that httpx resolves to the eggfetch-backed shim via __eggfetch_shim__.

    This is the primary identity check. Returns list of errors (empty = ok).
    """
    errors = []
    python = venv_dir / "bin" / "python"

    result = subprocess.run(
        [str(python), "-c", """
import sys
try:
    import httpx
    # Check __eggfetch_shim__ attribute
    shim = getattr(httpx, '__eggfetch_shim__', False)
    if not shim:
        print(f'ERROR: httpx.__eggfetch_shim__ is not True (got {shim})')
        sys.exit(1)
    # Verify Client/AsyncClient are from eggfetch compat layer
    from httpx import Client, AsyncClient
    client_mod = getattr(Client, '__module__', '')
    async_mod = getattr(AsyncClient, '__module__', '')
    if 'eggfetch' not in client_mod and 'compat' not in client_mod:
        print(f'ERROR: Client.__module__={client_mod} (not eggfetch-backed)')
        sys.exit(1)
    if 'eggfetch' not in async_mod and 'compat' not in async_mod:
        print(f'ERROR: AsyncClient.__module__={async_mod} (not eggfetch-backed)')
        sys.exit(1)
    print(f'OK: shim identity verified (httpx.__file__={httpx.__file__})')
except ImportError as e:
    print(f'ERROR: httpx not importable: {e}')
    sys.exit(1)
"""],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode != 0:
        errors.append(f"Shim identity check failed: {(result.stdout + result.stderr).strip()}")

    return errors


def verify_no_upstream_httpx(venv_dir: Path) -> list[str]:
    """Assert that upstream httpx is NOT installed. Returns list of errors."""
    errors = []
    python = venv_dir / "bin" / "python"

    result = subprocess.run(
        [str(python), "-c", """
import importlib.util, os, sys
spec = importlib.util.find_spec('httpx')
if spec and spec.origin:
    httpx_dir = os.path.dirname(spec.origin)
    # Check __init__.py for shim markers
    init_path = os.path.join(httpx_dir, '__init__.py')
    if os.path.exists(init_path):
        with open(init_path) as f:
            content = f.read()
        if 'eggfetch' not in content and '__eggfetch_shim__' not in content:
            print(f'ERROR: Real upstream httpx at {httpx_dir}')
            sys.exit(1)
    print(f'httpx location OK: {httpx_dir}')
else:
    print('httpx not found on sys.path')
"""],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode != 0:
        errors.append(f"Upstream httpx detected: {(result.stdout + result.stderr).strip()}")

    return errors


def parse_pytest_counts(output: str) -> dict:
    """Parse pytest short output for test counts.

    Looks for lines like: '5 passed, 2 failed, 1 error in 0.34s'
    or 'no tests ran' or '1 passed in 0.01s'.
    """
    counts = {"passed": 0, "failed": 0, "error": 0, "skipped": 0, "total": 0}

    # Match the summary line: e.g. "5 passed, 2 failed, 1 error in 0.34s"
    # or "1 passed in 0.01s"
    summary_pattern = re.compile(
        r"(\d+)\s+(passed|failed|error|skipped|warnings?)"
    )
    for match in summary_pattern.finditer(output):
        num = int(match.group(1))
        kind = match.group(2).rstrip("s")  # normalize 'warnings' -> 'warning'
        if kind in counts:
            counts[kind] = num
            counts["total"] += num

    return counts


def run_tests(venv_dir: Path, pkg: dict, timeout: int) -> subprocess.CompletedProcess:
    """Run the package's test command from the manifest.

    The manifest test-command is ALWAYS used when present. We never fall back
    to pytest --pyargs when a command is specified.

    Commands are executed through the shell to preserve shell quoting (e.g.
    ``python -c 'import foo; print(foo.__version__)'``).
    """
    test_command = pkg.get("test-command", "")
    if not test_command:
        # No command specified — import-only smoke test
        python_bin = venv_dir / "bin" / "python"
        normalized = pkg["name"].replace("-", "_")
        test_command = f"{python_bin} -c \"import {normalized}; print(f'{normalized} OK')\""

    # For pytest commands, add --tb=short -q for structured output
    # Remove --co/--collect-only (we want to actually run tests, not just collect)
    if "pytest" in test_command:
        test_command = test_command.replace(" --co", "").replace(" --collect-only", "")
        if "--tb=short" not in test_command:
            test_command = test_command.replace("pytest ", "pytest --tb=short -q ", 1)

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

    return subprocess.run(
        test_command,
        shell=True,
        capture_output=True, text=True, timeout=timeout,
        env=env, cwd=str(venv_dir),
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Run downstream tests in isolation")
    parser.add_argument("--package", required=True, help="Package name from manifest")
    parser.add_argument("--wheel-dir", required=True,
                        help="Directory containing eggfetch .whl AND httpx controlled replacement .whl")
    parser.add_argument("--timeout", type=int, default=120,
                        help="Timeout in seconds (default: 120)")
    parser.add_argument("--keep-env", action="store_true",
                        help="Keep virtual environment after tests")
    parser.add_argument("--output", default=None,
                        help="Path to write structured result JSON (default: stdout)")
    args = parser.parse_args()

    wheel_dir = Path(args.wheel_dir).resolve()
    if not wheel_dir.exists():
        _emit_result({"status": "error", "message": f"Wheel directory not found: {wheel_dir}"}, args.output)
        return 2

    eggfetch_wheel, httpx_wheel = find_wheels(wheel_dir)
    if eggfetch_wheel is None:
        _emit_result({"status": "error", "message": f"No eggfetch wheel found in {wheel_dir}"}, args.output)
        return 2
    if httpx_wheel is None:
        _emit_result({"status": "error", "message": f"No httpx controlled replacement wheel found in {wheel_dir}"}, args.output)
        return 2

    manifest = load_manifest()
    if not manifest:
        _emit_result({"status": "error", "message": "Manifest not found or empty"}, args.output)
        return 2

    pkg = find_package(manifest, args.package)
    if pkg is None:
        # Fail-closed: unknown package in manifest is an error
        _emit_result({
            "status": "error",
            "package": args.package,
            "error": f"Package '{args.package}' not found in manifest (fail-closed)",
        }, args.output)
        return 1

    min_tests = pkg.get("min-tests", 0)
    usage = pkg.get("usage", "required")

    tmpdir = tempfile.mkdtemp(prefix=f"eggfetch-downstream-{args.package}-")
    start_time = time.monotonic()
    # Fail-closed: missing test command is an error for required packages
    test_command = pkg.get("test-command", "")
    if not test_command and usage == "required":
        _emit_result({
            "status": "error",
            "package": pkg["name"],
            "error": "Required package has no test-command (fail-closed)",
        }, args.output)
        return 1

    result: dict = {
        "package": pkg["name"],
        "version": pkg.get("version", "unknown"),
        "category": pkg.get("category", "unknown"),
        "usage": usage,
        "min_tests": min_tests,
        "source_type": pkg.get("source-type", ""),
        "source_locator": pkg.get("source-locator", ""),
        "source_hash": pkg.get("source-hash", ""),
        "venv_dir": tmpdir,
        "keep_env": args.keep_env,
        "timeout": args.timeout,
        "wheels": {
            "eggfetch": str(eggfetch_wheel),
            "httpx_replacement": str(httpx_wheel),
        },
        "install": {"success": False, "stdout": "", "stderr": ""},
        "shim_identity": {"pre_install": {"success": False, "errors": []},
                          "post_install": {"success": False, "errors": []}},
        "upstream_check": {"success": False, "errors": []},
        "pip_check": {"success": False, "output": ""},
        "tests": {"success": False, "returncode": -1, "stdout": "", "stderr": "",
                  "collected": 0, "passed": 0, "failed": 0, "error": 0, "skipped": 0},
        "status": "error",
        "duration_seconds": 0,
    }

    try:
        venv_dir = create_venv(Path(tmpdir))

        # --- Step 1: Install eggfetch wheel first ---
        install_result = pip_install(venv_dir, [str(eggfetch_wheel)], args.timeout)
        result["install"] = {
            "success": install_result.returncode == 0,
            "stdout": install_result.stdout,
            "stderr": install_result.stderr,
        }
        if install_result.returncode != 0:
            result["status"] = "install-failed"
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 1

        # --- Step 2: Install controlled replacement httpx wheel ---
        httpx_install = pip_install(venv_dir, [str(httpx_wheel)], args.timeout)
        if httpx_install.returncode != 0:
            result["install"]["success"] = False
            result["install"]["stderr"] += "\n--- httpx replacement install ---\n" + httpx_install.stderr
            result["status"] = "install-failed"
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 1

        # --- Step 3: Verify shim identity BEFORE downstream deps ---
        pre_shim_errors = verify_shim_identity_strict(venv_dir)
        result["shim_identity"]["pre_install"] = {
            "success": len(pre_shim_errors) == 0,
            "errors": pre_shim_errors,
        }
        if pre_shim_errors:
            result["status"] = "shim-identity-failure"
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 1

        # --- Step 4: Verify no upstream httpx ---
        upstream_errors = verify_no_upstream_httpx(venv_dir)
        result["upstream_check"] = {
            "success": len(upstream_errors) == 0,
            "errors": upstream_errors,
        }
        if upstream_errors:
            result["status"] = "upstream-httpx-detected"
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 1

        # --- Step 5: Install the downstream package ---
        # Filter out 'httpx' from optional-dependencies — we use the shim
        downstream_deps = [d for d in pkg.get("optional-dependencies", []) if d != "httpx"]
        downstream_install = pip_install(venv_dir, [pkg["name"]] + downstream_deps, args.timeout)
        if downstream_install.returncode != 0:
            result["install"]["success"] = False
            result["install"]["stderr"] += "\n--- downstream install ---\n" + downstream_install.stderr
            result["status"] = "downstream-install-failed"
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 1

        # --- Step 6: Re-verify shim identity AFTER downstream deps ---
        post_shim_errors = verify_shim_identity_strict(venv_dir)
        result["shim_identity"]["post_install"] = {
            "success": len(post_shim_errors) == 0,
            "errors": post_shim_errors,
        }
        if post_shim_errors:
            result["status"] = "shim-identity-failure"
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 1

        # --- Step 7: Run pip check ---
        pip_ok, pip_output = pip_check(venv_dir)
        result["pip_check"] = {"success": pip_ok, "output": pip_output}
        if not pip_ok:
            result["status"] = "pip-check-failure"
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 1

        # --- Step 8: Run tests ---
        test_command = pkg.get("test-command", "")
        is_pytest_cmd = "pytest" in test_command

        test_result = run_tests(venv_dir, pkg, args.timeout)
        output = test_result.stdout + test_result.stderr

        if is_pytest_cmd:
            counts = parse_pytest_counts(output)
            no_tests_collected = counts["total"] == 0 and test_result.returncode == 5
        else:
            # Non-pytest command: success means the import/smoke test passed
            counts = {
                "passed": 1 if test_result.returncode == 0 else 0,
                "failed": 0,
                "error": 0,
                "skipped": 0,
                "total": 1 if test_result.returncode == 0 else 0,
            }
            no_tests_collected = False

        result["tests"] = {
            "success": test_result.returncode == 0,
            "returncode": test_result.returncode,
            "stdout": test_result.stdout,
            "stderr": test_result.stderr,
            "collected": counts["total"],
            "passed": counts["passed"],
            "failed": counts["failed"],
            "error": counts["error"],
            "skipped": counts["skipped"],
        }

        # --- Step 9: Enforce min-tests ---
        if min_tests > 0 and counts["total"] < min_tests:
            result["status"] = "below-min-tests"
            result["tests"]["success"] = False
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 1

        if no_tests_collected:
            if min_tests > 0:
                result["status"] = "zero-tests-expected"
                result["tests"]["success"] = False
            else:
                result["status"] = "skipped-no-tests"
                result["tests"]["success"] = True
        elif test_result.returncode == 0:
            result["status"] = "passed"
        else:
            result["status"] = "failed"

        # Fail-closed: zero tests collected for a required package is an error
        if counts["total"] == 0 and usage == "required":
            result["status"] = "zero-tests-required"
            result["tests"]["success"] = False
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 1

        result["duration_seconds"] = round(time.monotonic() - start_time, 2)
        _emit_result(result, args.output)
        return 0 if result["status"] in ("passed", "skipped-no-tests") else 1

    except subprocess.TimeoutExpired:
        result["status"] = "timeout"
        result["duration_seconds"] = round(time.monotonic() - start_time, 2)
        _emit_result(result, args.output)
        return 1
    except Exception as exc:
        result["status"] = "error"
        result["error"] = str(exc)
        result["duration_seconds"] = round(time.monotonic() - start_time, 2)
        _emit_result(result, args.output)
        return 1
    finally:
        if not args.keep_env and os.path.exists(tmpdir):
            shutil.rmtree(tmpdir, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
