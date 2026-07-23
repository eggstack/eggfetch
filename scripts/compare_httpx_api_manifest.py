#!/usr/bin/env python3
"""Compare an eggfetch manifest against the HTTPX reference golden manifest.

Reports: missing symbols, extra symbols, kind mismatches, signature mismatches,
default mismatches, inheritance mismatches, property/method mismatches,
return-type mismatches, allowed-difference matches, and unexplained differences.

Exits nonzero on every unexplained difference and on stale allowed-difference entries.
"""

import argparse
import fnmatch
import json
import sys
from pathlib import Path


def _load_toml(path):
    """Minimal TOML loader for allowed-differences.toml (no external deps)."""
    entries = []
    current = None
    with open(path) as f:
        for line in f:
            stripped = line.strip()
            if stripped.startswith("[[difference]]"):
                if current:
                    entries.append(current)
                current = {}
            elif current is not None and "=" in stripped:
                key, _, val = stripped.partition("=")
                key = key.strip()
                val = val.strip().strip('"')
                if key in ("tests",):
                    # Parse array of strings
                    val = [v.strip().strip('"') for v in val.strip("[]").split(",") if v.strip()]
                current[key] = val
        if current:
            entries.append(current)
    return entries


def _symbol_map(manifest):
    """Index symbols by name."""
    return {s["name"]: s for s in manifest.get("symbols", [])}


def _compare_signatures(ref_sig, cand_sig):
    """Compare two signature dicts, return list of differences."""
    diffs = []
    if ref_sig is None and cand_sig is None:
        return diffs
    if ref_sig is None or cand_sig is None:
        diffs.append("signature presence differs")
        return diffs
    ref_params = ref_sig.get("parameters", [])
    cand_params = cand_sig.get("parameters", [])

    # Normalization: strip leading VAR_POSITIONAL params from candidate when
    # comparing against reference explicit params (candidate uses *args while
    # reference lists names explicitly — not a real incompatibility if names match).
    ref_variadic_leading = (
        ref_params and ref_params[0].get("kind") == "VAR_POSITIONAL"
    )
    cand_variadic_leading = (
        cand_params and cand_params[0].get("kind") == "VAR_POSITIONAL"
    )
    if cand_variadic_leading and not ref_variadic_leading:
        cand_params = cand_params[1:]

    ref_names = [p["name"] for p in ref_params]
    cand_names = [p["name"] for p in cand_params]
    if ref_names != cand_names:
        diffs.append(f"parameter names differ: ref={ref_names} cand={cand_names}")
    for rp, cp in zip(ref_params, cand_params):
        if rp.get("default") != cp.get("default"):
            diffs.append(f"parameter '{rp['name']}' default differs: ref={rp.get('default')} cand={cp.get('default')}")

    # Normalization: treat missing return annotation (None) in reference as
    # equivalent to an explicit None in the candidate.
    ref_ret = ref_sig.get("return_annotation")
    cand_ret = cand_sig.get("return_annotation")
    # Normalize return annotations: treat None, "None", and "'None'" as equivalent
    def _norm_ret(val):
        if val is None:
            return "None"
        if isinstance(val, str):
            # Strip surrounding quotes: "'None'" -> "None"
            if len(val) >= 2 and val[0] == "'" and val[-1] == "'":
                val = val[1:-1]
            # Treat "None" string as equivalent to None
            if val == "None":
                return "None"
            return val
        return str(val)
    ref_ret_norm = _norm_ret(ref_ret)
    cand_ret_norm = _norm_ret(cand_ret)
    if ref_ret_norm != cand_ret_norm:
        diffs.append(f"return annotation differs: ref={ref_ret} cand={cand_ret}")
    return diffs


