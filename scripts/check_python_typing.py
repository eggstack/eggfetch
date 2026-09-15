#!/usr/bin/env python3
"""Run the repository's representative downstream typing fixture."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
fixture = ROOT / "crates/eggfetch-python/tests/typing/consumer.py"
result = subprocess.run(
    [sys.executable, "-m", "mypy", "--strict", "--no-incremental", str(fixture)],
    cwd=ROOT,
    env={**__import__("os").environ, "MYPYPATH": str(ROOT / "crates/eggfetch-python/python")},
)
raise SystemExit(result.returncode)
