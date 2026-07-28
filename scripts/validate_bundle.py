#!/usr/bin/env python3
"""Standalone bundle validator for candidate qualification bundles.

Validates the complete bundle directory structure, all three documents
(manifest, identity, bundle-index), and both wheel hashes in a single pass.

Per plan §1.6: independent bundle validation.

Usage:
    validate_bundle.py --bundle-root <dir> [--expected-sha <sha>]

Exit codes:
    0 — bundle is valid
    1 — validation failure
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))

from candidate_identity import (
    compute_identity_digest,
    compute_manifest_digest,
    validate_identity,
)
from generate_artifact_manifest import (
    compute_sha256,
    validate_bundle_index,
    validate_manifest,
)


def validate_complete_bundle(
    bundle_root: Path,
    expected_sha: str | None = None,
) -> list[str]:
    """Validate the entire bundle directory. Returns list of errors."""
    errors: list[str] = []

    # Check required files exist
    manifest_path = bundle_root / "artifact-manifest.json"
    identity_path = bundle_root / "candidate-identity.json"
    index_path = bundle_root / "bundle-index.json"
    wheels_dir = bundle_root / "wheels"

    for path, name in [
        (manifest_path, "artifact-manifest.json"),
        (identity_path, "candidate-identity.json"),
        (index_path, "bundle-index.json"),
    ]:
        if not path.exists():
            errors.append(f"missing required file: {name}")

    if not wheels_dir.exists():
        errors.append("missing wheels/ directory")

    if errors:
        return errors

    # Load and validate manifest
    with open(manifest_path) as f:
        manifest = json.load(f)

    manifest_errors = validate_manifest(manifest, bundle_root=bundle_root)
    errors.extend([f"manifest: {e}" for e in manifest_errors])

    # Load and validate identity
    with open(identity_path) as f:
        identity = json.load(f)

    identity_errors = validate_identity(identity)
    errors.extend([f"identity: {e}" for e in identity_errors])

    # Cross-validate: identity must reference the correct manifest digest
    expected_manifest_sha = compute_manifest_digest(manifest)
    actual_manifest_sha = identity.get("artifact_manifest_sha256", "")
    if actual_manifest_sha != expected_manifest_sha:
        errors.append(
            f"identity artifact_manifest_sha256 mismatch: "
            f"expected {expected_manifest_sha[:16]}..., got {actual_manifest_sha[:16]}..."
        )

    # Cross-validate: candidate_sha must match between manifest and identity
    if manifest.get("candidate_sha") != identity.get("candidate_sha"):
        errors.append(
            f"candidate_sha mismatch between manifest and identity: "
            f"manifest={manifest.get('candidate_sha', '')[:12]}, "
            f"identity={identity.get('candidate_sha', '')[:12]}"
        )

    # Load and validate bundle-index
    with open(index_path) as f:
        bundle_index = json.load(f)

    index_errors = validate_bundle_index(bundle_index, manifest, bundle_root)
    errors.extend([f"bundle-index: {e}" for e in index_errors])

    # Verify exactly two wheels
    wheel_files = list(wheels_dir.glob("*.whl")) if wheels_dir.exists() else []
    if len(wheel_files) != 2:
        errors.append(f"expected exactly 2 wheels, found {len(wheel_files)}")

    # Verify wheel hashes match manifest
    for art in manifest.get("artifacts", []):
        filename = art.get("filename", "")
        expected_hash = art.get("sha256", "")
        wheel_path = wheels_dir / filename
        if not wheel_path.exists():
            errors.append(f"manifest references missing wheel: {filename}")
            continue
        actual_hash = compute_sha256(wheel_path)
        if actual_hash != expected_hash:
            errors.append(
                f"wheel hash mismatch for {filename}: "
                f"manifest={expected_hash[:16]}..., actual={actual_hash[:16]}..."
            )

    # Verify expected SHA if provided
    if expected_sha:
        if manifest.get("candidate_sha") != expected_sha:
            errors.append(
                f"candidate_sha mismatch: expected {expected_sha[:12]}, "
                f"got {manifest.get('candidate_sha', '')[:12]}"
            )

    return errors


def main() -> None:
    parser = argparse.ArgumentParser(
        prog="validate_bundle.py",
        description="Validate a complete candidate bundle directory",
    )
    parser.add_argument("--bundle-root", required=True, help="Bundle root directory")
    parser.add_argument("--expected-sha", default=None, help="Expected candidate SHA")
    args = parser.parse_args()

    bundle_root = Path(args.bundle_root)
    if not bundle_root.exists():
        print(f"FATAL: bundle root not found: {bundle_root}", file=sys.stderr)
        sys.exit(1)

    errors = validate_complete_bundle(bundle_root, args.expected_sha)

    if errors:
        print(f"BUNDLE VALIDATION FAILED ({len(errors)} errors):", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        sys.exit(1)
    else:
        print("Bundle validation passed.")
        sys.exit(0)


if __name__ == "__main__":
    main()
