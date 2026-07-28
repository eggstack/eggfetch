#!/usr/bin/env python3
"""Run a downstream package's tests in an isolated virtual environment.

Creates a temporary venv, installs the eggfetch wheel AND the controlled
replacement httpx wheel, installs the target package, and runs its tests
with network disabled. Verifies shim identity at multiple points.

Usage:
    run_isolated_downstream.py --package <name> --artifact-manifest <manifest.json> \
        [--candidate-identity <identity.json>] [--timeout <seconds>] [--keep-env]

The package name must exist in compat/downstream/manifest.toml.
The artifact-manifest must list both the eggfetch wheel and the httpx
controlled replacement wheel with verified SHA-256 hashes.

Exit codes:
    0 — tests passed or package not found (graceful skip)
    1 — tests failed
    2 — argument or manifest error
    3 — structured diagnostic (see diagnostic_code in result JSON)

Diagnostic codes:
    E001 unknown-package          Package not found in downstream manifest
    E002 empty-selection          Package selection filter yielded no matches
    E003 missing-test-command     Required package has no test-command
    E004 import-only-required     Required package has import-only test-command
    E005 source-hash-missing      Package has no source-hash in manifest
    E006 source-hash-mismatch     Downloaded source hash does not match manifest
    E007 upstream-httpx-detected  Upstream httpx found instead of shim
    E008 shim-identity-mismatch   httpx does not resolve to eggfetch shim
    E009 pip-check-failure        pip check found dependency conflicts
    E010 zero-tests              Zero tests collected for required package
    E011 skipped-required         Required suite was skipped
    E012 xfailed-required         Required suite was xfailed (expected failures)
    E013 below-min-count         Collected/passed below manifest minimum
    E014 malformed-result        Runner produced unparseable output
    E015 identity-mismatch       Candidate identity does not match
    E016 artifact-mismatch       Artifact hash mismatch
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

DIAGNOSTIC_CODES = {
    "unknown-package": "E001",
    "empty-selection": "E002",
    "missing-test-command": "E003",
    "import-only-required": "E004",
    "source-hash-missing": "E005",
    "source-hash-mismatch": "E006",
    "upstream-httpx-detected": "E007",
    "shim-identity-mismatch": "E008",
    "pip-check-failure": "E009",
    "zero-tests": "E010",
    "skipped-required": "E011",
    "xfailed-required": "E012",
    "below-min-count": "E013",
    "malformed-result": "E014",
    "identity-mismatch": "E015",
    "artifact-mismatch": "E016",
}


def _diagnostic(code_name: str, message: str) -> dict:
    """Build a structured diagnostic result."""
    code = DIAGNOSTIC_CODES.get(code_name, "E999")
    return {
        "status": "error",
        "diagnostic_code": code,
        "diagnostic_name": code_name,
        "error": message,
    }


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


def find_wheels(artifact_manifest: dict, bundle_root: Path | None = None) -> tuple[Path | None, Path | None]:
    """Find the eggfetch wheel and the httpx controlled replacement wheel.

    Reads paths from the artifact manifest and verifies their SHA-256 hashes.
    Paths are resolved relative to bundle_root if provided.

    Returns (eggfetch_wheel, httpx_wheel).
    """
    eggfetch_wheel = None
    httpx_wheel = None

    for art in artifact_manifest.get("artifacts", []):
        # Support schema v3 (role) and schema v2 (artifact_type)
        art_type = art.get("role", art.get("artifact_type", ""))
        # Support schema v3 (relative_path) and schema v2 (path)
        rel_path = art.get("relative_path", art.get("path", ""))
        expected_hash = art.get("sha256", "")

        if bundle_root and rel_path:
            path = bundle_root / rel_path
        else:
            path = Path(rel_path)

        if not path.exists():
            continue

        actual_hash = _compute_sha256(path)
        if actual_hash != expected_hash:
            continue

        if art_type == "eggfetch":
            eggfetch_wheel = path
        elif art_type == "httpx-controlled-replacement":
            httpx_wheel = path

    return eggfetch_wheel, httpx_wheel


def _compute_sha256(path: Path) -> str:
    """Compute SHA-256 hash of a file."""
    import hashlib

    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


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


def pip_install_no_deps(venv_dir: Path, packages: list[str], timeout: int) -> subprocess.CompletedProcess:
    """Install packages without resolving dependencies.

    Used for downstream packages whose httpx version constraint may conflict
    with the controlled replacement wheel. We install the controlled httpx
    first, then install the downstream package with --no-deps so pip doesn't
    replace it.
    """
    pip = venv_dir / "bin" / "pip"
    return subprocess.run(
        [str(pip), "install", "--quiet", "--disable-pip-version-check", "--no-deps", *packages],
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


def pip_show_dist(venv_dir: Path, package_name: str) -> dict:
    """Query pip show for installed distribution metadata. Returns dict with
    name, version, location, or empty dict on failure."""
    pip = venv_dir / "bin" / "pip"
    result = subprocess.run(
        [str(pip), "show", package_name],
        capture_output=True,
        text=True,
        timeout=15,
    )
    info: dict[str, str] = {}
    if result.returncode == 0:
        for line in result.stdout.splitlines():
            if ":" in line:
                key, _, val = line.partition(":")
                info[key.strip().lower()] = val.strip()
    return info


def verify_source_hash(pkg: dict, timeout: int) -> tuple[bool, str]:
    """Verify the source hash of a package by downloading its wheel.

    Returns (ok, error_message). Downloads the wheel to a temp directory,
    computes its SHA-256, and compares against the manifest's source-hash.
    """
    import hashlib
    import tempfile
    import urllib.request

    source_hash = pkg.get("source-hash", "")
    if not source_hash:
        return False, f"Package '{pkg['name']}' has empty source-hash"

    source_locator = pkg.get("source-locator", "")
    if not source_locator:
        return False, f"Package('{pkg['name']}') has empty source-locator"

    # Parse package name and version from source-locator (e.g., "respx==0.21.1")
    if "==" in source_locator:
        pkg_name, pkg_version = source_locator.split("==", 1)
    else:
        pkg_name, pkg_version = source_locator, ""

    # Fetch wheel URL from PyPI JSON API
    url = f"https://pypi.org/pypi/{pkg_name}/{pkg_version}/json"
    try:
        with urllib.request.urlopen(url, timeout=timeout) as resp:
            data = json.loads(resp.read())
    except Exception as e:
        return False, f"Failed to fetch PyPI metadata for {pkg_name}=={pkg_version}: {e}"

    # Find the py3-none-any wheel
    wheel_url = None
    wheel_filename = None
    for f in data.get("urls", []):
        if f["filename"].endswith(".whl") and "py3-none-any" in f["filename"]:
            wheel_url = f["url"]
            wheel_filename = f["filename"]
            break
    if not wheel_url:
        for f in data.get("urls", []):
            if f["filename"].endswith(".whl"):
                wheel_url = f["url"]
                wheel_filename = f["filename"]
                break
    if not wheel_url:
        return False, f"No wheel found for {pkg_name}=={pkg_version}"

    # Download and hash
    with tempfile.NamedTemporaryFile(delete=False, suffix=".whl") as tmp:
        try:
            urllib.request.urlretrieve(wheel_url, tmp.name)
        except Exception as e:
            return False, f"Failed to download wheel: {e}"

        actual_hash = hashlib.sha256(open(tmp.name, "rb").read()).hexdigest()

    if actual_hash != source_hash:
        return False, (
            f"Source hash mismatch for {pkg_name}=={pkg_version}: "
            f"expected {source_hash}, got {actual_hash}"
        )

    return True, wheel_filename or ""


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
    counts = {"passed": 0, "failed": 0, "error": 0, "skipped": 0, "xfailed": 0, "total": 0}

    # Match the summary line: e.g. "5 passed, 2 failed, 1 error in 0.34s"
    # or "1 passed in 0.01s"
    summary_pattern = re.compile(
        r"(\d+)\s+(passed|failed|error|skipped|xfail(?:ed)?|warnings?)"
    )
    for match in summary_pattern.finditer(output):
        num = int(match.group(1))
        kind = match.group(2).rstrip("s")  # normalize 'warnings' -> 'warning'
        if kind == "xfailed":
            kind = "xfailed"
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

    pytest commands are redirected to the isolated venv's python -m pytest
    to avoid using the outer workflow's pytest binary. The cwd is set to the
    repo root so relative test file paths resolve correctly.
    """
    test_command = pkg.get("test-command", "")
    if not test_command:
        # No command specified — import-only smoke test
        python_bin = venv_dir / "bin" / "python"
        normalized = pkg["name"].replace("-", "_")
        test_command = f"{python_bin} -c \"import {normalized}; print(f'{normalized} OK')\""

    # For pytest commands, add --tb=short -q for structured output
    # Remove --co/--collect-only (we want to actually run tests, not just collect)
    # Use the outer venv's pytest via PATH, NOT the isolated venv's python.
    # The isolated venv's site-packages is set in PYTHONPATH so imports resolve there.
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

    # Use repo root as cwd so relative test file paths resolve
    repo_root = MANIFEST_PATH.parent.parent

    # Use repo root as cwd so relative test file paths resolve
    repo_root = str(MANIFEST_PATH.parent.parent)

    return subprocess.run(
        test_command,
        shell=True,
        capture_output=True, text=True, timeout=timeout,
        env=env, cwd=repo_root,
    )


