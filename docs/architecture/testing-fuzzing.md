# Testing & Fuzzing Deep Dive

This document covers the testing approach, test organization, property tests, and fuzz targets.

See also: [overview.md](overview.md).

## Test Organization

All tests are colocated in `#[cfg(test)] mod tests` blocks within each source file. No separate `tests/` directory for Rust integration tests.

### Test Counts

| Category | Approximate Count |
|----------|-------------------|
| Rust unit/integration | ~685 |
| Python (non-compat) | ~513 |
| Python (compat) | ~1475 |
| FFI | 30 |

## Running Tests

```sh
# Full Rust test suite
cargo test --workspace --all-features

# Core only, all features
cargo test -p eggfetch-core --all-features

# Python tests (must build wheel first)
cd crates/eggfetch-python && maturin develop
python -m pytest -p pytest_asyncio

# HTTPX compatibility tests (requires httpx==0.28.1)
pip install -r compat/httpx/0.28.1/requirements.txt
EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
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

## HTTPX Compatibility Testing

The compatibility test suite lives in `crates/eggfetch-python/tests/compat/` and verifies eggfetch behavior against `httpx==0.28.1`.

| File | Purpose |
|------|---------|
| `test_httpx_required.py` | Required tests that must pass; fail-closed on missing httpx |
| `test_httpx_extras.py` | Optional extras tests (HTTP/2, retry) |
| `test_behavior_cases.py` | Parametrized behavior cases with stable IDs |
| `fixtures.py` | Reusable test server and `BehaviorCase` dataclass |
| `conftest.py` | Skip auditor; fails CI on unexplained skips |

Run with `EGGFETCH_COMPAT_REQUIRED=1` for fail-closed behavior. The CI `compat-httpx` job enforces this.

Compatibility profiles and allowed differences live in `compat/httpx/0.28.1/`. The corrective kernel also covers buffered/one-shot redirect replay, disabled and structured timeout conversion, request-local cookies, single query serialization, incremental response decoding, raw stream lifecycle (consumed state, byte accounting, chunk-size adaptation, exactly-once close), and fail-closed lint tooling.

Direct differential tests against the pinned HTTPX reference remain Tier 2; Tier 1 does not install the reference package.

## Phase 5 Downstream Validation

Phase 5 validates eggfetch against real-world downstream consumers to ensure compatibility in practice.

### Downstream Consumer Portfolio

The `compat/downstream/` directory contains a 12-package consumer portfolio — real Python packages that depend on HTTPX or requests — tested against eggfetch to detect regressions:

```sh
python scripts/run_downstream_compat.py \
  --artifact-manifest /path/to/artifact-manifest.json \
  --required-only
```

The corrective transport matrix is in `test_socks_transport.py` and
`test_uds_transport.py`; environment precedence, socket-option boundaries,
and pinned reference behavior are covered by adjacent compatibility tests.
The isolated downstream runner qualified four release-blocking packages in
the final pass; exact-SHA evidence is recorded in the compatibility profile and
parity closure status. Private-module consumers and packages targeting HTTPX 0.27-era
signatures remain informational by design.

### Expanded Behavior Corpus

The behavior corpus grew from 24 to 29 cases in Phase 5. Parametrized tests with stable IDs verify edge cases across the HTTPX-compatible surface.

### Upstream HTTPX Test Inventory

36 derived test cases were extracted from the upstream HTTPX test suite and adapted for eggfetch. See:

- `compat/httpx/0.28.1/upstream-test-inventory.md` — catalog of upstream tests
- `compat/httpx/0.28.1/upstream-derived-cases.toml` — machine-readable mapping

### Performance Budgets

Performance budgets are defined in `compat/httpx/0.28.1/performance-budgets.toml` and enforce latency and throughput ceilings on critical paths.

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
