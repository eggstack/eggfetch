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

- [ ] The SynVoid concurrency shape is reproduced or rejected with
      eggfetch-owned evidence.
- [ ] Native 64 KiB streaming is measured at concurrency 1/2/4/8/16.
- [ ] A direct-Hyper control uses equivalent request/response geometry.
- [ ] Default/inert pool behavior is proven rather than assumed.
- [ ] Explicit logical-limit controls distinguish intended semaphore queueing.
- [ ] Per-phase evidence localizes or rejects a native pipeline contribution.
- [ ] Runtime-worker/connection-reuse controls are recorded.
- [ ] No production optimization lands before reproduction + localization.
- [ ] Any production change has before/after evidence and preserves the
      complete native body/pool/timeout contract.
- [ ] Public Rust/Python/C/CLI surfaces remain unchanged.
- [ ] No new compatibility waiver is introduced.
- [ ] Final evidence is added to `docs/architecture/benchmarks.md` only if it
      is stable and useful beyond this investigation.
- [ ] `plans/README.md` and `plans/ROADMAP.md` are reconciled at closure.

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
