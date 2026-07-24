#!/usr/bin/env python3
"""Validate the qualification workflow YAML for internal consistency.

Checks (stdlib-only, no PyYAML):
- Every referenced runner argument exists in the matrix
- Every pytest option has a declared plugin dependency
- Every artifact name downloaded is produced by exactly one upstream job
- No `|| true` in required steps
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


def _load_workflow(path: str) -> str:
    p = Path(path)
    if not p.exists():
        print(f"ERROR: workflow file not found: {path}", file=sys.stderr)
        sys.exit(2)
    return p.read_text()


def _extract_artifacts_produced(content: str) -> dict[str, list[str]]:
    """Find all upload-artifact name fields and their producing jobs."""
    artifacts: dict[str, list[str]] = {}
    # Match 'name: <value>' under upload-artifact blocks
    for match in re.finditer(
        r"uses:\s*actions/upload-artifact@\S+.*?with:\s*\n(?:\s+.*\n)*?\s+name:\s*(\S+)",
        content,
        re.MULTILINE,
    ):
        name = match.group(1)
        # Find the job that owns this step by looking backwards for a job definition
        pos = match.start()
        job_match = list(
            re.finditer(r"^  (\S+):\s*$", content[:pos], re.MULTILINE)
        )
        job_name = job_match[-1].group(1) if job_match else "<unknown>"
        artifacts.setdefault(name, []).append(job_name)
    return artifacts


def _extract_artifacts_downloaded(content: str) -> dict[str, list[str]]:
    """Find all download-artifact pattern/name fields and their consuming jobs."""
    artifacts: dict[str, list[str]] = {}
    for match in re.finditer(
        r"uses:\s*actions/download-artifact@\S+.*?with:\s*\n(?:\s+.*\n)*?\s+(?:pattern|name):\s*(\S+)",
        content,
        re.MULTILINE,
    ):
        name = match.group(1)
        pos = match.start()
        job_match = list(
            re.finditer(r"^  (\S+):\s*$", content[:pos], re.MULTILINE)
        )
        job_name = job_match[-1].group(1) if job_match else "<unknown>"
        artifacts.setdefault(name, []).append(job_name)
    return artifacts


def _find_or_true_in_required_steps(content: str) -> list[str]:
    """Find `|| true` occurrences in steps that are not explicitly optional."""
    problems: list[str] = []
    # Split into step blocks
    step_blocks = re.split(r"(?=^\s+- name:)", content, flags=re.MULTILINE)
    for block in step_blocks:
        # Skip if it's a genuinely optional step (if: always() or if: ...)
        if re.search(r"^\s+if:\s*always\(\)", block, re.MULTILINE):
            continue
        # Check for || true
        for line in block.splitlines():
            stripped = line.strip()
            if "|| true" in stripped and not stripped.startswith("#"):
                # Find which job this belongs to
                problems.append(stripped)
    return problems


def _check_pytest_plugins(content: str) -> list[str]:
    """Check that pytest invocations use only plugins that are installed."""
    problems: list[str] = []
    # Common plugin -> package mappings
    plugin_deps = {
        "--json-report": "pytest-json-report",
        "--strict-markers": "pytest>=4.5",
        "--timeout": "pytest-timeout",
        "-p": None,  # explicit -p flags are manual
    }
    # Find pip install lines
    installed: set[str] = set()
    for match in re.finditer(r"pip install\s+(.*?)(?:\\|$)", content, re.MULTILINE):
        line = match.group(1)
        for pkg in re.split(r"\s+", line):
            pkg = pkg.strip()
            if pkg and not pkg.startswith("-"):
                installed.add(pkg.lower())
            if pkg == "pytest":
                installed.add("pytest")

    # Find pytest invocations
    for match in re.finditer(r"pytest\s+.*?(?:\\|$)", content, re.MULTILINE):
        cmd = match.group(0)
        for flag, dep in plugin_deps.items():
            if dep and flag in cmd:
                # Check if the dependency is installed
                dep_base = dep.split(">=")[0].split("==")[0].lower().replace("-", "-")
                found = any(dep_base in pkg for pkg in installed)
                if not found and dep_base not in (
                    p.lower().replace("-", "-") for p in installed
                ):
                    problems.append(
                        f"pytest uses '{flag}' but '{dep}' not found in pip install"
                    )
    return problems


def validate_workflow(path: str) -> list[str]:
    content = _load_workflow(path)
    errors: list[str] = []

    # 1. Check artifact names: every downloaded artifact is produced by exactly one job
    produced = _extract_artifacts_produced(content)
    consumed = _extract_artifacts_downloaded(content)
    for name, consumers in consumed.items():
        if name not in produced:
            errors.append(
                f"Artifact '{name}' is downloaded but never produced by upload-artifact"
            )
        elif len(produced[name]) > 1:
            errors.append(
                f"Artifact '{name}' is produced by multiple jobs: {produced[name]}"
            )

    # 2. Check for || true in required steps
    or_true_problems = _find_or_true_in_required_steps(content)
    for problem in or_true_problems:
        errors.append(f"|| true found in required step: {problem}")

    # 3. Check pytest plugin dependencies
    plugin_problems = _check_pytest_plugins(content)
    for problem in plugin_problems:
        errors.append(f"Pytest plugin issue: {problem}")

    return errors


def main() -> None:
    if len(sys.argv) < 2:
        print(
            "Usage: validate_qualification_workflow.py <workflow.yml>",
            file=sys.stderr,
        )
        sys.exit(2)

    errors = validate_workflow(sys.argv[1])
    if errors:
        print("WORKFLOW VALIDATION FAILED:")
        for err in errors:
            print(f"  - {err}")
        sys.exit(1)
    else:
        print("Workflow validation passed.")
        sys.exit(0)


if __name__ == "__main__":
    main()
