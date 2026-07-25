"""Shared candidate identity schema for qualification artifacts.

All generated JSON artifacts must use this module to produce and validate
candidate identity fields. Schema version 4.

The identity binds:
  - candidate SHA;
  - workflow run ID and attempt;
  - workflow URL;
  - eggfetch version;
  - eggfetch wheel filename and SHA-256;
  - replacement ``httpx`` wheel filename and SHA-256;
  - reference HTTPX version;
  - artifact-manifest SHA-256;
  - producer schema version.

A canonical ``identity_digest`` is computed as
``sha256(canonical_identity_without_digest)`` and stored in the final
identity object.  When reading an identity the digest is recomputed and
compared to detect tampering.
"""

from __future__ import annotations

import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

SCHEMA_VERSION = "4"

REQUIRED_FIELDS = [
    "schema_version",
    "candidate_sha",
    "eggfetch_version",
    "reference_httpx_version",
    "artifact_manifest_sha256",
    "eggfetch_wheel",
    "httpx_replacement_wheel",
    "run_id",
    "run_attempt",
    "workflow_run_url",
    "producer",
    "started_at",
    "finished_at",
    "identity_digest",
]


def validate_sha(sha: str) -> bool:
    """Validate a 40-character hex SHA."""
    return isinstance(sha, str) and len(sha) == 40 and all(c in '0123456789abcdef' for c in sha)


def validate_sha256(digest: str) -> bool:
    """Validate a 64-character hex SHA-256 digest."""
    return isinstance(digest, str) and len(digest) == 64 and all(c in '0123456789abcdef' for c in digest)


def validate_wheel_record(record: dict) -> list[str]:
    """Validate a wheel record, returning list of errors."""
    errors: list[str] = []
    if not isinstance(record, dict):
        return ["wheel record must be a dict"]
    filename = record.get("filename")
    if not filename or not isinstance(filename, str):
        errors.append("wheel record missing 'filename'")
    sha = record.get("sha256")
    if not sha:
        errors.append("wheel record missing 'sha256'")
    elif not validate_sha256(sha):
        errors.append("wheel sha256 must be a 64-character hex string")
    return errors


def _canonical_json(obj: dict) -> bytes:
    """Serialize *obj* to canonical JSON (sorted keys, stable separators)."""
    return json.dumps(obj, sort_keys=True, separators=(",", ":")).encode("utf-8")


def compute_identity_digest(identity_without_digest: dict) -> str:
    """Compute the identity digest from the identity dict (without identity_digest)."""
    canonical = _canonical_json(identity_without_digest)
    return hashlib.sha256(canonical).hexdigest()


def load_artifact_manifest(path: str | Path) -> dict:
    """Load and validate an artifact manifest, returning the parsed dict.

    Raises ``ValueError`` if the manifest is invalid.
    """
    p = Path(path)
    if not p.exists():
        raise FileNotFoundError(f"Artifact manifest not found: {p}")
    try:
        with open(p) as f:
            manifest = json.load(f)
    except json.JSONDecodeError as exc:
        raise ValueError(f"Artifact manifest is not valid JSON: {exc}") from exc

    errors = validate_manifest(manifest)
    if errors:
        raise ValueError(f"Artifact manifest validation failed: {'; '.join(errors)}")
    return manifest


def validate_manifest(manifest: dict) -> list[str]:
    """Validate an artifact manifest. Returns list of errors."""
    errors: list[str] = []
    if not isinstance(manifest, dict):
        return ["manifest must be a JSON object"]

    if manifest.get("schema_version") != "1":
        errors.append(f"manifest schema_version must be '1', got '{manifest.get('schema_version')}'")

    sha = manifest.get("candidate_sha", "")
    if not validate_sha(sha):
        errors.append(f"manifest candidate_sha must be a 40-char hex SHA, got: {sha!r}")

    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        errors.append("manifest artifacts must be a non-empty list")
        return errors

    eggfetch_art = None
    httpx_art = None
    for i, art in enumerate(artifacts):
        if not isinstance(art, dict):
            errors.append(f"artifacts[{i}] must be a JSON object")
            continue
        art_type = art.get("artifact_type", "")
        if art_type == "eggfetch":
            eggfetch_art = art
        elif art_type == "httpx-controlled-replacement":
            httpx_art = art
        # Validate path traversal
        path = art.get("path", "")
        if not isinstance(path, str) or not path:
            errors.append(f"artifacts[{i}].path is missing or empty")
        elif ".." in path:
            errors.append(f"artifacts[{i}].path contains path traversal: {path}")

    if eggfetch_art is None:
        errors.append("manifest must contain an eggfetch artifact")
    if httpx_art is None:
        errors.append("manifest must contain an httpx-controlled-replacement artifact")

    return errors


