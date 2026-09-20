# Second-Pass Performance Benchmark and Guardrails

Planning baseline: `b6bcc33aa1ad04dfa22af219b82601d3c1377743`
Parent program: `plans/second-pass-performance-ownership-optimization-program.md`
Predecessor evidence: `plans/performance-benchmark-baseline-and-guardrails.md`
Normative verification policy: `docs/verification-policy.md`

## Objective

Create comparable post-first-campaign baseline evidence for the ownership and Python-memory paths targeted by the second-pass program before executable implementation begins.

This plan must land before Plans 2-5 make executable changes. Existing benchmark evidence remains valid for the prior campaign but does not isolate the newly identified request-header rebuild, URL ownership, native request clone, Python HeaderMap conversion, eager buffered iterators, async `aread()` copy, large-jar mutation, or safe body-capacity-hint candidates.

Do not add hard timing thresholds to routine CI.

## Required measurements

### 1. High-level H1/H2 request construction

Extend the existing benchmark harness with final prepared request construction/dispatch controls at representative outbound header counts such as 8, 50, and 200.

Measure at least:

- current borrowed `Headers` -> builder/header insertion path;
- warm tiny default H1 request end-to-end as a representative control;
- a request with default headers plus request overrides/multi-value headers;
- standard route without trace and with a trace observer to ensure trace-only costs are not conflated.

The correctness pair must verify identical method, URI, version, duplicate/multi-value headers, content length, user-agent, accept-encoding, and H2 forbidden-header behavior.

### 2. URL ownership controls

Add a narrow construction-level or allocation-oriented control that makes redundant `url::Url` cloning observable without claiming internet-latency effects.

If a stable allocation counter is not already available, use deterministic loopback repeated requests and report the optimization primarily as removal of known heap ownership work rather than fabricating a precise allocation count.

### 3. Native request ownership

Add a benchmark/control for `Client::send_native` or the existing native frame-preserving fixture with 8/50/200 request headers.

Correctness must prove:

- request URI and version are unchanged;
- duplicate headers survive;
- request body frames/trailers remain frame-preserving;
- target overrides retain current validation/routing semantics.

### 4. Python response-header conversion

Add Python qualification measurements for buffered and streaming response construction with representative header counts.

Track:

- constructor latency;
- peak RSS where practical for large header sets;
- exact header mapping/multi-value behavior.

The goal is to prove removal of the core->PyHeaders whole-map clone, not to optimize Python Headers method semantics.

### 5. Buffered Python iterator laziness

Extend `scripts/performance_benchmark.py` or an adjacent non-CI harness with large buffered response cases for:

- `iter_bytes(chunk_size=1024)`;
- `iter_text(chunk_size=1024)` on ASCII and multibyte UTF-8;
- `iter_lines()` with many short lines and one long final partial line.

Record separately:

- iterator construction time;
- time to first yielded item;
- full-consumption time;
- RSS/peak memory where practical.

Use bodies large enough to expose eager object/list materialization. The correctness control must compare the full output to current semantics.

### 6. Async aread full-body copy

Measure `await response.aread()` at multiple body sizes, including a multi-megabyte case. Track wall time and RSS/peak memory where practical.

Prove:

- result type is exactly `bytes`;
- returned bytes equal the wire-decoded body;
- subsequent content/text behavior remains compatible;
- body-state transition and repeated-consumption errors remain unchanged.

### 7. Optional cookie mutation and collection-capacity evidence

Before implementing Plan 5, add:

- CookieJar insertion/replacement/deletion sequences at 10, 1,000, and approximately 10,000 cookies, including a mix of expiring and session cookies;
- a control that verifies earliest-expiry behavior after replacing/deleting the minimum;
- streaming-to-buffer collection for known small/medium/large uncompressed decoded bodies and unknown-length bodies.

For body collection, do not use encoded compressed `Content-Length` as decoded reservation evidence. A reservation candidate must have an explicit conservative cap.

## API/behavior baseline

Before executable changes capture:

- current native Python API manifest result;
- Python typing surface result;
- HTTPX and HTTPX2 API-oracle counts;
- feature/default matrix;
- public Rust API baseline for publishable Rust-facing crates;
- C ABI surface;
- relevant CLI help surface.

Use the current repository tools rather than creating a new permanent evidence framework.

## Measurement rules

- Compare identical toolchain, build profile, target, feature set, fixture, body/header sizes, request count, concurrency, and Python version.
- Separate construction/time-to-first-item measurements from full-consumption measurements.
- Report multiple samples/median and dispersion where the harness supports it.
- Memory/copy optimizations require memory/allocation evidence or a directly provable eliminated full-size copy; latency-only noise is insufficient.
- Do not claim network latency improvements from loopback construction microbenchmarks.
- No benchmark may weaken backpressure, decoding, timeout, pooling, body-size limits, proxy validation, or API semantics.
- A candidate that adds complexity but produces only measurement noise must be rejected.

## Acceptance criteria

- [ ] Exact baseline SHA/environment/commands are recorded before implementation.
- [ ] Request-header ownership is measured at small/medium/large header counts.
- [ ] Native request ownership has a benchmark/control.
- [ ] Python header conversion is measured for buffered and streaming constructors.
- [ ] Buffered iterator construction/first-yield/full-consumption are measured separately.
- [ ] Python iterator cases include RSS/peak-memory evidence where practical.
- [ ] Async `aread()` large-body copy behavior is measured.
- [ ] Cookie mutation/body-capacity candidates have evidence before Plan 5 changes executable code.
- [ ] Every benchmark has a paired correctness assertion.
- [ ] No production behavior or public API changes in this plan.
- [ ] Tier 1 remains green.
