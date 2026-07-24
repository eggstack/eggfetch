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
    "id", "category", "symbol", "rationale", "owner", "review-milestone",
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


def _match_allowed(symbol, diff, allowed_entry):
    """Match an allowed entry against a difference record by exact tuple.

    An allowed entry matches only when all applicable fields match:
    - symbol
    - difference type
    - member
    - canonical reference value
    - canonical candidate value

    No wildcard, regex, glob, prefix, suffix, or symbol-only matching is allowed.
    """
    if symbol != allowed_entry.get("symbol", ""):
        return False
    if diff.get("difference_type", "") != allowed_entry.get("difference-type", ""):
        return False
    if diff.get("member", "") != allowed_entry.get("member", ""):
        return False
    # Canonical reference and candidate values — must match exactly
    ref_norm = _norm_default(diff.get("reference"))
    cand_norm = _norm_default(diff.get("candidate"))
    allowed_ref = allowed_entry.get("reference", "")
    allowed_cand = allowed_entry.get("candidate", "")
    if str(ref_norm) != allowed_ref:
        return False
    if str(cand_norm) != allowed_cand:
        return False
    return True


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
            if _match_allowed(symbol, diff_record, ad):
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
    resolved_in_active = []
    for ad in allowed_diffs:
        ad_id = ad.get("id")
        # Flag resolved category entries as errors (they should not be in active file)
        if ad.get("category") == "resolved":
            resolved_in_active.append({
                "id": ad_id,
                "symbol": ad.get("symbol", ""),
                "reason": "'resolved' category entry must not appear in active allowed-differences file",
            })
        if ad_id in used_ids:
            continue
        # Per plan: resolved entries must not waive current differences.
        # They may remain only as historical records in a separate resolved
        # ledger or must fail as stale if present in the active allowed file.
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
    results["resolved_in_active"] = resolved_in_active
    results["unexplained"] = unexplained
    return results


def validate_allowed_diffs(path):
    """Validate allowed-differences.toml schema. Returns list of errors.

    Non-resolved entries that carry a ``difference-type`` field must also
    provide ``member``, ``reference``, and ``candidate`` (exact typed tuple).
    Entries without ``difference-type`` are behavioral descriptions and are
    not matched against API manifest differences.

    Enforced rules:
    - ``resolved`` category entries in the active file are flagged as errors.
    - ``expiry`` field is validated when present.
    - Duplicate IDs fail validation.
    - Duplicate tuples (symbol, difference-type, member, reference, candidate) fail.
    - Wildcard entries (``*``) are rejected.
    """
    entries = _load_toml(path)
    errors = []
    seen_ids = set()
    seen_tuples: dict[tuple, str] = {}

    # Fields required for all entries
    _BASE_REQUIRED = (
        "id", "category", "symbol", "rationale", "owner", "review-milestone",
    )
    # Fields required for typed-tuple entries (non-resolved with difference-type)
    _TYPED_REQUIRED = (
        "difference-type", "member", "reference", "candidate",
    )

    for i, entry in enumerate(entries):
        entry_id = entry.get("id", f"<entry {i}>")

        # Base required fields for all entries
        for field in _BASE_REQUIRED:
            if field not in entry or not entry[field]:
                errors.append(f"[{entry_id}] missing required field: {field}")

        category = entry.get("category", "")
        if category and category not in _VALID_CATEGORIES:
            errors.append(f"[{entry_id}] invalid category: {category!r}")

        # resolved category entries must not appear in the active allowed file
        if category == "resolved":
            errors.append(
                f"[{entry_id}] 'resolved' category entry must not appear in "
                f"the active allowed-differences file (move to resolved-differences.toml)"
            )

        symbol = entry.get("symbol", "")
        # Check for wildcard in symbol field (reject all wildcards)
        if "*" in symbol:
            errors.append(f"[{entry_id}] wildcard in symbol: {symbol!r}")

        # Typed-tuple fields: required for non-resolved entries with difference-type
        has_diff_type = "difference-type" in entry
        if has_diff_type and category != "resolved":
            for field in _TYPED_REQUIRED:
                if field not in entry:
                    errors.append(f"[{entry_id}] typed entry missing field: {field}")
            # difference-type must be a valid type
            dt = entry.get("difference-type", "")
            if dt and dt not in _VALID_DIFFERENCE_TYPES:
                errors.append(f"[{entry_id}] invalid difference-type: {dt!r}")

            # Build tuple for duplicate detection
            tuple_key = (
                symbol,
                dt,
                entry.get("member", ""),
                entry.get("reference", ""),
                entry.get("candidate", ""),
            )
            if tuple_key in seen_tuples:
                errors.append(
                    f"[{entry_id}] duplicate tuple: same (symbol, difference-type, "
                    f"member, reference, candidate) as [{seen_tuples[tuple_key]}]"
                )
            else:
                seen_tuples[tuple_key] = entry_id

        # Duplicate ID detection
        if entry_id in seen_ids:
            errors.append(f"[{entry_id}] duplicate ID")
        seen_ids.add(entry_id)

        # Expiry field validation
        expiry = entry.get("expiry")
        if expiry:
            if not isinstance(expiry, str) or not expiry:
                errors.append(f"[{entry_id}] expiry must be a non-empty string, got {expiry!r}")
            else:
                try:
                    exp_date = datetime.fromisoformat(expiry)
                    if exp_date.tzinfo is None:
                        # Assume UTC for naive datetimes
                        from datetime import timezone as _tz
                        exp_date = exp_date.replace(tzinfo=_tz.utc)
                    now = datetime.now(_tz.utc) if exp_date.tzinfo else datetime.now()
                    if exp_date < now:
                        errors.append(f"[{entry_id}] expired entry: {expiry}")
                except ValueError:
                    errors.append(f"[{entry_id}] invalid expiry format: {expiry!r} (expected ISO-8601)")

    return errors


def _format_report(results):
    """Format results as human-readable text."""
    lines = []
    if results.get("resolved_in_active"):
        lines.append(f"RESOLVED ENTRIES IN ACTIVE FILE ({len(results['resolved_in_active'])}):")
        for r in results["resolved_in_active"]:
            lines.append(f"  - [{r['id']}] {r['symbol']}: {r['reason']}")
        lines.append("")
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
    if not any(results.get(k) for k in results if k not in ("allowed_matches", "resolved_in_active")):
        lines.append("All symbols match or are covered by allowed differences.")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description="Compare eggfetch manifest against HTTPX reference")
    parser.add_argument("--reference", help="Path to reference manifest JSON")
    parser.add_argument("--candidate", help="Path to candidate (eggfetch) manifest JSON")
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

    if not args.reference or not args.candidate:
        parser.error("--reference and --candidate are required unless --validate is used")

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

    has_failures = (
        bool(results["unexplained"])
        or bool(results["stale_allowed"])
        or bool(results.get("resolved_in_active"))
    )
    sys.exit(1 if has_failures else 0)


if __name__ == "__main__":
    main()
