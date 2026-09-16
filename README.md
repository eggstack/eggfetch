# eggfetch

[![CI](https://github.com/eggstack/eggfetch/actions/workflows/ci.yml/badge.svg)](https://github.com/eggstack/eggfetch/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/eggfetch-core)](https://crates.io/crates/eggfetch-core)
[![Crates.io Downloads](https://img.shields.io/crates/d/eggfetch-core)](https://crates.io/crates/eggfetch-core)
[![PyPI version](https://img.shields.io/pypi/v/eggfetch)](https://pypi.org/project/eggfetch/)
[![PyPI Downloads](https://static.pepy.tech/personalized-badge/eggfetch?period=total&units=INTERNATIONAL_SYSTEM&left_color=BLACK&right_color=GREEN&left_text=downloads)](https://pepy.tech/projects/eggfetch)
[![License](https://img.shields.io/crates/l/eggfetch-core)](LICENSE-MIT)

eggfetch is a Rust-native async HTTP client engine (tokio + hyper) with Python bindings and a CLI. There is exactly one networking implementation, living entirely in `eggfetch-core`; the Python sync API blocks on the async engine while releasing the GIL, and the async API integrates with asyncio.

## Features

- **HTTP/1.1, HTTP/2, HTTP/3** — ALPN negotiation; HTTP/3 over QUIC is experimental ([guide](docs/rust/guide.md))
- **Streaming** — response bodies stream without eager buffering (`bytes_stream()`, `text_lines()`), with trailers after EOF ([guide](docs/rust/guide.md))
- **Pooling and timeouts** — per-origin connection pools, phase-aware timeouts (pool/connect/write/read/total), and transport metrics ([pool/timeouts](docs/architecture/core-timeout-pool.md))
- **TLS** — rustls with per-client crypto providers, custom or additive CA roots, mTLS client certs, version policy, and verification toggle ([TLS](docs/concepts/tls.md))
- **Proxy** — HTTP forwarding, HTTPS CONNECT, proxy auth, per-request override, `NO_PROXY`, SOCKS5, and UDS routes ([proxy](docs/concepts/proxy.md))
- **Cookies, auth, multipart** — RFC 6265 jar, Basic/Bearer with redaction, streaming multipart uploads ([cookies](docs/concepts/cookies.md))
- **Retries and redirects** — policy-driven backoff with `Retry-After`, replayable-body redirect handling ([retry](docs/concepts/retry.md))
- **Compression** — feature-gated streaming gzip/brotli/zstd/deflate with zip-bomb limits ([compression](docs/concepts/compression.md))
- **Native Rust JSON (opt-in)** — `RequestBuilder::json()` / `Response::json()` via the `json` feature ([guide](docs/rust/guide.md))
- **Python API** — requests/HTTPX-compatible sync and async interfaces ([guide](docs/python/guide.md)), lazy request bodies, PEP 561 typing, plus versioned `eggfetch.compat.httpx` (0.28.1) and `eggfetch.compat.httpx2` (2.12.0) facades ([compatibility](#httpx-compatibility))
- **Upgrades** — 101 responses expose an owned `network_stream` (WebSocket/SSE building blocks); CONNECT tunnels stay body-iterator only
- **CLI** — streaming output, machine-readable formats, shell completions ([guide](docs/cli/guide.md))
- **C ABI and Node.js prototype** — opaque-handle FFI plus an experimental N-API wrapper ([ffi-and-node](docs/architecture/ffi-and-node.md))

## Installation

**Python:**

```bash
pip install eggfetch
```

**Rust:**

```toml
[dependencies]
eggfetch-core = "0.1"
```

The default features are the secure HTTP/1.1 client with Rustls and native roots. See the [feature profile matrix](docs/architecture/feature-flags.md#supported-core-profiles) for minimal, deterministic, and embedded recipes.

**CLI:**

```bash
cargo install eggfetch-cli
```

## Usage -- Python

```python
import eggfetch

r = eggfetch.get("https://httpbin.org/get")
print(r.status_code)
print(r.text)
```

```python
import eggfetch

with eggfetch.Client(headers={"User-Agent": "my-app/1.0"}) as client:
    r = client.get("https://httpbin.org/get")
    print(r.json())

    r = client.post("https://httpbin.org/post", json={"key": "value"})
    print(r.status_code)

    with client.stream("GET", "https://httpbin.org/stream-bytes/10000") as r:
        for chunk in r.iter_bytes():
            print(f"chunk: {len(chunk)} bytes")
```

```python
import asyncio
import eggfetch

async def main():
    async with eggfetch.AsyncClient() as client:
        r = await client.get("https://httpbin.org/get")
        print(r.status_code)

        responses = await asyncio.gather(
            client.get("https://httpbin.org/get"),
            client.get("https://httpbin.org/ip"),
        )
        for resp in responses:
            print(resp.json())

asyncio.run(main())
```

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

Versioned HTTPX-compatible facades over the same engine:

```python
from eggfetch.compat.httpx import Client  # HTTPX 0.28.1 surface
from eggfetch.compat.httpx2 import Client as H2Client  # httpx2 2.12.0 surface
```

See [`docs/python/guide.md`](docs/python/guide.md) for the full Python API reference.

The native package supports Python 3.10–3.14 and ships `py.typed` stubs for its public API. `Client` and the top-level sync helpers accept lazy sync `content=` iterables (async-only iterables are rejected before dispatch); `AsyncClient` additionally accepts lazy async iterables, pulled only as the transport asks for them. `eggfetch._native` is a private implementation module — import from `eggfetch`.

## Usage -- Rust

```rust
use eggfetch_core::Client;
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .timeout(eggfetch_core::Timeout::from_secs(30))
        .follow_redirects(true)
        .user_agent("my-app/1.0")
        .build();

    let mut resp = client.get("https://httpbin.org/get").send().await?;
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text().await?);

    // Streaming body
    let mut resp = client.get("https://httpbin.org/stream/3").send().await?;
    let mut stream = resp.bytes_stream()?;
    while let Some(chunk) = stream.next().await {
        println!("chunk: {} bytes", chunk?.len());
    }

    Ok(())
}
```

The opt-in `json` feature adds `RequestBuilder::json()` / `Response::json()` Serde helpers. For a private CA in addition to the selected native or WebPKI roots, use the additive TLS methods:

```rust
let tls = eggfetch_core::TlsConfig::builder()
    .additional_ca_certificate_path("/path/to/private-ca.pem")?
    .build();
let client = eggfetch_core::Client::builder().tls_config(tls).build();
```

`additional_ca_certificate_*` augments the base trust store; `ca_certificate_*` replaces it. Advanced embedding — custom dialers, direct/proxy address pinning, detailed failure introspection, frame-level bodies, the Tower service adapter, and physical-connection guards — is covered in [`docs/rust/guide.md`](docs/rust/guide.md).

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

## Examples

Runnable starting points (each takes an optional base URL argument, default `https://httpbin.org`):

- [`crates/eggfetch-core/examples/quickstart.rs`](crates/eggfetch-core/examples/quickstart.rs) — core client: configured GET/POST, timeout override, auth, streaming (`cargo run -p eggfetch-core --example quickstart`)
- [`examples/python_sync.py`](examples/python_sync.py) — sync client, JSON POST, streaming download
- [`examples/python_async.py`](examples/python_async.py) — async client with concurrent requests

More patterns are in [`docs/cookbook/`](docs/cookbook/).

## HTTPX Compatibility

Two versioned, independent facades over the single Rust engine — `eggfetch.compat.httpx` (0.28.1) and `eggfetch.compat.httpx2` (2.12.0, adds `FunctionAuth`, `Origin`/`URL.origin`, `QUERY`, SSE, optional WebSocket).

See [`docs/reference/compatibility.md`](docs/reference/compatibility.md) for the full feature matrix and retained differences.

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

- **Dependency auditing:** run the live preflight with `./scripts/check_security.sh` before publication
- **Secret redaction:** all `Debug`/`Display`/error output redacts credentials, cookies, bearer tokens, and proxy passwords
- **Threat model:** see [docs/architecture/threat-model.md](docs/architecture/threat-model.md)
- **Vulnerability reporting:** see [SECURITY.md](SECURITY.md)

## License

eggfetch is licensed under the [MIT License](LICENSE-MIT).

## MSRV

The minimum supported Rust version is **1.89**, declared in `workspace.package.rust-version` and checked in extended validation with the exact 1.89.0 toolchain. `rust-toolchain.toml` pins the stable channel for normal development.
