#!/usr/bin/env python3
"""Check the adapter manifests' explicit core feature boundaries."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def metadata() -> dict:
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(result.stdout)


def dependency(package: dict, name: str) -> dict:
    for item in package["dependencies"]:
        if item["name"] == name:
            return item
    raise AssertionError(f"{package['name']}: missing dependency {name}")


def main() -> None:
    packages = {item["name"]: item for item in metadata()["packages"]}
    ffi = packages["eggfetch-ffi"]
    node = packages["eggfetch-node"]
    python = packages["eggfetch-python"]

    ffi_core = dependency(ffi, "eggfetch-core")
    assert ffi_core["uses_default_features"] is False, (
        "eggfetch-ffi must not inherit eggfetch-core default features"
    )
    ffi_features = ffi["features"]
    assert ffi_features.get("tls-native-roots") == [
        "tls-rustls",
        "eggfetch-core/tls-native-roots",
    ], "FFI native-root forwarding must remain explicit"
    for name, edges in ffi_features.items():
        if name in {"default", "tls-native-roots"}:
            continue
        if name.startswith("tls-") or name in {
            "http1",
            "http2",
            "http3",
            "cookies",
            "proxy",
            "multipart",
            "compression-gzip",
            "compression-brotli",
            "compression-zstd",
            "compression-deflate",
        }:
            assert any(edge.startswith("eggfetch-core/") for edge in edges), (
                f"FFI feature {name} has no explicit core feature edge"
            )

    node_ffi = dependency(node, "eggfetch-ffi")
    assert node_ffi["uses_default_features"] is False
    assert set(node_ffi["features"]) == {
        "http1",
        "tls-rustls",
        "tls-native-roots",
    }, "Node must request only its supported transport/TLS profile"

    direct_tls = {
        item["name"]
        for item in python["dependencies"]
        if item["name"] in {"rustls", "tokio-rustls", "webpki-roots"}
    }
    assert not direct_tls, f"Python TLS dependencies must be owned by core: {sorted(direct_tls)}"
    print("Adapter feature ownership passed")


if __name__ == "__main__":
    main()
