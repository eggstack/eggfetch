# Testing & Fuzzing Deep Dive

This document covers the testing approach, test organization, property tests, and fuzz targets.

See also: [overview.md](overview.md).

## Test Organization

Unit tests are colocated in `#[cfg(test)] mod tests` blocks within each
source file; integration tests live in `crates/eggfetch-core/tests/`
(loopback fixtures only, no public internet).

### Test Counts

Test counts change with every commit and are not duplicated here. The
evidence bound to the qualified executable SHA (Rust, Python, compat, FFI
counts) is recorded in `plans/httpx-parity-correction-status.md`.

## Running Tests

```sh
# Full Rust test suite (single-threaded: resource-stabilization tests measure
# process RSS and go flaky under concurrency)
cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1

# Core only, all features
cargo test -p eggfetch-core --all-features -- --test-threads=1

# Python tests (rebuild the extension first; requires an active venv with
# Python 3.10+, maturin, pytest, pytest-asyncio)
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop -m crates/eggfetch-python/Cargo.toml
python -m pytest crates/eggfetch-python/tests/ -q --ignore=crates/eggfetch-python/tests/compat

# HTTPX compatibility tests (requires httpx==0.28.1)
pip install -r compat/httpx/0.28.1/requirements.txt
EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
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

## HTTP/3 Hardening Tests

- Unit tests in `src/transport/http3.rs`: idle/stream-limit derivation,
  64-entry bound, generation-scoped eviction, `OnceCell` non-poisoning,
  multi-address fallback (unreachable + live), connect-phase timeout via a
  UDP blackhole, prompt cancellation, and write-timeout propagation without
  eviction.
- `tests/h3_hardening.rs` (12 tests): connect/total precedence, stalled-body
  read timeout, shared concurrent init, per-host pool gating, failure
  non-poisoning, distinct-origin stabilization, fail/reconnect cycles,
  partial-body drop reuse, client-drop release, and prompt cancellation
  with continued usability.
- `tests/h3_alt_svc_discovery.rs`: Alt-Svc discovery/suppression/fallback/draining.
- `tests/h3_interop_qualification.rs` (20 deterministic loopback controls;
  external cases are opt-in through `scripts/h3_qualification.py`): mandatory Quinn self-interop,
  GET/HEAD, buffered POST upload, 1 MiB streaming download, 20-way
  multiplexed concurrency, H3 trailers after EOF, UDP-blackhole budget,
  bad-then-good non-poisoning, server-restart reconnect, early-close /
  mid-response-drop termination with recovery, prompt cancellation,
  100-request reuse soak, fail/reconnect stabilization, 70-origin
  boundedness beyond the 64-entry cache, cancellation storms,
  construction/drop loops, exact metrics with truthful `None` H3
  metadata, IPv6 capability detection, and a doc-pinned platform-scope
  check. The external adapter path can select GET/HEAD, buffered and
  streaming upload, large streaming download, trailers, multiplexing and
  reuse; server-controlled lifecycle cases remain unsupported unless the
  adapter can induce them. Adapter identity and every unsupported case are
  retained in machine-readable results. Absence is not graduation evidence.
- H3 diagnostics are tested for a bounded 64-entry copied-snapshot store in
  `transport/metrics.rs` and for real explicit/Alt-Svc route generation,
  remote address, Quinn path packet/loss counters, UDP datagram/byte
  counters, close classification, and redaction in
  `tests/h3_alt_svc_discovery.rs`. The unavailable multiplexed stream count
  is asserted as `None`; no connection handle or peer close-reason text is
  retained.

The implementation-neutral corpus is `qualification/http3/corpus.json`.
`scripts/h3_qualification.py` validates pinned adapter identity and runs the
local controls plus selected independent-server cases. The impairment matrix
is `qualification/http3/impairment-matrix.json`; its coordinator accepts a
Linux namespace/netem or equivalent runner but is never part of Tier 1.

## HTTPX Compatibility Testing

The compatibility test suite lives in `crates/eggfetch-python/tests/compat/` and verifies eggfetch behavior against `httpx==0.28.1` and the sibling `httpx2==2.12.0` facade (`eggfetch.compat.httpx2`). Core httpx2 differentials live in `test_httpx2_api_parity.py` + `test_httpx2_behavior.py`; streaming protocols live in `test_httpx2_sse.py` (H2X-SSE-001/002) + `test_httpx2_websocket.py` (H2X-WS-001..003, in-memory fake streams + MockTransport, no network).

| File | Purpose |
|------|---------|
| `test_httpx_required.py` | Required tests that must pass; fail-closed on missing httpx |
| `test_httpx_extras.py` | Optional extras tests (HTTP/2, retry) |
| `test_behavior_cases.py` | Parametrized behavior cases with stable IDs |
| `fixtures.py` | Reusable test server and `BehaviorCase` dataclass |
| `conftest.py` | Skip auditor; fails CI on unexplained skips |

Run with `EGGFETCH_COMPAT_REQUIRED=1` for fail-closed behavior. Tier 2 (`tier2_full_compat`) enforces this.

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
| Multipart | Boundary generation, encoder output (unit tests; no proptest usage in `multipart.rs`) |

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
| `fuzz_alt_svc` | Alt-Svc header parsing, cache learn/expiry/clear bounds, trust gating, suppressor observe/suppress/recover transitions |

No Rust fuzz target exists for SSE or WebSocket framing by design
(plan `httpx2-2.12-sse-and-websocket-parity.md` §9): SSE is Python-layer
application framing over streamed responses (differential/chunk-split
corpus in `test_httpx2_sse.py`, not a Rust parser), and WebSocket framing
is owned by the maintained `wsproto` dependency (EggFetch tests focus on
adapter/lifecycle state: handshake via the normal pipeline, 101-only
stream ownership, max-message across fragments, close/cancel).

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

Routine CI tests use Python 3.12 on ubuntu-latest (single job, no matrix);
wheel builds (`pypi.yml`) cover Python 3.10–3.13 across Linux/macOS/Windows.

```sh
# Build and install (from the repo root, inside the venv)
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop -m crates/eggfetch-python/Cargo.toml

# Run tests
python -m pytest crates/eggfetch-python/tests/ -q --ignore=crates/eggfetch-python/tests/compat
```

CI must install `pytest-asyncio` explicitly.

## Resource Regression

The `eggfetch-bench` crate includes a `resource_monitor` binary that checks for resource regressions:

```sh
cargo build --release -p eggfetch-bench --bin resource_monitor
./target/release/resource_monitor
```

Outputs a JSON report with pass/fail status against predefined thresholds.

The benchmark suites themselves (Criterion `microbench`/`e2e`/`resources`, the shared `BenchServer`) are documented in [benchmarks.md](benchmarks.md).
