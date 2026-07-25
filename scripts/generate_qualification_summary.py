#!/usr/bin/env python3
"""Generate a qualification summary from validated evidence.

Produces a machine-readable summary containing:
- exact candidate SHA, run ID, attempt
- wheel filenames and hashes
- evidence artifacts and digests
- overall_pass
- stage-c status

Usage:
    generate_qualification_summary.py \\
        --evidence <evidence.json> \\
        --artifact-manifest <artifact-manifest.json> \\
        --candidate-identity <candidate-identity.json> \\
        --bundle-index <bundle-index.json> \\
        --run-id <id> \\
        --run-attempt <n> \\
        --run-url <url> \\
        --output <path>
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def _fail(msg: str) -> None:
    print(f"FATAL: {msg}", file=sys.stderr)
    sys.exit(1)


def _load_json(path: Path, label: str) -> dict[str, Any]:
    if not path.exists():
        _fail(f"{label} file not found: {path}")
    try:
        with open(path) as f:
            data = json.load(f)
    except json.JSONDecodeError as exc:
        _fail(f"{label} is not valid JSON: {exc}")
    if not isinstance(data, dict):
        _fail(f"{label} must be a JSON object, got {type(data).__name__}")
    return data


def _validate_sha(sha: str, label: str) -> None:
    if not isinstance(sha, str) or len(sha) != 40:
        _fail(f"{label}: must be a 40-char hex string, got {sha!r}")
    if not all(c in "0123456789abcdef" for c in sha):
        _fail(f"{label}: contains non-hex characters")


def _compute_file_digest(path: Path) -> str:
    """Compute SHA-256 of a file's contents."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def generate_summary(
    evidence_path: Path,
    manifest_path: Path,
    identity_path: Path,
    bundle_index_path: Path,
    run_id: str,
    run_attempt: str,
    run_url: str,
    output_path: Path,
) -> dict[str, Any]:
    """Generate a qualification summary from retained artifacts."""
    evidence = _load_json(evidence_path, "evidence")
    manifest = _load_json(manifest_path, "artifact-manifest")
    identity = _load_json(identity_path, "candidate-identity")
    bundle_index = _load_json(bundle_index_path, "bundle-index")

    candidate_sha = evidence.get("candidate_sha", "")
    _validate_sha(candidate_sha, "evidence candidate_sha")

    # Verify SHA consistency across all documents
    for label, doc in [
        ("manifest", manifest),
        ("identity", identity),
        ("bundle-index", bundle_index),
    ]:
        doc_sha = doc.get("candidate_sha", "")
        if doc_sha and doc_sha != candidate_sha:
            _fail(f"{label}.candidate_sha ({doc_sha}) does not match evidence ({candidate_sha})")

    # Verify identity digest
    identity_digest = identity.get("identity_digest", "")
    evidence_identity = evidence.get("identity_digest", "")
    if evidence_identity and identity_digest and evidence_identity != identity_digest:
        _fail(
            f"identity_digest mismatch: evidence={evidence_identity}, identity={identity_digest}"
        )

    # Extract wheel information from manifest
    wheels = []
    for art in manifest.get("artifacts", []):
        if isinstance(art, dict):
            wheels.append({
                "role": art.get("role", "unknown"),
                "filename": art.get("filename", ""),
                "sha256": art.get("sha256", ""),
                "size_bytes": art.get("size_bytes", 0),
                "relative_path": art.get("relative_path", ""),
            })

    # Compute digests of all documents
    evidence_digest = _compute_file_digest(evidence_path)
    manifest_digest = _compute_file_digest(manifest_path)
    identity_digest_hash = _compute_file_digest(identity_path)
    bundle_index_digest = _compute_file_digest(bundle_index_path)

    summary: dict[str, Any] = {
        "schema_version": "1",
        "candidate_sha": candidate_sha,
        "identity_digest": identity_digest,
        "run_id": run_id,
        "run_attempt": run_attempt,
        "run_url": run_url,
        "eggfetch_version": evidence.get("eggfetch_version", ""),
        "reference_httpx_version": "0.28.1",
        "compatibility_stage": evidence.get("compatibility_stage", "stage-c-candidate"),
        "wheels": wheels,
        "evidence": {
            "path": str(evidence_path),
            "sha256": evidence_digest,
            "overall_pass": evidence.get("overall_pass", False),
            "generated_at": evidence.get("generated_at", ""),
        },
        "artifact_manifest": {
            "path": str(manifest_path),
            "sha256": manifest_digest,
        },
        "candidate_identity": {
            "path": str(identity_path),
            "sha256": identity_digest_hash,
        },
        "bundle_index": {
            "path": str(bundle_index_path),
            "sha256": bundle_index_digest,
        },
        "qualification_pass": evidence.get("overall_pass", False),
        "generated_at": datetime.now(timezone.utc).isoformat(),
    }

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(summary, indent=2) + "\n")
    return summary


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate qualification summary from validated evidence",
    )
    parser.add_argument("--evidence", required=True)
    parser.add_argument("--artifact-manifest", required=True)
    parser.add_argument("--candidate-identity", required=True)
    parser.add_argument("--bundle-index", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--run-attempt", required=True)
    parser.add_argument("--run-url", default="")
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    summary = generate_summary(
        evidence_path=Path(args.evidence),
        manifest_path=Path(args.artifact_manifest),
        identity_path=Path(args.candidate_identity),
        bundle_index_path=Path(args.bundle_index),
        run_id=args.run_id,
        run_attempt=args.run_attempt,
        run_url=args.run_url,
        output_path=Path(args.output),
    )
    print(f"Qualification summary written to {args.output}")
    print(f"  candidate_sha: {summary['candidate_sha']}")
    print(f"  qualification_pass: {summary['qualification_pass']}")


if __name__ == "__main__":
    main()
