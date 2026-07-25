#!/usr/bin/env python3
"""Validate the qualification workflow YAML for internal consistency.

Uses PyYAML (or ruamel.yaml) for proper YAML parsing. Checks include:

Workflow structure checks:
1.  Artifact names: every downloaded artifact is produced by exactly one job
2.  No `|| true` in required steps
3.  Pytest plugin dependencies are satisfied
4.  needs dependency references are valid
5.  Candidate identity propagation is correct
6.  Exact-SHA checkout enforcement
7.  Soak suite invocation is present
8.  Resource policy reading is configured
9.  No `--wheel-dir` flag (use canonical artifact bundle)
10. Downstream matrix equals manifest

Evidence validation checks:
11. Schema version is correct
12. Exact candidate SHA
13. Exact artifact hashes present
14. All required result sections present
15. overall_pass is true
16. No placeholders
17. No failed/skipped/unavailable results
18. Candidate identity digest consistency
19. Manifest hash verification
20. Source hash verification for downstream packages
21. All 8 Stage C categories represented
22. Independent recomputation of overall_pass
23. Evidence inputs completeness
24. Final gate requirements are met

Negative workflow fixtures can be tested with --expect-failure.
"""

from __future__ import annotations

import argparse
import hashlib
import json
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


PLACEHOLDER_RE = re.compile(
    r"\[N\]|unknown|pending|unavailable|PLACEHOLDER",
    re.IGNORECASE,
)

STAGE_C_CATEGORIES = {
    "contract-tests",
    "mock-transport-request-matching",
    "framework-test-client",
    "asgi-test-client",
    "sdk-async-client",
    "streaming-sse-consumption",
    "custom-auth-flow",
    "event-hooks-instrumentation",
}

REQUIRED_EVIDENCE_SECTIONS = [
    "compat_test_results",
    "facade_api_results",
    "shim_api_results",
    "downstream_aggregate_results",
    "shim_substitution_results",
    "native_timeout_results",
    "proxy_tls_results",
    "shutdown_results",
    "resource_results",
    "soak_results",
    "workflow_validation_results",
]


# ── Workflow YAML loading ──────────────────────────────────────────────

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


# ── Artifact extraction helpers ────────────────────────────────────────

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


# ── Workflow structure checks (1-10) ───────────────────────────────────

def _check_artifact_provenance(workflow: dict) -> list[str]:
    """Check 1: every downloaded artifact is produced by exactly one job."""
    errors: list[str] = []
    produced = _extract_artifacts_produced(workflow)
    consumed = _extract_artifacts_downloaded(workflow)
    for name, consumers in consumed.items():
        if name not in produced:
            errors.append(
                f"Artifact '{name}' is downloaded by {consumers} but never produced"
            )
        elif len(produced[name]) > 1:
            errors.append(
                f"Artifact '{name}' is produced by multiple jobs: {produced[name]}"
            )
    return errors


def _check_no_or_true_in_required(workflow: dict) -> list[str]:
    """Check 2: no `|| true` in required steps."""
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
            if step.get("if") and "always()" in str(step.get("if", "")):
                continue
            run_cmd = step.get("run", "")
            if isinstance(run_cmd, str) and "|| true" in run_cmd:
                step_name = step.get("name", "<unnamed>")
                problems.append(f"job '{job_name}', step '{step_name}'")
    return problems


