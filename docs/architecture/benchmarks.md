# Benchmarks Deep Dive

This document covers `eggfetch-bench` — the Criterion benchmark harnesses and the resource-regression monitor. The crate is not published and depends on `eggfetch-core` plus harness-only helpers (`bytes`, `futures-util`, `http`, `tokio`, `url`; dev-dependencies `criterion`, `flate2`).

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

All three benchmark suites use Criterion (`harness = false`) and are enabled by the benchmark crate's feature set: `cookies`, `multipart`, and all four compression codecs; core is built with `http1`, `http2`, `tls-rustls`, `tls-native-roots` (via default), `json`, `proxy`. This is a benchmark profile, not the core crate's default feature set. It does not answer the downstream embedding question; see [embedded-footprint.md](embedded-footprint.md) for the separate manual size/dependency qualification.

## BenchServer

`src/lib.rs` provides a minimal **blocking** HTTP server used by the e2e and resources suites (`resource_monitor` carries its own `ResourceServer`) so benchmarks never touch the network externally:

- Binds `127.0.0.1:0` (random free port); each connection handles exactly one request (`Connection: close`).
- `BenchServerConfig` controls: response body size, pre-response delay, chunked transfer encoding (chunk size + inter-chunk delay), and whether the request body is read-and-discarded before responding.
- Tracks a served-request counter for assertions between iterations.

A blocking server keeps the measured client the only async actor in the process — timing noise comes from the code under test, not a second runtime.

## Suites

### `microbench` — core internals

In-process costs without network I/O: URL parsing and request building, header map operations against raw `http::HeaderMap`, auth scheme construction (`BasicAuth`/`BearerAuth` `::new` only), retry-policy construction and retry decisions, cookie matching, multipart encoding, and decompression.

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
| `Auto` vs `Http1Only` | H1-fallback overhead (`BenchServer` is H1-only; no H2 negotiation measured) |
| proxy vs direct | Proxy overhead comparison |

### `resources` — allocation/throughput shape

Buffered vs streaming 1 MiB reads, long-lived client over 100 requests, pool saturation with 20 concurrent requests, parsing a 50-header response, and a no-redirect baseline request (`simple_request_no_redirect`; no redirect chain emitted). These complement `resource_monitor` by measuring throughput/memory *shape* while the monitor measures absolute peak RSS.

### Running

```sh
cargo bench -p eggfetch-bench --bench microbench
cargo bench -p eggfetch-bench --bench e2e
cargo bench -p eggfetch-bench --bench resources
```

## resource_monitor

A standalone binary that detects unbounded memory growth by measuring peak RSS across several scripted workloads (Linux `/proc/self/status`; macOS `ps -o rss=` subprocess — no unsafe, returns `None` where unavailable). It prints a JSON report to stdout with pass/fail status against predefined thresholds for CI consumption:

```sh
cargo build --release -p eggfetch-bench --bin resource_monitor
./target/release/resource_monitor
```

This is why Tier 1 runs workspace tests with `--test-threads=1` (`--workspace --exclude eggfetch-python`): resource-stabilization tests measure process RSS, and concurrent test execution makes that measurement scheduling-dependent. See [build-ci.md](build-ci.md).

The monitor uses a 64 MiB peak-minus-baseline delta cap and an independent
100 MiB absolute peak cap. The delta headroom accounts for normal allocator and
runtime growth across the sequential workloads; it is not a product memory
budget. The report records both values so a real absolute-growth regression
remains visible.

## Relation to Performance Budgets

Separate from these Rust benchmarks, the HTTPX compatibility qualification enforces latency/throughput ceilings defined in `compat/httpx/0.28.1/performance-budgets.toml`. See [testing-fuzzing.md](testing-fuzzing.md).

## API-safe performance campaign

The qualification-only Python harness is `scripts/performance_benchmark.py`.
It uses a deterministic loopback server and checks streamed bytes while
measuring small-chunk sync/async streaming, line iteration, and buffered
responses with and without a text read:

```sh
source .venv/bin/activate
python scripts/performance_benchmark.py --repeats 5
```

The harness is deliberately outside routine CI. Compare runs only when the
commit, toolchains, target, feature set, server fixture, and repeat settings
are identical. Timing is evidence, not a hard CI threshold.

The 2026-09-20 campaign baseline was recorded on the local Linux/x86_64
Intel Core i9-9900K host at `abf15eb97b10298fb200c2c2f4a1dceffddeb23b` and
requalified at executable freeze `18a1a432`. Matching short Criterion runs
reported response-header clone medians of 214 ns/1.14 us/4.48 us before and
178.63 ns/1.0469 us/4.1715 us after for 8/50/200 headers. Cookie lookup
medians for 10/1,000 cookies were 2.13 us/268.5 us before and 937.91
ns/105.58 us after. The Python harness medians moved from 11.31 ms to 2.232
ms for sync 1 KiB streaming, 140.23 ms to 101.16 ms for async streaming,
2.71 ms to 2.453 ms for lines, 2.64 ms to 0.987 ms for buffered no-text, and
2.62 ms to 2.170 ms for first text. These measurements are host-specific
qualification evidence, not universal performance budgets.
