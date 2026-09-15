# eggfetch

[![CI](https://github.com/eggstack/eggfetch/actions/workflows/ci.yml/badge.svg)](https://github.com/eggstack/eggfetch/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/eggfetch-core)](https://crates.io/crates/eggfetch-core)
[![Crates.io Downloads](https://img.shields.io/crates/d/eggfetch-core)](https://crates.io/crates/eggfetch-core)
[![PyPI version](https://img.shields.io/pypi/v/eggfetch)](https://pypi.org/project/eggfetch/)
[![PyPI Downloads](https://static.pepy.tech/personalized-badge/eggfetch?period=total&units=INTERNATIONAL_SYSTEM&left_color=BLACK&right_color=GREEN&left_text=downloads)](https://pepy.tech/projects/eggfetch)
[![License](https://img.shields.io/crates/l/eggfetch-core)](LICENSE-MIT)

eggfetch is a Rust-native async HTTP client engine (tokio + hyper) with Python bindings and a CLI. There is exactly one networking implementation, living entirely in `eggfetch-core`; the Python sync API blocks on the async engine while releasing the GIL, and the async API integrates with asyncio.

## Features

- **HTTP/1.1, HTTP/2, HTTP/3** — ALPN negotiation; HTTP/3 over QUIC stays experimental ([graduation gate](docs/architecture/core-tls-proxy-protocols.md))
- **Streaming** — high-level byte bodies without eager buffering (`bytes_stream()`, `text_lines()`), plus an additive native `http_body::Body` frame boundary that preserves DATA/trailers ([guide](docs/rust/guide.md))
- **Pooling, timeouts, observability** — separate logical pool and live Hyper-connection controls, phase-aware timeouts (pool/connect/write/read/total), established-I/O guardrails, and connector/lifecycle/DNS/TLS/H3 transport metrics ([pool/timeouts](docs/architecture/core-timeout-pool.md))
- **Native embedded transport control** — optional caller-owned raw-stream dialing, explicit Hyper stale-connection retry control, physical-connection admission, and established-I/O inactivity guardrails ([Rust guide](docs/rust/guide.md))
- **TLS** — rustls with explicit per-client crypto providers, replacement or additive CA roots, mTLS client certs, version policy, and verification toggle ([TLS](docs/concepts/tls.md))
- **Proxy** — HTTP forwarding, HTTPS CONNECT, proxy auth, per-request override, `NO_PROXY`, native proxy-peer/CONNECT/SOCKS5 route pinning; SOCKS5 and UDS routes ([proxy](docs/concepts/proxy.md))
- **Cookies, auth, multipart** — RFC 6265 jar, Basic/Bearer with redaction, streaming multipart uploads ([cookies](docs/concepts/cookies.md))
- **Retries and redirects** — policy-driven backoff with `Retry-After`, replayable-body redirect handling ([retry](docs/concepts/retry.md))
- **Compression** — feature-gated streaming gzip/brotli/zstd/deflate with zip-bomb limits ([compression](docs/concepts/compression.md))
- **Native Rust JSON (opt-in)** — replayable `RequestBuilder::json()`, single-consumption `Response::json()` via the `json` feature ([guide](docs/rust/guide.md))
- **Python API** — requests/HTTPX-compatible sync and async interfaces ([guide](docs/python/guide.md)), plus versioned `eggfetch.compat.httpx` (0.28.1) and `eggfetch.compat.httpx2` (2.12.0) facades ([compatibility](#httpx-compatibility))
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
eggfetch-core = { version = "0.1", features = ["http1", "tls-rustls", "tls-native-roots"] }
```

See the [feature profile matrix](docs/architecture/feature-flags.md#supported-core-profiles) for minimal, deterministic, and embedded recipes.

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

## Usage -- Rust

Native Rust consumers can keep their own route while eggfetch owns HTTP and
destination TLS. A `Dialer` receives only the logical host and effective port;
it does not replace URL/Host/SNI identity. A request target override changes
only the wire path/query. The custom route is intentionally
incompatible with built-in proxy, UDS, resolved-address, local-binding,
socket-option, and HTTP/3 routing.

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

The opt-in `json` feature adds `RequestBuilder::json()` / `Response::json()` Serde helpers. Native Rust callers can use `resolved_addresses()` for direct-only physical routing, or separately pin proxy peers with `Proxy::resolved_addresses()` and supported HTTPS CONNECT/local-SOCKS5 targets with `RequestBuilder::proxy_target_addresses()`; these snapshots never fall back to DNS and remain independent controls. See [`docs/rust/guide.md`](docs/rust/guide.md) for the full Rust API reference.

Native embedders that need structured timeout/DNS/refusal detail can opt into
`RequestBuilder::send_detailed()`; the [Rust guide](docs/rust/guide.md) shows
the string-free handling pattern.

Rustls crypto providers are selected per `TlsConfig`, not process-wide. Native
applications that need a different provider can depend on the matching Rustls
provider feature and pass its `Arc<CryptoProvider>` to
`TlsConfigBuilder::crypto_provider`; the ordinary eggfetch profile remains
ring-backed. Provider capabilities, including FIPS or post-quantum properties,
depend on the caller's exact provider build.

Native callers that need strict attempt accounting can set
`Client::builder().retry_canceled_requests(false)`. This disables only
Hyper's transparent retry after a reused idle connection is found unusable;
it does not disable or alter eggfetch's explicit `RetryPolicy`. The default is
`true`, and the setting does not apply to the independent HTTP/3 transport.
Embedded orchestrators can additionally set `PhysicalConnectionPolicy` and
`TransportIoTimeout` to bound live Hyper connections and established I/O
inactivity independently from logical pool limits and request timeouts. See
the [Rust guide](docs/rust/guide.md) for the integration boundary and limits.
The native `execute_http_body()` surface starts response read timeouts when
the returned body is first polled, preserves DATA/trailer frames, and leaves
redirects, retries, cookies, auth, decompression, and upgrades to the caller
or high-level API as documented there.
The public frame/provider/private-PKI boundary is also exercised by the
standalone `qualification/native-http-body-tls/` fixture; that manual fixture
is not part of the routine CI matrix.

For native Rust HTTPS clients that need a private CA in addition to the
selected native or WebPKI roots, use the additive TLS methods:

```rust
let tls = eggfetch_core::TlsConfig::builder()
    .additional_ca_certificate_path("/path/to/private-ca.pem")?
    .build();
let client = eggfetch_core::Client::builder().tls_config(tls).build();
```

`additional_ca_certificate_path`, `additional_ca_certificate_pem`, and
`additional_ca_certificate_der` augment the selected base trust store.
`ca_certificate_*` remains replacement-style, and the legacy
`add_ca_certificate_path` was deliberately not repurposed: existing callers
use it to build a replacement custom set.

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

Two versioned, independent facades over the single Rust engine — `eggfetch.compat.httpx` (0.28.1) and `eggfetch.compat.httpx2` (2.12.0, adds `FunctionAuth`, `Origin`/`URL.origin`, `QUERY`, SSE, optional WebSocket). Both are Stage C qualified on frozen executable SHA `43c68bd1bcff45301fc8b6b163b6b6e06d98a786`; HTTPX 1.0 preview under `compat/httpx/1.0-preview/` is reconnaissance only.

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

- **Dependency auditing:** `cargo-deny` configured in `deny.toml`
- **Secret redaction:** all `Debug`/`Display`/error output redacts credentials, cookies, bearer tokens, and proxy passwords
- **Threat model:** see [docs/architecture/threat-model.md](docs/architecture/threat-model.md)
- **Vulnerability reporting:** see [SECURITY.md](SECURITY.md)

## License

eggfetch is licensed under the [MIT License](LICENSE-MIT).

## MSRV

The minimum supported Rust version is **1.80**, declared in `workspace.package.rust-version` and checked in extended validation. `rust-toolchain.toml` pins the stable channel for development.