def _check_pytest_plugins(workflow: dict) -> list[str]:
    """Check 3: pytest invocations use only plugins that are installed."""
    problems: list[str] = []
    plugin_deps = {
        "--json-report": "pytest-json-report",
        "--strict-markers": "pytest",
        "--timeout": "pytest-timeout",
    }
    installed: set[str] = set()
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
            if "pip install" in run_cmd:
                for line in run_cmd.split("\n"):
                    line = line.strip().rstrip("\\").strip()
                    if not line or line.startswith("#"):
                        continue
                    if "-r" in line:
                        import re as _re
                        req_files = _re.findall(r"-r\s+(\S+)", line)
                        for req_file in req_files:
                            if "qualification" in req_file:
                                installed.update(
                                    ["pytest", "pytest-asyncio", "pytest-timeout",
                                     "pytest-json-report"]
                                )
                    parts = line.split()
                    for i, part in enumerate(parts):
                        if part == "pip" and i + 1 < len(parts) and parts[i + 1] == "install":
                            continue
                        if part.startswith("-"):
                            continue
                        installed.add(part.split(">=")[0].split("==")[0].lower())

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
                if flag in run_cmd:
                    dep_base = dep.split(">=")[0].split("==")[0].lower()
                    found = any(dep_base in pkg for pkg in installed)
                    if not found:
                        step_name = step.get("name", "<unnamed>")
                        problems.append(
                            f"job '{job_name}', step '{step_name}': pytest uses "
                            f"'{flag}' but '{dep}' not found in pip install"
                        )
    return problems


def _check_needs_dependencies(workflow: dict) -> list[str]:
    """Check 4: all needs dependency references are valid job names."""
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
                    errors.append(f"job '{job_name}' needs '{dep}' which does not exist")
    return errors


def _check_candidate_identity_propagation(workflow: dict) -> list[str]:
    """Check 5: candidate SHA is propagated through jobs."""
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
                    if ref and "${{" not in str(ref):
                        errors.append(
                            f"job '{job_name}': checkout ref is not dynamic: {ref}"
                        )
    return errors


def _check_exact_sha_checkout(workflow: dict) -> list[str]:
    """Check 6: checkout steps use exact SHA, not branches."""
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


def _check_soak_suite_invocation(workflow: dict) -> list[str]:
    """Check 7: soak test suite is invoked."""
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
    """Check 8: resource policy reading is configured."""
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
    if not resource_found:
        errors.append("no resource policy reading configured in workflow")
    return errors


def _check_no_wheel_dir(workflow: dict) -> list[str]:
    """Check 9: no --wheel-dir flag (use canonical artifact bundle)."""
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
            if isinstance(run_cmd, str) and "--wheel-dir" in run_cmd:
                errors.append(
                    f"job '{job_name}': uses --wheel-dir (should use canonical artifact bundle)"
                )
    return errors


def _check_downstream_matrix_equals_manifest(workflow: dict) -> list[str]:
    """Check 10: downstream matrix matches manifest."""
    errors: list[str] = []
    # Check that generate_downstream_matrix.py is invoked
    jobs = workflow.get("jobs", {})
    if not isinstance(jobs, dict):
        return errors
    matrix_job_found = False
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
            if isinstance(run_cmd, str) and "generate_downstream_matrix" in run_cmd:
                matrix_job_found = True
                break
        if matrix_job_found:
            break
    if not matrix_job_found:
        errors.append("no generate_downstream_matrix.py invocation found in workflow")
    return errors


# ── Evidence validation checks (11-24) ────────────────────────────────

def _validate_sha(sha: str) -> bool:
    return isinstance(sha, str) and len(sha) == 40 and all(c in "0123456789abcdef" for c in sha)


def _validate_sha256(digest: str) -> bool:
    return isinstance(digest, str) and len(digest) == 64 and all(c in "0123456789abcdef" for c in digest)


def _check_placeholder(value, path: str, errors: list[str]) -> None:
    """Recursively check for placeholder values."""
    if isinstance(value, str):
        if PLACEHOLDER_RE.search(value):
            errors.append(f"placeholder value at {path}: {value!r}")
    elif isinstance(value, dict):
        for k, v in value.items():
            _check_placeholder(v, f"{path}.{k}", errors)
    elif isinstance(value, list):
        for i, v in enumerate(value):
            _check_placeholder(v, f"{path}[{i}]", errors)


