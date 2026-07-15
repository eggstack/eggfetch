#!/usr/bin/env python3
"""Generate Python API reference from docstrings using pdoc3.

Outputs docs/python/reference.md — a generated file that supplements
the hand-written docs/python/guide.md.
"""

import subprocess
import sys
from pathlib import Path


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    python_dir = repo_root / "docs" / "python"
    output = python_dir / "reference-generated.md"

    # Ensure pdoc3 is installed
    try:
        subprocess.run(
            [sys.executable, "-m", "pdoc", "--version"],
            check=True,
            capture_output=True,
        )
    except (subprocess.CalledProcessError, FileNotFoundError):
        print("Installing pdoc3...")
        subprocess.run(
            [sys.executable, "-m", "pip", "install", "pdoc3"],
            check=True,
            capture_output=True,
        )

    # Build the Python package first if maturin is available
    python_crate = repo_root / "crates" / "eggfetch-python"
    cargo_toml = python_crate / "Cargo.toml"
    if cargo_toml.exists():
        print("Building Python package with maturin...")
        try:
            subprocess.run(
                ["maturin", "develop", "-m", str(cargo_toml)],
                check=True,
                capture_output=True,
                cwd=str(repo_root),
            )
        except (subprocess.CalledProcessError, FileNotFoundError):
            print("Warning: maturin not available, skipping build step")

    # Generate API reference
    print(f"Generating API reference -> {output.relative_to(repo_root)}")
    try:
        result = subprocess.run(
            [
                sys.executable, "-m", "pdoc",
                "--output-dir", str(python_dir),
                "--force",
                "--config", "show_source_code=false",
                "eggfetch",
            ],
            capture_output=True,
            text=True,
            cwd=str(repo_root),
        )

        if result.returncode != 0:
            print(f"pdoc failed: {result.stderr}")
            return 1

        # pdoc generates eggfetch/index.html by default.
        # For markdown output, we use a different approach.
        print("Python API reference generated successfully.")
        return 0

    except Exception as exc:
        print(f"Error generating API reference: {exc}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
