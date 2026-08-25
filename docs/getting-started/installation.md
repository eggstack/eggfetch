# Installation

## Python

```bash
pip install eggfetch
```

Requires Python 3.10 through 3.13. Binary wheels are available for
Linux, macOS, and Windows. There is no pure-Python fallback; the package
compiles a native Rust extension via maturin.

### Development install

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install pytest pytest-asyncio maturin
maturin develop -m crates/eggfetch-python/Cargo.toml
```

This builds the extension in-place for the active virtual environment.

## Rust

Add to your `Cargo.toml`:

```toml
[dependencies]
eggfetch-core = { version = "0.1", features = ["http1", "tls-rustls"] }
```

### Feature flags

| Feature | Description |
| --- | --- |
| `http1` | HTTP/1.1 support (default) |
| `http2` | HTTP/2 ALPN negotiation and multiplexing |
| `http3` | HTTP/3 QUIC transport (experimental) |
| `tls-rustls` | TLS via rustls (default) |
| `cookies` | Cookie jar and RFC 6265 handling |
| `proxy` | HTTP proxy and HTTPS CONNECT tunneling |
| `multipart` | Streaming multipart/form-data uploads |
| `compression-gzip` | gzip response decompression |
| `compression-brotli` | Brotli response decompression |
| `compression-zstd` | Zstd response decompression |
| `compression-deflate` | Deflate response decompression |
| `json` | JSON body serialization (Rust side) |

Default features: `http1` and `tls-rustls`. The Python crate enables
cookies, multipart, proxy, all compression codecs, http2, and http3 by
default.

## CLI

```bash
cargo install eggfetch-cli
```

Or build from source:

```bash
cargo build --release -p eggfetch-cli
```

The binary is placed in `target/release/eggfetch`.

### Pre-built binaries

Pre-built CLI binaries for Linux, macOS, and Windows are available on the
[GitHub Releases](https://github.com/eggstack/eggfetch/releases) page.
Download the archive for your platform and place the `eggfetch` binary
somewhere on your `PATH`.

## Platform support

| Platform | Status |
| --- | --- |
| Linux x86_64 | Fully supported, wheel + smoke tested at release |
| macOS arm64 (Apple silicon) | Fully supported, wheel + smoke tested at release |
| Windows x86_64 | Fully supported, wheel + smoke tested at release |
| macOS x86_64, Linux aarch64 | Build from source; no prebuilt wheels |
| Other platforms | May work, not release-tested |

## Rust version

The minimum supported Rust version (MSRV) is **1.80**, declared in
`workspace.package.rust-version` and checked in extended validation.
`rust-toolchain.toml` pins the stable channel for development.
