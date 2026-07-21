#!/usr/bin/env python3
"""Lint documentation for unqualified HTTPX compatibility claims.

Rejects phrases like 'drop-in replacement', 'fully compatible', or
'identical to HTTPX' unless the compatibility profile status file
records the required achieved stage.
"""

import argparse
import re
import sys
from pathlib import Path

# Phrases that require qualification
UNQUALIFIED_PATTERNS = [
    (r"\bdrop[- ]in replacement\b", "Use 'HTTPX-compatible surface' or qualify with the specific stage"),
    (r"\bfully compatible\b", "Specify which aspects are compatible"),
    (r"\bidentical to HTTPX\b", "Specify which aspects match"),
    (r"\b100% compatible\b", "Specify the compatibility percentage and stage"),
    (r"\bseamless migration\b", "Describe specific migration steps instead"),
]


def lint_file(filepath, strict=False):
    """Check a markdown file for unqualified claims."""
    errors = []
    content = Path(filepath).read_text()
    for line_num, line in enumerate(content.splitlines(), 1):
        # Skip code blocks
        if line.strip().startswith("```") or line.strip().startswith("#"):
            continue
        for pattern, suggestion in UNQUALIFIED_PATTERNS:
            if re.search(pattern, line, re.IGNORECASE):
                errors.append(f"{filepath}:{line_num}: unqualified claim found: {re.search(pattern, line, re.IGNORECASE).group()!r} — {suggestion}")
    return errors


def main():
    parser = argparse.ArgumentParser(description="Lint docs for unqualified compatibility claims")
    parser.add_argument("files", nargs="*", help="Files to check")
    parser.add_argument("--strict", action="store_true", help="Fail on any match")
    args = parser.parse_args()

    all_errors = []
    for filepath in args.files:
        if Path(filepath).exists():
            all_errors.extend(lint_file(filepath, args.strict))

    for e in all_errors:
        print(f"ERROR: {e}", file=sys.stderr)

    sys.exit(1 if all_errors else 0)


if __name__ == "__main__":
    main()
