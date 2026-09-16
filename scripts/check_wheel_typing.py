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
NEGATIVE_FIXTURES = (
    ROOT / "crates/eggfetch-python/tests/typing/negative_sync_async_body.py",
    ROOT / "crates/eggfetch-python/tests/typing/negative_verify_and_return.py",
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

        surface = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts/check_python_typing_surface.py"),
                "--package",
                str(target / "eggfetch"),
                "--manifest",
                str(ROOT / "crates/eggfetch-python/tests/native_api_manifest.json"),
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        if surface.returncode:
            print(surface.stdout, end="")
            print(surface.stderr, end="", file=sys.stderr)
            return surface.returncode

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

        for fixture in NEGATIVE_FIXTURES:
            negative = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "mypy",
                    "--strict",
                    "--no-site-packages",
                    "--no-incremental",
                    str(fixture),
                ],
                cwd=ROOT,
                env=env,
                capture_output=True,
                text=True,
            )
            if negative.returncode == 0:
                print(f"FAIL: negative wheel typing fixture unexpectedly passed: {fixture}")
                return 1

    print("Installed wheel typing smoke passed (positive and negative fixtures)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
