#!/usr/bin/env python3
"""Compare an eggfetch manifest against the HTTPX reference golden manifest.

Reports: missing symbols, extra symbols, kind mismatches, signature mismatches,
default mismatches, inheritance mismatches, property/method mismatches,
return-type mismatches, allowed-difference matches, and unexplained differences.

Exits nonzero on every unexplained difference and on stale allowed-difference entries.
"""

import argparse
import json
import sys
from datetime import datetime
from pathlib import Path


_REQUIRED_DIFF_FIELDS = (
    "id", "category", "symbol", "behavior-case",
    "reference-behavior", "eggfetch-behavior", "rationale",
)

_VALID_CATEGORIES = (
    "intentional-difference", "stage-bounded", "resolved",
    "not-applicable", "required-now",
)

_VALID_DIFFERENCE_TYPES = (
    "missing-symbol", "extra-symbol", "symbol-kind",
    "parameter-name", "parameter-kind", "parameter-requiredness",
    "parameter-default", "variadic-shape", "return-annotation",
    "base-class", "missing-property", "extra-property",
    "missing-method", "extra-method", "sync-async-kind",
)

_KNOWN_SENTINELS = {
    "<httpx._config.UnsetType object>": "UnsetType",
}


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
                    val = [v.strip().strip('"') for v in val.strip("[]").split(",") if v.strip()]
                current[key] = val
        if current:
            entries.append(current)
    return entries


def _symbol_map(manifest):
    """Index symbols by name."""
    return {s["name"]: s for s in manifest.get("symbols", [])}


def _norm_annotation(val):
    if val is None:
        return "None"
    if isinstance(val, str):
        if len(val) >= 2 and val[0] == "'" and val[-1] == "'":
            val = val[1:-1]
        if val == "None":
            return "None"
        return val
    return str(val)


def _norm_default(val):
    if val is None:
        return None
    s = str(val)
    return _KNOWN_SENTINELS.get(s, s)


def _compare_signatures(ref_sig, cand_sig, symbol):
    """Compare two signature dicts, return list of typed difference records."""
    diffs = []
    if ref_sig is None and cand_sig is None:
        return diffs
    if ref_sig is None or cand_sig is None:
        diffs.append({
            "symbol": symbol,
            "difference_type": "variadic-shape",
            "member": "__init__",
            "reference": str(ref_sig),
            "candidate": str(cand_sig),
        })
        return diffs
    ref_params = ref_sig.get("parameters", [])
    cand_params = cand_sig.get("parameters", [])

    ref_names = [p["name"] for p in ref_params]
    cand_names = [p["name"] for p in cand_params]
    if ref_names != cand_names:
        diffs.append({
            "symbol": symbol,
            "difference_type": "parameter-name",
            "member": symbol,
            "reference": str(ref_names),
            "candidate": str(cand_names),
        })

    for rp, cp in zip(ref_params, cand_params):
        rp_kind = rp.get("kind", "")
        cp_kind = cp.get("kind", "")
        if rp_kind != cp_kind:
            diffs.append({
                "symbol": symbol,
                "difference_type": "parameter-kind",
                "member": rp["name"],
                "reference": rp_kind,
                "candidate": cp_kind,
            })

        rp_default = _norm_default(rp.get("default"))
        cp_default = _norm_default(cp.get("default"))
        rp_default_str = str(rp_default)
        cp_default_str = str(cp_default)
        if rp_default_str != cp_default_str:
            diffs.append({
                "symbol": symbol,
                "difference_type": "parameter-default",
                "member": rp["name"],
                "reference": rp_default_str,
                "candidate": cp_default_str,
            })

    ref_ret = ref_sig.get("return_annotation")
    cand_ret = cand_sig.get("return_annotation")
    if _norm_annotation(ref_ret) != _norm_annotation(cand_ret):
        diffs.append({
            "symbol": symbol,
            "difference_type": "return-annotation",
            "member": symbol,
            "reference": str(ref_ret),
            "candidate": str(cand_ret),
        })
    return diffs


