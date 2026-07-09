# eggfetch

eggfetch is a Rust-native HTTP client engine with Python bindings and a CLI tool. The core is async-first: a Rust engine built on tokio and hyper provides connection pooling, timeouts, TLS, and streaming. The Python bindings expose both sync and async APIs; the sync API blocks on the async engine while releasing the GIL, and the async API integrates with asyncio. There is exactly one networking implementation, living entirely in Rust.

> **Status: Milestone B complete / core HTTP engine.**
> The workspace builds, passes lints, and executes real HTTP requests. `eggfetch-core` provides an async `Client`, `Request`/`RequestBuilder`/`Response`, headers, query parameters, byte bodies, HTTPS via rustls, buffered responses, and an error taxonomy. CLI and Python crates remain stubs.

## Architecture

eggfetch is a three-crate Cargo workspace:

- **eggfetch-core** owns all HTTP behavior: client, request builder, response, body, headers, timeout, error types, connection pooling, TLS, and streaming. Every networking dependency lives here.
- **eggfetch-cli** is a thin binary that delegates to eggfetch-core for all HTTP work. It handles argument parsing, output formatting, and exit codes only.
- **eggfetch-python** is the Python bindings adapter. It wraps eggfetch-core via PyO3/maturin and exposes sync and async Python APIs. It does not contain its own HTTP logic.

The invariant is strict: all network I/O goes through eggfetch-core. The CLI and Python crates are adapters.

## MVP Non-goals

eggfetch is not aiming for full requests/httpx drop-in parity in the MVP. Specifically, the MVP does not include:

- Full transport parity with HTTPX (ASGI/WSGI in-process transports, all SOCKS proxy modes)
- Trio/AnyIO support (asyncio only)
- HTTP/3 (QUIC transport)
- Every authentication flow, advanced proxy mounting, or requests compatibility edge case

See [plans/ROADMAP.md](plans/ROADMAP.md) for the full roadmap and compatibility policy.

## Target API Shapes

The Python API targets familiar requests/httpx patterns:

```python
import eggfetch as httpx

r = httpx.get("https://example.com", params={"q": "test"}, timeout=5.0)
print(r.status_code)
print(r.headers)
print(r.text)
r.raise_for_status()

with httpx.Client(headers={"User-Agent": "eggfetch"}) as client:
    r = client.post("https://example.com/api", json={"a": 1})

async with httpx.AsyncClient() as client:
    r = await client.get("https://example.com")
```

The Rust API remains idiomatic rather than Python-shaped:

```rust
use eggfetch_core::Client;

let client = Client::new();
let response = client
    .get("https://example.com")?
    .header("user-agent", "eggfetch")
    .query("q", "test")
    .send()
    .await?;

assert!(response.status().is_success());
let bytes = response.bytes()?;
```

### Milestone B: Core HTTP Engine (complete)

The core HTTP engine is implemented with the following capabilities:

- **Client / ClientBuilder** -- configurable async HTTP client with builder pattern.
- **Request / RequestBuilder** -- method, URL, headers, query parameters, and body assembly.
- **Response / ResponseBody** -- status, headers, and buffered byte body access.
- **RequestBody** -- supports byte buffers.
- **Headers** -- typed header map backed by `http::HeaderMap`.
- **HTTPS** -- TLS via rustls (default feature `tls-rustls`).
- **Error taxonomy** -- structured `Error` enum covering network, HTTP, timeout, and builder errors.

## Repository Layout

```text
Cargo.toml               workspace root
README.md
LICENSE-MIT
LICENSE-APACHE
SECURITY.md
CONTRIBUTING.md
AGENTS.md
rust-toolchain.toml      stable toolchain with rustfmt + clippy
rustfmt.toml             max_width 100
.clippy.toml             pedantic clippy config
.github/workflows/ci.yml CI pipeline
crates/
  eggfetch-core/         async HTTP engine (Milestone B complete)
  eggfetch-cli/          CLI binary (stub)
  eggfetch-python/       Python bindings (stub)
docs/
  architecture/          architecture documentation
plans/
  ROADMAP.md             full milestone roadmap
  milestone-a-repository-foundation.md
```

## Development

The workspace requires network dependencies (hyper, tokio, rustls, etc.) for eggfetch-core. To get started:

```sh
cargo check --workspace
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
```

## MSRV

The minimum supported Rust version is **1.80**. This is specified in `workspace.package.rust-version` and enforced by `rust-toolchain.toml`.

## License

eggfetch is dual-licensed under [MIT](LICENSE-MIT) and [Apache License, Version 2.0](LICENSE-APACHE). You may use this project under either license.

## Further Reading

- [plans/ROADMAP.md](plans/ROADMAP.md) -- full project roadmap, milestone sequence, correctness priorities, and release criteria.
- [plans/milestone-a-repository-foundation.md](plans/milestone-a-repository-foundation.md) -- the plan for Milestone A (workspace foundation, linting, CI, documentation).
- [plans/milestone-b-core-http-engine.md](plans/milestone-b-core-http-engine.md) -- the plan for Milestone B (core request/response model and HTTP engine).
