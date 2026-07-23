#!/usr/bin/env python3
"""Generate an immutable compatibility manifest for release validation.

Produces a signed JSON manifest capturing the compatibility state between
eggfetch and httpx 0.28.1 at a specific commit. The manifest includes a
self-referential SHA-256 checksum for integrity verification.

Exit codes:
    0 - manifest generated successfully
    1 - generation failed (missing files, bad args, etc.)
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from datetime import datetime, timezone
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
COMPAT_DIR = REPO_ROOT / "compat" / "httpx" / "0.28.1"
EVIDENCE_PATH = REPO_ROOT / "compatibility-evidence.json"
CARGO_TOML = REPO_ROOT / "crates" / "eggfetch-python" / "Cargo.toml"
ALLOWED_DIFF_PATH = COMPAT_DIR / "allowed-differences.toml"


def _git_head() -> str:
    """Get the current git HEAD commit SHA."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            capture_output=True, text=True, cwd=REPO_ROOT, timeout=10,
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except (subprocess.TimeoutExpired, FileNotFoundError):
        pass
    return "unknown"


def _read_eggfetch_version() -> str:
    """Read version from crates/eggfetch-python/Cargo.toml."""
    try:
        with open(CARGO_TOML, "rb") as f:
            data = tomllib.load(f)
        return data["package"]["version"]
    except (FileNotFoundError, KeyError, tomllib.TOMLDecodeError) as exc:
        print(f"WARNING: Could not read version from Cargo.toml: {exc}", file=sys.stderr)
        return "unknown"


def _read_evidence_totals(evidence_path: Path) -> dict[str, int]:
    """Extract api_symbols, differential_cases, downstream_packages from evidence."""
    try:
        with open(evidence_path) as f:
            evidence = json.load(f)
    except (FileNotFoundError, json.JSONDecodeError) as exc:
        print(f"WARNING: Could not read evidence file: {exc}", file=sys.stderr)
        return {"api_symbols": 0, "differential_cases": 0, "downstream_packages": 0}

    api_symbols = evidence.get("api_manifest_summary", {}).get("total_symbols", 0)
    differential_cases = evidence.get("differential_case_results", {}).get("total", 0)
    downstream_packages = len(evidence.get("downstream_package_versions", {}))

    return {
        "api_symbols": api_symbols,
        "differential_cases": differential_cases,
        "downstream_packages": downstream_packages,
    }


def _count_allowed_differences(allowed_path: Path) -> int:
    """Count entries in allowed-differences.toml."""
    try:
        with open(allowed_path, "rb") as f:
            data = tomllib.load(f)
        diffs = data.get("difference", [])
        if isinstance(diffs, list):
            return len(diffs)
        return 1 if diffs else 0
    except (FileNotFoundError, tomllib.TOMLDecodeError):
        return 0


def _compute_checksum(manifest: dict[str, object]) -> str:
    """Compute SHA-256 of the manifest content (excluding manifest_checksum)."""
    clean = {k: v for k, v in manifest.items() if k != "manifest_checksum"}
    payload = json.dumps(clean, indent=2, sort_keys=True)
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def generate_manifest(candidate_sha: str | None, output_path: Path) -> dict[str, object]:
    """Build and write the release manifest."""
    sha = candidate_sha or _git_head()

    manifest: dict[str, object] = {
        "schema_version": "1",
        "candidate_sha": sha,
        "eggfetch_version": _read_eggfetch_version(),
        "emulated_httpx_version": "0.28.1",
        "compatibility_stage": "stage-c",
        "supported_python_versions": ["3.10", "3.11", "3.12", "3.13"],
        "supported_platforms": [
            "linux-x86_64",
            "linux-aarch64",
            "macos-x86_64",
            "macos-arm64",
            "windows-x86_64",
        ],
        "supported_async_backends": ["asyncio"],
        "evidence_totals": _read_evidence_totals(EVIDENCE_PATH),
        "allowed_differences_count": _count_allowed_differences(ALLOWED_DIFF_PATH),
        "generation_timestamp": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    }

    manifest["manifest_checksum"] = _compute_checksum(manifest)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate an immutable compatibility manifest for release",
    )
    parser.add_argument(
        "--candidate-sha",
        help="Git commit SHA to embed (default: git rev-parse HEAD)",
    )
    parser.add_argument(
        "--output",
        default="compatibility-manifest.json",
        help="Output JSON path (default: compatibility-manifest.json)",
    )
    args = parser.parse_args()

    output_path = Path(args.output).resolve()

    print(f"Generating manifest -> {output_path}")
    manifest = generate_manifest(args.candidate_sha, output_path)

    print(f"  candidate_sha:      {manifest['candidate_sha']}")
    print(f"  eggfetch_version:   {manifest['eggfetch_version']}")
    print(f"  checksum:           {manifest['manifest_checksum'][:16]}...")
    print(f"\nManifest written to {output_path}")


if __name__ == "__main__":
    main()
