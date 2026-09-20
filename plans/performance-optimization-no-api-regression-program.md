# Performance Optimization Without API Regression Program

Planning baseline: 283d52cbf438abf5051527b1c764237d701d6fb3 (main, 2026-09-20; eggfetch 0.1.9)
Audit date: 2026-09-20
Normative verification policy: docs/verification-policy.md
Primary compatibility contracts: native Rust public API, Python native manifest, HTTPX 0.28.1, HTTPX2 2.12.0, C ABI, existing CLI behavior
Predecessor performance work: production-a-benchmarking-performance.md, resolved-target-route-cache-and-connection-reuse.md, hyper-idle-pool-policy-corrective-pass.md, native-pool-map-dependency-reduction.md

## Objective

Reduce avoidable CPU time, copying, allocation pressure, memory amplification, and lock/runtime contention in the current eggfetch implementation without changing the established public API surface or user-visible protocol semantics.

This is not a transport rewrite. The current architecture already has the important large-grain performance properties: Hyper owns reusable H1/H2 physical connections, route-specific clients are cached, logical pool state no longer depends on DashMap, timeout ownership is centralized, and streaming decompression has just undergone a correctness corrective. The highest-value remaining work is narrower: remove work that the current hot paths perform unnecessarily, improve Python streaming backpressure and chunk ownership, and remove avoidable cookie/body memory contention.

The program is evidence-driven. Every executable optimization must either remove an objectively redundant operation or be supported by before/after measurement. If a candidate does not produce a reproducible benefit, stop rather than increasing implementation complexity.

## Current source findings

### 1. Python sync streaming can block Tokio worker threads

crates/eggfetch-python/src/streaming.rs creates bounded std::sync::mpsc sync channels for synchronous byte, text, line, and raw-byte iterators. Their producers run as Tokio tasks and call blocking Sender::send. A slow Python consumer can fill the channel and block a runtime worker even though the producer itself is async.

The same module also converts Bytes to Vec in async/raw byte paths, splits oversized chunks by copying both the emitted prefix and remainder, and repeatedly front-drains pending Vec buffers. Small requested Python chunks over large network frames therefore create repeated copies and memmoves.

Line-oriented iteration similarly repeatedly drains from the front of String buffers.

### 2. Default logical-pool bookkeeping performs work when no logical limits exist

prepare_single_request constructs an OriginKey for every high-level request before calling Pool::acquire. Pool::acquire only uses that key for per-origin admission, but still clones the key into PoolGuard. PoolConfig::default has no logical request limits, so a default client pays String/key allocation for bookkeeping that cannot affect admission.

Final response handling also always wraps PoolGuard in Arc and attaches it to streaming response state. In the common case where there are no held logical permits and no read/total response timeout metadata, that lease is semantically inert.

These are implementation details; removing them must not change PoolMetrics, configured-limit behavior, timeout behavior, or the frozen public ResponseBody variant shapes.

### 3. Header/URI preparation contains avoidable per-request allocation

The Hyper response converter clones the full response HeaderMap before consuming the Hyper response.

The trace send path calls request.uri().to_string() before it knows whether a trace observer exists, so ordinary untraced requests allocate a URI String only to discard it.

Request-size validation similarly converts the prepared URI to String solely to obtain serialized request-target bytes/length.

Several request-building paths copy/reinsert header maps even when ownership can potentially be moved.

### 4. Cookie reads serialize through an unconditional pruning write lock

CookieJar::cookies_for_url, all_cookies, and get call expire_stale first. expire_stale takes the write lock and scans the complete map with retain even when there are no persistent cookies near expiry. cookies_for_url then takes a read lock and scans again for matches.

Under a shared cookie jar, ordinary read traffic therefore serializes through a writer and performs repeated O(n) stale scans.

### 5. Python buffered responses eagerly materialize text

PyResponse stores both response Bytes and a decoded String. from_core_response_with_body decodes the complete body at construction time regardless of whether the caller ever reads .text or uses JSON/text iteration. Binary and large-body consumers therefore pay memory and CPU for a representation they may not use.

This can be made lazy only if Python-visible behavior, charset semantics, cloning/history semantics, and exception behavior remain identical.

### 6. Additional low-risk copies remain

Examples include DirectConnector::with_resolved_target copying an address slice even though ResolvedTarget already owns an Arc-backed snapshot, MultipartEncoder copying an already-owned Bytes header slice before yielding it, and the buffered FFI response path converting core Bytes to Vec before the C ABI later performs its required caller-owned copy.

Proxy/SNI/direct connector TLS construction may also contain measurable churn on route/client misses or new proxy connections, but that work is measurement-gated because recent route-cache consolidation already amortizes most construction.

## Ordered implementation plans

Execute in this order:

1. performance-benchmark-baseline-and-guardrails.md
   - freeze reproducible pre-change benchmark/evidence cases;
   - add focused hot-path and Python streaming measurements;
   - establish API/behavior guardrails before executable optimization.

2. core-hot-path-allocation-and-ownership-optimization.md
   - skip default-path pool key/lease work that carries no state;
   - move rather than clone owned header/address/body data where safe;
   - eliminate non-traced URI formatting and request-size temporary Strings;
   - make only benchmark-supported connector/TLS residual changes.

3. python-streaming-backpressure-and-copy-optimization.md
   - remove blocking bounded-channel sends from Tokio worker tasks;
   - preserve bounded backpressure without unbounded buffering;
   - keep Bytes through Rust-side queues and split buffers without copying;
   - replace front-drain line/chunk algorithms with cursor/split ownership.

