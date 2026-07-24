#!/usr/bin/env python3
"""Guard: ensure no CI workflow attempts to publish this artifact.

Scans .github/workflows/*.yml for twine upload or pypa/gh-action-pypi-publish
that references the httpx controlled replacement. Exits nonzero if found.
"""

import glob
import os
import re
import sys

REPO_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "..", "..")
)
WORKFLOW_DIR = os.path.join(REPO_ROOT, ".github", "workflows")

PUBLISH_PATTERNS = [
    re.compile(r"twine\s+upload"),
    re.compile(r"pypa/gh-action-pypi-publish"),
]

CONTROLLED_REPLACEMENT_PATTERNS = [
    re.compile(r"httpx-controlled-replacement"),
    re.compile(r"httpx-0\.28\.1"),
]


def main() -> int:
    if not os.path.isdir(WORKFLOW_DIR):
        print(f"INFO: no workflow directory at {WORKFLOW_DIR}, skipping")
        return 0

    findings: list[str] = []

    for path in sorted(glob.glob(os.path.join(WORKFLOW_DIR, "*.yml"))):
        with open(path, encoding="utf-8") as f:
            content = f.read()

        has_publish = any(p.search(content) for p in PUBLISH_PATTERNS)
        has_controlled = any(p.search(content) for p in CONTROLLED_REPLACEMENT_PATTERNS)

        if has_publish and has_controlled:
            findings.append(os.path.relpath(path, REPO_ROOT))

    if findings:
        for f in findings:
            print(f"FAIL: publish action found in {f}", file=sys.stderr)
        return 1

    print("PASS: no publish attempts targeting the httpx controlled replacement")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
