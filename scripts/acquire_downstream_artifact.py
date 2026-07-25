#!/usr/bin/env python3
"""Download and verify exact downstream package artifacts.

Downloads a specific version of a package from PyPI, verifies its SHA-256
hash, and places it in a local directory for exact installation.

Usage:
    acquire_downstream_artifact.py \\
        --package <name> \\
        --version <version> \\
        --expected-sha256 <hash> \\
        --output-dir <dir>

Exit codes:
    0 — artifact downloaded and verified
    1 — error (hash mismatch, download failure, etc.)
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import urllib.request
from pathlib import Path


def _compute_sha256(data: bytes) -> str:
    """Compute SHA-256 of bytes."""
    return hashlib.sha256(data).hexdigest()


def _find_wheel_url(package: str, version: str) -> tuple[str, str]:
    """Find the wheel download URL and filename from PyPI JSON API."""
    url = f"https://pypi.org/pypi/{package}/{version}/json"
    try:
        with urllib.request.urlopen(url, timeout=30) as resp:
            data = json.loads(resp.read())
    except Exception as e:
        print(f"FATAL: failed to query PyPI for {package}=={version}: {e}", file=sys.stderr)
        sys.exit(1)

    releases = data.get("releases", {}).get(version, [])
    if not releases:
        # Try from urls field
        releases = data.get("urls", [])

    # Find a wheel (prefer py3-none-any)
    wheels = [r for r in releases if r.get("filename", "").endswith(".whl")]
    if not wheels:
        print(f"FATAL: no wheel found for {package}=={version}", file=sys.stderr)
        sys.exit(1)

    # Prefer py3-none-any, then any wheel
    py3_none = [w for w in wheels if "py3-none-any" in w.get("filename", "")]
    chosen = py3_none[0] if py3_none else wheels[0]

    return chosen["url"], chosen["filename"]


def acquire_artifact(
    package: str,
    version: str,
    expected_sha256: str,
    output_dir: Path,
) -> tuple[Path, str]:
    """Download and verify a downstream artifact.

    Returns (artifact_path, actual_sha256).
    """
    output_dir.mkdir(parents=True, exist_ok=True)

    download_url, filename = _find_wheel_url(package, version)
    output_path = output_dir / filename

    print(f"Downloading {filename} from {download_url[:80]}...")
    try:
        with urllib.request.urlopen(download_url, timeout=60) as resp:
            data = resp.read()
    except Exception as e:
        print(f"FATAL: download failed: {e}", file=sys.stderr)
        sys.exit(1)

    actual_sha256 = _compute_sha256(data)

    if actual_sha256 != expected_sha256:
        print(
            f"FATAL: SHA-256 mismatch for {filename}:\n"
            f"  expected: {expected_sha256}\n"
            f"  actual:   {actual_sha256}",
            file=sys.stderr,
        )
        sys.exit(1)

    output_path.write_bytes(data)
    print(f"Verified: {filename} ({len(data)} bytes, sha256={actual_sha256[:16]}...)")
    return output_path, actual_sha256


def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser for workflow validation tests."""
    parser = argparse.ArgumentParser(
        prog="acquire_downstream_artifact.py",
        description="Download and verify exact downstream package artifacts",
    )
    parser.add_argument("--package", required=True, help="Package name")
    parser.add_argument("--version", required=True, help="Exact version")
    parser.add_argument("--expected-sha256", required=True, help="Expected SHA-256 hash")
    parser.add_argument("--output-dir", required=True, help="Output directory for artifact")
    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()

    if len(args.expected_sha256) != 64:
        print(f"FATAL: --expected-sha256 must be 64 hex chars", file=sys.stderr)
        sys.exit(1)

    artifact_path, actual_hash = acquire_artifact(
        package=args.package,
        version=args.version,
        expected_sha256=args.expected_sha256,
        output_dir=Path(args.output_dir),
    )

    print(f"Artifact ready: {artifact_path}")


if __name__ == "__main__":
    main()
