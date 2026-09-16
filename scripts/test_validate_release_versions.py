#!/usr/bin/env python3
"""Regression tests for release version/ref validation."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts" / "validate_release_versions.py"


def cargo_version() -> str:
    text = (ROOT / "crates" / "eggfetch-core" / "Cargo.toml").read_text()
    match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if match is None:
        raise RuntimeError("workspace version missing")
    return match.group(1)


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


def main() -> None:
    version = cargo_version()
    cases = {
        "ordinary validation": ((), 0),
        "matching publish tag": (("--tag", f"v{version}", "--publish"), 0),
        "mismatched tag": (("--tag", "v0.0.0", "--publish"), 1),
        "invalid tag": (("--tag", "release-0.0.0", "--publish"), 1),
        "publish without tag": (("--publish",), 1),
    }
    for name, (args, expected) in cases.items():
        result = run(*args)
        if result.returncode != expected:
            raise SystemExit(
                f"{name} expected exit {expected}, got {result.returncode}\n"
                f"stdout: {result.stdout}\nstderr: {result.stderr}"
            )
    print("Release version/ref validation passed")


if __name__ == "__main__":
    main()