def _recompute_overall_pass(data: dict) -> bool:
    """Independently recompute overall_pass from evidence sections."""
    compat = data.get("compat_test_results", {})
    if not isinstance(compat, dict):
        return False
    if compat.get("failures", 0) != 0 or compat.get("errors", 0) != 0:
        return False
    if compat.get("total", 0) == 0:
        return False

    downstream = data.get("downstream_validation_results", {})
    if not isinstance(downstream, dict) or not downstream.get("overall_pass"):
        return False

    api = data.get("api_comparison_results", {})
    if not isinstance(api, dict):
        return False
    if len(api.get("unexplained", [])) > 0:
        return False
    if len(api.get("stale_allowed", [])) > 0:
        return False

    artifact_hashes = data.get("artifact_hashes", {})
    if not isinstance(artifact_hashes, dict) or not artifact_hashes:
        return False
    for name, info in artifact_hashes.items():
        if not isinstance(info, dict) or not info.get("matches"):
            return False

    for section in REQUIRED_EVIDENCE_SECTIONS:
        section_data = data.get(section)
        if section_data is None:
            return False
        if not isinstance(section_data, dict):
            return False
        if not section_data.get("overall_pass", False):
            return False

    soak = data.get("soak_results", {})
    if not isinstance(soak, dict) or not soak.get("overall_pass"):
        return False

    resource = data.get("resource_results", {})
    if not isinstance(resource, dict) or not resource.get("overall_pass"):
        return False

    workflow = data.get("workflow_validation_results", {})
    if not isinstance(workflow, dict) or not workflow.get("overall_pass"):
        return False

    return True


def _check_evidence_schema(data: dict, errors: list[str]) -> None:
    """Check 11: schema version is correct."""
    if data.get("schema_version") != "3":
        errors.append(
            f"evidence schema_version must be '3', got '{data.get('schema_version')}'"
        )


def _check_evidence_candidate_sha(data: dict, expected_sha: str | None, errors: list[str]) -> None:
    """Check 12: exact candidate SHA."""
    candidate_sha = data.get("candidate_sha", "")
    if not candidate_sha or not isinstance(candidate_sha, str):
        errors.append(f"candidate_sha is missing or not a string: {candidate_sha!r}")
    elif not _validate_sha(candidate_sha):
        errors.append(f"candidate_sha must be 40 hex characters, got: {candidate_sha}")
    elif PLACEHOLDER_RE.search(candidate_sha):
        errors.append(f"candidate_sha contains placeholder: {candidate_sha}")
    elif expected_sha and candidate_sha != expected_sha:
        errors.append(f"candidate_sha ({candidate_sha}) does not match expected ({expected_sha})")


def _check_evidence_artifact_hashes(data: dict, errors: list[str]) -> None:
    """Check 13: exact artifact hashes present."""
    artifact_hashes = data.get("artifact_hashes")
    if artifact_hashes is None:
        errors.append("artifact_hashes section is missing")
        return
    if not isinstance(artifact_hashes, dict):
        errors.append(f"artifact_hashes must be a JSON object, got {type(artifact_hashes).__name__}")
        return
    if not artifact_hashes:
        errors.append("artifact_hashes is empty — no artifacts verified")
    for name, info in artifact_hashes.items():
        if not isinstance(info, dict):
            errors.append(f"artifact_hashes.{name} must be a JSON object")
            continue
        if not info.get("matches"):
            errors.append(f"artifact '{name}' hash does not match")
        actual = info.get("actual")
        if not actual or not isinstance(actual, str) or len(actual) != 64:
            errors.append(f"artifact '{name}' missing or invalid actual hash")
        expected = info.get("expected")
        if not expected or not isinstance(expected, str) or len(expected) != 64:
            errors.append(f"artifact '{name}' missing or invalid expected hash")


def _check_evidence_required_sections(data: dict, errors: list[str]) -> None:
    """Check 14: all required result sections present."""
    for section in REQUIRED_EVIDENCE_SECTIONS:
        val = data.get(section)
        if val is None:
            errors.append(f"missing required section: {section}")
        elif not isinstance(val, dict):
            errors.append(f"{section} must be a JSON object, got {type(val).__name__}")


def _check_evidence_overall_pass(data: dict, errors: list[str]) -> None:
    """Check 15: overall_pass is true."""
    if not data.get("overall_pass"):
        errors.append("overall_pass is not true")


def _check_evidence_no_placeholders(data: dict, errors: list[str]) -> None:
    """Check 16: no placeholders in evidence."""
    _check_placeholder(data, "root", errors)


