# Installation

## Python

```bash
pip install eggfetch
```

Requires Python 3.10 through 3.15. Binary wheels are available for
Linux, macOS, and Windows. There is no pure-Python fallback; the package
compiles a native Rust extension via maturin.

Free-threaded CPython (PEP 703) is supported for Python 3.14 and 3.15.
The release matrix includes separate `cp314-cp314t` and `cp315-cp315t`
wheels for the same platforms as the GIL-enabled wheels, and importing
`eggfetch` leaves the interpreter's GIL disabled. Python 3.13 has no
free-threaded wheel: the supported PyO3 release dropped 3.13t.

The free-threaded Linux wheels target glibc 2.34 (`manylinux_2_34`) rather
than the `manylinux2014` baseline of the GIL-enabled wheels, because the
free-threaded CPython interpreters themselves require that glibc. The
free-threaded wheels are therefore usable on a narrower set of Linux
distributions than the GIL-enabled ones.

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
eggfetch-core = { version = "0.4", features = ["http1", "tls-rustls", "tls-native-roots"] }
```

### Feature flags

| Feature | Description |
| --- | --- |
| `http1` | HTTP/1.1 support (default; `native-http1` + `high-level-url` + `logical-retry` + `redirects` + `basic-auth`) |
| `http2` | HTTP/2 ALPN negotiation and multiplexing (same policy bundle over `native-http2`) |
| `native-http1` / `native-http2` | Low-level transport slices (transport + standard route + `advanced-routing`); omit `high-level-url` for minimal embedding |
| `transport-http1` / `transport-http2` | Primitive Hyper protocol slices; combine with `standard-route` for the leanest native transport |
| `standard-route` / `advanced-routing` | Ordinary DNS → TCP/TLS path vs custom Dialer, resolved-target/SNI, socket options, UDS |
| `standard-http1` / `standard-http2` | Lean high-level recipes without `advanced-routing` or the retry/redirect/Basic bundle |
| `high-level-url` | String/URL request API |
| `logical-retry` / `redirects` / `basic-auth` | Retry / redirect following / Basic auth (in `http1`/`http2`; omitted by lean `standard-http1`) |
| `http3` | HTTP/3 QUIC transport (experimental) |
| `tls-rustls` | TLS via rustls (default) |
| `tls-native-roots` | Prefer the operating system trust store; implies `tls-rustls` (default) |
| `cookies` | Cookie jar and RFC 6265 handling |
| `proxy` | HTTP proxy and HTTPS CONNECT tunneling |
| `multipart` | Streaming multipart/form-data uploads |
| `compression-gzip` | gzip response decompression |
| `compression-brotli` | Brotli response decompression |
| `compression-zstd` | Zstd response decompression |
| `compression-deflate` | Deflate response decompression |
| `json` | Native JSON request/response helpers via optional Serde dependencies |

Default features: `http1`, `tls-rustls`, and `tls-native-roots`. The Python crate enables
cookies, multipart, proxy, all compression codecs, http2, and http3 by
default.

For a deterministic embedded HTTPS client without system trust loading:

```toml
eggfetch-core = { version = "0.4", default-features = false, features = ["http1", "tls-rustls"] }
```

For a lean Bearer-only standard-route client (no advanced routing or
retry/redirect/Basic machinery):

```toml
eggfetch-core = { version = "0.4", default-features = false, features = ["standard-http1", "tls-rustls"] }
```

For cleartext-only: `features = ["http1"]` alone. Measured downstream
size/dependency evidence (full compat not a footprint win; lean
`standard-http1` measured improvement) lives in
`docs/architecture/embedded-footprint.md`; fixtures in
`qualification/embedded/`.

## CLI

```bash
cargo install eggfetch-cli
```

Or build from source:

```bash
cargo build --release -p eggfetch-cli
```

The binary is placed in `target/release/eggfetch`.

## Platform support

| Platform | Status |
| --- | --- |
| Linux x86_64 | Fully supported, wheel + smoke tested at release |
| Linux aarch64 | Fully supported, wheel + smoke tested at release |
| macOS arm64 (Apple silicon) | Fully supported, wheel + smoke tested at release |
| Windows x86_64 | Fully supported, wheel + smoke tested at release |
| macOS x86_64 | Build from source; no prebuilt wheels |
| Other platforms | May work, not release-tested |

Each of these platforms also receives free-threaded wheels for Python 3.14
and 3.15.

## Rust version

The minimum supported Rust version (MSRV) is **1.89**, declared in
`workspace.package.rust-version` and checked in extended validation with the
exact 1.89.0 toolchain.
`rust-toolchain.toml` pins the stable channel for development.
