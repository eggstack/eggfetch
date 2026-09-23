# Native Concurrent-Streaming Tail Investigation

Planning baseline: `8959ca890ee34f4cf456aed648315322f1e83ef7` (main, 2026-09-22; coordinated 0.2.0 release state)

Triggering downstream evidence:
- SynVoid Phase 63 final evidence: `dbowm91/synvoid@efbe2dc154298646fc6e6f29865e0cebb93b2150`
- SynVoid runtime under test: `c3568ef4580a49edf222c5e4e6ce5d4dca904e81`
- SynVoid benchmark baseline: `7083f339a43dd13d6c8f65e7acc9d03ee555b6ef`
- SynVoid harness: `benchmarks/http_transport/`
- eggfetch under test there: `eggfetch-core =0.2.0`

Normative verification policy: `docs/verification-policy.md`

## Objective

Reproduce and localize the downstream concurrent-streaming tail residual inside
eggfetch's own benchmark/qualification environment, then make a narrowly
measured internal optimization only if eggfetch-owned evidence identifies an
avoidable cost.

SynVoid's stronger Phase 63 requalification found broad transport parity but
one repeatable residual for synchronized 64 KiB native streaming bursts:

- concurrency 1-2: approximately parity;
- concurrency 4: approximately 8-19% lower median throughput in the observed
  sessions;
- concurrency 8: approximately 10% lower median throughput;
- p50 stays near parity while p95/p99 worsen on the eggfetch lane;
- sequential 64 KiB phase-split measurements show near-identical
  time-to-headers and body-drain phases;
- H2 multiplexed small requests, ordinary H1 concurrency, 1 KiB/1 MiB
  streaming, slow producer, and early-drop/recovery do not show the same
  persistent shape.

That is a useful downstream signal, not yet proof of an eggfetch defect or of
a particular internal cause. The standing "pool admission" hypothesis must
not be treated as established: the default `PoolConfig` has no logical
in-flight semaphores, `Pool::needs_origin_key()` is false without a
per-origin limit, and an inert `PoolGuard` carries no response lease when no
permit/timeout state exists. The investigation must distinguish logical
admission, native-body adaptation, Hyper connection reuse, response-body
lifecycle, runtime scheduling, and benchmark-driver effects.

## Scope

Primary code paths under investigation:

- `crates/eggfetch-core/src/client.rs::Client::execute_http_body`;
- `crates/eggfetch-core/src/pipeline/mod.rs::send_native_http_body`;
- `crates/eggfetch-core/src/pool.rs`;
- `crates/eggfetch-core/src/transport/hyper_client.rs` and standard Hyper
  client construction/reuse;
- `NativeRequestBody` request-frame adaptation;
- `NativeResponseBody` response-frame/lifecycle adaptation;
- `crates/eggfetch-bench/`.

This is an API-preserving performance investigation. It is not a new feature
program.

## Non-goals

- No public Rust/Python/C/CLI API change.
- No change to HTTPX/HTTPX2 compatibility behavior.
- No change to timeout classification or pool-limit semantics.
- No weakening of frame preservation, trailers, backpressure, cancellation,
  TLS, proxy, routing, or retry controls.
- No new connection-pool implementation.
- No fork/replacement of Hyper.
- No hard performance threshold in routine CI.
- No optimization based only on SynVoid's host-specific numbers.
- No SynVoid-specific branch or adapter in eggfetch production code.
- No HTTP/3 work.

## Workstream A — Add an eggfetch-owned native streaming reproduction

Extend the existing manual benchmark surface under `crates/eggfetch-bench/`
rather than creating a second permanent framework.

Add a native-frame streaming benchmark that uses
`Client::execute_http_body` with a deterministic multi-frame request body.

Required baseline geometry:

- HTTP/1.1 keepalive;
- one shared warm client;
- 64 KiB request body;
- 16 x 4 KiB DATA frames;
- deterministic small response body, fully drained;
- concurrency 1, 2, 4, 8, 16;
- at least 5 measured repetitions per concurrency;
- each primary repetition long enough to avoid millisecond-scale quantization;
- warmup before each measured block;
- zero public network dependencies.

Also include:

- 1 KiB (4 x 256 B);
- 1 MiB (64 x 16 KiB);
- early response-body drop + recovery;
- a controlled slow-producer case;
- H2 multiplexed control using the same logical request geometry where the
  existing benchmark fixture supports it.

