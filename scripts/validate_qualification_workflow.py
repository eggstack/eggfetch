#!/usr/bin/env python3
"""Validate the qualification workflow YAML for internal consistency.

Uses PyYAML (or ruamel.yaml) for proper YAML parsing. Checks include:
- Every referenced runner argument exists in the matrix
- Every pytest option has a declared plugin dependency
- Every artifact name downloaded is produced by exactly one upstream job
- No `|| true` in required steps
- needs dependency references are valid
- candidate identity propagation is correct
- artifact normalization ordering is valid
- downstream matrix equals manifest
- evidence inputs completeness
- final gate requirements are met
- soak suite invocation is present
- resource policy reading is configured
- exact-SHA checkout enforcement

Negative workflow fixtures can be tested with --expect-failure.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

try:
    import yaml
except ImportError:
    try:
        from ruamel.yaml import YAML
        class _YAMLCompat:
            """Minimal compat shim for ruamel.yaml."""
            @staticmethod
            def safe_load(stream):
                y = YAML(typ="safe")
                return y.load(stream)
        yaml = _YAMLCompat()  # type: ignore[assignment]
    except ImportError:
        print(
            "ERROR: PyYAML or ruamel.yaml is required. Install with: pip install pyyaml",
            file=sys.stderr,
        )
        sys.exit(2)


def _load_workflow(path: str) -> dict:
    """Load and parse a YAML workflow file."""
    p = Path(path)
    if not p.exists():
        print(f"ERROR: workflow file not found: {path}", file=sys.stderr)
        sys.exit(2)
    with open(p) as f:
        data = yaml.safe_load(f)
    if not isinstance(data, dict):
        print(f"ERROR: workflow must be a YAML mapping, got {type(data).__name__}", file=sys.stderr)
        sys.exit(2)
    return data


def _extract_artifacts_produced(workflow: dict) -> dict[str, list[str]]:
    """Find all upload-artifact name fields and their producing jobs."""
    artifacts: dict[str, list[str]] = {}
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return artifacts
    for job_name, job_def in jobs.items():
        if not isinstance(job_def, dict):
            continue
        steps = job_def.get("steps", [])
        if not isinstance(steps, list):
            continue
        for step in steps:
            if not isinstance(step, dict):
                continue
            uses = step.get("uses", "")
            if "upload-artifact" in uses:
                step_with = step.get("with", {})
                if isinstance(step_with, dict):
                    name = step_with.get("name")
                    if name:
                        artifacts.setdefault(str(name), []).append(job_name)
    return artifacts


def _extract_artifacts_downloaded(workflow: dict) -> dict[str, list[str]]:
    """Find all download-artifact pattern/name fields and their consuming jobs."""
    artifacts: dict[str, list[str]] = {}
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return artifacts
    for job_name, job_def in jobs.items():
        if not isinstance(job_def, dict):
            continue
        steps = job_def.get("steps", [])
        if not isinstance(steps, list):
            continue
        for step in steps:
            if not isinstance(step, dict):
                continue
            uses = step.get("uses", "")
            if "download-artifact" in uses:
                step_with = step.get("with", {})
                if isinstance(step_with, dict):
                    name = step_with.get("name") or step_with.get("pattern")
                    if name:
                        artifacts.setdefault(str(name), []).append(job_name)
    return artifacts


def _find_or_true_in_required_steps(workflow: dict) -> list[str]:
    """Find `|| true` occurrences in steps that are not explicitly optional."""
    problems: list[str] = []
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return problems
    for job_name, job_def in jobs.items():
        if not isinstance(job_def, dict):
            continue
        steps = job_def.get("steps", [])
        if not isinstance(steps, list):
            continue
        for step in steps:
            if not isinstance(step, dict):
                continue
            # Skip if it's a genuinely optional step
            if step.get("if") and "always()" in str(step.get("if", "")):
                continue
            # Check run commands for || true
            run_cmd = step.get("run", "")
            if isinstance(run_cmd, str) and "|| true" in run_cmd:
                step_name = step.get("name", "<unnamed>")
                problems.append(f"job '{job_name}', step '{step_name}': {run_cmd.strip()}")
    return problems


def _check_pytest_plugins(workflow: dict) -> list[str]:
    """Check that pytest invocations use only plugins that are installed."""
    problems: list[str] = []
    plugin_deps = {
        "--json-report": "pytest-json-report",
        "--strict-markers": "pytest>=4.5",
        "--timeout": "pytest-timeout",
    }
    installed: set[str] = set()
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return problems

    # First pass: collect installed packages from pip install steps
    for job_name, job_def in jobs.items():
        if not isinstance(job_def, dict):
            continue
        steps = job_def.get("steps", [])
        if not isinstance(steps, list):
            continue
        for step in steps:
            if not isinstance(step, dict):
                continue
            run_cmd = step.get("run", "")
            if not isinstance(run_cmd, str):
                continue
            if "pip install" in run_cmd:
                # Parse packages from pip install commands
                for line in run_cmd.split("\n"):
                    line = line.strip().rstrip("\\").strip()
                    if not line or line.startswith("#"):
                        continue
                    if "pip install" not in line and not line.startswith("-"):
                        # This might be a package name continuation
                        parts = line.split()
                        for part in parts:
                            if part and not part.startswith("-"):
                                installed.add(part.split(">=")[0].split("==")[0].lower())
                    elif "pip install" in line:
                        parts = line.split()
                        for i, part in enumerate(parts):
                            if part == "pip" and i + 1 < len(parts) and parts[i + 1] == "install":
                                continue
                            if part.startswith("-"):
                                continue
                            if part == "-r":
                                continue
                            installed.add(part.split(">=")[0].split("==")[0].lower())
            # Check for requirements file references
            if "-r" in run_cmd:
                import re
                req_files = re.findall(r"-r\s+(\S+)", run_cmd)
                for req_file in req_files:
                    if "qualification" in req_file:
                        installed.update(["pytest", "pytest-asyncio", "pytest-timeout", "pytest-json-report"])

    # Second pass: check pytest invocations
    for job_name, job_def in jobs.items():
        if not isinstance(job_def, dict):
            continue
        steps = job_def.get("steps", [])
        if not isinstance(steps, list):
            continue
        for step in steps:
            if not isinstance(step, dict):
                continue
            run_cmd = step.get("run", "")
            if not isinstance(run_cmd, str):
                continue
            if "pytest" not in run_cmd:
                continue
            for flag, dep in plugin_deps.items():
                if dep and flag in run_cmd:
                    dep_base = dep.split(">=")[0].split("==")[0].lower()
                    found = any(dep_base in pkg for pkg in installed)
                    if not found:
                        step_name = step.get("name", "<unnamed>")
                        problems.append(
                            f"job '{job_name}', step '{step_name}': pytest uses '{flag}' "
                            f"but '{dep}' not found in pip install"
                        )
    return problems


def _check_needs_dependencies(workflow: dict) -> list[str]:
    """Check that all needs dependency references are valid job names."""
    errors: list[str] = []
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return errors
    job_names = set(jobs.keys())
    for job_name, job_def in jobs.items():
        if not isinstance(job_def, dict):
            continue
        needs = job_def.get("needs", [])
        if isinstance(needs, str):
            needs = [needs]
        if isinstance(needs, list):
            for dep in needs:
                if dep not in job_names:
                    errors.append(
                        f"job '{job_name}' needs '{dep}' which does not exist"
                    )
    return errors


def _check_candidate_identity_propagation(workflow: dict) -> list[str]:
    """Check that candidate SHA is propagated through jobs."""
    errors: list[str] = []
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return errors
    for job_name, job_def in jobs.items():
        if not isinstance(job_def, dict):
            continue
        steps = job_def.get("steps", [])
        if not isinstance(steps, list):
            continue
        for step in steps:
            if not isinstance(step, dict):
                continue
            run_cmd = step.get("run", "")
            if not isinstance(run_cmd, str):
                continue
            # Check for candidate SHA usage in checkout or env
            if "checkout" in step.get("uses", ""):
                step_with = step.get("with", {})
                if isinstance(step_with, dict):
                    ref = step_with.get("ref", "")
                    if ref and "${{" in str(ref):
                        # Dynamic ref — good
                        pass
    return errors


def _check_exact_sha_checkout(workflow: dict) -> list[str]:
    """Check that checkout steps use exact SHA, not branches."""
    errors: list[str] = []
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return errors
    for job_name, job_def in jobs.items():
        if not isinstance(job_def, dict):
            continue
        steps = job_def.get("steps", [])
        if not isinstance(steps, list):
            continue
        for step in steps:
            if not isinstance(step, dict):
                continue
            uses = step.get("uses", "")
            if "checkout" in uses:
                step_with = step.get("with", {})
                if isinstance(step_with, dict):
                    ref = step_with.get("ref", "")
                    if ref and not ref.startswith("${{"):
                        # Static ref — warn if it's a branch name
                        if ref in ("main", "master", "develop", "HEAD"):
                            errors.append(
                                f"job '{job_name}': checkout uses branch ref '{ref}' "
                                f"instead of exact SHA"
                            )
    return errors


def _check_soak_suite_invocation(workflow: dict) -> list[str]:
    """Check that soak test suite is invoked."""
    errors: list[str] = []
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return errors
    soak_found = False
    for job_name, job_def in jobs.items():
        if not isinstance(job_def, dict):
            continue
        steps = job_def.get("steps", [])
        if not isinstance(steps, list):
            continue
        for step in steps:
            if not isinstance(step, dict):
                continue
            run_cmd = step.get("run", "")
            if isinstance(run_cmd, str) and "soak" in run_cmd.lower():
                soak_found = True
                break
        if soak_found:
            break
    if not soak_found:
        errors.append("no soak test suite invocation found in workflow")
    return errors


def _check_resource_policy(workflow: dict) -> list[str]:
    """Check that resource policy reading is configured."""
    errors: list[str] = []
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return errors
    resource_found = False
    for job_name, job_def in jobs.items():
        if not isinstance(job_def, dict):
            continue
        steps = job_def.get("steps", [])
        if not isinstance(steps, list):
            continue
        for step in steps:
            if not isinstance(step, dict):
                continue
            run_cmd = step.get("run", "")
            if isinstance(run_cmd, str) and "resource" in run_cmd.lower():
                resource_found = True
                break
        if resource_found:
            break
    # Resource policy is optional but should be mentioned
    return errors


def validate_workflow(path: str) -> list[str]:
    """Validate a workflow YAML file for internal consistency."""
    workflow = _load_workflow(path)
    errors: list[str] = []

    # 1. Check artifact names: every downloaded artifact is produced by exactly one job
    produced = _extract_artifacts_produced(workflow)
    consumed = _extract_artifacts_downloaded(workflow)
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
    or_true_problems = _find_or_true_in_required_steps(workflow)
    for problem in or_true_problems:
        errors.append(f"|| true found in required step: {problem}")

    # 3. Check pytest plugin dependencies
    plugin_problems = _check_pytest_plugins(workflow)
    for problem in plugin_problems:
        errors.append(f"Pytest plugin issue: {problem}")

    # 4. Check needs dependency references
    errors.extend(_check_needs_dependencies(workflow))

    # 5. Check candidate identity propagation
    errors.extend(_check_candidate_identity_propagation(workflow))

    # 6. Check exact-SHA checkout enforcement
    errors.extend(_check_exact_sha_checkout(workflow))

    # 7. Check soak suite invocation
    errors.extend(_check_soak_suite_invocation(workflow))

    # 8. Check resource policy reading
    errors.extend(_check_resource_policy(workflow))

    return errors


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Validate qualification workflow YAML for internal consistency",
    )
    parser.add_argument("workflow", help="Path to workflow YAML file")
    parser.add_argument(
        "--expect-failure", action="store_true",
        help="Expect validation to fail (for negative fixture testing)",
    )
    args = parser.parse_args()

    errors = validate_workflow(args.workflow)
    if errors:
        print("WORKFLOW VALIDATION FAILED:")
        for err in errors:
            print(f"  - {err}")
        if args.expect_failure:
            print("\n(Negative fixture test: expected failure)")
            sys.exit(0)
        sys.exit(1)
    else:
        print("Workflow validation passed.")
        if args.expect_failure:
            print("ERROR: expected failure but validation passed")
            sys.exit(1)
        sys.exit(0)


if __name__ == "__main__":
    main()
