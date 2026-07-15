#!/usr/bin/env python3
"""Generate CLI help output for documentation.

Extracts `--help` from the eggfetch binary and writes it to
docs/cli/help-output.txt as a reference.
"""

import subprocess
import sys
from pathlib import Path


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    cli_dir = repo_root / "docs" / "cli"
    output = cli_dir / "help-output.txt"

    # Try to find the eggfetch binary
    binary = None
    for candidate in [
        repo_root / "target" / "debug" / "eggfetch",
        repo_root / "target" / "release" / "eggfetch",
    ]:
        if candidate.exists():
            binary = candidate
            break

    if binary is None:
        print("eggfetch binary not found — skipping help output generation.")
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

        output.write_text(result.stdout, encoding="utf-8")
        print(f"CLI help output -> {output.relative_to(repo_root)}")
        return 0

    except Exception as exc:
        print(f"Error generating CLI help: {exc}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
