# eggfetch

eggfetch is a Rust-native HTTP client engine with Python bindings and a CLI tool. The core is async-first: a Rust engine built on tokio and hyper provides connection pooling, phase-aware timeouts, TLS configuration, streaming, and response decompression. The Python bindings expose both sync and async APIs; the sync API blocks on the async engine while releasing the GIL, and the async API integrates with asyncio. There is exactly one networking implementation, living entirely in Rust.

[![CI](https://github.com/eggstack/eggfetch/actions/workflows/ci.yml/badge.svg)](https://github.com/eggstack/eggfetch/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/eggfetch-core)](https://crates.io/crates/eggfetch-core)
[![PyPI version](https://img.shields.io/pypi/v/eggfetch)](https://pypi.org/project/eggfetch/)
[![License](https://img.shields.io/crates/l/eggfetch-core)](LICENSE-MIT)

## Features

- **HTTP/1.1, HTTP/2, HTTP/3** -- ALPN negotiation, multiplexed connections, experimental QUIC transport
- **Streaming** -- request and response bodies stream without eager buffering; `bytes_stream()` and `text_lines()` for incremental reads
- **Response decompression** -- gzip, brotli, zstd, deflate via feature-gated streaming decoders
- **Connection pooling** -- semaphore-based concurrency with per-origin limits and pool metrics
- **Phase-aware timeouts** -- pool, connect, write, read, and total timeout phases with cancellation safety
- **TLS** -- rustls with custom CA bundles, client certificates (mTLS), version policy, and verification toggle
- **Proxy** -- HTTP forwarding, HTTPS CONNECT tunneling, proxy auth, per-request override, `NO_PROXY` bypass
- **Cookies** -- RFC 6265 cookie jar with domain/path matching, cross-origin stripping
- **Authentication** -- Basic and Bearer auth with credential redaction in all output paths
- **Multipart** -- streaming multipart/form-data with known-length optimization
- **Retries** -- policy-driven retries with exponential backoff and `Retry-After` support
- **Python API** -- requests/HTTPX-compatible sync and async interfaces, GIL-releasing blocking I/O
- **HTTPX compatibility facade** -- compatible asyncio surface targeting HTTPX 0.28.1 (`eggfetch.compat.httpx`)
- **CLI** -- full-featured HTTP client with streaming output, machine-readable formats, and shell completions

## Installation

**Python:**

```bash
pip install eggfetch
```

**Rust:**

```toml
[dependencies]
eggfetch-core = { version = "0.1", features = ["http1", "tls-rustls"] }
```

**CLI:**

```bash
cargo install eggfetch-cli
```

Pre-built binaries for Linux, macOS, and Windows are available on the [GitHub Releases](https://github.com/eggstack/eggfetch/releases) page.

## Usage -- Python

### Quick requests

```python
import eggfetch

r = eggfetch.get("https://httpbin.org/get")
print(r.status_code)
print(r.text)
```

### Using a client

```python
import eggfetch

with eggfetch.Client(headers={"User-Agent": "my-app/1.0"}) as client:
    # Buffered response
    r = client.get("https://httpbin.org/get")
    print(r.json())

    # POST with JSON body
    r = client.post("https://httpbin.org/post", json={"key": "value"})
    print(r.status_code)

    # Streaming response
    with client.stream("GET", "https://httpbin.org/stream-bytes/10000") as r:
        for chunk in r.iter_bytes():
            print(f"chunk: {len(chunk)} bytes")
```

### Async client

```python
import asyncio
import eggfetch

async def main():
    async with eggfetch.AsyncClient() as client:
        r = await client.get("https://httpbin.org/get")
        print(r.status_code)

        # Concurrent requests
        responses = await asyncio.gather(
            client.get("https://httpbin.org/get"),
            client.get("https://httpbin.org/ip"),
        )
        for resp in responses:
            print(resp.json())

asyncio.run(main())
```

### Configuration

```python
import eggfetch

client = eggfetch.Client(
    timeout=10.0,
    headers={"User-Agent": "my-app/1.0"},
    limits=eggfetch.Limits(max_connections=100),
    verify="/path/to/ca-bundle.pem",        # custom CA bundle
    cert=("/path/to/cert.pem", "/path/to/key.pem"),  # mTLS
    proxy="http://proxy:8080",
    http2=True,
)
```

### HTTPX-compatible facade

```python
from eggfetch.compat.httpx import Client, AsyncClient

# HTTPX 0.28.1 asyncio-compatible facade
client = Client()
response = client.get("https://example.com")
```

See [`docs/python/guide.md`](docs/python/guide.md) for the full Python API reference.

## Usage -- Rust

### Basic requests

```rust
use eggfetch_core::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    // GET request
    let resp = client.get("https://httpbin.org/get").send().await?;
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text().await?);

    // POST with JSON
    let resp = client
        .post("https://httpbin.org/post")
        .header("Content-Type", "application/json")
        .body(r#"{"key": "value"}"#)
        .send()
        .await?;
    println!("Status: {}", resp.status());

    Ok(())
}
```

### Builder pattern

```rust
use eggfetch_core::{Client, Timeout};

let client = Client::builder()
    .timeout(Timeout::from_secs(30))
    .follow_redirects(true)
    .max_redirects(5)
    .user_agent("my-app/1.0")
    .automatic_decompression(true)
    .build();

let resp = client
    .get("https://httpbin.org/get")?
    .header("accept", "application/json")
    .query("page", "1")
    .send()
    .await?;
```

### Streaming

```rust
use eggfetch_core::Client;
use futures_util::StreamExt;

let mut resp = client.get("https://httpbin.org/stream/3").send().await?;
let mut stream = resp.bytes_stream()?;

while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    println!("chunk: {} bytes", chunk.len());
}
```

### Feature flags

```toml
[dependencies]
eggfetch-core = { version = "0.1", features = [
    "http1",          # HTTP/1.1 (default)
    "http2",          # HTTP/2 via ALPN
    "tls-rustls",     # TLS via rustls (default)
    "cookies",        # RFC 6265 cookie jar
    "proxy",          # HTTP proxy and CONNECT tunneling
    "compression-gzip",
    "compression-brotli",
    "compression-zstd",
    "compression-deflate",
    "multipart",      # streaming multipart/form-data
] }
```

See [`docs/rust/guide.md`](docs/rust/guide.md) for the full Rust API reference.

## Usage -- CLI

```bash
# GET request
eggfetch https://httpbin.org/get

# POST JSON
eggfetch -X POST https://httpbin.org/post --json '{"key": "value"}'

# With authentication
eggfetch --auth user:pass https://httpbin.org/basic-auth/user/pass

# Streaming download
eggfetch --output file.bin https://httpbin.org/stream-bytes/10000

# Machine-readable output
eggfetch --json-output https://httpbin.org/get
```

See [`docs/cli/guide.md`](docs/cli/guide.md) for the full CLI reference.

## HTTPX Compatibility

eggfetch provides an HTTPX 0.28.1-compatible asyncio facade via `eggfetch.compat.httpx`. The compatibility profile is pinned in `compat/httpx/0.28.1/` with machine-readable API manifests, allowed-difference tracking, and a parity case registry.

The facade is a **Stage C candidate**, bounded to the pinned HTTPX 0.28.1 asyncio-supported surface. Its corrective parity kernel is required in routine validation; full HTTPX compatibility and the API oracle remain extended validation gates. This is not an unrestricted replacement for every HTTPX transport or concurrency backend.

Key differences from HTTPX:
- Trio/AnyIO not supported (asyncio only, tokio-based)
- SOCKS proxy not supported
- `ssl_context` transport parameter not supported (TLS handled by Rust engine)
- Python 3.8/3.9 not supported (requires 3.10+)
- Redirects with buffered retained bodies replay correctly; arbitrary one-shot body iterators are rejected before the next hop
- Request-local cookies and explicit Cookie headers are preserved within the facade jar model; native cookie kwargs are not used
- Response streaming is asyncio-compatible and supports incremental text decoding and chunk-size control
- Compatibility raw iteration defaults to `chunk_size=None`, marks live streams consumed before the first source read, counts consumed source bytes before chunk adaptation, and closes automatically only on normal exhaustion; close a partially consumed response explicitly.
- Native compressed-response raw parity is implemented through a core-owned one-shot pre-decompression boundary: a streaming response selects either exact encoded raw bytes or the existing decoded path on first body consumption. The compatibility facade remains a bounded Stage C candidate and does not claim unrestricted HTTPX replacement. Core's existing decoded-header policy still removes `Content-Encoding` and `Content-Length` when automatic decompression is enabled; the compatibility facade restores only the original wire values for those two headers from a narrow read-only snapshot.

Remaining differences are documented in `compat/httpx/0.28.1/allowed-differences.toml`; the compatibility claim is limited to the pinned HTTPX 0.28.1 profile and the supported asyncio surface.

See [`docs/reference/compatibility.md`](docs/reference/compatibility.md) for the full feature matrix.

## Documentation

| Section | Description |
|---------|-------------|
| [getting-started/](docs/getting-started/) | Installation and quickstart guide |
| [concepts/](docs/concepts/) | Architecture, lifecycle, timeouts, streaming, cookies, auth, proxy, TLS |
| [rust/guide.md](docs/rust/guide.md) | Rust API guide with examples |
| [python/guide.md](docs/python/guide.md) | Python sync/async API guide |
| [cli/guide.md](docs/cli/guide.md) | CLI reference and usage guide |
| [migration/](docs/migration/) | Migration guides from requests and HTTPX |
| [cookbook/](docs/cookbook/) | Practical runnable examples |
| [reference/](docs/reference/) | Compatibility matrix, feature matrix, error reference |
| [security/](docs/security/) | Security guidelines and troubleshooting |
| [architecture/](docs/architecture/) | Internal architecture documentation |
| [ffi/](docs/ffi/) | C ABI and FFI binding guide |

## Security

eggfetch follows a security-hardening program covering dependencies, TLS, redirects, auth, cookies, proxies, decompression, multipart, retries, and protocol handling.

- **Dependency auditing:** `cargo-deny` configured in `deny.toml`
- **Secret redaction:** All `Debug`/`Display`/error output redacts credentials, cookies, bearer tokens, and proxy passwords
- **Threat model:** See [docs/architecture/threat-model.md](docs/architecture/threat-model.md)
- **Vulnerability reporting:** See [SECURITY.md](SECURITY.md)

## License

eggfetch is dual-licensed under [MIT](LICENSE-MIT) and [Apache License, Version 2.0](LICENSE-APACHE). You may use this project under either license.

## MSRV

The minimum supported Rust version is **1.80**. This is specified in `workspace.package.rust-version` and enforced by `rust-toolchain.toml`.