def _extract_wheel_record(artifact: dict) -> dict:
    """Extract a minimal wheel record (filename + sha256) from a manifest artifact."""
    return {
        "filename": artifact.get("filename", ""),
        "sha256": artifact.get("sha256", ""),
    }


def create_identity(
    candidate_sha: str,
    eggfetch_version: str,
    artifact_manifest_path: str | Path,
    run_id: str,
    run_attempt: str,
    workflow_run_url: str,
    producer: str = "candidate_identity.py",
    started_at: str | None = None,
    finished_at: str | None = None,
) -> dict:
    """Create a candidate identity record (schema v4).

    Derives wheel records from the artifact manifest rather than accepting
    caller-provided wheel values.  Computes and stores ``identity_digest``.
    """
    if not validate_sha(candidate_sha):
        raise ValueError(f"candidate_sha must be a 40-char hex SHA, got: {candidate_sha!r}")
    if not eggfetch_version:
        raise ValueError("eggfetch_version must be a non-empty string")
    if not run_id:
        raise ValueError("run_id must be a non-empty string")
    if not run_attempt:
        raise ValueError("run_attempt must be a non-empty string")
    if not workflow_run_url:
        raise ValueError("workflow_run_url must be a non-empty string")
    if not producer:
        raise ValueError("producer must be a non-empty string")

    manifest = load_artifact_manifest(artifact_manifest_path)

    # Verify manifest candidate_sha matches
    if manifest.get("candidate_sha") != candidate_sha:
        raise ValueError(
            f"artifact manifest candidate_sha ({manifest.get('candidate_sha')}) "
            f"does not match requested candidate_sha ({candidate_sha})"
        )

    # Derive wheel records from the manifest
    eggfetch_art = None
    httpx_art = None
    for art in manifest.get("artifacts", []):
        if art.get("artifact_type") == "eggfetch":
            eggfetch_art = art
        elif art.get("artifact_type") == "httpx-controlled-replacement":
            httpx_art = art

    if eggfetch_art is None or httpx_art is None:
        raise ValueError("artifact manifest must contain both eggfetch and httpx-controlled-replacement artifacts")

    eggfetch_wheel = _extract_wheel_record(eggfetch_art)
    httpx_wheel = _extract_wheel_record(httpx_art)

    if not eggfetch_wheel["filename"] or not eggfetch_wheel["sha256"]:
        raise ValueError("eggfetch wheel record missing filename or sha256")
    if not httpx_wheel["filename"] or not httpx_wheel["sha256"]:
        raise ValueError("httpx replacement wheel record missing filename or sha256")

    # Compute manifest SHA-256
    manifest_canonical = _canonical_json(manifest)
    manifest_sha256 = hashlib.sha256(manifest_canonical).hexdigest()

    now = datetime.now(timezone.utc).isoformat()
    started = started_at or now
    finished = finished_at or now

    # Build identity without identity_digest for digest computation
    identity_without_digest: dict = {
        "schema_version": SCHEMA_VERSION,
        "candidate_sha": candidate_sha,
        "eggfetch_version": eggfetch_version,
        "reference_httpx_version": "0.28.1",
        "artifact_manifest_sha256": manifest_sha256,
        "eggfetch_wheel": eggfetch_wheel,
        "httpx_replacement_wheel": httpx_wheel,
        "run_id": run_id,
        "run_attempt": run_attempt,
        "workflow_run_url": workflow_run_url,
        "producer": producer,
        "started_at": started,
        "finished_at": finished,
    }

    identity_digest = compute_identity_digest(identity_without_digest)
    identity_without_digest["identity_digest"] = identity_digest

    return identity_without_digest


