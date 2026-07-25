"""Shared candidate identity schema for qualification artifacts.

All generated JSON artifacts must use this module to produce and validate
candidate identity fields. Schema version 4.

Usage:
    candidate_identity.py generate \\
        --artifact-manifest <manifest.json> \\
        --candidate-sha <sha> \\
        --run-id <id> \\
        --run-attempt <n> \\
        --workflow-run-url <url> \\
        --output <identity.json>

    candidate_identity.py validate <identity.json> \\
        --artifact-manifest <manifest.json> \\
        --expected-sha <sha>
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

SCHEMA_VERSION = "4"

REQUIRED_FIELDS = [
    "schema_version",
    "candidate_sha",
    "artifact_manifest_sha256",
    "eggfetch_version",
    "eggfetch_wheel",
    "httpx_replacement_wheel",
    "reference_httpx_version",
    "producer",
    "started_at",
    "finished_at",
    "run_id",
    "run_attempt",
    "identity_digest",
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


def compute_identity_digest(identity: dict) -> str:
    """Compute identity digest as SHA-256 of canonical JSON excluding identity_digest."""
    subset = {k: v for k, v in identity.items() if k != "identity_digest"}
    canonical = json.dumps(subset, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def compute_manifest_digest(manifest: dict) -> str:
    """Compute SHA-256 digest of canonical manifest JSON."""
    canonical = json.dumps(manifest, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def create_identity(
    candidate_sha: str,
    eggfetch_version: str,
    eggfetch_wheel: dict,
    httpx_replacement_wheel: dict,
    producer: str,
    run_id: str = "",
    run_attempt: str = "",
    started_at: str | None = None,
    finished_at: str | None = None,
    workflow_run_url: str = "",
    artifact_manifest_sha256: str = "",
) -> dict:
    """Create a candidate identity record with computed digest."""
    started = started_at or datetime.now(timezone.utc).isoformat()
    if finished_at:
        finished = finished_at
    else:
        finished = datetime.now(timezone.utc).isoformat()
        dt_started = datetime.fromisoformat(started)
        dt_finished = datetime.fromisoformat(finished)
        if dt_finished <= dt_started:
            from datetime import timedelta
            finished = (dt_started + timedelta(seconds=1)).isoformat()

    identity: dict = {
        "schema_version": SCHEMA_VERSION,
        "candidate_sha": candidate_sha,
        "artifact_manifest_sha256": artifact_manifest_sha256,
        "eggfetch_version": eggfetch_version,
        "eggfetch_wheel": eggfetch_wheel,
        "httpx_replacement_wheel": httpx_replacement_wheel,
        "reference_httpx_version": "0.28.1",
        "producer": producer,
        "run_id": run_id,
        "run_attempt": run_attempt,
        "started_at": started,
        "finished_at": finished,
    }
    if workflow_run_url:
        identity["workflow_run_url"] = workflow_run_url

    identity["identity_digest"] = compute_identity_digest(identity)
    return identity


def validate_identity(identity: dict) -> list[str]:
    """Validate a candidate identity record, returning list of errors."""
    errors = []

    for field in REQUIRED_FIELDS:
        if field not in identity:
            errors.append(f"missing required field: {field}")

    if errors:
        return errors

    if identity["schema_version"] != SCHEMA_VERSION:
        errors.append(f"schema_version must be '{SCHEMA_VERSION}', got '{identity['schema_version']}'")

    if not validate_sha(identity["candidate_sha"]):
        errors.append(f"candidate_sha must be a 40-char hex SHA, got '{identity['candidate_sha']}'")

    manifest_sha = identity.get("artifact_manifest_sha256", "")
    if not isinstance(manifest_sha, str) or len(manifest_sha) != 64:
        errors.append(f"artifact_manifest_sha256 must be a 64-char hex string, got: {manifest_sha!r}")
    elif not all(c in "0123456789abcdef" for c in manifest_sha):
        errors.append(f"artifact_manifest_sha256 must be a 64-char hex string")

    if not isinstance(identity["eggfetch_version"], str) or not identity["eggfetch_version"]:
        errors.append("eggfetch_version must be a non-empty string")

    errors.extend(validate_wheel_record(identity.get("eggfetch_wheel", {})))
    errors.extend(validate_wheel_record(identity.get("httpx_replacement_wheel", {})))

    if identity.get("reference_httpx_version") != "0.28.1":
        errors.append(f"reference_httpx_version must be '0.28.1', got '{identity.get('reference_httpx_version')}'")

    if not isinstance(identity.get("producer"), str) or not identity["producer"]:
        errors.append("producer must be a non-empty string")

    for field in ("run_id", "run_attempt"):
        val = identity.get(field)
        if not isinstance(val, str) or not val:
            errors.append(f"{field} must be a non-empty string, got {val!r}")

    workflow_url = identity.get("workflow_run_url")
    if workflow_url is not None:
        if not isinstance(workflow_url, str) or not workflow_url:
            errors.append(f"workflow_run_url must be a non-empty string when present, got {workflow_url!r}")

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

    digest = identity.get("identity_digest")
    if not digest or not isinstance(digest, str):
        errors.append("identity_digest must be a non-empty string")
    elif len(digest) != 64 or not all(c in "0123456789abcdef" for c in digest):
        errors.append(f"identity_digest must be a 64-char hex string, got: {digest!r}")
    else:
        expected = compute_identity_digest(identity)
        if digest != expected:
            errors.append(
                f"identity_digest mismatch: expected {expected[:16]}..., got {digest[:16]}..."
            )

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


def _generate_from_manifest(
    artifact_manifest_path: Path,
    candidate_sha: str,
    run_id: str,
    run_attempt: str,
    workflow_run_url: str,
    producer: str,
    eggfetch_version: str,
    started_at: str,
    finished_at: str,
) -> dict:
    """Generate identity from an artifact manifest (schema v3)."""
    with open(artifact_manifest_path) as f:
        manifest = json.load(f)

    manifest_sha = compute_manifest_digest(manifest)

    eggfetch_wheel = {}
    httpx_wheel = {}
    for art in manifest.get("artifacts", []):
        # Support schema v3 (role) and schema v2 (artifact_type)
        art_role = art.get("role", art.get("artifact_type", ""))
        art_filename = art.get("filename", art.get("name", ""))
        art_hash = art.get("sha256", "")
        if art_role == "eggfetch":
            eggfetch_wheel = {"filename": art_filename, "sha256": art_hash}
        elif art_role == "httpx-controlled-replacement":
            httpx_wheel = {"filename": art_filename, "sha256": art_hash}

    if not eggfetch_wheel:
        print("FATAL: no eggfetch artifact found in manifest", file=sys.stderr)
        sys.exit(1)
    if not httpx_wheel:
        print("FATAL: no httpx-controlled-replacement artifact found in manifest", file=sys.stderr)
        sys.exit(1)

    return create_identity(
        candidate_sha=candidate_sha,
        eggfetch_version=eggfetch_version,
        eggfetch_wheel=eggfetch_wheel,
        httpx_replacement_wheel=httpx_wheel,
        producer=producer,
        run_id=run_id,
        run_attempt=run_attempt,
        started_at=started_at,
        finished_at=finished_at,
        workflow_run_url=workflow_run_url,
        artifact_manifest_sha256=manifest_sha,
    )


def _cmd_generate(args: argparse.Namespace) -> None:
    """Handle 'generate' subcommand."""
    started_at = datetime.now(timezone.utc).isoformat()

    identity = _generate_from_manifest(
        artifact_manifest_path=Path(args.artifact_manifest),
        candidate_sha=args.candidate_sha,
        run_id=args.run_id,
        run_attempt=args.run_attempt,
        workflow_run_url=args.workflow_run_url,
        producer=args.producer,
        eggfetch_version=args.eggfetch_version or "0.0.0",
        started_at=started_at,
        finished_at=args.finished_at or started_at,
    )

    if not args.finished_at:
        from datetime import timedelta
        dt = datetime.fromisoformat(started_at)
        identity["finished_at"] = (dt + timedelta(seconds=1)).isoformat()
        identity["identity_digest"] = compute_identity_digest(identity)

    errors = validate_identity(identity)
    if errors:
        print("FATAL: generated identity has validation errors:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        sys.exit(1)

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(identity, f, indent=2)
        f.write("\n")

    print(f"Candidate identity written to {output_path}")
    print(f"  candidate_sha: {identity['candidate_sha'][:16]}...")
    print(f"  identity_digest: {identity['identity_digest'][:16]}...")


def _cmd_validate(args: argparse.Namespace) -> None:
    """Handle 'validate' subcommand."""
    all_ok = True
    for path in args.identity_files:
        data, errors = load_and_validate(path)
        if errors:
            print(f"FAIL {path}:")
            for err in errors:
                print(f"  - {err}")
            all_ok = False
        else:
            print(f"OK {path}: candidate={data['candidate_sha'][:12]}")

    if args.artifact_manifest and all_ok:
        with open(args.artifact_manifest) as f:
            manifest = json.load(f)
        manifest_sha = compute_manifest_digest(manifest)

        for path in args.identity_files:
            with open(path) as f:
                data = json.load(f)
            if data.get("artifact_manifest_sha256") != manifest_sha:
                print(f"FAIL {path}: artifact_manifest_sha256 mismatch")
                all_ok = False

    if args.expected_sha:
        for path in args.identity_files:
            with open(path) as f:
                data = json.load(f)
            if data.get("candidate_sha") != args.expected_sha:
                print(f"FAIL {path}: candidate_sha mismatch: expected {args.expected_sha[:12]}, got {data['candidate_sha'][:12]}")
                all_ok = False

    sys.exit(0 if all_ok else 1)


def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser for workflow validation tests."""
    parser = argparse.ArgumentParser(
        prog="candidate_identity.py",
        description="Generate and validate candidate identity records",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    gen = subparsers.add_parser("generate", help="Generate a candidate identity")
    gen.add_argument("--artifact-manifest", required=True, help="Path to artifact-manifest.json")
    gen.add_argument("--candidate-sha", required=True, help="40-char hex SHA")
    gen.add_argument("--run-id", required=True, help="GitHub run ID")
    gen.add_argument("--run-attempt", required=True, help="Run attempt number")
    gen.add_argument("--workflow-run-url", default="", help="Workflow run URL")
    gen.add_argument("--producer", default="qualification/normalize", help="Producer identifier")
    gen.add_argument("--eggfetch-version", default="", help="eggfetch version")
    gen.add_argument("--finished-at", default="", help="Finished timestamp (ISO-8601)")
    gen.add_argument("--output", required=True, help="Output identity JSON path")

    val = subparsers.add_parser("validate", help="Validate candidate identity file(s)")
    val.add_argument("identity_files", nargs="+", help="Path(s) to identity JSON file(s)")
    val.add_argument("--artifact-manifest", default=None, help="Cross-validate against manifest")
    val.add_argument("--expected-sha", default=None, help="Expected candidate SHA")

    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()

    if args.command == "generate":
        _cmd_generate(args)
    elif args.command == "validate":
        _cmd_validate(args)
    else:
        parser.print_help()
        sys.exit(2)


if __name__ == "__main__":
    main()