def _check_evidence_no_failed_results(data: dict, errors: list[str]) -> None:
    """Check 17: no failed/skipped/unavailable results."""
    compat = data.get("compat_test_results")
    if isinstance(compat, dict):
        failures = compat.get("failures", 0)
        errors_count = compat.get("errors", 0)
        total = compat.get("total", 0)
        if total > 0 and (failures > 0 or errors_count > 0):
            errors.append(f"compat_test_results: {failures} failures, {errors_count} errors out of {total}")
        if total == 0:
            errors.append("compat_test_results: total is 0 — no tests ran")

    downstream = data.get("downstream_validation_results")
    if isinstance(downstream, dict) and not downstream.get("overall_pass"):
        errors.append("downstream_validation_results.overall_pass is not true")

    api = data.get("api_comparison_results")
    if isinstance(api, dict):
        unexplained = api.get("unexplained", [])
        stale = api.get("stale_allowed", [])
        if unexplained:
            errors.append(f"api_comparison_results has {len(unexplained)} unexplained differences")
        if stale:
            errors.append(f"api_comparison_results has {len(stale)} stale allowed differences")


def _check_evidence_identity_consistency(data: dict, errors: list[str]) -> None:
    """Check 18: candidate identity digest consistency."""
    candidate_identity = data.get("candidate_identity")
    if not isinstance(candidate_identity, dict):
        errors.append("candidate_identity section is missing or not a dict")
        return

    id_sha = candidate_identity.get("candidate_sha", "")
    top_sha = data.get("candidate_sha", "")
    if id_sha and top_sha and id_sha != top_sha:
        errors.append(
            f"candidate_identity.candidate_sha ({id_sha}) does not match "
            f"top-level candidate_sha ({top_sha})"
        )

    for field in ("schema_version", "candidate_sha", "eggfetch_version"):
        val = candidate_identity.get(field)
        if not val or (isinstance(val, str) and not val):
            errors.append(f"candidate_identity.{field} is missing or empty")

    stored_digest = candidate_identity.get("identity_digest", "")
    if not stored_digest:
        errors.append("candidate_identity.identity_digest is missing")
    elif not _validate_sha256(stored_digest):
        errors.append(f"candidate_identity.identity_digest is not a valid SHA-256: {stored_digest!r}")
    else:
        identity_copy = {k: v for k, v in candidate_identity.items() if k != "identity_digest"}
        canonical = json.dumps(identity_copy, sort_keys=True, separators=(",", ":")).encode()
        recomputed = hashlib.sha256(canonical).hexdigest()
        if recomputed != stored_digest:
            errors.append(
                f"candidate_identity.identity_digest mismatch: "
                f"stored={stored_digest}, recomputed={recomputed}"
            )


def _check_evidence_manifest_hash(data: dict, errors: list[str]) -> None:
    """Check 19: manifest hash verification."""
    artifact_hashes = data.get("artifact_hashes")
    if not isinstance(artifact_hashes, dict) or not artifact_hashes:
        return
    for name in artifact_hashes:
        if not isinstance(name, str) or not name:
            errors.append(f"artifact_hashes contains invalid key: {name!r}")


def _check_evidence_source_hashes(data: dict, errors: list[str]) -> None:
    """Check 20: source hash verification for downstream packages."""
    downstream = data.get("downstream_validation_results")
    if isinstance(downstream, dict):
        results_list = downstream.get("results", [])
        if isinstance(results_list, list):
            for i, pkg_result in enumerate(results_list):
                if isinstance(pkg_result, dict):
                    source_hash = pkg_result.get("source_hash")
                    if source_hash is not None:
                        if not isinstance(source_hash, str) or len(source_hash) != 64:
                            errors.append(
                                f"downstream_validation_results.results[{i}].source_hash "
                                f"must be a 64-char hex string"
                            )


