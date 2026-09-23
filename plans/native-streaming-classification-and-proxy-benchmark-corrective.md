# Native Streaming Classification and Proxy Benchmark Corrective

Status: active corrective handoff (2026-09-23).

Planning baseline: `105fab505d622cb24f5bb0ad5cd2bdbeb9ce54c7`.

Predecessor:
`plans/native-concurrent-streaming-tail-investigation.md`.

Normative verification policy:
`docs/verification-policy.md`.

## Objective

Close the two remaining evidence/benchmark-quality issues left after the
native concurrent-streaming investigation without reopening the eggfetch 0.2
runtime or inventing a transport optimization.

This corrective owns exactly:

1. tightening the final native-streaming classification so it does not claim a
   Hyper/runtime cause more strongly than the measurements establish; and
2. repairing and qualifying the existing `eggfetch-bench`
   `proxy_overhead/proxied_get_1k` fixture, which currently fails with
   `HyperClient(SendRequest, hyper::Error(IncompleteMessage))`.

The proxy benchmark failure is presumed to be fixture-owned until a corrected,
protocol-valid fixture reproduces a product failure.

## Current evidence and why a corrective is needed

The completed native-streaming investigation established:

- the SynVoid synchronized H1 tail shape does not reproduce consistently across
  Tokio worker configurations;
- default logical pool admission is inert for the tested native path;
- request construction and response drain are small;
- the remaining measurable native-vs-direct delta is concentrated primarily in
  dispatch-to-response-headers;
- H2 long-tail observations also occur in direct Hyper controls;
- profiling was unavailable on the execution host because
  `perf_event_paranoid=4`, and no production candidate was localized.

That is sufficient to justify **no production optimization**.

It is not sufficient to prove that every residual H1 delta is caused by
"underlying Hyper/runtime behavior". The H1 direct-Hyper versus eggfetch-native
comparison still includes eggfetch's private connector/lifecycle and native
request/response adapters. The truthful terminal description is therefore:

> The downstream synchronized tail regression was not reproduced consistently;
> the effect is runtime-sensitive, default logical admission is exonerated, H2
> long tails are not eggfetch-specific, and a small native pre-header residual
> remains unlocalized. No production change is justified without stronger
> reproducible/profile evidence.

Separately, the existing e2e proxy fixture contains a protocol-flow defect.
`BenchProxy::handle_proxy_connection` consumes the client's complete header
block before route selection. In the non-CONNECT HTTP-forwarding branch it then
attempts to read and forward the headers a second time. The first parse has
already consumed them, so the origin-facing request can be left without a
complete terminating header block until the client side times out/closes. This
is consistent with the recorded `hyper::Error(IncompleteMessage)` and must be
fixed before treating the benchmark as evidence about eggfetch proxy behavior.

## Non-goals

- No public Rust/Python/C/CLI API changes.
- No change to HTTPX/HTTPX2 compatibility bindings.
- No change to native `execute_http_body` semantics.
- No timeout, pool, TLS, retry, proxy-routing, or frame-lifecycle redesign.
- No performance optimization of the native path in this corrective.
- No new production dependency.
- No hard benchmark threshold in CI.
- No re-opening of the 0.2.0 release qualification.
- No claim that a fixed benchmark's timing is portable across hosts.
- No use of the benchmark fixture as production proxy implementation.

## Workstream A — Correct the native-streaming terminal classification

Update current authority/guidance that uses the stronger
"underlying Hyper/runtime behavior" terminal label.

At minimum inspect:

- `plans/native-concurrent-streaming-tail-investigation.md`;
- `plans/README.md`;
- `plans/ROADMAP.md`;
- `docs/architecture/benchmarks.md`.

Preserve all measured numbers and raw JSONL files.

Replace the final classification with language equivalent to:

**Not reproduced consistently / residual unlocalized.**

Required points:

- SynVoid's original synchronized H1 tail shape is not stable in eggfetch's
  controlled runs.
- Runtime worker configuration materially changes the observed throughput
  deltas.