Keep request-body construction and response-drain boundaries identical across
all comparison controls.

Record raw per-run throughput and p50/p95/p99, not only Criterion's aggregate
mean.

## Workstream B — Establish a transport-only control

The upstream investigation needs a control that can separate eggfetch native
pipeline overhead from Hyper/runtime behavior.

Add a benchmark-only direct-Hyper control using the same Hyper/hyper-util
versions already present in the workspace and the same:

- server fixture;
- request URI/method/headers;
- body frame geometry;
- concurrency;
- response drain;
- HTTP version;
- idle-pool policy.

The direct-Hyper control is evidence machinery only. It must not become a
public eggfetch API or production escape hatch.

Where practical, reuse eggfetch's existing internal Hyper builder policy
rather than accidentally benchmarking a materially different client
configuration. If that requires exposing production internals publicly, do
not do it; create a benchmark-only crate-private/test-util bridge or record
the configuration delta explicitly.

Required comparison matrix:

1. direct Hyper body -> Hyper response;
2. eggfetch `execute_http_body` with default/inert logical pool;
3. eggfetch `execute_http_body` with an explicit per-origin limit comfortably
   above tested concurrency;
4. eggfetch high-level streaming path only as a secondary diagnostic, not as
   the native-contract baseline.

If the residual exists equally in direct Hyper, close the eggfetch-specific
optimization branch unless another eggfetch-owned cost is separately proven.

## Workstream C — Prove whether logical pool admission participates

Instrument or benchmark the pool without changing public metrics semantics.

For the default native benchmark configuration prove:

- `Pool::needs_origin_key() == false`;
- no global semaphore exists;
- no per-origin semaphore exists;
- `PoolMetrics::acquisition_waits == 0`;
- the response receives no pool lease solely for admission when no timeout
  state is configured.

Then repeat with explicit logical limits:

- global limit above concurrency;
- per-origin limit above concurrency;
- per-origin limit exactly equal to concurrency;
- constrained limit below concurrency as a positive control.

The goal is to distinguish:

- inert-pool call overhead;
- semaphore/table contention;
- intentional queueing.

Do not rewrite `Pool` merely because it appears in the call path. Existing
`needs_origin_key` and inert-lease optimizations already remove much of the
default-path ownership work.

## Workstream D — Split request phases

Add benchmark-only phase timing around the native path sufficient to localize
tail growth. Prefer test-util/benchmark instrumentation that cannot affect
normal builds.

Measure, where technically separable without semantic changes:

1. request/body construction;
2. native validation/URI/header adaptation;
3. logical pool acquisition;
4. Hyper dispatch until response headers;
5. response-body drain;
6. total request.

The SynVoid phase-split result found sequential TTH/drain parity, while the
concurrent residual appears only at concurrency >=4. Reproduce or reject that
shape upstream.

Do not add permanent high-cardinality production metrics merely to support
this investigation.

## Workstream E — Concurrency scaling and scheduler controls

For the 64 KiB native workload run at least:

- concurrency 1;
- 2;
- 4;
- 8;
- 16;
- optionally 32 if the host remains stable.

For each, record:

- throughput;
- p50/p95/p99;
- total failures;
- connections accepted by the loopback server;
- whether connections are reused;
- pool acquisition waits;
- target runtime worker count.

Run at least two Tokio runtime configurations:

- current benchmark default;
- explicitly controlled multi-thread worker count representative of
  downstream use.

If the tail effect moves primarily with runtime worker count rather than
transport path, record it as scheduler/benchmark interaction instead of
changing transport code.

## Workstream F — Profile only a reproduced residual

If eggfetch-owned runs reproduce a >5% median throughput deficit or a
persistent material p95/p99 increase relative to the direct-Hyper control,
profile before changing production code.

Preferred environments:

- Linux x86_64/aarch64: `perf`, samply, or equivalent;
- macOS arm64: Instruments or samply when available.

Candidate observation points, not assumed causes:

- `BodyExt::map_err(...).boxed_unsync()`;
- `NativeRequestBody` frame polling/write-timeout wrapper;
- request HeaderMap -> eggfetch `Headers` -> Hyper HeaderMap movement;
- inert `Pool::acquire(None)` future/call overhead;
- Hyper sender readiness / idle-pool checkout;
- response `NativeResponseBody` frame forwarding;
- task wakeups under synchronized request-body producers;
- allocator/boxing pressure;
- connection establishment/reuse behavior under concurrent H1 bodies.

