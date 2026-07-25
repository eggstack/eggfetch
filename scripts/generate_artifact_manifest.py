#!/usr/bin/env python3
"""Generate an artifact manifest for built wheel artifacts.

Produces a schema-2 artifact-manifest.json that lists all built wheel
artifacts with their SHA-256 hashes, sizes, and candidate identity.

Usage:
    generate_artifact_manifest.py --wheel-dir <dir> --candidate-sha <sha> \\
        --run-id <id> --run-attempt <n> --output <path.json> \\
        [--workflow-name <name>] [--producer-job <job>] \\
        [--bundle-dir <dir>] [--allow-extra-wheels] \\
        [--generate-identity]

The manifest format:
    {
      "schema_version": "2",
      "candidate_sha": "<40-char-hex>",
      "run_id": "<github-run-id>",
      "run_attempt": "<attempt-number>",
      "workflow_name": "<workflow-name>",
      "producer_job": "<producer-job>",
      "generated_at": "<iso-8601>",
      "candidate_identity": { ... },
      "artifacts": [
        {
          "name": "eggfetch-0.x.0-py3-none-any.whl",
          "path": "wheels/eggfetch-0.x.0-py3-none-any.whl",
          "sha256": "<64-char-hex>",
          "size_bytes": 123456,
          "artifact_type": "eggfetch",
          "wheel_distribution_name": "eggfetch",
          "wheel_version": "0.x.0",
          "wheel_tags": "py3-none-any"
        },
        ...
      ]
    }

Exit codes:
    0 — manifest generated successfully
    1 — error (missing directory, no wheels, hash mismatch, etc.)
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path

SCHEMA_VERSION = "2"

# Expected wheel types for default manifest (no --allow-extra-wheels)
_EXPECTED_ARTIFACT_TYPES = {"eggfetch", "httpx-controlled-replacement"}

# Wheel filename pattern: {distribution}-{version}(-{build tag})?-{python tag}-{abi tag}-{platform tag}.whl
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


def classify_artifact(filename: str) -> str:
    """Classify a wheel file by its artifact type."""
    name_lower = filename.lower()
    if name_lower.startswith("eggfetch-"):
        return "eggfetch"
    if name_lower.startswith("httpx-") and "controlled" in name_lower:
        return "httpx-controlled-replacement"
    if name_lower.startswith("httpx-"):
        return "httpx"
    return "other"


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


def _select_single_wheel(wheels: list[Path], label: str) -> Path:
    """Select exactly one wheel from a list. Fail if zero or multiple."""
    if not wheels:
        raise ValueError(f"No {label} wheel found")
    if len(wheels) > 1:
        names = [w.name for w in wheels]
        raise ValueError(f"Multiple {label} wheels found (expected exactly one): {names}")
    return wheels[0]


def generate_manifest(
    wheel_dir: Path,
    candidate_sha: str,
    run_id: str,
    run_attempt: str,
    workflow_name: str = "",
    producer_job: str = "",
    allow_extra_wheels: bool = False,
    candidate_identity_path: Path | None = None,
    bundle_dir: Path | None = None,
) -> dict:
    """Generate the artifact manifest from a wheel directory.

    If bundle_dir is provided, copies exactly the eggfetch and
    httpx-controlled-replacement wheels into bundle_dir/wheels/ and
    uses relative paths in the manifest.
    """
    if not wheel_dir.exists():
        raise FileNotFoundError(f"Wheel directory not found: {wheel_dir}")

    if len(candidate_sha) != 40 or not all(c in "0123456789abcdef" for c in candidate_sha):
        raise ValueError(f"candidate_sha must be a 40-char hex string, got: {candidate_sha!r}")

    wheels = sorted(wheel_dir.glob("*.whl"))
    if not wheels:
        raise ValueError(f"No .whl files found in {wheel_dir}")

    # Classify wheels
    eggfetch_wheels = [w for w in wheels if classify_artifact(w.name) == "eggfetch"]
    httpx_wheels = [w for w in wheels if classify_artifact(w.name) == "httpx-controlled-replacement"]

    if not allow_extra_wheels:
        eggfetch_whl = _select_single_wheel(eggfetch_wheels, "eggfetch")
        httpx_whl = _select_single_wheel(httpx_wheels, "httpx-controlled-replacement")
        selected = [eggfetch_whl, httpx_whl]
    else:
        selected = wheels

    # If bundle_dir is provided, copy wheels and use relative paths
    if bundle_dir:
        wheels_dest = bundle_dir / "wheels"
        wheels_dest.mkdir(parents=True, exist_ok=True)
        for whl in selected:
            dest = wheels_dest / whl.name
            shutil.copy2(whl, dest)

    artifacts = []
    for whl in selected:
        sha256 = compute_sha256(whl)
        artifact_type = classify_artifact(whl.name)
        wheel_info = _parse_wheel_name(whl.name) or {}

        if bundle_dir:
            # Relative path from bundle root
            rel_path = f"wheels/{whl.name}"
            art_path = rel_path
        else:
            art_path = str(whl.resolve())

        artifacts.append({
            "name": whl.name,
            "path": art_path,
            "sha256": sha256,
            "size_bytes": whl.stat().st_size,
            "artifact_type": artifact_type,
            "wheel_distribution_name": wheel_info.get("distribution_name", ""),
            "wheel_version": wheel_info.get("version", ""),
            "wheel_tags": wheel_info.get("tags", ""),
        })

    # Reject unlisted extra wheels unless explicitly allowed
    if not allow_extra_wheels:
        found_types = {a["artifact_type"] for a in artifacts}
        unexpected = found_types - _EXPECTED_ARTIFACT_TYPES
        if unexpected:
            raise ValueError(
                f"Unexpected artifact types found (use --allow-extra-wheels to permit): "
                f"{', '.join(sorted(unexpected))}"
            )

    # Load candidate identity if provided
    candidate_identity = None
    if candidate_identity_path and candidate_identity_path.exists():
        with open(candidate_identity_path) as f:
            candidate_identity = json.load(f)

    manifest = {
        "schema_version": SCHEMA_VERSION,
        "candidate_sha": candidate_sha,
        "run_id": run_id,
        "run_attempt": run_attempt,
        "workflow_name": workflow_name,
        "producer_job": producer_job,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "candidate_identity": candidate_identity,
        "artifacts": artifacts,
    }

    return manifest


def validate_manifest(manifest: dict, base_dir: Path | None = None) -> list[str]:
    """Validate an artifact manifest. Returns list of errors.

    If base_dir is provided, artifact paths are resolved relative to it.
    """
    errors: list[str] = []

    if not isinstance(manifest, dict):
        return ["manifest must be a JSON object"]

    required_fields = ["schema_version", "candidate_sha", "run_id",
                       "run_attempt", "generated_at", "artifacts"]
    for field in required_fields:
        if field not in manifest:
            errors.append(f"missing required field: {field}")

    if errors:
        return errors

    if manifest["schema_version"] != SCHEMA_VERSION:
        errors.append(f"schema_version must be '{SCHEMA_VERSION}', got '{manifest['schema_version']}'")

    sha = manifest["candidate_sha"]
    if len(sha) != 40 or not all(c in "0123456789abcdef" for c in sha):
        errors.append(f"candidate_sha must be a 40-char hex string, got: {sha!r}")

    if not isinstance(manifest["artifacts"], list) or len(manifest["artifacts"]) == 0:
        errors.append("artifacts must be a non-empty list")
        return errors

    # Path traversal protection
    for i, art in enumerate(manifest["artifacts"]):
        if not isinstance(art, dict):
            errors.append(f"artifacts[{i}] must be a JSON object")
            continue
        for field in ("name", "path", "sha256", "size_bytes", "artifact_type"):
            if field not in art:
                errors.append(f"artifacts[{i}].{field} is missing")
        if "sha256" in art:
            if len(art["sha256"]) != 64 or not all(c in "0123456789abcdef" for c in art["sha256"]):
                errors.append(f"artifacts[{i}].sha256 must be a 64-char hex string")
        if "size_bytes" in art:
            if not isinstance(art["size_bytes"], int) or art["size_bytes"] < 0:
                errors.append(f"artifacts[{i}].size_bytes must be a non-negative integer")
        if "path" in art:
            if _has_path_traversal(art["path"]):
                errors.append(f"artifacts[{i}].path contains path traversal: {art['path']}")
            else:
                # Resolve path relative to base_dir if provided
                check_path = (base_dir / art["path"]) if base_dir else Path(art["path"])
                if not check_path.exists():
                    errors.append(f"artifacts[{i}].path does not exist: {art['path']}")
        # Validate wheel metadata fields if present
        for wfield in ("wheel_distribution_name", "wheel_version", "wheel_tags"):
            wval = art.get(wfield)
            if wval is not None and (not isinstance(wval, str) or not wval):
                errors.append(f"artifacts[{i}].{wfield} must be a non-empty string when present")

    # Must have at least eggfetch and httpx artifacts
    types = {a.get("artifact_type") for a in manifest["artifacts"]}
    if "eggfetch" not in types:
        errors.append("manifest must contain an eggfetch artifact")
    if "httpx-controlled-replacement" not in types:
        errors.append("manifest must contain an httpx-controlled-replacement artifact")

    return errors


def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser for workflow validation tests."""
    parser = argparse.ArgumentParser(
        prog="generate_artifact_manifest.py",
        description="Generate artifact manifest for built wheel artifacts",
    )
    parser.add_argument("--wheel-dir", required=True, help="Directory containing .whl files")
    parser.add_argument("--candidate-sha", required=True, help="40-char hex SHA")
    parser.add_argument("--run-id", required=True, help="GitHub run ID")
    parser.add_argument("--run-attempt", required=True, help="Run attempt number")
    parser.add_argument("--output", required=True, help="Output manifest path")
    parser.add_argument("--workflow-name", default="", help="Workflow name")
    parser.add_argument("--producer-job", default="", help="Producer job name")
    parser.add_argument("--bundle-dir", default=None,
                        help="Create candidate bundle at this dir (copies wheels, uses relative paths)")
    parser.add_argument("--allow-extra-wheels", action="store_true",
                        help="Allow artifact types beyond eggfetch and httpx-controlled-replacement")
    parser.add_argument("--candidate-identity", default=None,
                        help="Path to candidate-identity.json to embed in manifest")
    parser.add_argument("--generate-identity", action="store_true",
                        help="Also generate candidate-identity.json alongside the manifest")
    parser.add_argument("--validate-only", action="store_true",
                        help="Only validate an existing manifest")
    parser.add_argument("--manifest", help="Manifest path for --validate-only")
    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()

    if args.validate_only:
        if not args.manifest:
            parser.error("--manifest is required with --validate-only")
        with open(args.manifest) as f:
            manifest = json.load(f)
        base_dir = Path(args.manifest).parent if args.manifest else None
        errors = validate_manifest(manifest, base_dir=base_dir)
        if errors:
            print(f"VALIDATION FAILED ({len(errors)} errors):")
            for e in errors:
                print(f"  - {e}")
            sys.exit(1)
        else:
            print("Artifact manifest validation passed.")
            sys.exit(0)

    wheel_dir = Path(args.wheel_dir).resolve()
    candidate_identity_path = Path(args.candidate_identity) if args.candidate_identity else None
    bundle_dir = Path(args.bundle_dir) if args.bundle_dir else None

    try:
        manifest = generate_manifest(
            wheel_dir, args.candidate_sha, args.run_id, args.run_attempt,
            workflow_name=args.workflow_name,
            producer_job=args.producer_job,
            allow_extra_wheels=args.allow_extra_wheels,
            candidate_identity_path=candidate_identity_path,
            bundle_dir=bundle_dir,
        )
    except (FileNotFoundError, ValueError) as e:
        print(f"FATAL: {e}")
        sys.exit(1)

    # Self-validate
    errors = validate_manifest(manifest, base_dir=bundle_dir)
    if errors:
        print(f"FATAL: manifest has validation errors:")
        for e in errors:
            print(f"  - {e}")
        sys.exit(1)

    # Hash mismatch validation: verify artifact hashes against disk
    for art in manifest["artifacts"]:
        if bundle_dir:
            art_path = bundle_dir / art["path"]
        else:
            art_path = Path(art["path"])
        if art_path.exists():
            actual_sha = compute_sha256(art_path)
            if actual_sha != art["sha256"]:
                print(f"FATAL: hash mismatch for {art['name']}: "
                      f"manifest={art['sha256']}, disk={actual_sha}")
                sys.exit(1)

    # Determine output path
    if bundle_dir:
        output_path = bundle_dir / "artifact-manifest.json"
    else:
        output_path = Path(args.output)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")

    # Generate candidate-identity.json alongside the manifest if requested
    if args.generate_identity:
        identity_path = output_path.parent / "candidate-identity.json"
        # Import from sibling script
        try:
            from candidate_identity import create_identity, validate_identity
            eggfetch_whl = {}
            httpx_whl = {}
            for art in manifest["artifacts"]:
                if art["artifact_type"] == "eggfetch":
                    eggfetch_whl = {"filename": art["name"], "sha256": art["sha256"]}
                elif art["artifact_type"] == "httpx-controlled-replacement":
                    httpx_whl = {"filename": art["name"], "sha256": art["sha256"]}
            identity = create_identity(
                candidate_sha=manifest["candidate_sha"],
                eggfetch_version=eggfetch_whl.get("filename", "").split("-")[1] if eggfetch_whl else "0.0.0",
                eggfetch_wheel=eggfetch_whl,
                httpx_replacement_wheel=httpx_whl,
                producer=args.producer_job or "unknown",
                run_id=manifest["run_id"],
                run_attempt=manifest["run_attempt"],
            )
            id_errors = validate_identity(identity)
            if id_errors:
                print(f"FATAL: generated identity has validation errors:")
                for e in id_errors:
                    print(f"  - {e}")
                sys.exit(1)
        except ImportError:
            # Fallback: basic identity
            identity = {
                "schema_version": "3",
                "candidate_sha": manifest["candidate_sha"],
                "run_id": manifest["run_id"],
                "run_attempt": manifest["run_attempt"],
                "producer": args.producer_job or "unknown",
                "started_at": manifest["generated_at"],
                "finished_at": manifest["generated_at"],
            }
        with open(identity_path, "w") as f:
            json.dump(identity, f, indent=2)
            f.write("\n")
        print(f"Candidate identity written to {identity_path}")

    print(f"Artifact manifest written to {output_path}")
    print(f"  schema_version: {manifest['schema_version']}")
    print(f"  candidate_sha: {manifest['candidate_sha'][:16]}...")
    print(f"  artifacts: {len(manifest['artifacts'])}")
    for art in manifest["artifacts"]:
        print(f"    {art['artifact_type']}: {art['name']} ({art['size_bytes']} bytes)")


if __name__ == "__main__":
    main()
