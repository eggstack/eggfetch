#!/usr/bin/env python3
"""Self-test pinned Rust toolchain resolution for the public API oracle."""

from __future__ import annotations

import importlib.util
import os
import subprocess
from pathlib import Path
from unittest.mock import patch


SCRIPT = Path(__file__).with_name("check_rust_public_api.py")
SPEC = importlib.util.spec_from_file_location("check_rust_public_api", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def test_toolchain_binaries_are_explicitly_resolved() -> None:
    with patch.dict(os.environ, {"RUST_API_RUSTUP_TOOLCHAIN": "nightly-test"}):
        with patch.object(
            MODULE.subprocess,
            "run",
            side_effect=[
                subprocess.CompletedProcess([], 0, "/rust/bin/cargo\n", ""),
                subprocess.CompletedProcess([], 0, "/rust/bin/rustc\n", ""),
                subprocess.CompletedProcess([], 0, "/rust/bin/rustdoc\n", ""),
            ],
        ) as run:
            env = MODULE._toolchain_env()

    assert env["RUSTUP_TOOLCHAIN"] == "nightly-test"
    assert env["CARGO"] == "/rust/bin/cargo"
    assert env["RUSTC"] == "/rust/bin/rustc"
    assert env["RUSTDOC"] == "/rust/bin/rustdoc"
    assert [call.args[0][-1] for call in run.call_args_list] == ["cargo", "rustc", "rustdoc"]


def test_missing_pinned_toolchain_fails_closed() -> None:
    with patch.dict(os.environ, {"RUST_API_RUSTUP_TOOLCHAIN": "nightly-missing"}):
        with patch.object(
            MODULE.subprocess,
            "run",
            return_value=subprocess.CompletedProcess(
                [], 1, "", "toolchain is not installed"
            ),
        ):
            try:
                MODULE._toolchain_env()
            except SystemExit as exc:
                assert "nightly-missing" in str(exc)
            else:
                raise AssertionError("missing Rust toolchain must fail closed")


def main() -> int:
    test_toolchain_binaries_are_explicitly_resolved()
    test_missing_pinned_toolchain_fails_closed()
    print("Rust public API toolchain self-test passed (2 cases)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
