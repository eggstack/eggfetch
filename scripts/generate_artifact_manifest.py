#!/usr/bin/env python3
"""Generate an artifact manifest for built wheel artifacts.

Produces a schema-1 artifact-manifest.json that lists all built wheel
artifacts with their SHA-256 hashes, sizes, and candidate identity.

Usage:
    generate_artifact_manifest.py --eggfetch-wheel-dir <dir> \
        --httpx-wheel-dir <dir> \
        --candidate-sha <sha> \
        --output-dir <dir> \
        [--run-id <id>] [--run-attempt <n>] \
        [--workflow-name <name>] [--producer-job <job>]

The manifest format:
    {
      "schema_version": "1",
      "candidate_sha": "<40-char-hex>",
      "producer": "generate_artifact_manifest.py",
      "artifacts": [
        {
          "artifact_type": "eggfetch",
          "distribution": "eggfetch",
          "version": "<version>",
          "filename": "<filename>",
          "path": "<canonical path>",
          "sha256": "<64-char-hex>",
          "size_bytes": 123
        },
        {
          "artifact_type": "httpx-controlled-replacement",
          "distribution": "httpx",
          "version": "0.28.1",
          "filename": "<filename>",
          "path": "<canonical path>",
          "sha256": "<64-char-hex>",
          "size_bytes": 123
        }
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
import re
import shutil
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

SCHEMA_VERSION = "1"

# Wheel filename pattern: {distribution}-{version}(-{build tag})?-{python tag}-{abi tag}-{platform tag}.whl
_WHEEL_NAME_RE = re.compile(
    r"^(?P<distribution>[A-Za-z0-9](?:[A-Za-z0-9._-]*[A-Za-z0-9])?)"
    r"-(?P<version>[A-Za-z0-9_.!]+)"
    r"(?:-(?P<build>\d[A-Za-z0-9_.]*))?"
    r"-(?P<python>[A-Za-z0-9_.]+)"
    r"-(?P<abi>[A-Za-z0-9_.]+)"
    r"-(?P<platform>[A-Za-z0-9_.]+)"
    r"\.whl$"
)

# Shim marker files that must exist inside the controlled replacement wheel
_SHIM_MARKER_PATHS = (
    "eggfetch/compat/httpx/__init__.py",
    "eggfetch/compat/httpx/_client.py",
)


def compute_sha256(path: Path) -> str:
    """Compute SHA-256 hash of a file by streaming."""
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
    return {
        "distribution_name": m.group("distribution"),
        "version": m.group("version"),
    }


def _has_path_traversal(path_str: str) -> bool:
    """Check if a path contains traversal sequences."""
    return ".." in path_str


def _find_single_wheel(wheel_dir: Path, expected_prefix: str) -> Path:
    """Find exactly one wheel in *wheel_dir* whose name starts with *expected_prefix*.

    Raises ``ValueError`` if zero or more than one match is found.
    """
    if not wheel_dir.exists():
        raise FileNotFoundError(f"Wheel directory not found: {wheel_dir}")

    wheels = sorted(wheel_dir.glob("*.whl"))
    matches = [w for w in wheels if w.name.lower().startswith(expected_prefix)]

    if not matches:
        raise ValueError(
            f"No wheel starting with '{expected_prefix}' found in {wheel_dir}. "
            f"Available wheels: {[w.name for w in wheels]}"
        )
    if len(matches) > 1:
        raise ValueError(
            f"Multiple wheels starting with '{expected_prefix}' found in {wheel_dir}: "
            f"{[w.name for w in matches]}"
        )
    return matches[0]


def _verify_wheel_metadata(wheel_path: Path, expected_distribution: str,
                           expected_version: str | None = None) -> dict[str, str]:
    """Verify distribution name and version from wheel metadata.

    Returns a dict with ``distribution`` and ``version`` keys.
    """
    info = _parse_wheel_name(wheel_path.name)
    if info is None:
        raise ValueError(f"Could not parse wheel filename: {wheel_path.name}")

    distribution = info["distribution_name"]
    version = info["version"]

    if distribution != expected_distribution:
        raise ValueError(
            f"Wheel distribution name mismatch for {wheel_path.name}: "
            f"expected '{expected_distribution}', got '{distribution}'"
        )

    if expected_version is not None and version != expected_version:
        raise ValueError(
            f"Wheel version mismatch for {wheel_path.name}: "
            f"expected '{expected_version}', got '{version}'"
        )

    return {"distribution": distribution, "version": version}


def _verify_shim_marker(wheel_path: Path) -> None:
    """Verify that the controlled replacement wheel contains the eggfetch shim marker.

    Raises ``ValueError`` if the marker is missing (indicating an upstream
    HTTPX wheel was substituted for the replacement).
    """
    import zipfile

    try:
        with zipfile.ZipFile(wheel_path, "r") as zf:
            names = zf.namelist()
    except zipfile.BadZipFile as exc:
        raise ValueError(f"Wheel is not a valid zip archive: {wheel_path.name}: {exc}") from exc

    for marker in _SHIM_MARKER_PATHS:
        if not any(marker in n for n in names):
            raise ValueError(
                f"Controlled replacement wheel {wheel_path.name} is missing "
                f"eggfetch shim marker '{marker}'. "
                f"This may be an upstream HTTPX wheel, not the controlled replacement."
            )


def _copy_wheel(src: Path, dest_dir: Path) -> Path:
    """Copy a wheel file into *dest_dir* (not symlink)."""
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest = dest_dir / src.name
    shutil.copy2(src, dest)
    return dest


def generate_manifest(
    eggfetch_wheel_dir: Path,
    httpx_wheel_dir: Path,
    candidate_sha: str,
    output_dir: Path,
    run_id: str = "",
    run_attempt: str = "",
    workflow_name: str = "",
    producer_job: str = "",
) -> dict:
    """Generate the artifact manifest from wheel source directories.

    Copies both wheels into *output_dir* and emits ``artifact-manifest.json``
    atomically alongside them.
    """
    if len(candidate_sha) != 40 or not all(c in "0123456789abcdef" for c in candidate_sha):
        raise ValueError(f"candidate_sha must be a 40-char hex string, got: {candidate_sha!r}")

    output_dir.mkdir(parents=True, exist_ok=True)

    # Find exactly one eggfetch wheel
    eggfetch_src = _find_single_wheel(eggfetch_wheel_dir, "eggfetch-")
    # Find exactly one httpx controlled replacement wheel
    httpx_src = _find_single_wheel(httpx_wheel_dir, "httpx-")

    # Copy wheels into canonical directory
    eggfetch_dest = _copy_wheel(eggfetch_src, output_dir)
    httpx_dest = _copy_wheel(httpx_src, output_dir)

    # Verify metadata
    eggfetch_meta = _verify_wheel_metadata(eggfetch_dest, "eggfetch")
    httpx_meta = _verify_wheel_metadata(httpx_dest, "httpx", expected_version="0.28.1")

    # Verify shim marker on the replacement wheel
    _verify_shim_marker(httpx_dest)

    # Compute hashes
    eggfetch_sha = compute_sha256(eggfetch_dest)
    httpx_sha = compute_sha256(httpx_dest)

    # Compute manifest SHA-256 for identity binding
    manifest_for_hash = {
        "schema_version": SCHEMA_VERSION,
        "candidate_sha": candidate_sha,
        "producer": "generate_artifact_manifest.py",
        "artifacts": [
            {
                "artifact_type": "eggfetch",
                "distribution": eggfetch_meta["distribution"],
                "version": eggfetch_meta["version"],
                "filename": eggfetch_dest.name,
                "path": str(eggfetch_dest.resolve()),
                "sha256": eggfetch_sha,
                "size_bytes": eggfetch_dest.stat().st_size,
            },
            {
                "artifact_type": "httpx-controlled-replacement",
                "distribution": httpx_meta["distribution"],
                "version": httpx_meta["version"],
                "filename": httpx_dest.name,
                "path": str(httpx_dest.resolve()),
                "sha256": httpx_sha,
                "size_bytes": httpx_dest.stat().st_size,
            },
        ],
    }

    # Build final manifest
    manifest = dict(manifest_for_hash)
    manifest["run_id"] = run_id
    manifest["run_attempt"] = run_attempt
    if workflow_name:
        manifest["workflow_name"] = workflow_name
    if producer_job:
        manifest["producer_job"] = producer_job
    manifest["generated_at"] = datetime.now(timezone.utc).isoformat()

    # Emit atomically
    manifest_path = output_dir / "artifact-manifest.json"
    _write_atomic(manifest_path, manifest)

    # Self-validate
    errors = validate_manifest(manifest)
    if errors:
        raise ValueError(f"Generated manifest has validation errors: {'; '.join(errors)}")

    # Verify hashes match disk
    for art in manifest["artifacts"]:
        disk_sha = compute_sha256(Path(art["path"]))
        if disk_sha != art["sha256"]:
            raise ValueError(
                f"Hash mismatch for {art['filename']}: manifest={art['sha256']}, disk={disk_sha}"
            )

    return manifest


def _write_atomic(path: Path, data: dict) -> None:
    """Write JSON to *path* atomically via a temp file + rename."""
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=str(path.parent), suffix=".tmp")
    try:
        with open(fd, "w") as f:
            json.dump(data, f, indent=2)
            f.write("\n")
        Path(tmp).replace(path)
    except Exception:
        Path(tmp).unlink(missing_ok=True)
        raise


def validate_manifest(manifest: dict) -> list[str]:
    """Validate an artifact manifest. Returns list of errors."""
    errors: list[str] = []

    if not isinstance(manifest, dict):
        return ["manifest must be a JSON object"]

    required_fields = ["schema_version", "candidate_sha", "producer", "artifacts"]
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

    eggfetch_found = False
    httpx_found = False
    for i, art in enumerate(manifest["artifacts"]):
        if not isinstance(art, dict):
            errors.append(f"artifacts[{i}] must be a JSON object")
            continue

        art_type = art.get("artifact_type", "")
        if art_type == "eggfetch":
            eggfetch_found = True
        elif art_type == "httpx-controlled-replacement":
            httpx_found = True

        for field in ("artifact_type", "distribution", "version", "filename",
                       "path", "sha256", "size_bytes"):
            if field not in art:
                errors.append(f"artifacts[{i}].{field} is missing")

        if "sha256" in art:
            sha_val = art["sha256"]
            if not isinstance(sha_val, str) or len(sha_val) != 64 or not all(c in "0123456789abcdef" for c in sha_val):
                errors.append(f"artifacts[{i}].sha256 must be a 64-char hex string")

        if "size_bytes" in art:
            if not isinstance(art["size_bytes"], int) or art["size_bytes"] < 0:
                errors.append(f"artifacts[{i}].size_bytes must be a non-negative integer")

        if "path" in art:
            if _has_path_traversal(art["path"]):
                errors.append(f"artifacts[{i}].path contains path traversal: {art['path']}")

        if "filename" in art:
            if not isinstance(art["filename"], str) or not art["filename"]:
                errors.append(f"artifacts[{i}].filename must be a non-empty string")

    if not eggfetch_found:
        errors.append("manifest must contain an eggfetch artifact")
    if not httpx_found:
        errors.append("manifest must contain an httpx-controlled-replacement artifact")

    return errors


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate artifact manifest for built wheel artifacts",
    )
    parser.add_argument("--eggfetch-wheel-dir", required=True,
                        help="Directory containing the eggfetch wheel")
    parser.add_argument("--httpx-wheel-dir", required=True,
                        help="Directory containing the httpx controlled replacement wheel")
    parser.add_argument("--candidate-sha", required=True, help="40-char hex SHA")
    parser.add_argument("--output-dir", required=True,
                        help="Output directory for manifest and copied wheels")
    parser.add_argument("--run-id", default="", help="GitHub run ID")
    parser.add_argument("--run-attempt", default="", help="Run attempt number")
    parser.add_argument("--workflow-name", default="", help="Workflow name")
    parser.add_argument("--producer-job", default="", help="Producer job name")
    parser.add_argument("--validate-only", action="store_true",
                        help="Only validate an existing manifest")
    parser.add_argument("--manifest", help="Manifest path for --validate-only")
    args = parser.parse_args()

    if args.validate_only:
        if not args.manifest:
            parser.error("--manifest is required with --validate-only")
        with open(args.manifest) as f:
            manifest = json.load(f)
        errors = validate_manifest(manifest)
        if errors:
            print(f"VALIDATION FAILED ({len(errors)} errors):")
            for e in errors:
                print(f"  - {e}")
            sys.exit(1)
        else:
            print("Artifact manifest validation passed.")
            sys.exit(0)

    try:
        manifest = generate_manifest(
            eggfetch_wheel_dir=Path(args.eggfetch_wheel_dir).resolve(),
            httpx_wheel_dir=Path(args.httpx_wheel_dir).resolve(),
            candidate_sha=args.candidate_sha,
            output_dir=Path(args.output_dir).resolve(),
            run_id=args.run_id,
            run_attempt=args.run_attempt,
            workflow_name=args.workflow_name,
            producer_job=args.producer_job,
        )
    except (FileNotFoundError, ValueError) as e:
        print(f"FATAL: {e}")
        sys.exit(1)

    print(f"Artifact manifest written to {Path(args.output_dir).resolve() / 'artifact-manifest.json'}")
    print(f"  schema_version: {manifest['schema_version']}")
    print(f"  candidate_sha: {manifest['candidate_sha'][:16]}...")
    print(f"  artifacts: {len(manifest['artifacts'])}")
    for art in manifest["artifacts"]:
        print(f"    {art['artifact_type']}: {art['filename']} ({art['size_bytes']} bytes)")


if __name__ == "__main__":
    main()
