#!/usr/bin/env python3
"""Produce the downstream behavioral-fixture artifact manifest.

Tier 2's downstream gate fails closed when the artifact manifest is absent:
the release-blocking portfolio is only meaningful when it runs against a
specific eggfetch wheel and a specific controlled HTTPX replacement, and the
manifest is what pins those two artifacts by content hash.

The manifest used to have no in-repository producer, so supplying it meant
building two wheels by hand and hand-writing the JSON. This script is that
producer. It does not change the gate's contract: the gate still fails
closed when the manifest is missing, because producing the manifest is still
a separate, explicit step. What changes is that the step is one command
instead of a hand-run procedure.

    python scripts/build_downstream_artifact.py

The manifest is written to
``target/downstream-qualification/artifact-manifest.json`` by default. The
paths it records are relative to the repository root, which is the working
directory the consumer runs from.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "target" / "downstream-qualification"
MANIFEST_NAME = "artifact-manifest.json"

EGGFETCH_ROLE = "eggfetch"
HTTPX_ROLE = "httpx-controlled-replacement"

# The consumer reads these two roles and nothing else.
REQUIRED_ROLES = (EGGFETCH_ROLE, HTTPX_ROLE)


def sha256(path: Path) -> str:
    """Return the SHA-256 of a file, hex encoded."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def relative_path(path: Path, root: Path) -> str:
    """Return `path` relative to `root`, or absolute when it is outside it.

    The consumer resolves the recorded path against its own working
    directory, which is the repository root. An artifact built outside the
    repository keeps an absolute path so it still resolves.
    """
    try:
        return str(path.relative_to(root))
    except ValueError:
        return str(path)


def describe(role: str, path: Path, root: Path) -> dict:
    """Describe one artifact the way the consumer reads it."""
    parts = path.name.split("-")
    distribution = parts[0] if parts else ""
    version = parts[1] if len(parts) > 1 else ""
    return {
        "role": role,
        "distribution": distribution,
        "version": version,
        "filename": path.name,
        "relative_path": relative_path(path, root),
        "sha256": sha256(path),
        "size_bytes": path.stat().st_size,
    }


def write_manifest(output_dir: Path, artifacts: list[dict]) -> Path:
    """Write the artifact manifest and return its path."""
    output_dir.mkdir(parents=True, exist_ok=True)
    manifest_path = output_dir / MANIFEST_NAME
    manifest = {
        "schema_version": 3,
        "artifacts": artifacts,
    }
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return manifest_path


def run(command: list[str], description: str, cwd: Path | None = None,
        env: dict[str, str] | None = None) -> None:
    """Run a build command, failing the script when it does not succeed."""
    print(f"  building {description} ...", flush=True)
    result = subprocess.run(command, cwd=cwd, env=env, check=False)
    if result.returncode != 0:
        raise SystemExit(
            f"FAIL: {description} failed with exit code {result.returncode}"
        )


def newest_wheel(directory: Path) -> Path:
    """Return the most recently written wheel in a directory."""
    candidates = sorted(directory.glob("*.whl"), key=lambda item: item.stat().st_mtime)
    if not candidates:
        raise SystemExit(f"FAIL: no wheel was produced in {directory}")
    return candidates[-1]


def build_eggfetch_wheel(output_dir: Path) -> Path:
    """Build the eggfetch wheel from this checkout."""
    wheels = output_dir / "wheels"
    wheels.mkdir(parents=True, exist_ok=True)
    run(
        [
            sys.executable,
            "-m",
            "maturin",
            "build",
            "-m",
            str(ROOT / "crates/eggfetch-python/Cargo.toml"),
            "--out",
            str(wheels),
        ],
        "the eggfetch wheel",
    )
    return newest_wheel(wheels)


def build_httpx_replacement() -> Path:
    """Build the controlled httpx replacement wheel."""
    package_dir = ROOT / "compat" / "httpx-controlled-replacement"
    env = dict(os.environ)
    env["PYTHON"] = sys.executable
    run(
        ["bash", str(package_dir / "build.sh")],
        "the controlled httpx replacement wheel",
        cwd=package_dir,
        env=env,
    )
    return newest_wheel(package_dir / "dist")


def self_test() -> int:
    """Check that a produced manifest is readable by the real consumer.

    The point of the manifest is that the downstream runner can resolve both
    wheels from it by content hash. Writing JSON that nothing reads would
    satisfy the producer and still fail the gate, so the self-test loads the
    consumer and asks it to find the wheels.
    """
    consumer_path = ROOT / "scripts" / "run_isolated_downstream.py"
    spec = importlib.util.spec_from_file_location(
        "run_isolated_downstream", consumer_path
    )
    if spec is None or spec.loader is None:
        print(f"FAIL: cannot load the consumer at {consumer_path}", file=sys.stderr)
        return 1
    consumer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(consumer)

    failures: list[str] = []
    with tempfile.TemporaryDirectory(prefix="eggfetch-downstream-self-test-") as raw:
        bundle_root = Path(raw)
        (bundle_root / "wheels").mkdir()
        eggfetch_wheel = bundle_root / "wheels" / "eggfetch-0.2.2-cp312-cp312-linux_x86_64.whl"
        eggfetch_wheel.write_bytes(b"eggfetch wheel placeholder")
        httpx_wheel = bundle_root / "httpx-0.28.1-py3-none-any.whl"
        httpx_wheel.write_bytes(b"httpx replacement placeholder")

        artifacts = [
            describe(EGGFETCH_ROLE, eggfetch_wheel, bundle_root),
            describe(HTTPX_ROLE, httpx_wheel, bundle_root),
        ]
        manifest_path = write_manifest(bundle_root, artifacts)
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

        roles = sorted(item["role"] for item in manifest["artifacts"])
        if tuple(roles) != tuple(sorted(REQUIRED_ROLES)):
            failures.append(f"roles are {roles}, expected {sorted(REQUIRED_ROLES)}")

        found_eggfetch, found_httpx = consumer.find_wheels(manifest, bundle_root=bundle_root)
        if found_eggfetch != eggfetch_wheel:
            failures.append("the consumer did not resolve the eggfetch wheel")
        if found_httpx != httpx_wheel:
            failures.append("the consumer did not resolve the httpx replacement wheel")

        # A manifest whose hash does not match the file must not be accepted:
        # that is the property that makes the manifest a pin rather than a
        # list of paths.
        tampered = json.loads(json.dumps(manifest))
        tampered["artifacts"][0]["sha256"] = "0" * 64
        found_eggfetch, _ = consumer.find_wheels(tampered, bundle_root=bundle_root)
        if found_eggfetch is not None:
            failures.append("the consumer accepted a wheel whose hash does not match")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1
    print(f"Downstream artifact manifest self-test passed ({len(REQUIRED_ROLES)} roles)")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help="directory the manifest is written to",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="check that a produced manifest is readable by the consumer",
    )
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    output_dir = args.output.resolve()
    print("Producing the downstream behavioral-fixture artifact manifest")
    eggfetch_wheel = build_eggfetch_wheel(output_dir)
    httpx_wheel = build_httpx_replacement()

    artifacts = [
        describe(EGGFETCH_ROLE, eggfetch_wheel, ROOT),
        describe(HTTPX_ROLE, httpx_wheel, ROOT),
    ]
    manifest_path = write_manifest(output_dir, artifacts)

    print(f"Wrote {manifest_path}")
    for artifact in artifacts:
        print(
            f"  {artifact['role']}: {artifact['filename']} "
            f"({artifact['size_bytes']} bytes, sha256 {artifact['sha256'][:16]}…)"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
