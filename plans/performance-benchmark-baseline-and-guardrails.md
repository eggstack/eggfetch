# Performance Benchmark Baseline and Guardrails

Planning baseline: 283d52cbf438abf5051527b1c764237d701d6fb3 (main, 2026-09-20; eggfetch 0.1.9)
Parent program: plans/performance-optimization-no-api-regression-program.md
Normative verification policy: docs/verification-policy.md

## Objective

Create reproducible before-change evidence for the hot paths identified by the performance audit and define compatibility guardrails before changing executable code.

This plan must land before performance implementation. Its purpose is not to optimize anything. It establishes what is being measured, prevents later benchmark drift from masquerading as an improvement, and ensures the campaign can distinguish latency/throughput gains from changed buffering, pooling, timeout, or API behavior.

## Current benchmark coverage and gap

The workspace already contains eggfetch-bench with microbench.rs, e2e.rs, and resources.rs. Existing coverage includes URL construction, header operations, cookie operations, multipart, buffered gzip decompression, retry decisions, request construction, warm requests, concurrency, body sizes, streaming TTFB/full-body behavior, uploads, HTTP-version comparison, proxy overhead, buffered-vs-streaming resources, pool saturation, response-header parsing, and redirects.

However, scripts/check.sh extended currently executes only the microbench target. More importantly, current cases do not isolate the newly identified costs:

- default logical-pool bookkeeping with no logical limits;
- response header ownership/copy cost at varying header counts;
- no-trace versus trace request dispatch preparation;
- sync Python streaming with a deliberately slow consumer;
- small Python chunk_size over large transport frames;
- decoded and raw sync/async streaming copy behavior;
- newline-dense Python line iteration;
- cookie read concurrency when no cookie is expiring;
- large binary buffered Python responses where .text is never accessed;
- FFI buffered response copy amplification;
- specialized route-client construction/miss versus cache-hit behavior.

## Required implementation

### 1. Freeze benchmark metadata

Record in this plan's closure/evidence section when implemented:

- exact baseline SHA;
- rustc/cargo versions;
- target triple and OS;
- Python version;
- release/bench profile settings;
- relevant Cargo features;
- CPU model where practical;
- benchmark command lines;
- whether runs were local native hardware, VM, or CI.

Do not compare numbers from meaningfully different machines/toolchains as though they were one before/after series.

### 2. Add narrow Rust benchmark cases

Prefer extending eggfetch-bench rather than creating a second benchmark crate.

Required cases:

1. warm tiny H1 request with default PoolConfig and no configured timeout;
2. same warm request with global-only logical admission;
3. same warm request with per-origin admission;
4. response header sets at representative sizes such as 8, 50, and 200 headers;
5. request preparation with trace absent versus trace observer installed;
6. body collection for known uncompressed Content-Length at small, medium, and multi-megabyte sizes;
7. cookie lookup for small and larger jars;
8. cookie lookup under concurrent readers where no expiry is due;
9. route-client cache hit and bounded cache-miss construction for resolved/SNI/proxy paths where fixtures already exist;
10. FFI buffered body handling at representative body sizes if it can be measured without turning Criterion into an ABI harness.

Use local deterministic servers/fixtures. Keep DNS/internet variability out of microbench evidence.

### 3. Add Python performance harnesses

Add a non-CI or qualification-oriented Python benchmark script/harness under the existing benchmark/test architecture. Do not make routine CI timing-sensitive.

Required cases:

- sync iter_bytes with upstream frames materially larger than chunk_size, including 64 KiB or larger frames split into 1 KiB/8 KiB chunks;
- async iter_bytes equivalent;
- sync/async iter_raw equivalent;
- slow sync consumer that intentionally stalls after queue fill to expose runtime-worker blocking/backpressure behavior;
- multiple concurrent slow sync streams to expose worker starvation;
- iter_lines over newline-dense input and a long partial-line sequence;
- large buffered binary response creation without .text access;
- the same response followed by first .text access;
- repeated .text access proving caching;
- representative JSON/text response controls.

Track wall time plus process RSS/peak memory where practical. The key evidence for copy-removal and lazy-text work is memory/allocation behavior, not only elapsed time.

### 4. Add behavior controls adjacent to benchmarks

Every performance case must have a correctness assertion or paired test for the semantics it could accidentally change.

At minimum:

- byte-for-byte streamed output;
- exact requested chunk-size behavior except the documented final short chunk;
- raw versus decoded transport bytes;
- cancellation/drop releases the response/pool state;
- stream errors occur at the same logical position/class;
- cookie expiry never emits an expired cookie;
- cookie ordering/deduplication remains identical;
- lazy buffered text uses the same charset/fallback decoding result;
- traced event target/method values remain identical;
- header duplicates/order semantics required by the existing surfaces remain intact.

### 5. Establish public/API baselines

Capture the current API compatibility evidence before executable changes:

- native Python API manifest result;
- current HTTPX/HTTPX2 API oracle counts;
- Cargo feature/default definitions;
- public Rust API snapshot for the publishable Rust-facing crates;
- C ABI exported function/header surface;
- relevant CLI help/exit-code surface.

A pinned one-time cargo-semver-checks or cargo-public-api invocation is acceptable for Rust baseline evidence. If used, record the exact tool version. Do not add it as a routine CI dependency unless separately requested.

### 6. Do not expand routine CI

Do not modify the repository's single routine workflow merely to run expensive performance benchmarks. scripts/check.sh extended may retain its current benchmark policy.

If benchmark commands are documented in docs/architecture/benchmarks.md, keep performance qualification clearly separate from deterministic pass/fail validation.

## Baseline scenarios to record

At minimum capture pre-change values for:

- warm tiny default request throughput/latency;
- warm request with 50 response headers;
- default request preparation/no trace;
- sync Python 1 MiB stream, 64 KiB upstream frames, 1 KiB requested chunks;
- async equivalent;
- sync slow-consumer multi-stream case;
- Python line iteration on at least 10k short lines;
- cookie reads with 10 and 1,000 stored cookies and no due expiry;
- concurrent cookie readers;
- Python buffered 10 MiB binary response construction with no text access;
- FFI buffered 10 MiB response if harnessed;
- one representative resolved/SNI/proxy client cache hit/miss case.

The exact sizes may be adjusted if the existing fixtures make another nearby size substantially easier, but preserve the scenario intent and record the final parameters.

## Acceptance criteria

- [ ] Baseline SHA and environment are recorded.
- [ ] New cases isolate the identified costs instead of only measuring whole-request internet latency.
- [ ] Python slow-consumer evidence can detect Tokio worker starvation or its absence.
- [ ] Memory-oriented cases report RSS/allocation evidence where practical.
- [ ] Every benchmark has a correctness control.
- [ ] Existing benchmark targets still build and run.
- [ ] No production behavior or public API changes in this plan.
- [ ] Tier 1 remains green.
- [ ] The benchmark harness itself does not add a production dependency.
- [ ] The benchmark documentation explains how to reproduce the campaign baseline.

## Non-goals

Do not optimize code, add hard timing thresholds to routine CI, benchmark external public endpoints, alter runtime worker counts to hide starvation, disable limits/decompression/cookies to manufacture wins, or redesign the benchmark framework beyond what is needed for this campaign.
