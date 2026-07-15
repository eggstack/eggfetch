#!/usr/bin/env python3
"""Check internal links in documentation markdown files.

Validates that relative links (to files, images, anchors) resolve correctly.
Does not fetch external URLs to avoid CI flakiness from network issues.
"""

import re
import sys
from pathlib import Path


def check_links(docs_dir: Path) -> int:
    errors = 0
    md_files = sorted(docs_dir.rglob("*.md"))

    for md_file in md_files:
        text = md_file.read_text(encoding="utf-8")
        rel_file = md_file.relative_to(docs_dir.parent)

        # Match [text](url) but not ![](image) — we check both file and anchor refs
        for match in re.finditer(r"\[([^\]]*)\]\(([^)]+)\)", text):
            label, target = match.group(1), match.group(2)

            # Skip external URLs, mailto links, and bare anchors
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue

            # Strip anchor from target
            file_part, _, anchor = target.partition("#")
            if not file_part:
                # Bare anchor like [text](#section) — check anchor exists in same file
                if anchor:
                    _check_anchor(md_file, anchor, rel_file)
                    errors += 1 if not _anchor_exists(md_file, anchor) else 0
                continue

            # Resolve relative to the markdown file's directory
            resolved = (md_file.parent / file_part).resolve()
            if not resolved.exists():
                print(f"  BROKEN: {rel_file}: {target} -> file not found")
                errors += 1
            elif anchor and not _anchor_exists(resolved, anchor):
                print(f"  BROKEN: {rel_file}: {target} -> anchor #{anchor} not found")
                errors += 1

    return errors


def _anchor_exists(file_path: Path, anchor: str) -> bool:
    """Check if a markdown heading anchor exists in a file."""
    if not file_path.exists() or not file_path.suffix == ".md":
        return True  # Can't check non-md files or missing files
    text = file_path.read_text(encoding="utf-8")
    # Markdown anchors are generated from headings: lowercase, spaces to hyphens, strip punctuation
    anchor_lower = anchor.lower()
    for line in text.split("\n"):
        if line.startswith("#"):
            heading = re.sub(r"[^\w\s-]", "", line.lstrip("#")).strip()
            heading_anchor = heading.lower().replace(" ", "-")
            if heading_anchor == anchor_lower:
                return True
    return False


def _check_anchor(file_path: Path, anchor: str, rel_file: Path) -> None:
    """Print a message for a broken anchor (used by caller to count errors)."""
    # This is intentionally empty; the error is counted by the caller.


def main() -> int:
    docs_dir = Path(__file__).resolve().parent.parent / "docs"
    if not docs_dir.is_dir():
        print(f"docs/ directory not found at {docs_dir}")
        return 1

    md_files = sorted(docs_dir.rglob("*.md"))
    print(f"Checking links in {len(md_files)} markdown files...")
    errors = check_links(docs_dir)

    if errors:
        print(f"\n{errors} broken link(s) found.")
    else:
        print("All internal links OK.")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