Do not optimize an item merely because it is measurable. Require evidence
that it contributes materially to the reproduced residual.

## Workstream G — Optimization rules if a cause is proven

Acceptable changes are private, local, and semantics-preserving. Examples may
include:

- removing an objectively redundant allocation/box in the native-only path;
- avoiding an inert async/admission operation only if equivalence is proven;
- reducing unnecessary task wakeups or temporary ownership;
- tightening internal client reuse if current behavior is measurably
  suboptimal.

Any candidate must preserve:

- public `Client::execute_http_body` signature and behavior;
- DATA/trailer frame preservation;
- one-shot request-body semantics;
- backpressure;
- cancellation/drop behavior;
- write/read/total timeout semantics;
- logical pool limits and metrics;
- H1/H2 protocol selection;
- custom routing/TLS policy;
- existing Hyper canceled-request retry control;
- `NativeResponseBody` public opacity and lifecycle contract.

Do not add unsafe code or a new dependency solely for this optimization
without a separate justification.

## Workstream H — Evidence and downstream handback

Record an execution section in this plan containing:

- exact before SHA;
- exact candidate/final SHA;
- host/toolchain/target/profile;
- benchmark commands;
- raw/summary result paths;
- phase measurements;
- profiler method/result;
- accepted/rejected optimization candidates;
- verification results;
- final classification.

Final classification must be one of:

1. **eggfetch cause reproduced and corrected** — quantified before/after
   evidence and compatibility gates green;
2. **eggfetch cause reproduced, residual accepted** — mechanism understood,
   no safe/valuable fix;
3. **not reproduced upstream** — downstream residual remains host/harness
   specific until new evidence appears;
4. **underlying Hyper/runtime behavior** — no eggfetch workaround added.

If a correction lands, provide SynVoid with the first release/tag containing
it; do not require SynVoid to vendor a commit.

## Verification

Run focused tests/bench correctness first, then the repository's normative
tiers appropriate to executable core changes.

At minimum for an executable change:

```sh
cargo fmt --all -- --check
cargo test -p eggfetch-core
cargo test -p eggfetch-core --features test-util
cargo bench -p eggfetch-bench --bench e2e -- --noplot
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
./scripts/check_security.sh
```

Also run the relevant native-body feature slices and the Rust public API oracle
per `docs/verification-policy.md`.

If the plan closes as evidence-only/no production change, do not fabricate a
new full release qualification freeze. Record exactly which tests and manual
benchmarks were run.

## Acceptance criteria

- [x] The SynVoid concurrency shape is reproduced or rejected with
      eggfetch-owned evidence.
- [x] Native 64 KiB streaming is measured at concurrency 1/2/4/8/16.
- [x] A direct-Hyper control uses equivalent request/response geometry.
- [x] Default/inert pool behavior is proven rather than assumed.
- [x] Explicit logical-limit controls distinguish intended semaphore queueing.
- [x] Per-phase evidence localizes or rejects a native pipeline contribution.
- [x] Runtime-worker/connection-reuse controls are recorded.
- [x] No production optimization lands before reproduction + localization.
- [x] Any production change has before/after evidence and preserves the
      complete native body/pool/timeout contract.
- [x] Public Rust/Python/C/CLI surfaces remain unchanged.
- [x] No new compatibility waiver is introduced.
- [x] Final evidence is added to `docs/architecture/benchmarks.md` only if it
      is stable and useful beyond this investigation. The runner and usage are
      documented there; host-bound numeric results remain in the plan evidence.
- [x] `plans/README.md` and `plans/ROADMAP.md` are reconciled at closure.

## Stop conditions

Stop without an executable change if:

- the residual does not reproduce in eggfetch's controlled harness;
- direct Hyper shows the same scaling/tail behavior;
- results are dominated by host/runtime noise after controlled repetitions;
- the only apparent fix would change public behavior, pool limits, timeout
  semantics, frame preservation, or transport ownership;
- a proposed optimization adds complexity without a repeatable material win.

A no-change result is successful if it truthfully narrows ownership of the
residual.

## Execution and closure (2026-09-23)

### Exact revisions and environment