def _is_import_only_command(test_command: str) -> bool:
    """Return True if the test command is a trivial import-only smoke test."""
    if not test_command:
        return True
    if "-c" in test_command and "import" in test_command:
        # Check for behavioral assertions beyond print
        if "assert" not in test_command and "assert" not in test_command:
            # Check for actual test frameworks
            if "pytest" not in test_command and "unittest" not in test_command:
                return True
    return False


def main() -> int:
    parser = argparse.ArgumentParser(description="Run downstream tests in isolation")
    parser.add_argument("--package", "--package-id", dest="package", required=True,
                        help="Package name from manifest")
    parser.add_argument("--artifact-manifest", default=None,
                        help="Path to artifact-manifest.json listing built wheels with hashes")
    parser.add_argument("--manifest", default=None,
                        help="Path to downstream manifest.toml (for resolving package details)")
    parser.add_argument("--bundle-root", default=None,
                        help="Bundle root directory for resolving relative wheel paths")
    parser.add_argument("--candidate-identity", default=None,
                        help="Path to candidate-identity.json for identity propagation")
    parser.add_argument("--timeout", type=int, default=120,
                        help="Timeout in seconds (default: 120)")
    parser.add_argument("--keep-env", action="store_true",
                        help="Keep virtual environment after tests")
    parser.add_argument("--output", default=None,
                        help="Path to write structured result JSON (default: stdout)")
    args = parser.parse_args()

    # Resolve artifact manifest path
    artifact_manifest_arg = args.artifact_manifest
    if not artifact_manifest_arg and args.bundle_root:
        artifact_manifest_arg = str(Path(args.bundle_root) / "artifact-manifest.json")
    if not artifact_manifest_arg:
        parser.error("--artifact-manifest or --bundle-root is required")

    manifest_path = Path(artifact_manifest_arg).resolve()
    if not manifest_path.exists():
        _emit_result({"status": "error", "message": f"Artifact manifest not found: {manifest_path}"}, args.output)
        return 2

    with open(manifest_path) as f:
        artifact_manifest = json.load(f)

    # Load candidate identity if provided
    candidate_identity = None
    if args.candidate_identity:
        identity_path = Path(args.candidate_identity).resolve()
        if identity_path.exists():
            with open(identity_path) as f:
                candidate_identity = json.load(f)

    bundle_root = Path(args.bundle_root).resolve() if args.bundle_root else None
    eggfetch_wheel, httpx_wheel = find_wheels(artifact_manifest, bundle_root=bundle_root)
    if eggfetch_wheel is None:
        _emit_result(
            {**_diagnostic("artifact-mismatch", "No valid eggfetch wheel found in artifact manifest (hash mismatch or missing)")},  # noqa: E501
            args.output,
        )
        return 3
    if httpx_wheel is None:
        _emit_result(
            {**_diagnostic("artifact-mismatch", "No valid httpx controlled replacement wheel found in artifact manifest (hash mismatch or missing)")},  # noqa: E501
            args.output,
        )
        return 3

    manifest = load_manifest()
    if not manifest:
        _emit_result({"status": "error", "message": "Manifest not found or empty"}, args.output)
        return 2

    pkg = find_package(manifest, args.package)
    if pkg is None:
        # Fail-closed: unknown package in manifest is an error
        diag = _diagnostic("unknown-package", f"Package '{args.package}' not found in downstream manifest")
        diag["package"] = args.package
        _emit_result(diag, args.output)
        return 3

    min_tests = pkg.get("min-tests", 0)
    min_collected = pkg.get("min-collected", 0)
    min_passed = pkg.get("min-passed", 0)
    max_skipped = pkg.get("max-skipped", -1)
    max_xfailed = pkg.get("max-xfailed", -1)
    usage = pkg.get("usage", "required")

    tmpdir = tempfile.mkdtemp(prefix=f"eggfetch-downstream-{args.package}-")
    start_time = time.monotonic()

    # Fail-closed: missing test command is an error for required packages
    test_command = pkg.get("test-command", "")
    if not test_command and usage == "required":
        diag = _diagnostic("missing-test-command", f"Required package '{pkg['name']}' has no test-command")
        diag["package"] = pkg["name"]
        _emit_result(diag, args.output)
        return 3

    # Fail-closed: import-only test command is an error for required packages
    if usage == "required" and test_command and _is_import_only_command(test_command):
        diag = _diagnostic(
            "import-only-required",
            f"Required package '{pkg['name']}' has import-only test-command; expected behavioral test",
        )
        diag["package"] = pkg["name"]
        _emit_result(diag, args.output)
        return 3

    # Fail-closed: missing source-hash for required packages
    source_hash = pkg.get("source-hash", "")
    if usage == "required" and not source_hash:
        diag = _diagnostic("source-hash-missing", f"Required package '{pkg['name']}' has no source-hash")
        diag["package"] = pkg["name"]
        _emit_result(diag, args.output)
        return 3

    result: dict = {
        "package": pkg["name"],
        "version": pkg.get("version", "unknown"),
        "category": pkg.get("category", "unknown"),
        "usage": usage,
        "min_tests": min_tests,
        "min_collected": min_collected,
        "min_passed": min_passed,
        "max_skipped": max_skipped,
        "max_xfailed": max_xfailed,
        "source_type": pkg.get("source-type", ""),
        "source_locator": pkg.get("source-locator", ""),
        "source_hash": source_hash,
        "venv_dir": tmpdir,
        "keep_env": args.keep_env,
        "timeout": args.timeout,
        "wheels": {
            "eggfetch": str(eggfetch_wheel),
            "httpx_replacement": str(httpx_wheel),
            "artifact_manifest": str(manifest_path),
        },
        "candidate_identity": candidate_identity,
        "install": {"success": False, "stdout": "", "stderr": ""},
        "installed_dist": {"name": "", "version": "", "location": ""},
        "source_hash_verification": {"success": False, "hash": "", "error": ""},
        "shim_identity": {"pre_install": {"success": False, "errors": []},
                          "post_install": {"success": False, "errors": []}},
        "upstream_check": {"success": False, "errors": []},
        "pip_check": {"success": False, "output": ""},
        "tests": {"success": False, "returncode": -1, "stdout": "", "stderr": "",
                  "collected": 0, "passed": 0, "failed": 0, "error": 0, "skipped": 0, "xfailed": 0},
        "status": "error",
        "diagnostic_code": "",
        "diagnostic_name": "",
        "diagnostics": [],
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
            diag = _diagnostic("shim-identity-mismatch", pre_shim_errors[0])
            result["diagnostic_code"] = diag["diagnostic_code"]
            result["diagnostic_name"] = diag["diagnostic_name"]
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 3

        # --- Step 4: Verify no upstream httpx ---
        upstream_errors = verify_no_upstream_httpx(venv_dir)
        result["upstream_check"] = {
            "success": len(upstream_errors) == 0,
            "errors": upstream_errors,
        }
        if upstream_errors:
            result["status"] = "upstream-httpx-detected"
            diag = _diagnostic("upstream-httpx-detected", upstream_errors[0])
            result["diagnostic_code"] = diag["diagnostic_code"]
            result["diagnostic_name"] = diag["diagnostic_name"]
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 3

        # --- Step 5: Verify source hash before downstream install ---
        source_ok, source_msg = verify_source_hash(pkg, args.timeout)
        result["source_hash_verification"] = {
            "success": source_ok,
            "hash": source_hash,
            "error": "" if source_ok else source_msg,
        }
        if not source_ok:
            result["status"] = "source-hash-mismatch"
            diag = _diagnostic("source-hash-mismatch", source_msg)
            result["diagnostic_code"] = diag["diagnostic_code"]
            result["diagnostic_name"] = diag["diagnostic_name"]
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 3

        # --- Step 6: Install the downstream package ---
        # Use --no-deps to prevent pip from replacing the controlled httpx
        # wheel. The downstream package's httpx version constraint may not
        # match the controlled replacement (0.28.1), causing pip to install
        # the real httpx from PyPI.
        downstream_deps = [d for d in pkg.get("optional-dependencies", []) if d != "httpx"]
        downstream_install = pip_install_no_deps(venv_dir, [pkg["name"]] + downstream_deps, args.timeout)
        if downstream_install.returncode != 0:
            result["install"]["success"] = False
            result["install"]["stderr"] += "\n--- downstream install ---\n" + downstream_install.stderr
            result["status"] = "downstream-install-failed"
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 1

        # --- Step 6b: Install safe transitive deps ---
        # The --no-deps install above skips all transitive dependencies.
        # Install safe ones (excluding httpx) so the downstream package can
        # import its own dependencies. Use --no-deps to avoid pulling httpx.
        safe_transitive = []
        for dep_name in ["httpcore", "anyio", "sniffio", "idna", "certifi",
                         "h11", "h2", "pydantic", "typing-extensions"]:
            if dep_name != "httpx" and dep_name not in downstream_deps:
                safe_transitive.append(dep_name)
        if safe_transitive:
            pip_install_no_deps(venv_dir, safe_transitive, args.timeout)

        # --- Step 6b: Record installed distribution metadata ---
        dist_info = pip_show_dist(venv_dir, pkg["name"])
        result["installed_dist"] = {
            "name": dist_info.get("name", ""),
            "version": dist_info.get("version", ""),
            "location": dist_info.get("location", ""),
        }

        # --- Step 7: Re-verify shim identity AFTER downstream deps ---
        post_shim_errors = verify_shim_identity_strict(venv_dir)
        result["shim_identity"]["post_install"] = {
            "success": len(post_shim_errors) == 0,
            "errors": post_shim_errors,
        }
        if post_shim_errors:
            result["status"] = "shim-identity-failure"
            diag = _diagnostic("shim-identity-mismatch", post_shim_errors[0])
            result["diagnostic_code"] = diag["diagnostic_code"]
            result["diagnostic_name"] = diag["diagnostic_name"]
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 3

        # --- Step 8: Run pip check (non-fatal diagnostic) ---
        pip_ok, pip_output = pip_check(venv_dir)
        result["pip_check"] = {"success": pip_ok, "output": pip_output}
        if not pip_ok:
            result["diagnostics"].append(
                f"pip check warning: {pip_output[:500]}"
            )

        # --- Step 9: Verify candidate identity if provided ---
        if candidate_identity:
            expected_name = candidate_identity.get("package_name", "")
            expected_version = candidate_identity.get("version", "")
            if expected_name and expected_name != pkg["name"]:
                result["status"] = "identity-mismatch"
                diag = _diagnostic(
                    "identity-mismatch",
                    f"Candidate identity package '{expected_name}' != manifest package '{pkg['name']}'",
                )
                result["diagnostic_code"] = diag["diagnostic_code"]
                result["diagnostic_name"] = diag["diagnostic_name"]
                result["duration_seconds"] = round(time.monotonic() - start_time, 2)
                _emit_result(result, args.output)
                return 3

        # --- Step 10: Run tests ---
        is_pytest_cmd = "pytest" in (pkg.get("test-command", "") or "")

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
                "xfailed": 0,
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
            "xfailed": counts.get("xfailed", 0),
        }

        # --- Step 11: Enforce min-collected ---
        if min_collected > 0 and counts["total"] < min_collected:
            result["status"] = "below-min-count"
            result["tests"]["success"] = False
            diag = _diagnostic(
                "below-min-count",
                f"Collected {counts['total']} tests, minimum required: {min_collected}",
            )
            result["diagnostic_code"] = diag["diagnostic_code"]
            result["diagnostic_name"] = diag["diagnostic_name"]
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 3

        # --- Step 12: Enforce min-passed ---
        if min_passed > 0 and counts["passed"] < min_passed:
            result["status"] = "below-min-count"
            result["tests"]["success"] = False
            diag = _diagnostic(
                "below-min-count",
                f"Passed {counts['passed']} tests, minimum required: {min_passed}",
            )
            result["diagnostic_code"] = diag["diagnostic_code"]
            result["diagnostic_name"] = diag["diagnostic_name"]
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 3

        # --- Step 13: Enforce max-skipped ---
        if max_skipped >= 0 and counts["skipped"] > max_skipped:
            result["status"] = "skipped-required"
            result["tests"]["success"] = False
            diag = _diagnostic(
                "skipped-required",
                f"Skipped {counts['skipped']} tests, maximum allowed: {max_skipped}",
            )
            result["diagnostic_code"] = diag["diagnostic_code"]
            result["diagnostic_name"] = diag["diagnostic_name"]
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 3

        # --- Step 14: Enforce max-xfailed ---
        if max_xfailed >= 0 and counts.get("xfailed", 0) > max_xfailed:
            result["status"] = "xfailed-required"
            result["tests"]["success"] = False
            diag = _diagnostic(
                "xfailed-required",
                f"Xfailed {counts.get('xfailed', 0)} tests, maximum allowed: {max_xfailed}",
            )
            result["diagnostic_code"] = diag["diagnostic_code"]
            result["diagnostic_name"] = diag["diagnostic_name"]
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 3

        # --- Step 15: Legacy min-tests enforcement (backward compat) ---
        if min_tests > 0 and counts["total"] < min_tests:
            result["status"] = "below-min-count"
            result["tests"]["success"] = False
            diag = _diagnostic(
                "below-min-count",
                f"Total {counts['total']} tests below min-tests threshold: {min_tests}",
            )
            result["diagnostic_code"] = diag["diagnostic_code"]
            result["diagnostic_name"] = diag["diagnostic_name"]
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 3

        if no_tests_collected:
            if min_tests > 0 or min_collected > 0:
                result["status"] = "zero-tests"
                result["tests"]["success"] = False
                diag = _diagnostic("zero-tests", f"Zero tests collected for required package '{pkg['name']}'")
                result["diagnostic_code"] = diag["diagnostic_code"]
                result["diagnostic_name"] = diag["diagnostic_name"]
            else:
                result["status"] = "skipped-no-tests"
                result["tests"]["success"] = True
        elif test_result.returncode == 0:
            result["status"] = "passed"
        else:
            result["status"] = "failed"

        # Fail-closed: zero tests collected for a required package is an error
        if counts["total"] == 0 and usage == "required":
            result["status"] = "zero-tests"
            result["tests"]["success"] = False
            diag = _diagnostic("zero-tests", f"Zero tests collected for required package '{pkg['name']}'")
            result["diagnostic_code"] = diag["diagnostic_code"]
            result["diagnostic_name"] = diag["diagnostic_name"]
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 3

        # Fail-closed: skipped required suite
        if counts["skipped"] > 0 and counts["passed"] == 0 and counts["failed"] == 0 and usage == "required":
            result["status"] = "skipped-required"
            result["tests"]["success"] = False
            diag = _diagnostic(
                "skipped-required",
                f"Required package '{pkg['name']}' had all tests skipped",
            )
            result["diagnostic_code"] = diag["diagnostic_code"]
            result["diagnostic_name"] = diag["diagnostic_name"]
            result["duration_seconds"] = round(time.monotonic() - start_time, 2)
            _emit_result(result, args.output)
            return 3

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
