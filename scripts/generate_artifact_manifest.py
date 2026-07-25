#!/usr/bin/env python3
"""Generate or validate an artifact manifest for built wheel artifacts.

Produces a schema-3 artifact-manifest.json that lists all built wheel
artifacts with their SHA-256 hashes, sizes, and role assignments.

Subcommands:
    generate    Create a new artifact manifest from wheel files
    validate    Validate an existing artifact manifest against bundle on disk

Usage (generate):
    generate_artifact_manifest.py generate \\
        --eggfetch-wheel <path> \\
        --httpx-replacement-wheel <path> \\
        --candidate-sha <sha> \\
        --run-id <id> \\
        --run-attempt <n> \\
        --workflow-name Qualification \\
        --producer-job normalize-candidate-artifacts \\
        --bundle-dir <dir> \\
        --output <dir>/artifact-manifest.json

Usage (validate):
    generate_artifact_manifest.py validate \\
        --manifest <dir>/artifact-manifest.json \\
        --bundle-root <dir> \\
        --expected-sha <sha>

Manifest schema (v3):
    {
      "schema_version": "3",
      "candidate_sha": "<40-char-hex>",
      "run_id": "<github-run-id>",
      "run_attempt": "<attempt-number>",
      "workflow_name": "<workflow-name>",
      "producer_job": "<producer-job>",
      "generated_at": "<iso-8601>",
      "artifacts": [
        {
          "role": "eggfetch",
          "distribution": "eggfetch",
          "version": "<version>",
          "filename": "<wheel filename>",
          "relative_path": "wheels/<filename>",
          "sha256": "<64-char-hex>",
          "size_bytes": <int>,
          "tags": "<wheel tags>"
        },
        ...
      ]
    }

Exit codes:
    0 — success
    1 — error (missing files, hash mismatch, validation failure, etc.)
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path

SCHEMA_VERSION = "3"

_WHEEL_NAME_RE = __import__("re").compile(
    r"^(?P<distribution>[A-Za-z0-9](?:[A-Za-z0-9._-]*[A-Za-z0-9])?)"
    r"-(?P<version>[A-Za-z0-9_.!]+)"
    r"(?:-(?P<build>\d[A-Za-z0-9_.]*))?"
    r"-(?P<python>[A-Za-z0-9_.]+)"
    r"-(?P<abi>[A-Za-z0-9_.]+)"
    r"-(?P<platform>[A-Za-z0-9_.]+)"
    r"\.whl$"
)


def compute_sha256(path: Path) -> str:
    """Compute SHA-256 hash of a file."""
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def _parse_wheel_name(filename: str) -> dict[str, str] | None:
    """Parse wheel filename into distribution/version/tags components."""
    m = _WHEEL_NAME_RE.match(filename)
    if not m:
        return None
    tags = f"{m.group('python')}-{m.group('abi')}-{m.group('platform')}"
    return {
        "distribution_name": m.group("distribution"),
        "version": m.group("version"),
        "tags": tags,
    }


def _has_path_traversal(path_str: str) -> bool:
    """Check if a path contains traversal sequences."""
    return ".." in path_str or path_str.startswith("/")


def _build_artifact_entry(
    wheel_path: Path,
    role: str,
    bundle_dir: Path | None,
) -> dict:
    """Build a single artifact entry for a wheel file."""
    wheel_info = _parse_wheel_name(wheel_path.name) or {}
    sha256 = compute_sha256(wheel_path)
    size_bytes = wheel_path.stat().st_size

    if bundle_dir:
        rel_path = f"wheels/{wheel_path.name}"
    else:
        rel_path = str(wheel_path.resolve())

    return {
        "role": role,
        "distribution": wheel_info.get("distribution_name", ""),
        "version": wheel_info.get("version", ""),
        "filename": wheel_path.name,
        "relative_path": rel_path,
        "sha256": sha256,
        "size_bytes": size_bytes,
        "tags": wheel_info.get("tags", ""),
    }


def _cmd_generate(args: argparse.Namespace) -> None:
    """Handle 'generate' subcommand."""
    eggfetch_path = Path(args.eggfetch_wheel).resolve()
    httpx_path = Path(args.httpx_replacement_wheel).resolve()
    bundle_dir = Path(args.bundle_dir) if args.bundle_dir else None

    # Validate inputs
    if not eggfetch_path.exists():
        print(f"FATAL: eggfetch wheel not found: {eggfetch_path}", file=sys.stderr)
        sys.exit(1)
    if not httpx_path.exists():
        print(f"FATAL: httpx replacement wheel not found: {httpx_path}", file=sys.stderr)
        sys.exit(1)
    if not eggfetch_path.name.endswith(".whl"):
        print(f"FATAL: eggfetch wheel is not a .whl file: {eggfetch_path.name}", file=sys.stderr)
        sys.exit(1)
    if not httpx_path.name.endswith(".whl"):
        print(f"FATAL: httpx replacement wheel is not a .whl file: {httpx_path.name}", file=sys.stderr)
        sys.exit(1)

    candidate_sha = args.candidate_sha
    if len(candidate_sha) != 40 or not all(c in "0123456789abcdef" for c in candidate_sha):
        print(f"FATAL: candidate_sha must be a 40-char hex string, got: {candidate_sha!r}", file=sys.stderr)
        sys.exit(1)

    # Copy wheels into bundle if requested
    if bundle_dir:
        wheels_dest = bundle_dir / "wheels"
        wheels_dest.mkdir(parents=True, exist_ok=True)
        shutil.copy2(eggfetch_path, wheels_dest / eggfetch_path.name)
        shutil.copy2(httpx_path, wheels_dest / httpx_path.name)

    # Build artifact entries with explicit roles
    eggfetch_art = _build_artifact_entry(eggfetch_path, "eggfetch", bundle_dir)
    httpx_art = _build_artifact_entry(httpx_path, "httpx-controlled-replacement", bundle_dir)

    manifest = {
        "schema_version": SCHEMA_VERSION,
        "candidate_sha": candidate_sha,
        "run_id": args.run_id,
        "run_attempt": args.run_attempt,
        "workflow_name": args.workflow_name or "",
        "producer_job": args.producer_job or "",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "artifacts": [eggfetch_art, httpx_art],
    }

    # Determine output path
    if bundle_dir:
        output_path = bundle_dir / "artifact-manifest.json"
    else:
        output_path = Path(args.output)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")

    # Self-validate
    base_dir = bundle_dir if bundle_dir else output_path.parent
    errors = validate_manifest(manifest, bundle_root=base_dir)
    if errors:
        print(f"FATAL: generated manifest has validation errors:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        sys.exit(1)

    print(f"Artifact manifest written to {output_path}")
    print(f"  schema_version: {manifest['schema_version']}")
    print(f"  candidate_sha: {candidate_sha[:16]}...")
    print(f"  artifacts: {len(manifest['artifacts'])}")
    for art in manifest["artifacts"]:
        print(f"    {art['role']}: {art['filename']} ({art['size_bytes']} bytes)")


def _cmd_validate(args: argparse.Namespace) -> None:
    """Handle 'validate' subcommand."""
    manifest_path = Path(args.manifest)
    if not manifest_path.exists():
        print(f"FATAL: manifest not found: {manifest_path}", file=sys.stderr)
        sys.exit(1)

    with open(manifest_path) as f:
        manifest = json.load(f)

    bundle_root = Path(args.bundle_root) if args.bundle_root else manifest_path.parent

    errors = validate_manifest(manifest, bundle_root=bundle_root)

    if args.expected_sha:
        manifest_sha = manifest.get("candidate_sha", "")
        if manifest_sha != args.expected_sha:
            errors.append(
                f"candidate_sha mismatch: expected {args.expected_sha[:12]}, "
                f"got {manifest_sha[:12]}"
            )

    if errors:
        print(f"VALIDATION FAILED ({len(errors)} errors):", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        sys.exit(1)
    else:
        print("Artifact manifest validation passed.")
        sys.exit(0)


def validate_manifest(manifest: dict, bundle_root: Path | None = None) -> list[str]:
    """Validate an artifact manifest. Returns list of errors.

    If bundle_root is provided, artifact paths are resolved relative to it
    and file existence, sizes, and hashes are verified against disk.
    """
    errors: list[str] = []

    if not isinstance(manifest, dict):
        return ["manifest must be a JSON object"]

    required_fields = [
        "schema_version", "candidate_sha", "run_id",
        "run_attempt", "generated_at", "artifacts",
    ]
    for field in required_fields:
        if field not in manifest:
            errors.append(f"missing required field: {field}")

    if errors:
        return errors

    if manifest["schema_version"] != SCHEMA_VERSION:
        errors.append(
            f"schema_version must be '{SCHEMA_VERSION}', "
            f"got '{manifest['schema_version']}'"
        )

    sha = manifest["candidate_sha"]
    if len(sha) != 40 or not all(c in "0123456789abcdef" for c in sha):
        errors.append(f"candidate_sha must be a 40-char hex string, got: {sha!r}")

    artifacts = manifest.get("artifacts", [])
    if not isinstance(artifacts, list) or len(artifacts) == 0:
        errors.append("artifacts must be a non-empty list")
        return errors

    # Validate each artifact entry
    roles_seen: set[str] = set()
    for i, art in enumerate(artifacts):
        if not isinstance(art, dict):
            errors.append(f"artifacts[{i}] must be a JSON object")
            continue

        for field in ("role", "distribution", "version", "filename", "relative_path", "sha256", "size_bytes"):
            if field not in art:
                errors.append(f"artifacts[{i}].{field} is missing")

        role = art.get("role", "")
        if role:
            if role in roles_seen:
                errors.append(f"duplicate artifact role: {role}")
            roles_seen.add(role)

        sha256 = art.get("sha256", "")
        if sha256:
            if len(sha256) != 64 or not all(c in "0123456789abcdef" for c in sha256):
                errors.append(f"artifacts[{i}].sha256 must be a 64-char hex string")

        size_bytes = art.get("size_bytes")
        if size_bytes is not None:
            if not isinstance(size_bytes, int) or size_bytes < 0:
                errors.append(f"artifacts[{i}].size_bytes must be a non-negative integer")

        rel_path = art.get("relative_path", "")
        if rel_path:
            if _has_path_traversal(rel_path):
                errors.append(f"artifacts[{i}].relative_path contains path traversal: {rel_path}")
            elif bundle_root:
                full_path = bundle_root / rel_path
                if not full_path.exists():
                    errors.append(f"artifacts[{i}].relative_path does not exist: {rel_path}")
                else:
                    # Verify hash and size
                    actual_sha = compute_sha256(full_path)
                    if sha256 and actual_sha != sha256:
                        errors.append(
                            f"artifacts[{i}].sha256 mismatch: "
                            f"manifest={sha256[:16]}..., disk={actual_sha[:16]}..."
                        )
                    actual_size = full_path.stat().st_size
                    if size_bytes is not None and actual_size != size_bytes:
                        errors.append(
                            f"artifacts[{i}].size_bytes mismatch: "
                            f"manifest={size_bytes}, disk={actual_size}"
                        )

    # Must have exactly eggfetch and httpx-controlled-replacement roles
    if "eggfetch" not in roles_seen:
        errors.append("manifest must contain an artifact with role 'eggfetch'")
    if "httpx-controlled-replacement" not in roles_seen:
        errors.append("manifest must contain an artifact with role 'httpx-controlled-replacement'")

    # Exactly two artifacts for standard bundle
    if len(artifacts) != 2:
        errors.append(f"manifest must contain exactly 2 artifacts, got {len(artifacts)}")

    return errors


def compute_manifest_digest(manifest: dict) -> str:
    """Compute SHA-256 digest of canonical manifest JSON."""
    canonical = json.dumps(manifest, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser with subcommands."""
    parser = argparse.ArgumentParser(
        prog="generate_artifact_manifest.py",
        description="Generate or validate artifact manifest for built wheel artifacts",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    # generate subcommand
    gen = subparsers.add_parser("generate", help="Generate a new artifact manifest")
    gen.add_argument("--eggfetch-wheel", required=True, help="Path to eggfetch .whl file")
    gen.add_argument("--httpx-replacement-wheel", required=True, help="Path to httpx replacement .whl file")
    gen.add_argument("--candidate-sha", required=True, help="40-char hex SHA")
    gen.add_argument("--run-id", required=True, help="GitHub run ID")
    gen.add_argument("--run-attempt", required=True, help="Run attempt number")
    gen.add_argument("--output", required=True, help="Output manifest path")
    gen.add_argument("--workflow-name", default="", help="Workflow name")
    gen.add_argument("--producer-job", default="", help="Producer job name")
    gen.add_argument("--bundle-dir", default=None,
                     help="Create candidate bundle at this dir (copies wheels, uses relative paths)")

    # validate subcommand
    val = subparsers.add_parser("validate", help="Validate an existing artifact manifest")
    val.add_argument("--manifest", required=True, help="Manifest path to validate")
    val.add_argument("--bundle-root", default=None,
                     help="Bundle root directory for resolving relative paths (default: manifest directory)")
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