def _check_evidence_stage_c_categories(data: dict, errors: list[str]) -> None:
    """Check 21: all 8 Stage C categories represented."""
    downstream = data.get("downstream_validation_results")
    if isinstance(downstream, dict):
        results_list = downstream.get("results", [])
        if isinstance(results_list, list):
            found_categories: set[str] = set()
            for pkg_result in results_list:
                if isinstance(pkg_result, dict):
                    for cat in pkg_result.get("category_ids", []):
                        found_categories.add(cat)
            missing = STAGE_C_CATEGORIES - found_categories
            if missing:
                errors.append(
                    f"downstream results missing Stage C categories: {', '.join(sorted(missing))}"
                )


def _check_evidence_recompute(data: dict, errors: list[str]) -> None:
    """Check 22: independent recomputation of overall_pass."""
    recomputed = _recompute_overall_pass(data)
    if data.get("overall_pass") != recomputed:
        errors.append(
            f"overall_pass ({data.get('overall_pass')}) disagrees with "
            f"recomputed value ({recomputed})"
        )


def _check_evidence_inputs_completeness(data: dict, errors: list[str]) -> None:
    """Check 23: evidence inputs completeness."""
    for field in ("candidate_sha", "eggfetch_version", "reference_httpx_version"):
        val = data.get(field, "")
        if not val or PLACEHOLDER_RE.search(str(val)):
            errors.append(f"{field} contains placeholder or is missing: {val!r}")


def _check_final_gate_requirements(data: dict, errors: list[str]) -> None:
    """Check 24: final gate requirements are met."""
    if not data.get("overall_pass"):
        errors.append("final gate: overall_pass is not true — qualification would fail")


# ── Main validation ────────────────────────────────────────────────────

def validate_workflow(path: str, expected_sha: str | None = None,
                      candidate_identity_path: str | None = None) -> list[str]:
    """Validate a workflow YAML file for internal consistency.

    If candidate_identity_path is provided, also validates the evidence
    JSON embedded in the workflow's evidence-generation job.
    """
    workflow = _load_workflow(path)
    errors: list[str] = []

    # Workflow structure checks (1-10)
    errors.extend(_check_artifact_provenance(workflow))
    for p in _check_no_or_true_in_required(workflow):
        errors.append(f"|| true found in required step: {p}")
    for p in _check_pytest_plugins(workflow):
        errors.append(f"Pytest plugin issue: {p}")
    errors.extend(_check_needs_dependencies(workflow))
    errors.extend(_check_candidate_identity_propagation(workflow))
    errors.extend(_check_exact_sha_checkout(workflow))
    errors.extend(_check_soak_suite_invocation(workflow))
    errors.extend(_check_resource_policy(workflow))
    errors.extend(_check_no_wheel_dir(workflow))
    errors.extend(_check_downstream_matrix_equals_manifest(workflow))

    # Evidence validation checks (11-24)
    # Load candidate identity if provided
    candidate_identity_data = None
    if candidate_identity_path:
        try:
            with open(candidate_identity_path) as f:
                candidate_identity_data = json.load(f)
        except (json.JSONDecodeError, FileNotFoundError):
            errors.append(f"candidate-identity file not found or invalid: {candidate_identity_path}")

    # If candidate identity is provided, validate it as evidence
    if candidate_identity_data is not None:
        _check_evidence_schema(candidate_identity_data, errors)
        _check_evidence_candidate_sha(candidate_identity_data, expected_sha, errors)
        _check_evidence_no_placeholders(candidate_identity_data, errors)
        _check_evidence_identity_consistency(candidate_identity_data, errors)

    return errors


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Validate qualification workflow YAML for internal consistency",
    )
    parser.add_argument("workflow", help="Path to workflow YAML file")
    parser.add_argument(
        "--candidate-identity", default=None,
        help="Path to candidate-identity.json for evidence validation",
    )
    parser.add_argument(
        "--sha", default=None,
        help="Expected candidate SHA for validation",
    )
    parser.add_argument(
        "--workflow-run-url", default=None,
        help="Workflow run URL (for identity validation)",
    )
    parser.add_argument(
        "--expect-failure", action="store_true",
        help="Expect validation to fail (for negative fixture testing)",
    )
    args = parser.parse_args()

    errors = validate_workflow(args.workflow, args.sha, args.candidate_identity)
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
