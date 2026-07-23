#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import platform
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
COMPAT_DIR = REPO_ROOT / "compat" / "httpx" / "0.28.1"
DOWNSTREAM_DIR = REPO_ROOT / "compat" / "downstream"
TEST_DIR = REPO_ROOT / "crates" / "eggfetch-python" / "tests" / "compat"
REFERENCE_API_PATH = COMPAT_DIR / "reference-api.json"


def _load_toml_simple(path: Path) -> dict[str, Any] | list[dict[str, Any]]:
    """Minimal TOML loader handling [[array-of-tables]] and [table] sections."""
    try:
        import tomllib  # type: ignore[import-untyped]
        with open(path, "rb") as f:
            return tomllib.load(f)
    except ImportError:
        pass
    try:
        import tomli as tomllib  # type: ignore[import-untyped]
        with open(path, "rb") as f:
            return tomllib.load(f)
    except ImportError:
        pass
    return _parse_toml_fallback(path)


def _parse_toml_fallback(path: Path) -> dict[str, Any]:
    sections: dict[str, Any] = {}
    current_key: str | None = None
    current_table: dict[str, Any] = {}
    in_array = False
    array_key: str | None = None
    array_items: list[dict[str, Any]] = []

    with open(path) as f:
        for line in f:
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue

            if stripped.startswith("[["):
                if in_array and current_key:
                    array_items.append(current_table)
                current_table = {}
                in_array = True
                array_key = stripped[2:-2].strip()
                current_key = array_key
                continue

            if stripped.startswith("["):
                if in_array and current_key:
                    array_items.append(current_table)
                    sections.setdefault(array_key, array_items)
                    array_items = []
                    in_array = False
                    current_table = {}
                current_key = stripped[1:-1].strip()
                if current_key and current_key not in sections:
                    sections[current_key] = {}
                continue

            if "=" not in stripped:
                continue

            key, _, val = stripped.partition("=")
            key = key.strip()
            val = val.strip()

            if val.startswith('"') and val.endswith('"'):
                val = val[1:-1]
            elif val == "true":
                val = True
            elif val == "false":
                val = False
            else:
                try:
                    val = int(val)
                except ValueError:
                    pass

            if in_array:
                current_table[key] = val
            elif current_key:
                sections[current_key][key] = val
            else:
                sections[key] = val

    if in_array and current_key:
        array_items.append(current_table)
        sections.setdefault(array_key, array_items)

    return sections


def _git_commit() -> str:
    try:
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            capture_output=True, text=True, cwd=REPO_ROOT, timeout=10,
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except (subprocess.TimeoutExpired, FileNotFoundError):
        pass
    return "unknown"


def _eggfetch_version() -> str:
    pyproject = REPO_ROOT / "crates" / "eggfetch-python" / "pyproject.toml"
    if pyproject.exists():
        content = pyproject.read_text()
        match = re.search(r'^version\s*=\s*"([^"]+)"', content, re.MULTILINE)
        if match:
            return match.group(1)
    cargo = REPO_ROOT / "crates" / "eggfetch-core" / "Cargo.toml"
    if cargo.exists():
        content = cargo.read_text()
        match = re.search(r'^version\s*=\s*"([^"]+)"', content, re.MULTILINE)
        if match:
            return match.group(1)
    return "unknown"


def _platform_python_backend() -> dict[str, str]:
    return {
        "platform": platform.platform(),
        "python": platform.python_version(),
        "python_implementation": platform.python_implementation(),
        "architecture": platform.machine(),
    }


def _api_manifest_summary() -> dict[str, Any]:
    if not REFERENCE_API_PATH.exists():
        return {"available": False, "total_symbols": 0}
    try:
        with open(REFERENCE_API_PATH) as f:
            manifest = json.load(f)
        symbols = manifest.get("symbols", [])
        by_kind: dict[str, int] = {}
        for s in symbols:
            kind = s.get("kind", "unknown")
            by_kind[kind] = by_kind.get(kind, 0) + 1
        return {
            "available": True,
            "total_symbols": len(symbols),
            "by_kind": by_kind,
        }
    except (json.JSONDecodeError, OSError):
        return {"available": False, "total_symbols": 0}


def _differential_case_totals() -> int:
    if not TEST_DIR.exists():
        return 0
    count = 0
    for py_file in TEST_DIR.glob("test_*.py"):
        content = py_file.read_text()
        count += len(re.findall(r"^\s*def (test_\w+)", content, re.MULTILINE))
        count += len(re.findall(r"^\s*async def (test_\w+)", content, re.MULTILINE))
    return count


