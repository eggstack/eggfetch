#!/usr/bin/env python3
"""Generate human-readable Markdown compatibility report from evidence JSON.

Reads evidence produced by generate_compatibility_evidence.py (schema v2)
and renders a Markdown report.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def _load_evidence(path: str) -> dict[str, Any]:
    with open(path) as f:
        return json.load(f)


def _generate_markdown(evidence: dict[str, Any]) -> str:
    lines: list[str] = []
    overall = evidence.get("overall_pass", False)
    stage = evidence.get("compatibility_stage", "unknown")
    ev_version = evidence.get("eggfetch_version", "unknown")
    ref_version = evidence.get("reference_httpx_version", "unknown")
    commit = evidence.get("eggfetch_commit", "unknown")
    candidate_sha = evidence.get("candidate_sha", "unknown")
    generated = evidence.get("generated_at", "unknown")
    schema = evidence.get("schema_version", "unknown")

    lines.append("# Compatibility Evidence Report")
    lines.append("")

    lines.append("## Summary")
    lines.append("")
    lines.append(f"| Field | Value |")
    lines.append(f"|-------|-------|")
    lines.append(f"| Status | **{'PASS' if overall else 'FAIL'}** |")
    lines.append(f"| Schema Version | {schema} |")
    lines.append(f"| Compatibility Stage | {stage} |")
    lines.append(f"| eggfetch Version | {ev_version} |")
    lines.append(f"| Reference HTTPX Version | {ref_version} |")
    lines.append(f"| Candidate SHA | `{candidate_sha[:12]}` |")
    lines.append(f"| eggfetch Commit | `{commit[:12]}` |")
    lines.append(f"| Generated At | {generated} |")
    lines.append("")

    compat = evidence.get("compat_test_results", {})
    lines.append("## Compat Test Results")
    lines.append("")
    if compat:
        total = compat.get("total", 0)
        passed = compat.get("passed", 0)
        failures = compat.get("failures", 0)
        errors = compat.get("errors", 0)
        pass_rate = f"{(passed / total * 100):.1f}" if total > 0 else "N/A"
        lines.append(f"| Metric | Value |")
        lines.append(f"|--------|-------|")
        lines.append(f"| Total | {total} |")
        lines.append(f"| Passed | {passed} |")
        lines.append(f"| Failed | {failures} |")
        lines.append(f"| Errors | {errors} |")
        lines.append(f"| Pass Rate | {pass_rate}% |")
    else:
        lines.append("No compat test results available.")
    lines.append("")

    downstream = evidence.get("downstream_validation_results", {})
    lines.append("## Downstream Validation")
    lines.append("")
    if downstream:
        ds_pass = downstream.get("overall_pass", False)
        results = downstream.get("results", [])
        passed_list = [r for r in results if r.get("status") == "passed"]
        failed_list = [r for r in results if r.get("status") == "failed"]
        skipped_list = [r for r in results if r.get("status") in ("skipped", "skipped-no-tests")]
        error_list = [r for r in results if r.get("status") in ("error", "timeout", "install-failed")]

        lines.append(f"| Metric | Value |")
        lines.append(f"|--------|-------|")
        lines.append(f"| Overall Pass | {'Yes' if ds_pass else 'No'} |")
        lines.append(f"| Total Packages | {len(results)} |")
        lines.append(f"| Passed | {len(passed_list)} |")
        lines.append(f"| Failed | {len(failed_list)} |")
        lines.append(f"| Skipped | {len(skipped_list)} |")
        lines.append(f"| Errors | {len(error_list)} |")
        lines.append("")

        if failed_list or error_list:
            lines.append("### Failed Packages")
            lines.append("")
            for pkg in failed_list + error_list:
                name = pkg.get("package", "unknown")
                status = pkg.get("status", "unknown")
                error = pkg.get("error", "")
                lines.append(f"- **{name}** ({status}): {error}")
            lines.append("")
    else:
        lines.append("No downstream validation results available.")
    lines.append("")

    api = evidence.get("api_comparison_results", {})
    lines.append("## API Comparison")
    lines.append("")
    if api:
        missing = api.get("missing_symbols", [])
        extra = api.get("extra_symbols", [])
        kind_mismatches = api.get("kind_mismatches", [])
        sig_mismatches = api.get("signature_mismatches", [])
        unexplained = api.get("unexplained", [])
        stale = api.get("stale_allowed", [])
        allowed_matches = api.get("allowed_matches", [])

        lines.append(f"| Metric | Count |")
        lines.append(f"|--------|-------|")
        lines.append(f"| Missing Symbols | {len(missing)} |")
        lines.append(f"| Extra Symbols | {len(extra)} |")
        lines.append(f"| Kind Mismatches | {len(kind_mismatches)} |")
        lines.append(f"| Signature Mismatches | {len(sig_mismatches)} |")
        lines.append(f"| Allowed Matches | {len(allowed_matches)} |")
        lines.append(f"| Stale Allowed | {len(stale)} |")
        lines.append(f"| Unexplained | {len(unexplained)} |")
        lines.append("")

        if missing:
            lines.append("### Missing Symbols")
            for s in missing:
                lines.append(f"- `{s}`")
            lines.append("")

        if unexplained:
            lines.append("### Unexplained Differences")
            for u in unexplained:
                if isinstance(u, dict):
                    lines.append(f"- `{u.get('symbol', u)}`")
                else:
                    lines.append(f"- `{u}`")
            lines.append("")
    else:
        lines.append("No API comparison results available.")
    lines.append("")

    artifact_data = evidence.get("artifact_hashes", {})
    lines.append("## Artifact Hash Verification")
    lines.append("")
    if artifact_data:
        all_match = all(v.get("matches", False) for v in artifact_data.values())
        lines.append(f"| Status | {'All hashes match' if all_match else 'MISMATCH DETECTED'} |")
        lines.append("")
        lines.append(f"| Artifact | Expected | Actual | Match |")
        lines.append(f"|----------|----------|--------|-------|")
        for name, info in artifact_data.items():
            expected = info.get("expected", "?")[:16]
            actual = info.get("actual", "?")
            if actual:
                actual = actual[:16]
            match = "Yes" if info.get("matches") else "No"
            lines.append(f"| {name} | `{expected}...` | `{actual}...` | {match} |")
        lines.append("")

    platform_info = evidence.get("platform_python_backend", {})
    lines.append("## Platform Details")
    lines.append("")
    lines.append(f"| Field | Value |")
    lines.append(f"|-------|-------|")
    for key, val in platform_info.items():
        label = key.replace("_", " ").title()
        lines.append(f"| {label} | {val} |")
    lines.append("")

    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate human-readable Markdown compatibility report from evidence JSON",
    )
    parser.add_argument(
        "--input", default="compatibility-evidence.json",
        help="Input evidence JSON path (default: compatibility-evidence.json)",
    )
    parser.add_argument(
        "--output", default="compatibility-report.md",
        help="Output Markdown path (default: compatibility-report.md)",
    )
    args = parser.parse_args()

    evidence_path = Path(args.input)
    if not evidence_path.exists():
        print(f"Error: evidence file not found: {args.input}", file=sys.stderr)
        sys.exit(1)

    evidence = _load_evidence(args.input)
    markdown = _generate_markdown(evidence)

    out = Path(args.output)
    out.write_text(markdown)
    print(f"Report written to {args.output}")


if __name__ == "__main__":
    main()