- Default logical pool admission is not the cause in the tested configuration.
- H2 long-tail observations appear in direct Hyper and native lanes.
- A smaller H1 native pre-header delta remains measurable in some runs.
- The remaining H1 delta cannot be assigned specifically to Hyper/runtime or to
  an eggfetch adapter without profile or stronger isolating evidence.
- No production optimization is justified.
- No compatibility or release blocker results.

Do not rewrite the investigation history as "full parity" and do not delete the
accepted host-bound residual measurements.

## Workstream B — Repair the proxy benchmark fixture protocol flow

Fix `BenchProxy` in
`crates/eggfetch-bench/benchmarks/e2e.rs` so each inbound request is parsed
exactly once and the forwarded request is syntactically complete.

### Required HTTP-forwarding behavior

For a non-CONNECT request:

1. read the request line once;
2. read the header block once;
3. retain the parsed/raw headers needed for forwarding;
4. derive the upstream authority from absolute-form target and/or `Host`;
5. convert an absolute-form proxy target to the correct origin-form
   path/query when sending to the origin server;
6. write exactly one upstream request line;
7. write the retained end-to-end headers once;
8. strip proxy-only headers such as `Proxy-Connection` /
   `Proxy-Authorization` where applicable;
9. ensure a terminating `\r\n` after the forwarded header block;
10. forward a request body correctly if the fixture is ever used with one, or
    explicitly reject unsupported body-bearing requests in a deterministic way;
11. relay the upstream response back to the benchmark client without depending
    on a read timeout to delimit the request.

The current GET benchmark has no request body, but the fixture should not
silently misframe one.

### CONNECT behavior

Retain CONNECT support only if it is independently correct.

For CONNECT:

- use the request target as the tunnel authority;
- do not rely on `Host` overriding a valid CONNECT target;
- send one valid `200 Connection Established\r\n\r\n`;
- relay bidirectionally;
- terminate relay tasks cleanly when either side closes.

Do not broaden this corrective into production proxy behavior.

## Workstream C — Add deterministic fixture correctness tests

The benchmark must not be the only thing proving its own fixture.

Prefer extracting the small proxy fixture into a benchmark-local module or
`eggfetch-bench` test utility so focused tests can exercise it without
Criterion timing.

Required tests:

### Plain HTTP forwarding

- loopback origin records request line and headers;
- client sends a proxied GET using eggfetch;
- origin observes exactly one complete request;
- origin-form path/query is correct;
- `Host` is correct;
- proxy-only headers are absent where expected;
- response body reaches the client;
- no timeout is used as request framing.

### Repeated requests

- multiple proxied requests complete successfully;
- benchmark fixture does not accumulate stuck handler threads;
- connection accounting is deterministic enough for benchmark metadata.

### CONNECT smoke test

If CONNECT remains in the fixture:

- tunnel establishment succeeds against a controlled loopback target;
- bytes pass in both directions;
- shutdown completes without orphaned relay threads.

If CONNECT cannot be made deterministic in this small fixture, remove CONNECT
from the benchmark fixture and keep the e2e benchmark scoped honestly to HTTP
forward proxy overhead. Do not leave misleading comments claiming the
`proxied_get_1k` HTTP case is a CONNECT benchmark.

## Workstream D — Requalify the e2e benchmark

After fixture correction, run the focused benchmark first:

```sh
cargo bench -p eggfetch-bench --bench e2e -- proxy_overhead/proxied_get_1k --noplot
```

Then run the entire e2e benchmark:

```sh
cargo bench -p eggfetch-bench --bench e2e -- --noplot
```

Acceptance here is correctness/completion, not a particular speed number.

Record:

- exact SHA;
- host/toolchain/profile;
- focused result;
- full-e2e result;
- whether direct/proxied cases completed;
- any remaining non-fixture failure.

If the corrected fixture still produces
`HyperClient(SendRequest, hyper::Error(IncompleteMessage))`, stop and reduce
to a focused product-level proxy test before changing core code.