4. cookie-and-buffered-response-memory-optimization.md
   - avoid unconditional cookie write-lock pruning on read traffic;
   - reduce cookie-match temporary allocation where semantics permit;
   - lazily materialize Python buffered response text while preserving compatibility.

5. post-performance-requalification-and-closure.md
   - freeze one final executable SHA;
   - compare all performance evidence against the planning baseline;
   - prove Rust/Python/C/CLI API compatibility and behavioral parity;
   - run Tier 1, extended, package, security, exact-SHA HTTPX/HTTPX2 qualification, and downstream checks required by repository policy.

Plans 2 and 3 may be developed independently after Plan 1 lands, but do not combine them into one large commit merely for convenience. Plan 4 should follow baseline measurement and can overlap Plan 2 if edits do not conflict. Plan 5 is the sole final qualification/closure owner.

## Cross-program invariants

The following are non-negotiable:

- eggfetch-core remains the only HTTP networking engine.
- Hyper remains the physical H1/H2 connection pool; no second socket pool is added.
- No public Rust type, method, variant shape, feature name/default, Python export/signature/property, C ABI symbol/signature, or established CLI surface is removed or renamed.
- ResponseBody public variant shapes remain frozen exactly as required by AGENTS.md.
- Timeout phase meanings, total-deadline ownership, retry/redirect budgets, decompression limits/errors, raw-vs-decoded selection, trailers, upgrades, and connection-release timing remain behaviorally compatible.
- HTTPX 0.28.1 and HTTPX2 2.12.0 compatibility exceptions may not be broadened to make an optimization pass.
- Python sync and async streaming retain bounded memory/backpressure, ordering, chunk-size contracts, cancellation, single-consumption, and exception propagation.
- Cookie expiry, case sensitivity, path ordering, secure/domain/path matching, and Set-Cookie behavior remain unchanged.
- No unbounded queue is introduced to make throughput measurements look better.
- No new production dependency is added for a micro-optimization unless measurement proves that dependency is worth its footprint and maintenance cost.
- Recent decompression corrective internals are out of scope unless a benchmark exposes a separate problem and a new narrow corrective plan is written first.
- HTTP/3 and Node remain experimental; this campaign does not promote either surface.
- Routine CI topology remains unchanged unless separately requested.

## Measurement rules

Performance claims must be made only from comparable measurements:

- identical commit build profile, target, Rust/Python versions, feature set, server fixture, request count, concurrency, body/header sizes, and compression settings;
- separate cold/client-construction measurements from warm reused-client measurements;
- report median and a dispersion measure or multiple independent samples rather than one wall-clock result;
- record allocation/RSS evidence when the optimization primarily targets copying/memory rather than latency;
- do not use loopback microbench results to claim internet latency improvements;
- do not accept an optimization that wins only by changing backpressure, buffering, timeout, pooling, decoding, or error semantics.

No permanent hard performance threshold is required in routine CI. Benchmarks are qualification evidence, not a flaky pass/fail gate.

## Compatibility proof

The final candidate must demonstrate:

- unchanged native Python API manifest;
- unchanged HTTPX/HTTPX2 API-oracle results with zero new unexplained differences;
- unchanged feature/default matrix;
- no incompatible native Rust public-API delta relative to the planning baseline;
- unchanged C ABI exported function signatures and ownership rules;
- existing Node prototype tests remain green where the artifact is available;
- downstream compatibility remains green under the existing qualification workflow.

For Rust API proof, use a pinned one-time public-API/semver comparison tool or an equivalent reproducible baseline/current diff during closure. Do not add a permanent CI dependency merely to obtain this evidence. Any reported incompatibility must be investigated; do not waive it because the program is labeled performance-only.

## Program completion criteria

This program is complete when:

- baseline evidence exists before executable optimization;
- the default request path performs no logical-pool origin-key work unless per-origin policy requires it;
- an inert response lease does not allocate solely to carry no permit/timeout state;
- ordinary untraced requests avoid trace-only URI String creation;
- Hyper response/request ownership avoids proven redundant map copying where safe;
- synchronous Python streaming cannot block a Tokio worker on bounded queue backpressure;
- Python byte chunking no longer repeatedly copies/memmoves remainders in Rust;
- cookie read traffic avoids unconditional full-map write-locked pruning;
- buffered Python responses do not decode/store text until a text-dependent operation needs it;
- any connector/TLS residual optimization is supported by measurement or explicitly closed as not worth added complexity;
- no public API or semantic compatibility regression is introduced;
- final benchmark evidence shows the campaign did not regress representative warm request, streaming, cookie, and adapter workloads;
- final repository qualification is green and recorded in the closure plan.

## Non-goals

Do not turn this into HTTP/3 optimization, a new DNS cache, Happy Eyeballs redesign, a new cookie implementation, decoder rewrite, public zero-copy API, unsafe buffer sharing across Python/C boundaries, Node maturation, wholesale allocator replacement, custom executor, custom connection pool, or benchmark-driven weakening of security/resource limits.

## Closure rule

Plans 2-4 modify executable behavior and invalidate the current exact-SHA compatibility evidence. Keep Tier 1 and focused tests green while implementing, but renew exact-SHA compatibility only once on the final executable candidate in Plan 5. After that freeze, only documentation/profile/plan-index closure may land without reopening qualification.
