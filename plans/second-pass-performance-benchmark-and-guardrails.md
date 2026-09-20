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

## Execution record — baseline captured 2026-09-20

Baseline SHA: `96d5681d6ac7e31f17b200f7feda8849de5f29b5` after the requested
`git pull --rebase`. Environment: Linux x86_64, CPython 3.12.3 in `.venv`,
rustc/cargo 1.98.1, default optimized Criterion profile. Commands were
`python scripts/performance_benchmark.py --repeats 3`, the native API and
typing-surface checks, and the existing 8/50/200 response-header clone
Criterion control. The API/typing checks passed (66/66 exports and the
reviewed 66-export/32-base/24-member typing surface). Existing clone medians
were approximately 180 ns, 1.05 us, and 4.13 us for 8/50/200 headers.

The new controls were then added without production changes. A short local
run of `request_ownership` recorded borrowed-rebuild versus owned-header
medians of approximately 889 ns vs 360 ns, 3.81 us vs 1.00 us, and 13.92 us
vs 3.23 us at 8/50/200 headers; native rebuild versus owned controls were
approximately 625 ns vs 363 ns, 2.53 us vs 1.03 us, and 9.44 us vs 3.27 us.
These are construction-only evidence, not end-to-end latency claims. The
extended Python cases now include buffered/streaming header mapping, lazy
iterator phase timings and process peak RSS, plus 1 KiB/1 MiB/4 MiB `aread()`
correctness controls. The exact before/after results are renewed in the final
closure record after executable work is frozen.

## Execution record — controls renewed 2026-09-20

The production changes were measured with the same loopback fixture and
optimized Python/Rust environment. The renewed harness passed all correctness
assertions, including duplicate headers, UTF-8 chunk boundaries, CRLF lines,
independent iterators, exact async `bytes` results, and 1 KiB/1 MiB/4 MiB
`aread()` bodies. The three-repeat medians were approximately 1.72 ms for
sync byte iteration, 2.62 ms for sync line iteration, 1.11 ms for buffered
response construction without text access, 2.59 ms for the first buffered
text access, and 118.8 ms for the async streaming control. Buffered byte/text/
line first-yield plus full-consumption phases were approximately 12.0 us /
0.41 ms, 6.38 ms / 23.51 ms, and 5.58 ms / 50.40 ms respectively; peak RSS
was approximately 54.1, 55.7, and 69.9 MiB for those cases. Header mapping
controls at 8/50/80 headers remained in the approximately 0.70–0.90 ms
loopback range, and async `aread()` remained approximately 1.35/2.88/7.38 ms
at 1 KiB/1 MiB/4 MiB. These loopback values are guardrails and allocation
proxies, not claims about network latency.

The cookie mutation controls were also run at 10/1,000/10,000 cookies with
the required ten-sample minimum; the medians were approximately 2.11 us,
200 us, and 2.15 ms, with no material change. A decoded-body capacity hint was
not implemented: the safe response boundary does not expose decoded length,
and using wire `Content-Length` would be incorrect for compression. This is a
deliberate rejected candidate, not an omitted measurement.
