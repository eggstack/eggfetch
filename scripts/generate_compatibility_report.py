#!/usr/bin/env python3
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
    generated = evidence.get("generated_at", "unknown")

    lines.append("# Compatibility Evidence Report")
    lines.append("")

    lines.append("## Summary")
    lines.append("")
    lines.append(f"| Field | Value |")
    lines.append(f"|-------|-------|")
    lines.append(f"| Status | **{'PASS' if overall else 'FAIL'}** |")
    lines.append(f"| Compatibility Stage | {stage} |")
    lines.append(f"| eggfetch Version | {ev_version} |")
    lines.append(f"| Reference HTTPX Version | {ref_version} |")
    lines.append(f"| eggfetch Commit | `{commit[:12]}` |")
    lines.append(f"| Generated At | {generated} |")
    lines.append("")

    api = evidence.get("api_manifest_summary", {})
    lines.append("## API Coverage")
    lines.append("")
    if api.get("available"):
        lines.append(f"| Metric | Count |")
        lines.append(f"|--------|-------|")
        lines.append(f"| Total Symbols | {api.get('total_symbols', 0)} |")
        for kind, count in api.get("by_kind", {}).items():
            lines.append(f"| {kind} | {count} |")
    else:
        lines.append("API manifest not available.")
    lines.append("")

    results = evidence.get("differential_case_results", {})
    totals = evidence.get("differential_case_totals", 0)
    lines.append("## Differential Test Results")
    lines.append("")
    if results.get("skipped"):
        lines.append("Tests were **skipped** (metadata-only report).")
    else:
        total = results.get("total", 0)
        passed = results.get("passed", 0)
        failures = results.get("failures", 0)
        errors = results.get("errors", 0)
        pass_rate = f"{(passed / total * 100):.1f}" if total > 0 else "N/A"
        lines.append(f"| Metric | Value |")
        lines.append(f"|--------|-------|")
        lines.append(f"| Declared Test Count | {totals} |")
        lines.append(f"| pytest Total | {total} |")
        lines.append(f"| Passed | {passed} |")
        lines.append(f"| Failed | {failures} |")
        lines.append(f"| Errors | {errors} |")
        lines.append(f"| Pass Rate | {pass_rate}% |")
    lines.append("")

    downstream_versions = evidence.get("downstream_package_versions", {})
    downstream_results = evidence.get("downstream_results", {})
    lines.append("## Downstream Portfolio")
    lines.append("")
    if downstream_versions or downstream_results:
        lines.append(f"| Package | Expected | Installed | Available |")
        lines.append(f"|---------|----------|-----------|-----------|")
        all_pkgs = set(downstream_versions.keys()) | set(downstream_results.keys())
        for pkg in sorted(all_pkgs):
            info = downstream_results.get(pkg, {})
            expected = downstream_versions.get(pkg, "unknown")
            installed = info.get("installed_version", "-")
            available = "Yes" if info.get("available") else "No"
            lines.append(f"| {pkg} | {expected} | {installed} | {available} |")
    else:
        lines.append("No downstream packages declared.")
    lines.append("")

    allowed = evidence.get("allowed_differences", [])
    lines.append("## Allowed Differences")
    lines.append("")
    if allowed:
        lines.append(f"| ID | Category | Symbol | Stage Impact | Security |")
        lines.append(f"|----|----------|--------|--------------|----------|")
        for diff in allowed:
            did = diff.get("id", "?")
            cat = diff.get("category", "?")
            sym = diff.get("symbol", "?")
            impact = diff.get("compatibility-stage-impact", "?")
            sec = "Yes" if diff.get("security-improvement") else "No"
            lines.append(f"| {did} | {cat} | {sym} | {impact} | {sec} |")
    else:
        lines.append("No allowed differences declared.")
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