def _match_allowed(symbol, diff):
    allowed_symbol = diff.get("symbol", "")
    return symbol == allowed_symbol


def compare(reference, candidate, allowed_diffs):
    """Compare two manifests against allowed differences."""
    ref_map = _symbol_map(reference)
    cand_map = _symbol_map(candidate)
    used_ids = set()

    results = {
        "differences": [],
        "allowed_matches": [],
        "stale_allowed": [],
        "unexplained": [],
    }

    for name in ref_map:
        if name not in cand_map:
            results["differences"].append({
                "symbol": name,
                "difference_type": "missing-symbol",
                "member": "",
                "reference": "present",
                "candidate": "absent",
            })

    for name in cand_map:
        if name not in ref_map:
            results["differences"].append({
                "symbol": name,
                "difference_type": "extra-symbol",
                "member": "",
                "reference": "absent",
                "candidate": "present",
            })

    for name in sorted(set(ref_map) & set(cand_map)):
        ref = ref_map[name]
        cand = cand_map[name]

        if ref.get("kind") != cand.get("kind"):
            results["differences"].append({
                "symbol": name,
                "difference_type": "symbol-kind",
                "member": "",
                "reference": ref.get("kind", ""),
                "candidate": cand.get("kind", ""),
            })

        sig_diffs = _compare_signatures(ref.get("signature"), cand.get("signature"), name)
        results["differences"].extend(sig_diffs)

        ref_bases = sorted(ref.get("bases", []))
        cand_bases = sorted(cand.get("bases", []))
        if ref_bases and ref_bases != cand_bases:
            results["differences"].append({
                "symbol": name,
                "difference_type": "base-class",
                "member": "",
                "reference": str(ref_bases),
                "candidate": str(cand_bases),
            })

        ref_props = {p["name"] for p in ref.get("properties", [])}
        cand_props = {p["name"] for p in cand.get("properties", [])}
        for prop in sorted(ref_props - cand_props):
            results["differences"].append({
                "symbol": name,
                "difference_type": "missing-property",
                "member": prop,
                "reference": "present",
                "candidate": "absent",
            })
        for prop in sorted(cand_props - ref_props):
            results["differences"].append({
                "symbol": name,
                "difference_type": "extra-property",
                "member": prop,
                "reference": "absent",
                "candidate": "present",
            })

        ref_methods = {m["name"] for m in ref.get("methods", [])}
        cand_methods = {m["name"] for m in cand.get("methods", [])}
        for method in sorted(ref_methods - cand_methods):
            results["differences"].append({
                "symbol": name,
                "difference_type": "missing-method",
                "member": method,
                "reference": "present",
                "candidate": "absent",
            })
        for method in sorted(cand_methods - ref_methods):
            results["differences"].append({
                "symbol": name,
                "difference_type": "extra-method",
                "member": method,
                "reference": "absent",
                "candidate": "present",
            })

    allowed_by_id = {d.get("id"): d for d in allowed_diffs}
    unexplained = []
    for diff_record in results["differences"]:
        symbol = diff_record["symbol"]
        matched = False
        for ad in allowed_diffs:
            if _match_allowed(symbol, ad):
                results["allowed_matches"].append({
                    "id": ad.get("id"),
                    "category": ad.get("category"),
                    "difference": diff_record,
                })
                used_ids.add(ad.get("id"))
                matched = True
                break
        if not matched:
            unexplained.append(diff_record)

    stale = []
    for ad in allowed_diffs:
        ad_id = ad.get("id")
        if ad_id in used_ids:
            continue
        if ad.get("category") in ("resolved", "not-applicable"):
            continue
        symbol = ad.get("symbol", "")
        if symbol in ref_map and symbol in cand_map:
            ref_s = ref_map[symbol]
            cand_s = cand_map[symbol]
            if ref_s.get("kind") == cand_s.get("kind"):
                sym_has_diffs = any(
                    d["symbol"] == symbol for d in results["differences"]
                )
                if not sym_has_diffs:
                    stale.append({
                        "id": ad_id,
                        "symbol": symbol,
                        "reason": "symbol matches reference; difference may be resolved",
                    })

    results["stale_allowed"] = stale
    results["unexplained"] = unexplained
    return results


