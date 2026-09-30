# eggfetch

[![CI](https://github.com/eggstack/eggfetch/actions/workflows/ci.yml/badge.svg)](https://github.com/eggstack/eggfetch/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/eggfetch-core)](https://crates.io/crates/eggfetch-core)
[![Crates.io Downloads](https://img.shields.io/crates/d/eggfetch-core)](https://crates.io/crates/d/eggfetch-core)
[![PyPI version](https://img.shields.io/pypi/v/eggfetch)](https://pypi.org/project/eggfetch/)
[![PyPI Downloads](https://static.pepy.tech/personalized-badge/eggfetch?period=total&units=INTERNATIONAL_SYSTEM&left_color=BLACK&right_color=GREEN&left_text=downloads)](https://pepy.tech/projects/eggfetch)
[![License](https://img.shields.io/crates/l/eggfetch-core)](LICENSE-MIT)

eggfetch is a Rust-native async HTTP client engine (tokio + hyper) with Python bindings and a CLI. All networking lives in `eggfetch-core`; the Python sync API blocks on the async engine while releasing the GIL, and the async API integrates with asyncio.

## Features

- **HTTP/1.1, HTTP/2, and experimental HTTP/3** with ALPN negotiation
- **Streaming** request and response bodies with backpressure, plus trailers
- **Pooling and phase-aware timeouts** (pool/connect/write/read/total) with transport metrics
- **TLS** via rustls: additive or replacement CA roots, mTLS, version policy ([TLS](docs/concepts/tls.md))
- **Proxy**: HTTP forwarding, HTTPS CONNECT, proxy auth, `NO_PROXY`, SOCKS5 ([proxy](docs/concepts/proxy.md))
- **Cookies, auth, multipart, compression**: RFC 6265 jar, Basic/Bearer, streaming uploads, gzip/brotli/zstd/deflate
- **Retries and redirects**: backoff with `Retry-After`, replayable-body redirect handling
- **Python**: requests/HTTPX-compatible sync and async APIs plus versioned `eggfetch.compat.httpx` (0.28.1) and `eggfetch.compat.httpx2` (2.12.0) facades ([compatibility](docs/reference/compatibility.md))
- **CLI** with streaming output, machine-readable formats, and shell completions ([guide](docs/cli/guide.md))

## Installation

```bash
pip install eggfetch                        # Python 3.10–3.15
cargo install eggfetch-cli                  # CLI
```

```toml
[dependencies]
eggfetch-core = "0.2"                       # Rust (MSRV 1.89)
```

See [installation](docs/getting-started/installation.md) for wheels, feature recipes, and platform support.

## Quickstart

```python
import eggfetch

r = eggfetch.get("https://httpbin.org/get")
print(r.status_code, r.json())

with eggfetch.Client(headers={"User-Agent": "my-app/1.0"}) as client:
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
        responses = await asyncio.gather(
            client.get("https://httpbin.org/get"),
            client.get("https://httpbin.org/ip"),
        )
        for resp in responses:
            print(resp.status_code, resp.json())

asyncio.run(main())
```

```rust
use eggfetch_core::{Client, Timeout};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .timeout(Timeout::from_secs(30))
        .follow_redirects(true)
        .user_agent("my-app/1.0")
        .build();

    let mut resp = client.get("https://httpbin.org/get")?.send().await?;
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text().await?);

    let mut resp = client.get("https://httpbin.org/stream/3")?.send().await?;
    let mut stream = resp.bytes_stream()?;
    while let Some(chunk) = stream.next().await {
        println!("chunk: {} bytes", chunk?.len());
    }
    Ok(())
}
```

```bash
eggfetch https://httpbin.org/get
eggfetch -X POST https://httpbin.org/post --json '{"key": "value"}'
eggfetch --auth user:pass https://httpbin.org/basic-auth/user/pass
eggfetch --output file.bin https://httpbin.org/stream-bytes/10000
```

## Examples

Runnable starting points (each takes an optional base URL, default `https://httpbin.org`):

- `cargo run -p eggfetch-core --example quickstart` — configured GET/POST, timeout override, auth, streaming
- `python examples/python_sync.py` — sync client, JSON POST, streaming download
- `python examples/python_async.py` — async client with concurrent requests

More patterns are in [`docs/cookbook/`](docs/cookbook/).

## Documentation

- [Getting started](docs/getting-started/quickstart.md) — installation and first requests
- [Rust guide](docs/rust/guide.md) · [Python guide](docs/python/guide.md) · [CLI guide](docs/cli/guide.md)
- [Migration](docs/migration/from-httpx.md) from HTTPX/requests · [Compatibility](docs/reference/compatibility.md) · [C ABI / FFI](docs/ffi/)

## Security

All `Debug`/`Display`/error output redacts credentials, cookies, and tokens. See [SECURITY.md](SECURITY.md) to report a vulnerability.

## License

eggfetch is licensed under the [MIT License](LICENSE-MIT).