Do not "fix" a persistent failure by swallowing the response error in the
benchmark.

## Workstream E — Product-defect escalation gate

Only open a production proxy corrective if all are true:

- the proxy fixture emits protocol-valid requests;
- a focused non-Criterion test reproduces the failure;
- the failure occurs through eggfetch's production proxy path;
- direct origin behavior and fixture transport are known-good;
- the failing request/response bytes or protocol state identify a product-owned
  cause.

If those conditions are not met, keep this corrective entirely inside
benchmark/tests/docs.

If a genuine product defect is found, stop this plan after recording the
minimal reproducer and create a separate production plan. Do not smuggle a
transport fix into benchmark cleanup.

## Workstream F — Verification

For benchmark/test/docs-only changes run at minimum:

```sh
cargo fmt --all -- --check
cargo check -p eggfetch-bench
cargo clippy -p eggfetch-bench --all-targets -- -D warnings
cargo test -p eggfetch-bench
cargo test -p eggfetch-core -- --test-threads=1
cargo bench -p eggfetch-bench --bench e2e -- proxy_overhead/proxied_get_1k --noplot
cargo bench -p eggfetch-bench --bench e2e -- --noplot
./scripts/check.sh
```

Also run `git diff --check` and the repository's internal documentation-link
checks if they are not already included in Tier 1.

Extended/package/security/release qualification is not required for a
benchmark/test/docs-only correction unless executable production code changes.

If production code changes, this plan is no longer sufficient: create the
separate product corrective and run the verification tiers required there.

## Workstream G — Closure documentation

Append an execution/closure section to this plan recording:

- fixture root cause;
- files changed;
- focused fixture tests;
- benchmark results;
- classification wording correction;
- verification commands/results;
- whether any product defect was discovered;
- final proof-bearing SHA.

Update:

- `plans/README.md`;
- `plans/ROADMAP.md`;
- `docs/architecture/benchmarks.md`;
- predecessor investigation wording where necessary.

At closure the plan index must not claim an active cleanup remains.

## Acceptance criteria

- [ ] The native-streaming final classification no longer over-attributes the
      residual to Hyper/runtime.
- [ ] Raw benchmark evidence remains unchanged.
- [ ] The remaining H1 pre-header delta is described as unlocalized.
- [ ] `BenchProxy` parses each HTTP request header block only once.
- [ ] Forwarded HTTP requests are syntactically complete without timeout-based
      framing.
- [ ] Absolute-form targets are forwarded to origins in correct origin form.
- [ ] Proxy-only headers are handled deliberately.
- [ ] Focused fixture correctness tests pass.
- [ ] `proxy_overhead/proxied_get_1k` completes without
      `IncompleteMessage`.
- [ ] Full `e2e` benchmark completes, or any remaining failure is isolated
      and truthfully classified.
- [ ] No production proxy/native transport change is made without a separate
      product-defect plan.
- [ ] Public API/compatibility surfaces remain unchanged.
- [ ] Plan index/roadmap/benchmark docs are reconciled at closure.

## Rejection criteria

Reject this corrective if it:

- labels the small H1 residual as definitely Hyper/runtime without new proof;
- claims full parity by deleting inconvenient measurements;
- removes the proxy benchmark merely to make the suite green;
- swallows `IncompleteMessage` or response errors;
- keeps the double-read header flow;
- relies on a socket read timeout to terminate an HTTP request header block;
- turns benchmark helper code into a public production API;
- changes core proxy behavior before a protocol-valid fixture reproducer
  exists;
- adds hard host-specific timing thresholds to CI.

## Expected terminal state

- The native-streaming investigation remains closed with a precise
  "not consistently reproduced / residual unlocalized" conclusion.
- The proxy e2e benchmark fixture is protocol-correct and the full benchmark
  suite no longer carries the known `IncompleteMessage` fixture failure.
- No production eggfetch behavior changes unless a separately planned,
  independently reproduced product defect is discovered.