def validate_allowed_diffs(path):
    """Validate allowed-differences.toml schema. Returns list of errors."""
    entries = _load_toml(path)
    errors = []
    seen_ids = set()

    for i, entry in enumerate(entries):
        entry_id = entry.get("id", f"<entry {i}>")

        for field in _REQUIRED_DIFF_FIELDS:
            if field not in entry or not entry[field]:
                errors.append(f"[{entry_id}] missing required field: {field}")

        category = entry.get("category", "")
        if category and category not in _VALID_CATEGORIES:
            errors.append(f"[{entry_id}] invalid category: {category!r}")

        symbol = entry.get("symbol", "")
        if "*" in symbol:
            errors.append(f"[{entry_id}] wildcard in symbol: {symbol!r}")

        for key, val in entry.items():
            if key == "tests" and isinstance(val, list):
                continue
            if isinstance(val, str) and "*" in val and key == "symbol":
                continue

        if entry_id in seen_ids:
            errors.append(f"[{entry_id}] duplicate ID")
        seen_ids.add(entry_id)

        expiry = entry.get("expiry")
        if expiry:
            try:
                exp_date = datetime.fromisoformat(expiry)
                if exp_date < datetime.now():
                    errors.append(f"[{entry_id}] expired entry: {expiry}")
            except ValueError:
                errors.append(f"[{entry_id}] invalid expiry format: {expiry!r}")

    return errors


def _format_report(results):
    """Format results as human-readable text."""
    lines = []
    if results["differences"]:
        lines.append(f"DIFFERENCES ({len(results['differences'])}):")
        for d in results["differences"]:
            member_info = f" ({d['member']})" if d.get("member") else ""
            lines.append(f"  - [{d['difference_type']}] {d['symbol']}{member_info}: ref={d['reference']} cand={d['candidate']}")
        lines.append("")
    if results["allowed_matches"]:
        lines.append(f"ALLOWED DIFFERENCE MATCHES ({len(results['allowed_matches'])}):")
        for m in results["allowed_matches"]:
            d = m["difference"]
            member_info = f" ({d['member']})" if d.get("member") else ""
            lines.append(f"  - [{m['id']}] ({m['category']}) [{d['difference_type']}] {d['symbol']}{member_info}")
        lines.append("")
    if results["stale_allowed"]:
        lines.append(f"STALE ALLOWED DIFFERENCES ({len(results['stale_allowed'])}):")
        for s in results["stale_allowed"]:
            lines.append(f"  - [{s['id']}] {s['symbol']}: {s['reason']}")
        lines.append("")
    if results["unexplained"]:
        lines.append(f"UNEXPLAINED DIFFERENCES ({len(results['unexplained'])}):")
        for u in results["unexplained"]:
            member_info = f" ({u['member']})" if u.get("member") else ""
            lines.append(f"  - [{u['difference_type']}] {u['symbol']}{member_info}: ref={u['reference']} cand={u['candidate']}")
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
    parser.add_argument("--validate", action="store_true", help="Validate allowed-differences.toml schema")
    args = parser.parse_args()

    if args.validate:
        errors = validate_allowed_diffs(args.allowed)
        if errors:
            print(f"VALIDATION FAILED ({len(errors)} errors):")
            for e in errors:
                print(f"  - {e}")
            sys.exit(1)
        else:
            print("Validation passed.")
            sys.exit(0)

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

    has_failures = bool(results["unexplained"]) or bool(results["stale_allowed"])
    sys.exit(1 if has_failures else 0)


if __name__ == "__main__":
    main()