- Planning baseline: `8959ca890ee34f4cf456aed648315322f1e83ef7`.
- Execution baseline before implementation: `b3c009df90f9ab09e91e8fa7464653dceb300dd8`.
- Initial benchmark/evidence commit: `18a54d1ba63843ff50288606182a74f599063cb0`.
- Counter-corrected harness, refreshed evidence, and final implementation
  commit: `5be6a9ef57742245858f3fc0393e8cfaeca625de`.
- Host/target: Linux x86_64, `x86_64-unknown-linux-gnu`; release profile.
- Toolchain: `rustc 1.98.1 (48a229cea 2026-09-01)`.
- Tokio configurations: default (16 effective workers on this host) and
  controlled four workers.
- The host was shared during the investigation. The measured H1 blocks used
  2,500 requests per worker and five repetitions; H2 used 250 requests per
  worker and five repetitions. Treat the raw numbers as host evidence, not a
  portable performance guarantee.

### Benchmark and source evidence

The manual `native_streaming_tail` binary in
`crates/eggfetch-bench/benchmarks/native_streaming_tail.rs` implements the
loopback Hyper fixture, direct-Hyper controls, native body cases, phase timing,
runtime selection, and lifecycle controls. Server connection/request counters
are reset after warmup before each measured repetition, so connection/reuse
fields are per measured block. The primary commands were:

```sh
./target/release/native_streaming_tail --reps 5 --requests 2500 --max-concurrency 16 --scenario h1
./target/release/native_streaming_tail --reps 5 --requests 2500 --h2-requests 250 --max-concurrency 16 --workers 4 --scenario primary
./target/release/native_streaming_tail --reps 5 --h2-requests 250 --max-concurrency 16 --h2-only
./target/release/native_streaming_tail --reps 5 --requests 2500 --max-concurrency 4 --only-concurrency 4 --workers 4 --scenario sizes
./target/release/native_streaming_tail --reps 5 --requests 25 --workers 4 --scenario slow
```

The H2-only concurrency-four rows in the body-size control file were emitted
with the same H2-only command plus `--only-concurrency 4` and `--workers 4`.
Direct Hyper and native controls share the loopback fixture, URI, method,
request-frame geometry, concurrency, response drain, and protocol. Direct
Hyper uses `hyper-util`'s legacy client with an HTTP-only `HttpConnector`,
canceled-request retry enabled, and its default idle-pool policy. The native
client uses eggfetch's standard Hyper builder and private connector/lifecycle
wrappers plus native request/response adapters. Those internal wrapper and
adapter differences are the remaining configuration delta; they are recorded
in `docs/architecture/benchmarks.md` and were not exposed through a public
benchmark bridge.
The raw JSONL records are:

- `plans/native-concurrent-streaming-tail-default.jsonl` — default runtime,
  H1 primary matrix (direct Hyper, native default, above-limit global and
  per-origin, exact per-origin, constrained per-origin, and high-level
  secondary), concurrency 1/2/4/8/16, five repetitions each.
- `plans/native-concurrent-streaming-tail-workers4.jsonl` — same H1 primary
  matrix under four Tokio workers, plus H2 direct/native-default/above-limit
  controls at concurrency 1/2/4/8/16.
- `plans/native-concurrent-streaming-tail-h2-default.jsonl` — H2 controls on
  the default runtime at concurrency 1/2/4/8/16.
- `plans/native-concurrent-streaming-tail-controls-workers4.jsonl` — 1 KiB,
  64 KiB, and 1 MiB body geometries plus H2, at four workers and concurrency
  four.
- `plans/native-concurrent-streaming-tail-lifecycle-workers4.jsonl` — slow
  producer and early-drop/recovery controls.

Every measured run in these files reports zero request failures. H1 default
pool runs recorded zero logical acquisition waits. The default pool path has
no configured global semaphore, `needs_origin_key()` is false without a
per-origin limit, and the response lease is only attached when the guard has
permit or timeout response state. Above/equal-limit controls also recorded no
waits. The constrained positive control queued as expected; its wait counts
over five repetitions were substantial (for example, 24,995 at concurrency
two, 47,863 at four, 98,943 at eight, and 197,690 at sixteen on the default
runtime).
The pool does not explain the inert default path through semaphore contention.

The measured phase split is request/body construction, dispatch until response
headers, response drain, and total. It cannot isolate every internal operation
listed in Workstream D without production instrumentation. Construction and
drain medians were around a microsecond in the primary cases; most of the
small native-vs-direct difference appeared in dispatch-to-headers. No
high-cardinality production metrics were added.

