# Post-Performance Requalification and Closure

Planning baseline: 283d52cbf438abf5051527b1c764237d701d6fb3 (main, 2026-09-20; eggfetch 0.1.9)
Parent program: plans/performance-optimization-no-api-regression-program.md
Predecessor executable plans:
- plans/performance-benchmark-baseline-and-guardrails.md
- plans/core-hot-path-allocation-and-ownership-optimization.md
- plans/python-streaming-backpressure-and-copy-optimization.md
- plans/cookie-and-buffered-response-memory-optimization.md
Normative verification policy: docs/verification-policy.md

## Objective

Freeze one final executable candidate for the performance campaign, prove that measured improvements are real under comparable conditions, and requalify all established public/API/behavior contracts before declaring the campaign complete.

This plan owns final exact-SHA qualification. Child implementation plans should keep Tier 1 and focused tests green but must not repeatedly move the canonical compatibility freeze.

## Entry conditions

Do not begin closure until:

- the benchmark baseline plan has recorded its exact environment/scenarios;
- all intended executable optimization commits are present;
- each optimization has focused correctness tests;
- any measurement-gated connector/TLS idea is either implemented with evidence or explicitly rejected/closed;
- there are no known API-manifest changes or compatibility exceptions introduced to accommodate performance work;
- recent decompression corrective tests remain green.

If executable/test/build/dependency/validation behavior changes after the freeze, establish a new freeze and rerun affected gates.

## Part A — freeze and source audit

Record:

- final executable/test SHA;
- parent/base SHA from the program;
- complete changed-file list;
- Cargo.toml/Cargo.lock diff;
- feature/default diff;
- Python public package/stub/manifest diff;
- FFI/Node public surface diff;
- benchmark harness changes separately from production changes.

Explicitly verify no accidental changes to:

- public ResponseBody variants;
- Error variants/kinds and timeout phases;
- PoolConfig/Limits fields and alias precedence;
- retry/redirect/cookie/decompression semantics;
- Python exports, signatures, typing properties, exception hierarchy;
- C ABI function signatures/ownership;
- CLI command/help/exit mapping.

## Part B — performance evidence

Rerun every required baseline scenario under the same environment/settings.

At minimum report before/after for:

1. warm tiny default request;
2. warm 50-header response;
3. trace=None request preparation;
4. default logical-pool/no-timeout path;
5. sync Python large-frame/small-chunk bytes;
6. async equivalent;
7. sync raw/async raw representative case;
8. slow sync multi-stream runtime-progress case;
9. newline-dense Python lines;
10. cookie read small/large jars;
11. concurrent cookie readers;
12. buffered 10 MiB binary Python response without text;
13. first and repeated .text;
14. FFI large buffered response if implemented;
15. specialized route cache hit/miss if connector changes landed.

For each changed subsystem, record whether the win is CPU/time, allocation count, RSS, reduced lock contention, or runtime responsiveness.

Do not collapse all results into one synthetic score.

### Regression rule

Investigate any reproducible material regression in a representative unaffected workload. Do not accept a large general regression because one microbench improved.

No strict universal percentage is required, but a change whose only evidence is noise and which adds complexity should be reverted.

## Part C — native Rust public API proof

Compare the final candidate to the planning baseline for publishable Rust-facing crates.

Use a pinned, reproducible public-API/semver comparison tool where practical and record its version/commands. Because eggfetch is pre-1.0, do not rely solely on a tool's semver policy classification; manually inspect any public item addition/removal/change.

Required explicit proof:

- no removed/renamed public items;
- no changed public field/variant shapes;
- no tightened trait bounds/generic signatures that break downstream source;
- no feature/default removal;
- ResponseBody exhaustive matches written against 0.1.9 still compile;
- native frame/tower/downstream qualification fixtures still compile.

Purely additive private helpers are expected.

## Part D — Python and compatibility proof

Run:

- native Python API manifest checker;
- typing surface/fixture checks;
- complete native Python behavior tests;
- full HTTPX 0.28.1 compatibility suite;
- full HTTPX2 2.12.0 compatibility suite;
- both API manifest/oracle comparisons;
- merge-lossless tests;
- lifecycle/shutdown/soak tests relevant to streaming runtime changes.

No new allowed difference may be added for this campaign.

Pay particular attention to:

- streaming chunk sizes and cancellation;
- raw versus decoded compressed bodies;
- trace callbacks;
- timeout behavior under slow consumers;
- cookie ordering/expiry;
- lazy Response.text and JSON.

## Part E — FFI, Node, feature, downstream proof

Run:

- cargo test -p eggfetch-ffi --all-features;
- existing Node Rust tests and JS surface if the native artifact is available;
- no-default-features and documented feature matrix;
- docs/doctests;
- downstream compatibility workflow when its artifact manifest is available;
- any standalone native/Tower/embedded fixtures affected by ownership changes.

For the C ABI, verify exported symbols/signatures against baseline and exercise response body allocation/free ownership.

Node remains experimental; do not convert an existing truthful optional skip into a release blocker unrelated to this campaign.

## Part F — repository gates

On the final executable SHA run the current repository-required gates, including:

- ./scripts/check.sh
- ./scripts/check.sh extended
- ./scripts/check.sh package
- ./scripts/check_security.sh
- exact Rust 1.89.0 MSRV checks required by Tier 2

Record every optional skip exactly as the verification policy requires. Do not describe a skipped check as passed.

If current release policy requires repeated full compatibility passes for exact-SHA renewal, run the same count used by the live qualification convention and record all results.

## Part G — documentation and status closure

After the executable freeze is fully green, update only documentation/status records:

- this plan with benchmark tables/evidence;
- parent program status;
- plans/README.md;
- docs/architecture/benchmarks.md for stable reproduction commands/results that belong there;
- any architecture prose whose private implementation description changed materially;
- current compatibility profiles/ledger if executable changes require exact-SHA renewal under repository policy.

Do not publish a release merely because this performance campaign closes unless a separate release request exists.

## Required closure record

Fill in:

Planning baseline SHA:
Final executable SHA:
Rust toolchain:
Python:
Platform/CPU:
Tier 1:
Extended:
Package:
Security:
MSRV:
Rust public API diff:
Native Python manifest:
HTTPX API oracle:
HTTPX2 API oracle:
Full compat run(s):
FFI:
Node:
Downstream:
Benchmark summary:
Largest CPU/latency improvement:
Largest memory/allocation improvement:
Slow-consumer runtime-progress result:
Cookie contention result:
Known performance regressions:
Rejected/no-benefit candidates:
Docs-only closure SHA:

## Completion criteria

- [ ] All intended executable optimization work is frozen at one SHA.
- [ ] Comparable before/after evidence is recorded.
- [ ] No representative material regression is left unexplained.
- [ ] Native Rust public API remains source compatible with the baseline.
- [ ] ResponseBody public shape is unchanged.
- [ ] Python native manifest/typing/API oracles have zero new drift.
- [ ] HTTPX/HTTPX2 full compatibility is green with no new exception.
- [ ] Streaming backpressure/cancellation/timeout tests are green.
- [ ] Cookie and lazy-text behavioral tests are green.
- [ ] FFI ABI/ownership tests are green.
- [ ] Feature matrix, Tier 1, extended, package, security, and MSRV are green.
- [ ] Downstream qualification is green when required/available.
- [ ] Exact-SHA qualification records point to the final executable freeze.
- [ ] plans/README.md marks this program complete only after evidence is recorded.

## Stop conditions

Do not close if an optimization changes a public API, weakens timeout/resource/security semantics, requires a new compatibility waiver, makes sync streaming unbounded, hides expired cookies, changes decoding behavior, or produces only non-reproducible benchmark noise while increasing complexity.
