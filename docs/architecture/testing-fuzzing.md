# Testing & Fuzzing Deep Dive

This document covers the testing approach, test organization, property tests, and fuzz targets.

See also: [overview.md](overview.md).

## Test Organization

All tests are colocated in `#[cfg(test)] mod tests` blocks within each source file. No separate `tests/` directory for Rust integration tests.

### Test Counts

| Category | Approximate Count |
|----------|-------------------|
| Rust unit/integration | ~750+ |
| Python | ~463+ |
| FFI | ~40+ |

## Running Tests

```sh
# Full Rust test suite
cargo test --workspace --all-features

# Core only, all features
cargo test -p eggfetch-core --all-features

# Python tests (must build wheel first)
cd crates/eggfetch-python && maturin develop
python -m pytest -p pytest_asyncio
```

## Feature-Gated Test Subsets

Pre-release validation runs feature-gated subsets to ensure each feature compiles and tests independently:

```sh
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-brotli
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-zstd
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-deflate
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
```

## Property Testing

Proptest property tests are colocated in `eggfetch-core` modules. They verify round-trip invariants and state-machine correctness.

### What Property Tests Cover

| Module | Properties Verified |
|--------|---------------------|
| URL | Parse → serialize round-trip |
| Headers | Case-insensitive lookup, insertion, validation |
| Cookies | Parse → serialize, domain/path matching |
| Redirects | Method rewrite rules, header stripping |
| Retry | Backoff calculation, Retry-After parsing |
| PEM | Parse → serialize for CA bundles and client certs |
| Timeout | State machine transitions |
| Multipart | Boundary generation, encoder output |

Property tests run on stable Rust:

```sh
cargo test -p eggfetch-core --all-features
```

## Fuzz Testing

Fuzz targets live in `fuzz/fuzz_targets/` and use cargo-fuzz with libFuzzer. Nightly Rust is required.

### Targets

| Target | Subsystem |
|--------|-----------|
| `fuzz_headers` | Header parsing and validation |
| `fuzz_cookie` | Cookie parsing, matching, jar operations |
| `fuzz_redirect` | Redirect policy and replay logic |
| `fuzz_multipart` | Multipart encoder boundary and streaming |
| `fuzz_compression` | Gzip, deflate, brotli, zstd decompression |
| `fuzz_proxy` | Proxy configuration and NO_PROXY matching |
| `fuzz_proxy_response` | Proxy CONNECT response parsing |
| `fuzz_timeout` | Timeout state machine and scheduling |
| `fuzz_retry` | Retry policy, backoff, Retry-After parsing |
| `fuzz_tls` | TLS configuration and SNI handling |
| `fuzz_url` | URL parsing and normalization |

### Running Fuzz Targets

```sh
cd fuzz && cargo +nightly fuzz run <target>
cd fuzz && cargo +nightly fuzz build
```

### Harness Rules

- No external network access.
- Deterministic execution.
- Bounded memory and time.
- Operates on in-memory data structures and mock transports.

## Python Tests

CI matrix: Python 3.10–3.13 on Ubuntu, macOS, Windows (12 combinations).

```sh
# Build and install
cd crates/eggfetch-python && maturin develop

# Run tests
python -m pytest -p pytest_asyncio
```

CI must install `pytest-asyncio` explicitly.

## Resource Regression

The `eggfetch-bench` crate includes a `resource_monitor` binary that checks for resource regressions:

```sh
cargo build --release -p eggfetch-bench --bin resource_monitor
./target/release/resource_monitor
```

Outputs a JSON report with pass/fail status against predefined thresholds.
