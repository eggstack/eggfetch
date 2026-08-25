# Benchmarks Deep Dive

This document covers `eggfetch-bench` — the Criterion benchmark harnesses and the resource-regression monitor. The crate is not published and depends only on `eggfetch-core`.

See also: [overview.md](overview.md), [testing-fuzzing.md](testing-fuzzing.md) (property/fuzz testing, performance budgets).

## Layout

```
crates/eggfetch-bench/
├── src/lib.rs                    # Shared BenchServer test server + helpers
└── benchmarks/
    ├── microbench.rs             # [[bench]] core-internal microbenchmarks
    ├── e2e.rs                    # [[bench]] full-client end-to-end benchmarks
    ├── resources.rs              # [[bench]] resource-oriented benchmarks
    └── resource_monitor.rs       # [[bin]]  RSS regression monitor
```

All three benchmark suites use Criterion (`harness = false`) and are enabled by default features: `cookies`, `multipart`, and all four compression codecs; core is built with `http1`, `http2`, `tls-rustls`, `json`, `proxy`.

## BenchServer

`src/lib.rs` provides a minimal **blocking** HTTP server used by the e2e suite (and the resource monitor) so benchmarks never touch the network externally:

- Binds `127.0.0.1:0` (random free port); each connection handles exactly one request (`Connection: close`).
- `BenchServerConfig` controls: response body size, pre-response delay, chunked transfer encoding (chunk size + inter-chunk delay), and whether the request body is read-and-discarded before responding.
- Tracks a served-request counter for assertions between iterations.

A blocking server keeps the measured client the only async actor in the process — timing noise comes from the code under test, not a second runtime.

## Suites

### `microbench` — core internals

In-process costs without network I/O: URL parsing and request building, header map operations against raw `http::HeaderMap`, auth scheme construction (`BasicAuth`/`BearerAuth`), retry policy construction, and similar `eggfetch-core` object-level work.

### `e2e` — full client against BenchServer

Whole-request-path measurements over loopback TCP with `HttpVersionPolicy::Http1Only` unless explicitly testing H2:

| Group | What it measures |
|-------|------------------|
| `one_shot_get_1k` | Cold client + single GET, 1 KiB body |
| warm client | Reused client amortizing pool/TLS setup |
| `concurrent_10_get` | Ten concurrent GETs through the pool |
| body sizes | Response-size scaling |
| streaming body | Incremental consumption via `bytes_stream()` |
| `upload_256k` | Request-body upload path |
| HTTP/2 handshake | H2 connection establishment cost |
| proxy vs direct | Proxy overhead comparison |

### `resources` — allocation/throughput shape

Buffered vs streaming 1 MiB reads, long-lived client over 100 requests, pool saturation with 20 concurrent requests, parsing a 50-header response, and redirect-chain overhead. These complement `resource_monitor` by measuring throughput/memory *shape* while the monitor measures absolute peak RSS.

### Running

```sh
cargo bench -p eggfetch-bench --bench microbench
cargo bench -p eggfetch-bench --bench e2e
cargo bench -p eggfetch-bench --bench resources
```

## resource_monitor

A standalone binary that detects unbounded memory growth by measuring peak RSS across several scripted workloads (Linux `/proc/self/status`; macOS `task_info` — no unsafe, returns `None` where unavailable). It prints a JSON report to stdout with pass/fail status against predefined thresholds for CI consumption:

```sh
cargo build --release -p eggfetch-bench --bin resource_monitor
./target/release/resource_monitor
```

This is why Tier 1 runs workspace tests with `--test-threads=1` (`--workspace --exclude eggfetch-python`): resource-stabilization tests measure process RSS, and concurrent test execution makes that measurement scheduling-dependent. See [build-ci.md](build-ci.md).

## Relation to Performance Budgets

Separate from these Rust benchmarks, the HTTPX compatibility qualification enforces latency/throughput ceilings defined in `compat/httpx/0.28.1/performance-budgets.toml`. See [testing-fuzzing.md](testing-fuzzing.md).
