#!/usr/bin/env python3
"""Run positive and negative representative downstream typing fixtures."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
env = {**__import__("os").environ, "MYPYPATH": str(ROOT / "crates/eggfetch-python/python")}


def run(fixture: Path, *, expected_success: bool) -> int:
    result = subprocess.run(
        [sys.executable, "-m", "mypy", "--strict", "--no-incremental", str(fixture)],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
    )
    if expected_success and result.returncode != 0:
        print(result.stdout, end="")
        print(result.stderr, end="", file=sys.stderr)
        return result.returncode
    if not expected_success and result.returncode == 0:
        print(f"FAIL: negative typing fixture unexpectedly passed: {fixture}")
        return 1
    print(f"Typing fixture passed ({fixture.name})")
    return 0


for path, expected_success in (
    (ROOT / "crates/eggfetch-python/tests/typing/consumer.py", True),
    (ROOT / "crates/eggfetch-python/tests/typing/compat_consumer.py", True),
    (ROOT / "crates/eggfetch-python/tests/typing/negative_sync_async_body.py", False),
):
    code = run(path, expected_success=expected_success)
    if code:
        raise SystemExit(code)