For H1 on the default 16-worker runtime, median direct/native-default
throughput (requests/second) was: C1 15,714/15,283 (-2.7%); C2
32,991/31,275 (-5.2%); C4 57,715/52,581 (-8.9%); C8 85,126/85,956
(+1.0%); and C16 110,359/96,607 (-12.5%). At C8 and C16 native p95/p99
were lower than direct Hyper; at C1-C4 the p95/p99 increases were small. On
four workers, the same medians were C1
16,362/15,391 (-5.9%); C2 31,802/29,743 (-6.5%); C4 63,869/61,735
(-3.3%); C8 87,342/84,396 (-3.4%); and C16 99,575/98,757 (-0.8%). The
largest throughput gap therefore changes with runtime worker configuration;
the four-worker p95/p99 increases remained modest, while default-runtime p95/p99
differences changed sign at higher concurrency. The downstream synchronized
tail shape did not reproduce consistently.

The H2 controls had broadly similar direct and native results. Occasional
approximately 41 ms tail observations occurred in direct and native lanes
alike. Slow-producer timings were close between direct and native, and the
early-drop test successfully drained a subsequent request on the same
connection. Connection reuse and accepted connection counts are recorded per
run in JSONL.

### Profiling, candidate disposition, and classification

`perf` is installed, but sampling was unavailable in this environment:
`perf_event_paranoid=4` and the process lacks the required performance
profiling capabilities. `samply` was not installed. Because the concurrency
deficit moved with worker count and did not produce a stable tail regression,
there was no reproducible, localized production candidate to optimize. The
small pre-header native-path delta remains unassigned internally; no allocation,
boxing, admission, or transport change is justified without profile evidence.
No production code, public API, compatibility behavior, timeout/pool semantics,
or compatibility waiver changed.

**Final classification: not reproduced consistently / residual unlocalized.**
SynVoid's synchronized H1 tail shape was not stable in the controlled runs,
and runtime worker configuration materially changed the observed throughput
deltas. Default logical pool admission was not the cause in the tested
configuration. H2 long-tail observations appeared in direct Hyper and native
lanes. A smaller H1 native pre-header delta remains measurable in some runs,
but the comparison includes eggfetch's private connector/lifecycle and native
request/response adapters; without profiling or stronger isolation it cannot
be assigned specifically to Hyper/runtime or an eggfetch adapter. No
production optimization is justified, and no compatibility or release blocker
results. The host-bound measurements and raw JSONL evidence remain accepted.

### Verification

Completed checks:

- `cargo fmt --all -- --check` and `git diff --check` — passed.
- `cargo check -p eggfetch-bench --bin native_streaming_tail`, focused
  benchmark clippy with `-D warnings`, and optimized benchmark build — passed.
- `cargo test -p eggfetch-core -- --test-threads=1` and the same with
  `--features test-util` — each passed (848 tests).
- Native HTTP body tests — passed for all features, `native-http1`, and
  `native-http1,native-http2` feature slices. An H2-only run of this
  H1-oriented test file was attempted but its fixture panicked because the
  `http1` feature was absent; this does not indicate a product or benchmark
  failure.
- Rust public API oracle, internal documentation links, and documentation
  examples — passed.
- `./scripts/check.sh` — passed (Tier 1; Node JS surface reported its explicit
  skip because the built artifact was absent).
- `cargo bench -p eggfetch-bench --bench e2e -- --noplot` and the focused
  `proxy_overhead/proxied_get_1k` retry — both reached the existing proxy
  overhead case and failed with `HyperClient(SendRequest,
  hyper::Error(IncompleteMessage))` in the unchanged e2e proxy fixture. This
  failure is recorded rather than attributed to the new manual benchmark.

No extended, package, security publication, or release qualification tier was
run: this evidence-only closure makes no production or release change and does
not create a new release freeze.

### Downstream dependency audit

The plan index and roadmap now record this investigation as complete. A search
of the plan index and roadmap found no future plan that names this investigation
as a prerequisite. The pending issue #24 publication/tag/PyPI work, Python
3.15 wheel rehearsal, HTTPX 1.0 stable-trigger action, and independent HTTP/3
interoperability evidence remain independent. None was blocked by this plan,
so none has a status change or can be marked newly unblocked based on these
results.