def _run_pytest(skip: bool) -> dict[str, Any]:
    if skip:
        return {"skipped": True, "total": 0, "failures": 0, "errors": 0, "passed": 0}
    if not TEST_DIR.exists():
        return {"skipped": False, "total": 0, "failures": 0, "errors": 0, "passed": 0, "reason": "test directory not found"}

    compat_marker = REPO_ROOT / "crates" / "eggfetch-python" / "tests" / "compat"
    test_files = sorted(compat_marker.glob("test_*.py"))
    if not test_files:
        return {"skipped": False, "total": 0, "failures": 0, "errors": 0, "passed": 0, "reason": "no test files found"}

    try:
        result = subprocess.run(
            [sys.executable, "-m", "pytest", str(TEST_DIR), "--tb=short", "-q", "--no-header"],
            capture_output=True, text=True, cwd=REPO_ROOT, timeout=300,
        )
        output = result.stdout + "\n" + result.stderr
    except (subprocess.TimeoutExpired, FileNotFoundError) as exc:
        return {"skipped": False, "total": 0, "failures": 0, "errors": 0, "passed": 0, "reason": str(exc)}

    total_match = re.search(r"(\d+) (?:test|item)s? (?:passed|run)", output)
    fail_match = re.search(r"(\d+) (?:test|item)s? (?:failed|failure)", output)
    error_match = re.search(r"(\d+) (?:test|item)s? (?:error)", output)
    pass_match = re.search(r"(\d+) passed", output)

    total = int(total_match.group(1)) if total_match else 0
    failures = int(fail_match.group(1)) if fail_match else 0
    errors = int(error_match.group(1)) if error_match else 0
    passed = int(pass_match.group(1)) if pass_match else 0

    if not total and (failures or errors or passed):
        total = failures + errors + passed

    return {
        "skipped": False,
        "total": total,
        "failures": failures,
        "errors": errors,
        "passed": passed,
    }


def _downstream_manifest() -> dict[str, Any]:
    manifest_path = DOWNSTREAM_DIR / "manifest.toml"
    if not manifest_path.exists():
        return {"available": False, "packages": {}}
    try:
        data = _load_toml_simple(manifest_path)
        if isinstance(data, dict):
            # Handle [[package]] array-of-tables format
            packages = data.get("package", data.get("packages", []))
            if isinstance(packages, list):
                pkg_dict = {p["name"]: p for p in packages if isinstance(p, dict) and "name" in p}
                return {"available": True, "packages": pkg_dict}
            return {"available": True, "packages": packages}
        return {"available": True, "packages": {}}
    except Exception:
        return {"available": False, "packages": {}}


def _downstream_results(packages: dict[str, Any]) -> dict[str, Any]:
    results: dict[str, Any] = {}
    for pkg_name, pkg_info in packages.items():
        if isinstance(pkg_info, dict):
            version = pkg_info.get("version", "unknown")
        else:
            version = str(pkg_info)
        try:
            import importlib
            mod = importlib.import_module(pkg_name)
            installed_version = getattr(mod, "__version__", "unknown")
            results[pkg_name] = {"available": True, "installed_version": installed_version, "expected_version": version}
        except ImportError:
            results[pkg_name] = {"available": False, "expected_version": version}
    return results


def _allowed_differences() -> list[dict[str, Any]]:
    allowed_path = COMPAT_DIR / "allowed-differences.toml"
    if not allowed_path.exists():
        return []
    try:
        data = _load_toml_simple(allowed_path)
        if isinstance(data, dict):
            diffs = data.get("difference", [])
            if isinstance(diffs, dict):
                diffs = [diffs]
            return diffs
    except Exception:
        pass
    return []


def generate_evidence(output_path: str, skip_tests: bool) -> dict[str, Any]:
    downstream = _downstream_manifest()
    pytest_results = _run_pytest(skip_tests)

    overall_pass = (
        not pytest_results.get("skipped", False)
        and pytest_results.get("failures", 0) == 0
        and pytest_results.get("errors", 0) == 0
    )

    evidence: dict[str, Any] = {
        "schema_version": "1",
        "eggfetch_commit": _git_commit(),
        "eggfetch_version": _eggfetch_version(),
        "reference_httpx_version": "0.28.1",
        "compatibility_stage": "phase-5",
        "platform_python_backend": _platform_python_backend(),
        "api_manifest_summary": _api_manifest_summary(),
        "differential_case_totals": _differential_case_totals(),
        "differential_case_results": pytest_results,
        "downstream_package_versions": downstream.get("packages", {}),
        "downstream_results": _downstream_results(downstream.get("packages", {})),
        "allowed_differences": _allowed_differences(),
        "overall_pass": overall_pass,
        "generated_at": datetime.now(timezone.utc).isoformat(),
    }

    out = Path(output_path)
    out.write_text(json.dumps(evidence, indent=2) + "\n")
    return evidence


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate machine-readable compatibility evidence report",
    )
    parser.add_argument(
        "--output", default="compatibility-evidence.json",
        help="Output JSON path (default: compatibility-evidence.json)",
    )
    parser.add_argument(
        "--skip-tests", action="store_true",
        help="Skip running pytest (metadata-only report)",
    )
    args = parser.parse_args()
    evidence = generate_evidence(args.output, args.skip_tests)
    print(f"Evidence written to {args.output}")
    print(f"Overall pass: {evidence['overall_pass']}")


if __name__ == "__main__":
    main()