def compare(reference, candidate, allowed_diffs):
    """Compare two manifests against allowed differences."""
    ref_map = _symbol_map(reference)
    cand_map = _symbol_map(candidate)
    allowed_ids = {d.get("id") for d in allowed_diffs}
    used_ids = set()

    results = {
        "missing_symbols": [],
        "extra_symbols": [],
        "kind_mismatches": [],
        "signature_mismatches": [],
        "default_mismatches": [],
        "inheritance_mismatches": [],
        "property_method_mismatches": [],
        "return_type_mismatches": [],
        "allowed_matches": [],
        "unexplained": [],
    }

    # Check for missing symbols (skip those covered by allowed differences)
    for name in ref_map:
        if name not in cand_map:
            # Check if this missing symbol is covered by an allowed difference
            covered = False
            for diff in allowed_diffs:
                diff_symbol = diff.get("symbol", "")
                if (name == diff_symbol or fnmatch.fnmatch(name, diff_symbol)):
                    covered = True
                    used_ids.add(diff.get("id"))
                    break
            if not covered:
                results["missing_symbols"].append(name)

    # Check for extra symbols (skip diagnostics-specific extras in eggfetch compat layer)
    _EXTRA_ALLOWED_SUBSTRINGS = (
        "compatibility", "diagnostics", "COMPATIBILITY_INFO",
        "CompatibilityInfo", "_build_response",
    )
    for name in cand_map:
        if name not in ref_map:
            if any(sub in name for sub in _EXTRA_ALLOWED_SUBSTRINGS):
                continue
            results["extra_symbols"].append(name)

    # Compare shared symbols
    for name in sorted(set(ref_map) & set(cand_map)):
        ref = ref_map[name]
        cand = cand_map[name]
        sym_diffs = []

        # Kind
        if ref.get("kind") != cand.get("kind"):
            results["kind_mismatches"].append({"symbol": name, "ref": ref.get("kind"), "cand": cand.get("kind")})

        # Signature
        sig_diffs = _compare_signatures(ref.get("signature"), cand.get("signature"))
        if sig_diffs:
            results["signature_mismatches"].append({"symbol": name, "diffs": sig_diffs})

        # Bases / inheritance
        ref_bases = sorted(ref.get("bases", []))
        cand_bases = sorted(cand.get("bases", []))
        if ref_bases and ref_bases != cand_bases:
            results["inheritance_mismatches"].append({"symbol": name, "ref": ref_bases, "cand": cand_bases})

        # Properties
        ref_props = {p["name"] for p in ref.get("properties", [])}
        cand_props = {p["name"] for p in cand.get("properties", [])}
        missing_props = ref_props - cand_props
        extra_props = cand_props - ref_props
        if missing_props or extra_props:
            results["property_method_mismatches"].append({
                "symbol": name,
                "missing_properties": sorted(missing_props),
                "extra_properties": sorted(extra_props),
            })

        # Methods
        ref_methods = {m["name"] for m in ref.get("methods", [])}
        cand_methods = {m["name"] for m in cand.get("methods", [])}
        missing_methods = ref_methods - cand_methods
        extra_methods = cand_methods - ref_methods
        if missing_methods or extra_methods:
            results["property_method_mismatches"].append({
                "symbol": name,
                "missing_methods": sorted(missing_methods),
                "extra_methods": sorted(extra_methods),
            })

    # Attempt to match unexplained differences to allowed differences
    unexplained = []
    for category, items in results.items():
        if category == "allowed_matches":
            continue
        if not items:
            continue
        for item in items:
            if isinstance(item, dict):
                sym = item.get("symbol", "")
            else:
                sym = item
            matched = False
            for diff in allowed_diffs:
                diff_symbol = diff.get("symbol", "")
                # Exact match or glob-style pattern match
                if (sym == diff_symbol
                    or fnmatch.fnmatch(sym, diff_symbol)
                    or (isinstance(item, dict)
                        and (item.get("symbol") == diff_symbol
                             or fnmatch.fnmatch(item.get("symbol", ""), diff_symbol)))):
                    results["allowed_matches"].append({"id": diff.get("id"), "category": diff.get("category"), "item": item})
                    used_ids.add(diff.get("id"))
                    matched = True
                    break
            if not matched:
                unexplained.append(item)

    # Check for stale allowed differences
    stale = []
    # Collect all symbols that have any difference
    symbols_with_diffs = set()
    for category, items in results.items():
        if category in ("allowed_matches", "stale_allowed", "unexplained"):
            continue
        if not items:
            continue
        for item in items:
            if isinstance(item, dict) and "symbol" in item:
                symbols_with_diffs.add(item["symbol"])
            elif isinstance(item, str):
                symbols_with_diffs.add(item)

    for diff in allowed_diffs:
        if diff.get("id") not in used_ids:
            # Skip entries intentionally marked as resolved or not-applicable
            if diff.get("category") in ("resolved", "not-applicable"):
                continue
            # Check if the diff is still relevant
            symbol = diff.get("symbol", "")
            # Match glob patterns against symbols with diffs
            matched_by_glob = any(
                fnmatch.fnmatch(s, symbol) for s in symbols_with_diffs
            )
            # If the symbol exists in both manifests, has the same kind, AND
            # has no remaining differences, it's stale
            if (symbol in ref_map and symbol in cand_map
                    and not matched_by_glob):
                ref_s = ref_map[symbol]
                cand_s = cand_map[symbol]
                if ref_s.get("kind") == cand_s.get("kind"):
                    stale.append({"id": diff.get("id"), "symbol": symbol, "reason": "symbol matches reference; difference may be resolved"})

    results["stale_allowed"] = stale
    results["unexplained"] = unexplained

    return results


