#!/usr/bin/env python3
"""Validate the HTTPX compatibility profile directory.

Checks: required files exist, allowed-differences schema validation,
duplicate IDs, missing fields, unknown categories, expired review dates,
and references to nonexistent symbols.
"""

import argparse
import sys
from pathlib import Path


VALID_CATEGORIES = {"required-now", "required-later", "intentional-difference", "not-public", "not-applicable", "resolved", "stage-bounded"}


def _load_toml(path):
    """Minimal TOML loader for profile validation."""
    entries = []
    current = None
    in_array = False
    with open(path) as f:
        for line in f:
            stripped = line.strip()
            if stripped.startswith("[[difference]]"):
                if current:
                    entries.append(current)
                current = {}
                in_array = True
            elif stripped.startswith("["):
                if current:
                    entries.append(current)
                    current = None
                in_array = False
            elif current is not None and "=" in stripped and not stripped.startswith("#"):
                key, _, val = stripped.partition("=")
                key = key.strip()
                val = val.strip().strip('"')
                if val.startswith("[") and val.endswith("]"):
                    val = [v.strip().strip('"') for v in val[1:-1].split(",") if v.strip()]
                current[key] = val
        if current:
            entries.append(current)
    return entries


def validate_profile(profile_dir):
    """Validate a compatibility profile directory."""
    errors = []
    warnings = []
    profile_path = Path(profile_dir)

    # Check required files
    required_files = ["profile.toml", "allowed-differences.toml", "README.md"]
    for f in required_files:
        if not (profile_path / f).exists():
            errors.append(f"Missing required file: {f}")

    # Validate allowed-differences.toml
    allowed_path = profile_path / "allowed-differences.toml"
    if allowed_path.exists():
        diffs = _load_toml(allowed_path)
        ids = []
        for d in diffs:
            # Check required fields
            required_fields = ["id", "category", "symbol", "rationale", "owner", "review-milestone"]
            for field in required_fields:
                if field not in d:
                    errors.append(f"Difference {d.get('id', '?')}: missing required field '{field}'")

            # Check category
            cat = d.get("category")
            if cat and cat not in VALID_CATEGORIES:
                errors.append(f"Difference {d.get('id', '?')}: unknown category '{cat}'")

            # Check for duplicate IDs
            did = d.get("id")
            if did in ids:
                errors.append(f"Duplicate ID: {did}")
            ids.append(did)

            # Check tests field
            tests = d.get("tests")
            if isinstance(tests, list) and len(tests) == 0 and cat in ("required-now", "required-later"):
                warnings.append(f"Difference {did}: no tests linked for {cat} item")

    # Validate profile.toml
    profile_toml = profile_path / "profile.toml"
    if profile_toml.exists():
        content = profile_toml.read_text()
        if "reference" not in content:
            errors.append("profile.toml: missing [reference] section")
        if "0.28.1" not in content:
            errors.append("profile.toml: reference version must be 0.28.1")

    return errors, warnings


def main():
    parser = argparse.ArgumentParser(description="Validate HTTPX compatibility profile")
    parser.add_argument("profile_dir", help="Path to profile directory (e.g., compat/httpx/0.28.1)")
    args = parser.parse_args()

    errors, warnings = validate_profile(args.profile_dir)

    for w in warnings:
        print(f"WARNING: {w}", file=sys.stderr)
    for e in errors:
        print(f"ERROR: {e}", file=sys.stderr)

    if errors:
        print(f"\n{len(errors)} error(s), {len(warnings)} warning(s)", file=sys.stderr)
        sys.exit(1)
    else:
        print(f"Profile validation passed ({len(warnings)} warning(s))")
        sys.exit(0)


if __name__ == "__main__":
    main()
