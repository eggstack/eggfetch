#!/usr/bin/env python3
"""Verify CLI help output matches the documented CLI guide.

Checks that all flags mentioned in docs/cli/guide.md actually exist in the
--help output, and that there are no undocumented flags in --help.
"""

import re
import subprocess
import sys
from pathlib import Path


def get_help_flags(help_text: str) -> set[str]:
    """Extract flag names from --help output."""
    flags = set()
    for line in help_text.splitlines():
        # Match lines like:  -v, --verbose    Show verbose output
        # Or:               --max-redirects <N>  Maximum redirects
        for match in re.finditer(r"(--?[\w][\w-]*)", line):
            flag = match.group(1)
            flags.add(flag)
    return flags


def get_documented_flags(md_path: Path) -> set[str]:
    """Extract flag names from markdown documentation."""
    text = md_path.read_text(encoding="utf-8")
    flags = set()
    # Match inline code flags: `--verbose`, `-v`
    for match in re.finditer(r"`(--?[\w][\w-]*)`", text):
        flag = match.group(1)
        # Skip non-flag references like `eggfetch` or `GET`
        if flag.startswith("-"):
            flags.add(flag)
    # Match code block flags
    for match in re.finditer(r"(?:^|\s)(--?[\w][\w-]*)", text, re.MULTILINE):
        flag = match.group(1)
        if flag.startswith("-") and len(flag) > 1:
            # Filter out common non-flag patterns
            if flag not in {"--", "---"}:
                flags.add(flag)
    return flags


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    guide_path = repo_root / "docs" / "cli" / "guide.md"

    if not guide_path.exists():
        print(f"CLI guide not found at {guide_path}")
        return 0  # Not a hard failure

    # Find the binary
    binary = None
    for candidate in [
        repo_root / "target" / "release" / "eggfetch",
        repo_root / "target" / "debug" / "eggfetch",
    ]:
        if candidate.exists():
            binary = candidate
            break

    if binary is None:
        print("eggfetch binary not found — skipping CLI help sync check.")
        print("Run `cargo build -p eggfetch-cli` first.")
        return 0

    try:
        result = subprocess.run(
            [str(binary), "--help"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        if result.returncode != 0:
            print(f"eggfetch --help failed: {result.stderr}")
            return 1
    except Exception as exc:
        print(f"Error running eggfetch --help: {exc}")
        return 1

    help_flags = get_help_flags(result.stdout)
    doc_flags = get_documented_flags(guide_path)

    # Filter out standard flags that don't need documentation
    standard_flags = {"--help", "-h"}
    help_flags -= standard_flags

    missing_in_docs = help_flags - doc_flags
    missing_in_help = doc_flags - help_flags

    errors = 0

    if missing_in_docs:
        print(f"Flags in --help but not documented in {guide_path.name}:")
        for flag in sorted(missing_in_docs):
            print(f"  {flag}")
        errors += 1

    if missing_in_help:
        print(f"Flags documented in {guide_path.name} but not in --help:")
        for flag in sorted(missing_in_help):
            print(f"  {flag}")
        errors += 1

    if not errors:
        print("CLI help output matches documentation.")
    return errors


if __name__ == "__main__":
    sys.exit(main())
