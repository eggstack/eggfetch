#!/usr/bin/env python3
"""Verify the frozen eggfetch-core Rust public surface and semver contract."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BASELINE = "03ecba973010e2858bf16a2b5f84d51ce70adae4"
SNAPSHOT_ROOT = ROOT / "compat/rust-public-api"

PROFILES: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("default", ()),
    ("http1-tls-rustls", ("--no-default-features", "--features", "http1,tls-rustls")),
    ("http2-tls-rustls", ("--no-default-features", "--features", "http2,tls-rustls")),
    (
        "standard-http1-tls-rustls",
        ("--no-default-features", "--features", "standard-http1,tls-rustls"),
    ),
    (
        "native-http1-tls-rustls",
        ("--no-default-features", "--features", "native-http1,tls-rustls"),
    ),
    ("all-features", ("--all-features",)),
)


def _tool(name: str, default: str) -> list[str]:
    value = os.environ.get(name, default)
    return value.split()


def _run_public_api(tool: list[str], feature_args: tuple[str, ...]) -> str:
    command = [
        *tool,
        "-p",
        "eggfetch-core",
        "-sss",
        "--color",
        "never",
        *feature_args,
    ]
    result = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        env={
            **os.environ,
            "RUSTUP_TOOLCHAIN": os.environ.get(
                "RUST_API_RUSTUP_TOOLCHAIN", "nightly-2026-05-07"
            ),
        },
    )
    if result.returncode:
        sys.stderr.write(result.stderr)
        raise SystemExit(f"public API command failed: {' '.join(command)}")
    return result.stdout


def _run_semver(tool: list[str], feature_args: tuple[str, ...]) -> None:
    command = [
        *tool,
        "check-release",
        "-p",
        "eggfetch-core",
        "--baseline-rev",
        BASELINE,
        "--release-type",
        "patch",
        "--color",
        "never",
        "--only-explicit-features",
        *feature_args,
    ]
    result = subprocess.run(command, cwd=ROOT, check=False, text=True)
    if result.returncode:
        raise SystemExit(f"semver check failed: {' '.join(command)}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--skip-semver", action="store_true")
    args = parser.parse_args()

    public_api = _tool("CARGO_PUBLIC_API", "cargo-public-api")
    semver = _tool("CARGO_SEMVER_CHECKS", "cargo-semver-checks")
    failures: list[str] = []
    with tempfile.TemporaryDirectory(prefix="eggfetch-rust-api-") as temp:
        temp_root = Path(temp)
        for name, feature_args in PROFILES:
            expected_path = SNAPSHOT_ROOT / f"{name}.txt"
            if not expected_path.is_file():
                failures.append(f"missing snapshot: {expected_path}")
                continue
            actual = _run_public_api(public_api, feature_args)
            actual_path = temp_root / f"{name}.txt"
            actual_path.write_text(actual)
            expected = expected_path.read_text()
            if actual != expected:
                failures.append(
                    f"public API drift for {name}; compare {expected_path} with {actual_path}"
                )

    if not args.skip_semver:
        # The semver checker accepts a single feature set per invocation.  The
        # exact snapshot loop above remains the stronger six-profile oracle;
        # the semver pass uses the default profile as its stable cross-check.
        _run_semver(semver, ())

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print(f"Rust public API oracle passed ({len(PROFILES)} profiles)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
