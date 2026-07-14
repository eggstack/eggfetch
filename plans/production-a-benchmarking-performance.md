# Production Track A Plan: Benchmarking and Performance

## Objective

Build a reproducible benchmark and profiling program for eggfetch across Rust, Python sync, Python async, CLI, direct, proxied, compressed, and protocol-specific paths. Performance work must remain subordinate to correctness and security.

## Scope

Measure:

- request latency and throughput
- connection reuse and multiplexing
- allocations and CPU
- memory under buffered and streaming load
- import/startup cost
- Python/Rust boundary overhead
- sync versus async Python behavior
- redirects, cookies/auth, multipart, compression, proxy, TLS, retry, HTTP/2 overhead

Compare against requests, HTTPX, aiohttp, and reqwest with carefully matched semantics.

## Benchmark architecture

Create:

```text
crates/eggfetch-bench/
benchmarks/
  rust/
  python/
  fixtures/
  reports/
```

Use Criterion for microbenchmarks where appropriate and purpose-built local servers for end-to-end tests. Pin CPU affinity/governor where possible and record hardware/software metadata.

## Benchmark classes

### Microbenchmarks

- URL/query construction
- header conversion
- cookie matching
- auth application
- multipart encoding
- decompression
- retry decision/backoff calculation
- Python conversion overhead

### End-to-end

- one-shot request
- warm persistent client
- concurrent async requests
- small/large bodies
- streaming first-byte and full-body times
- uploads
- HTTP/1 versus HTTP/2
- proxy forward/CONNECT
- compression codecs

### Resource tests

- peak RSS for large buffered/streaming bodies
- allocation counts if tooling permits
- file descriptor/socket behavior
- long-lived client stability

## Fairness rules

Match:

- TLS verification
- connection reuse
- decompression
- redirects
- concurrency
- body handling
- proxy/protocol settings

Publish exact commands and versions. Avoid marketing comparisons with mismatched defaults.

## Regression management

Store machine-readable baselines. Add non-flaky CI smoke benchmarks only for catastrophic regressions; run full benchmarks manually or on dedicated runners. Define thresholds per stable environment.

## Profiling

Use platform tools such as `perf`, Instruments, samply, heaptrack/dhat, and Python profiling where useful. Document hotspots before optimization.

## Optimization policy

Every optimization requires:

- measured bottleneck
- benchmark improvement
- no semantic/security regression
- tests for altered code
- dependency-cost review

## Deliverables

- benchmark harness and fixture servers
- reproducible comparison scripts
- baseline report
- performance budget for key paths
- documented proxy-reuse limitation and later benchmark if reuse is implemented

## Acceptance criteria

- benchmarks run reproducibly
- comparisons are semantically fair
- results include variance and environment metadata
- major performance claims are evidence-backed
- regression tracking exists without making CI flaky
