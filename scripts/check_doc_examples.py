#!/usr/bin/env python3
"""Extract Python code blocks from markdown files and syntax-check them."""

import ast
import re
import sys
from pathlib import Path


def extract_python_blocks(md_path: Path) -> list[tuple[int, str]]:
    """Return (line_number, code) for each ```python block in a markdown file."""
    text = md_path.read_text(encoding="utf-8")
    blocks: list[tuple[int, str]] = []
    lines = text.split("\n")
    i = 0
    while i < len(lines):
        if lines[i].strip().startswith("```python"):
            start = i + 1
            code_lines: list[str] = []
            i += 1
            while i < len(lines) and not lines[i].strip().startswith("```"):
                code_lines.append(lines[i])
                i += 1
            code = "\n".join(code_lines)
            if code.strip():
                blocks.append((start + 1, code))  # 1-indexed
        i += 1
    return blocks


def check_syntax(code: str, filename: str) -> bool:
    """Try to compile the code. Return True if valid."""
    try:
        ast.parse(code, filename=filename)
        return True
    except SyntaxError as exc:
        print(f"  SyntaxError in {filename}:{exc.lineno}: {exc.msg}")
        return False


def main() -> int:
    docs_dir = Path(__file__).resolve().parent.parent / "docs"
    if not docs_dir.is_dir():
        print(f"docs/ directory not found at {docs_dir}")
        return 1

    md_files = sorted(docs_dir.rglob("*.md"))
    if not md_files:
        print("No markdown files found.")
        return 1

    errors = 0
    checked = 0
    for md_file in md_files:
        blocks = extract_python_blocks(md_file)
        for lineno, code in blocks:
            checked += 1
            rel = md_file.relative_to(docs_dir.parent)
            if not check_syntax(code, f"{rel}:{lineno}"):
                errors += 1

    print(f"Checked {checked} Python blocks across {len(md_files)} markdown files.")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
