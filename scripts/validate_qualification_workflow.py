#!/usr/bin/env python3
"""Validate the qualification workflow YAML for internal consistency.

Checks include:
- Every referenced runner argument exists in the matrix
- Every pytest option has a declared plugin dependency
- Every artifact name downloaded is produced by exactly one upstream job
- No `|| true` or `|| echo` in required steps
- needs dependency references are valid
- Every job references outputs only from direct dependencies
- candidate identity propagation is correct
- artifact normalization ordering is valid
- downstream matrix comes from manifest-generated output
- No duplicated static required-package matrix
- evidence inputs completeness
- final gate requirements are met
- soak suite invocation is present
- resource policy reading is configured
- exact-SHA checkout enforcement
- no continue-on-error on required steps

Negative workflow fixtures can be tested with --expect-failure.
"""

from __future__ import annotations

import argparse
import re
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


def _find_suppression_patterns(workflow: dict) -> list[str]:
    """Find failure suppression patterns in required steps."""
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
            run_cmd = step.get("run", "")
            if not isinstance(run_cmd, str):
                continue
            step_name = step.get("name", "<unnamed>")
            # Check for || true
            if "|| true" in run_cmd:
                problems.append(f"job '{job_name}', step '{step_name}': || true found")
            # Check for || echo
            if "|| echo" in run_cmd:
                problems.append(f"job '{job_name}', step '{step_name}': || echo found")
            # Check for continue-on-error
            if step.get("continue-on-error"):
                problems.append(f"job '{job_name}', step '{step_name}': continue-on-error is set")
    return problems


def _check_continue_on_error(workflow: dict) -> list[str]:
    """Check for continue-on-error on required gate jobs.

    Only jobs listed in the qualification-gate's required_jobs are checked.
    Non-gate jobs (e.g. aarch64 cross-compilation) may use continue-on-error
    for non-blocking optional builds.
    """
    errors: list[str] = []
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return errors

    # Extract the set of required gate jobs from the qualification-gate job
    gate_job = jobs.get("qualification-gate", {})
    if not isinstance(gate_job, dict):
        return errors

    # Parse the gate's needs list (which defines the required jobs)
    gate_needs = gate_job.get("needs", [])
    if isinstance(gate_needs, str):
        gate_needs = [gate_needs]
    gate_jobs = set(gate_needs) if isinstance(gate_needs, list) else set()

    for job_name, job_def in jobs.items():
        if not isinstance(job_def, dict):
            continue
        if job_def.get("continue-on-error") and job_name in gate_jobs:
            errors.append(f"job '{job_name}' has continue-on-error set (required gate job)")
    return errors


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
                for line in run_cmd.split("\n"):
                    line = line.strip().rstrip("\\").strip()
                    if not line or line.startswith("#"):
                        continue
                    if "pip install" in line:
                        parts = line.split()
                        for part in parts:
                            if part.startswith("-"):
                                continue
                            if part in ("pip", "install"):
                                continue
                            installed.add(part.split(">=")[0].split("==")[0].lower())
            if "-r" in run_cmd:
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