def validate_identity(identity: dict) -> list[str]:
    """Validate a candidate identity record, returning list of errors."""
    errors: list[str] = []

    if not isinstance(identity, dict):
        return ["identity must be a JSON object"]

    for field in REQUIRED_FIELDS:
        if field not in identity:
            errors.append(f"missing required field: {field}")

    if errors:
        return errors

    if identity["schema_version"] != SCHEMA_VERSION:
        errors.append(f"schema_version must be '{SCHEMA_VERSION}', got '{identity['schema_version']}'")

    if not validate_sha(identity["candidate_sha"]):
        errors.append(f"candidate_sha must be a 40-char hex SHA, got '{identity['candidate_sha']}'")

    if not isinstance(identity["eggfetch_version"], str) or not identity["eggfetch_version"]:
        errors.append("eggfetch_version must be a non-empty string")

    if identity.get("reference_httpx_version") != "0.28.1":
        errors.append(f"reference_httpx_version must be '0.28.1', got '{identity.get('reference_httpx_version')}'")

    if not validate_sha256(identity.get("artifact_manifest_sha256", "")):
        errors.append("artifact_manifest_sha256 must be a 64-char hex string")

    errors.extend(validate_wheel_record(identity.get("eggfetch_wheel", {})))
    errors.extend(validate_wheel_record(identity.get("httpx_replacement_wheel", {})))

    if not isinstance(identity.get("producer"), str) or not identity["producer"]:
        errors.append("producer must be a non-empty string")

    for field in ("run_id", "run_attempt", "workflow_run_url"):
        val = identity.get(field)
        if not isinstance(val, str) or not val:
            errors.append(f"{field} must be a non-empty string, got {val!r}")

    # Validate identity_digest
    stored_digest = identity.get("identity_digest", "")
    if not validate_sha256(stored_digest):
        errors.append("identity_digest must be a 64-char hex string")
    else:
        # Recompute and compare
        identity_copy = {k: v for k, v in identity.items() if k != "identity_digest"}
        recomputed = compute_identity_digest(identity_copy)
        if recomputed != stored_digest:
            errors.append(
                f"identity_digest mismatch: stored={stored_digest}, recomputed={recomputed}"
            )

    # Validate timestamp order: started_at < finished_at
    started = identity.get("started_at")
    finished = identity.get("finished_at")
    if started and finished and isinstance(started, str) and isinstance(finished, str):
        try:
            dt_started = datetime.fromisoformat(started)
            dt_finished = datetime.fromisoformat(finished)
            if dt_started >= dt_finished:
                errors.append(
                    f"started_at ({started}) must be before finished_at ({finished})"
                )
        except ValueError:
            errors.append(f"invalid timestamp format: started_at={started!r}, finished_at={finished!r}")

    return errors


def load_and_validate(path: str | Path) -> tuple[dict | None, list[str]]:
    """Load and validate an identity file. Returns (identity, errors)."""
    try:
        with open(path) as f:
            data = json.load(f)
    except (json.JSONDecodeError, FileNotFoundError) as e:
        return None, [f"failed to load {path}: {e}"]

    errors = validate_identity(data)
    return data, errors


def main():
    """CLI entry point for validation."""
    if len(sys.argv) < 2:
        print("Usage: candidate_identity.py <identity.json> [...]", file=sys.stderr)
        sys.exit(2)

    all_ok = True
    for path in sys.argv[1:]:
        data, errors = load_and_validate(path)
        if errors:
            print(f"FAIL {path}:")
            for err in errors:
                print(f"  - {err}")
            all_ok = False
        else:
            print(f"OK {path}: candidate={data['candidate_sha'][:12]}")

    sys.exit(0 if all_ok else 1)


if __name__ == "__main__":
    main()