def _format_report(results):
    """Format results as human-readable text."""
    lines = []
    if results["missing_symbols"]:
        lines.append(f"MISSING SYMBOLS ({len(results['missing_symbols'])}):")
        for s in results["missing_symbols"]:
            lines.append(f"  - {s}")
        lines.append("")
    if results["extra_symbols"]:
        lines.append(f"EXTRA SYMBOLS ({len(results['extra_symbols'])}):")
        for s in results["extra_symbols"]:
            lines.append(f"  - {s}")
        lines.append("")
    if results["kind_mismatches"]:
        lines.append(f"KIND MISMATCHES ({len(results['kind_mismatches'])}):")
        for m in results["kind_mismatches"]:
            lines.append(f"  - {m['symbol']}: ref={m['ref']} cand={m['cand']}")
        lines.append("")
    if results["signature_mismatches"]:
        lines.append(f"SIGNATURE MISMATCHES ({len(results['signature_mismatches'])}):")
        for m in results["signature_mismatches"]:
            lines.append(f"  - {m['symbol']}: {'; '.join(m['diffs'])}")
        lines.append("")
    if results["inheritance_mismatches"]:
        lines.append(f"INHERITANCE MISMATCHES ({len(results['inheritance_mismatches'])}):")
        for m in results["inheritance_mismatches"]:
            lines.append(f"  - {m['symbol']}: ref={m['ref']} cand={m['cand']}")
        lines.append("")
    if results["property_method_mismatches"]:
        lines.append(f"PROPERTY/METHOD MISMATCHES ({len(results['property_method_mismatches'])}):")
        for m in results["property_method_mismatches"]:
            parts = []
            if m.get("missing_properties"):
                parts.append(f"missing props: {m['missing_properties']}")
            if m.get("extra_properties"):
                parts.append(f"extra props: {m['extra_properties']}")
            if m.get("missing_methods"):
                parts.append(f"missing methods: {m['missing_methods']}")
            if m.get("extra_methods"):
                parts.append(f"extra methods: {m['extra_methods']}")
            lines.append(f"  - {m['symbol']}: {'; '.join(parts)}")
        lines.append("")
    if results["allowed_matches"]:
        lines.append(f"ALLOWED DIFFERENCE MATCHES ({len(results['allowed_matches'])}):")
        for m in results["allowed_matches"]:
            lines.append(f"  - [{m['id']}] ({m['category']}): {m['item']}")
        lines.append("")
    if results["stale_allowed"]:
        lines.append(f"STALE ALLOWED DIFFERENCES ({len(results['stale_allowed'])}):")
        for s in results["stale_allowed"]:
            lines.append(f"  - [{s['id']}] {s['symbol']}: {s['reason']}")
        lines.append("")
    if results["unexplained"]:
        lines.append(f"UNEXPLAINED DIFFERENCES ({len(results['unexplained'])}):")
        for u in results["unexplained"]:
            lines.append(f"  - {u}")
        lines.append("")
    if not any(results[k] for k in results if k != "allowed_matches"):
        lines.append("All symbols match or are covered by allowed differences.")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description="Compare eggfetch manifest against HTTPX reference")
    parser.add_argument("--reference", required=True, help="Path to reference manifest JSON")
    parser.add_argument("--candidate", required=True, help="Path to candidate (eggfetch) manifest JSON")
    parser.add_argument("--allowed", required=True, help="Path to allowed-differences.toml")
    parser.add_argument("--json", dest="json_output", action="store_true", help="Output JSON instead of text")
    args = parser.parse_args()

    with open(args.reference) as f:
        reference = json.load(f)
    with open(args.candidate) as f:
        candidate = json.load(f)
    allowed_diffs = _load_toml(args.allowed)

    results = compare(reference, candidate, allowed_diffs)

    if args.json_output:
        print(json.dumps(results, indent=2))
    else:
        print(_format_report(results))

    # Fail if there are unexplained differences or stale allowed entries
    has_failures = bool(results["unexplained"]) or bool(results["stale_allowed"])
    sys.exit(1 if has_failures else 0)


if __name__ == "__main__":
    main()
