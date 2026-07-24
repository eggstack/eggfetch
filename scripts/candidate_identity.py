"""Shared candidate identity schema for qualification artifacts.

All generated JSON artifacts must use this module to produce and validate
candidate identity fields. Schema version 3.
"""

import json
import sys
from datetime import datetime, timezone
from pathlib import Path

SCHEMA_VERSION = "3"

REQUIRED_FIELDS = [
    "schema_version",
    "candidate_sha",
    "eggfetch_version", 
    "eggfetch_wheel",
    "httpx_replacement_wheel",
    "reference_httpx_version",
    "producer",
    "started_at",
    "finished_at",
]


def validate_sha(sha: str) -> bool:
    """Validate a 40-character hex SHA."""
    return isinstance(sha, str) and len(sha) == 40 and all(c in '0123456789abcdef' for c in sha)


def validate_wheel_record(record: dict) -> list[str]:
    """Validate a wheel record, returning list of errors."""
    errors = []
    if not isinstance(record, dict):
        return ["wheel record must be a dict"]
    if "filename" not in record or not record["filename"]:
        errors.append("wheel record missing 'filename'")
    if "sha256" not in record or not record["sha256"]:
        errors.append("wheel record missing 'sha256'")
    elif not isinstance(record["sha256"], str) or len(record["sha256"]) != 64:
        errors.append("wheel sha256 must be a 64-character hex string")
    return errors


def create_identity(
    candidate_sha: str,
    eggfetch_version: str,
    eggfetch_wheel: dict,
    httpx_replacement_wheel: dict,
    producer: str,
    started_at: str | None = None,
    finished_at: str | None = None,
) -> dict:
    """Create a candidate identity record."""
    now = datetime.now(timezone.utc).isoformat()
    return {
        "schema_version": SCHEMA_VERSION,
        "candidate_sha": candidate_sha,
        "eggfetch_version": eggfetch_version,
        "eggfetch_wheel": eggfetch_wheel,
        "httpx_replacement_wheel": httpx_replacement_wheel,
        "reference_httpx_version": "0.28.1",
        "producer": producer,
        "started_at": started_at or now,
        "finished_at": finished_at or now,
    }


def validate_identity(identity: dict) -> list[str]:
    """Validate a candidate identity record, returning list of errors."""
    errors = []
    
    for field in REQUIRED_FIELDS:
        if field not in identity:
            errors.append(f"missing required field: {field}")
    
    if not errors:
        if identity["schema_version"] != SCHEMA_VERSION:
            errors.append(f"schema_version must be '{SCHEMA_VERSION}', got '{identity['schema_version']}'")
        
        if not validate_sha(identity["candidate_sha"]):
            errors.append(f"candidate_sha must be a 40-char hex SHA, got '{identity['candidate_sha']}'")
        
        if not isinstance(identity["eggfetch_version"], str) or not identity["eggfetch_version"]:
            errors.append("eggfetch_version must be a non-empty string")
        
        errors.extend(validate_wheel_record(identity.get("eggfetch_wheel", {})))
        errors.extend(validate_wheel_record(identity.get("httpx_replacement_wheel", {})))
        
        if identity.get("reference_httpx_version") != "0.28.1":
            errors.append(f"reference_httpx_version must be '0.28.1', got '{identity.get('reference_httpx_version')}'")
        
        if not isinstance(identity.get("producer"), str) or not identity["producer"]:
            errors.append("producer must be a non-empty string")
    
    return errors


def load_and_validate(path: str) -> tuple[dict | None, list[str]]:
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
