#!/usr/bin/env python3
"""Type-check representative consumers against an installed wheel.

The wheel is installed into a temporary target directory with dependencies
disabled. Mypy then resolves ``eggfetch`` from that installed artifact, not
from the repository's Python source tree.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = (
    ROOT / "crates/eggfetch-python/tests/typing/consumer.py",
    ROOT / "crates/eggfetch-python/tests/typing/compat_consumer.py",
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--wheel", required=True, type=Path)
    wheel = parser.parse_args().wheel.resolve()
    if not wheel.is_file() or wheel.suffix != ".whl":
        print(f"FAIL: wheel not found: {wheel}", file=sys.stderr)
        return 1

    with tempfile.TemporaryDirectory(prefix="eggfetch-wheel-typing-") as directory:
        target = Path(directory) / "site"
        install = subprocess.run(
            [
                sys.executable,
                "-m",
                "pip",
                "install",
                "--no-deps",
                "--target",
                str(target),
                str(wheel),
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        if install.returncode:
            print(install.stdout, end="")
            print(install.stderr, end="", file=sys.stderr)
            return install.returncode

        env = {**os.environ, "MYPYPATH": str(target)}
        command = [
            sys.executable,
            "-m",
            "mypy",
            "--strict",
            "--no-site-packages",
            "--no-incremental",
            *(str(fixture) for fixture in FIXTURES),
        ]
        result = subprocess.run(command, cwd=ROOT, env=env)
        if result.returncode:
            return result.returncode

    print("Installed wheel typing smoke passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