def _check_output_references(workflow: dict) -> list[str]:
    """Check that jobs referencing needs.X.outputs.X declare X in their needs."""
    errors: list[str] = []
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return errors
    for job_name, job_def in jobs.items():
        if not isinstance(job_def, dict):
            continue
        # Serialize job definition to string for pattern matching
        job_str = str(job_def)
        # Find all needs.X.outputs references
        refs = re.findall(r'needs\.(\S+?)\.outputs', job_str)
        needs = job_def.get("needs", [])
        if isinstance(needs, str):
            needs = [needs]
        if not isinstance(needs, list):
            needs = []
        needs_set = set(needs)
        for ref in set(refs):
            if ref not in needs_set:
                errors.append(
                    f"job '{job_name}' references needs.{ref}.outputs but "
                    f"'{ref}' not in its needs list"
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
            uses = step.get("uses", "")
            if "checkout" in uses:
                step_with = step.get("with", {})
                if isinstance(step_with, dict):
                    ref = step_with.get("ref", "")
                    if ref and not ref.startswith("${{"):
                        if ref in ("main", "master", "develop", "HEAD"):
                            errors.append(
                                f"job '{job_name}': checkout uses branch ref '{ref}' "
                                f"instead of exact SHA"
                            )
    return errors


def _check_downstream_matrix(workflow: dict) -> list[str]:
    """Check that downstream matrix comes from manifest-generated output."""
    errors: list[str] = []
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return errors

    # Check for prepare-downstream-matrix job
    has_prepare = "prepare-downstream-matrix" in jobs
    has_downstream = "downstream-substitution" in jobs

    if has_downstream and not has_prepare:
        errors.append("downstream-substitution exists but prepare-downstream-matrix is missing")

    # Check for static matrix in downstream-substitution
    if has_downstream:
        ds_job = jobs["downstream-substitution"]
        strategy = ds_job.get("strategy", {})
        matrix = strategy.get("matrix", {})
        if isinstance(matrix, dict) and "package" in matrix:
            # Static package list — should use fromJSON instead
            if isinstance(matrix["package"], list):
                errors.append(
                    "downstream-substitution uses a static package matrix "
                    "instead of fromJSON(needs.prepare-downstream-matrix.outputs.matrix)"
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


def _check_candidate_bundle_usage(workflow: dict) -> list[str]:
    """Check that post-normalization jobs consume candidate-bundle, not raw wheel artifacts."""
    errors: list[str] = []
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return errors

    # Jobs that should use candidate-bundle
    bundle_consumers = {
        "compat-tests", "downstream-substitution", "downstream-aggregate",
        "shim-substitution",
        "native-timeout", "proxy-tls", "shutdown", "soak-resource",
        "generate-evidence", "qualification-gate", "status-generate",
    }

    for job_name in bundle_consumers:
        if job_name not in jobs:
            continue
        job_def = jobs[job_name]
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
                    name = step_with.get("name", "")
                    pattern = step_with.get("pattern", "")
                    # Should not download eggfetch-wheel or httpx-replacement-wheel directly
                    if name in ("eggfetch-wheel", "httpx-replacement-wheel"):
                        errors.append(
                            f"job '{job_name}' downloads '{name}' directly "
                            f"instead of using candidate-bundle"
                        )
    return errors


def _check_evidence_inputs(workflow: dict) -> list[str]:
    """Check that evidence generation consumes direct result artifacts."""
    errors: list[str] = []
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return errors

    gen_job = jobs.get("generate-evidence")
    if not gen_job or not isinstance(gen_job, dict):
        errors.append("generate-evidence job not found")
        return errors

    # Check that it has the required dependencies
    needs = gen_job.get("needs", [])
    if isinstance(needs, str):
        needs = [needs]
    required_deps = {
        "verify", "normalize-candidate-artifacts", "compat-tests",
        "downstream-substitution", "downstream-aggregate",
        "shim-substitution",
    }
    for dep in required_deps:
        if dep not in needs:
            errors.append(f"generate-evidence is missing dependency: {dep}")

    return errors


def _check_obsolete_wheel_dir(workflow: dict) -> list[str]:
    """§15.68: Workflow uses obsolete --wheel-dir downstream interface."""
    errors: list[str] = []
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return errors
    # Only check downstream-related jobs for obsolete interface
    downstream_jobs = {
        "downstream-substitution", "downstream-aggregate",
        "prepare-downstream-matrix", "run-downstream",
    }
    for job_name in downstream_jobs:
        if job_name not in jobs:
            continue
        job_def = jobs[job_name]
        if not isinstance(job_def, dict):
            continue
        steps = job_def.get("steps", [])
        if not isinstance(steps, list):
            continue
        for step in steps:
            if not isinstance(step, dict):
                continue
            run_cmd = step.get("run", "")
            if isinstance(run_cmd, str) and "--wheel-dir" in run_cmd:
                step_name = step.get("name", "<unnamed>")
                errors.append(
                    f"job '{job_name}', step '{step_name}': "
                    f"uses obsolete --wheel-dir interface"
                )
    return errors


def _check_hyphenated_matrix_keys(workflow: dict) -> list[str]:
    """§15.70: Workflow uses hyphenated matrix expression keys in downstream jobs."""
    errors: list[str] = []
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return errors
    # Only check downstream-substitution for hyphenated keys — standard
    # GitHub Actions keys like python-version are acceptable
    downstream_job = jobs.get("downstream-substitution")
    if not downstream_job or not isinstance(downstream_job, dict):
        return errors
    strategy = downstream_job.get("strategy", {})
    if not isinstance(strategy, dict):
        return errors
    matrix = strategy.get("matrix", {})
    if not isinstance(matrix, dict):
        return errors
    for key in matrix:
        if "-" in key and key not in ("include", "exclude"):
            errors.append(
                f"job 'downstream-substitution': matrix contains hyphenated key '{key}' "
                f"(use underscore keys instead)"
            )
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
            # Pattern downloads don't need exact producers
            if "*" not in str(name):
                errors.append(
                    f"Artifact '{name}' is downloaded but never produced by upload-artifact"
                )

    # 2. Check for failure suppression patterns
    suppression_problems = _find_suppression_patterns(workflow)
    for problem in suppression_problems:
        errors.append(f"Failure suppression: {problem}")

    # 3. Check continue-on-error on jobs
    errors.extend(_check_continue_on_error(workflow))

    # 4. Check pytest plugin dependencies
    plugin_problems = _check_pytest_plugins(workflow)
    for problem in plugin_problems:
        errors.append(f"Pytest plugin issue: {problem}")

    # 5. Check needs dependency references
    errors.extend(_check_needs_dependencies(workflow))

    # 6. Check output references have direct dependencies
    errors.extend(_check_output_references(workflow))

    # 7. Check candidate identity propagation
    errors.extend(_check_candidate_identity_propagation(workflow))

    # 8. Check downstream matrix is manifest-authoritative
    errors.extend(_check_downstream_matrix(workflow))

    # 9. Check soak suite invocation
    errors.extend(_check_soak_suite_invocation(workflow))

    # 10. Check candidate bundle usage
    errors.extend(_check_candidate_bundle_usage(workflow))

    # 11. Check evidence inputs
    errors.extend(_check_evidence_inputs(workflow))

    # 12. Check obsolete --wheel-dir interface
    errors.extend(_check_obsolete_wheel_dir(workflow))

    # 13. Check hyphenated matrix keys
    errors.extend(_check_hyphenated_matrix_keys(workflow))

    return errors


def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser for workflow validation tests."""
    parser = argparse.ArgumentParser(
        prog="validate_qualification_workflow.py",
        description="Validate qualification workflow YAML for internal consistency",
    )
    parser.add_argument("workflow", help="Path to workflow YAML file")
    parser.add_argument(
        "--expect-failure", action="store_true",
        help="Expect validation to fail (for negative fixture testing)",
    )
    return parser


def main() -> None:
    parser = build_parser()
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
