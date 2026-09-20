# Post-Performance Requalification and Closure

Planning baseline: `abf15eb97b10298fb200c2c2f4a1dceffddeb23b` (main, 2026-09-20; eggfetch 0.1.9)
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

Planning baseline SHA: `abf15eb97b10298fb200c2c2f4a1dceffddeb23b`
Final executable SHA: `18a1a432` (`18a1a432` is the executable freeze; the
following documentation commits do not change production behavior).
Rust toolchain: `rustc 1.98.1`, `cargo 1.98.1`; exact MSRV `1.89.0` passed.
Python: CPython 3.12.3 in `.venv`.
Platform/CPU: Linux 6.8.0-139-generic x86_64, Intel Core i9-9900K @ 3.60 GHz.
Tier 1: PASS — 798 core tests, 568 native Python tests, 134 compatibility
smoke tests, FFI/CONNECT/doctests/API/typing checks; Node JS surface skipped
because `crates/eggfetch-node/eggfetch.node` is absent.
Extended: PASS — full compatibility, API oracles, feature matrix, MSRV,
documentation/link, resource, lifecycle, soak, merge-lossless, and benchmark
checks; Node artifact and downstream artifact manifest skipped per policy.
Package: PASS — crate dry-run/package validation, CPython 3.12 wheel smoke,
content, and typing checks.
Security: PASS — `cargo-deny` 0.19.0 and `cargo-audit` 0.22.2; existing
duplicate `getrandom` entries were reported, with advisories/bans/licenses/
sources clean.
MSRV: PASS — Rust 1.89.0 no-default, HTTP/TLS, all-feature, workspace, and
documentation checks.
Rust public API diff: no Cargo or Cargo.lock changes; no public Rust item,
feature/default, `ResponseBody` variant, error, timeout, or CLI contract
changes. Compatibility fixtures and public-shape tests passed.
Native Python manifest: PASS, 66 exports unchanged; typing surface/fixtures
also passed.
HTTPX API oracle: PASS, 71 allowed matches with zero unexplained/stale/
resolved-active drift.
HTTPX2 API oracle: PASS, 79 allowed matches with zero unexplained/stale/
resolved-active drift.
Full compat run(s): Tier 2 completed the pinned HTTPX 0.28.1 and HTTPX2 2.12.0
qualification suites plus lifecycle/soak controls; no new exception.
FFI: PASS — 34 FFI tests and package/API ownership checks.
Node: Rust tests passed; JS surface skipped because the native artifact is not
present.
Downstream: skipped because the qualification artifact manifest is not
present; no downstream result is claimed.
Benchmark summary: on the recorded baseline host, matching short Criterion
runs moved response-header clone medians from 214 ns/1.14 us/4.48 us (8/50/
200 headers) to 178.63 ns/1.0469 us/4.1715 us. Cookie lookup moved from 2.13
us/268.5 us (10/1000 cookies) to 937.91 ns/105.58 us. The Python harness
(`--repeats 5`) moved sync 1 KiB streaming from 11.31 ms to 2.232 ms, async
from 140.23 ms to 101.16 ms, lines from 2.71 ms to 2.453 ms, buffered without
text from 2.64 ms to 0.987 ms, and first text from 2.62 ms to 2.170 ms.
These are local timing evidence, not universal budgets.
Largest CPU/latency improvement: cookie matching and sync small-chunk
streaming, from the measurements above.
Largest memory/allocation improvement: lazy buffered response text and
`Bytes`-owned streaming remainders; RSS resource monitoring stayed within the
existing 64 MiB delta/100 MiB absolute caps.
Slow-consumer runtime-progress result: the bounded `SyncBridge` awaits
capacity on the producer side and passes all streaming/lifecycle/soak tests;
no separate timing budget was introduced into routine CI.
Cookie contention result: expiry watermark skips the write-locked stale scan
before the due time; cookie ordering/expiry/deduplication tests and the
small/large lookup controls passed.
Known performance regressions: none reproducible. The extended 100-sample
decompression comparison briefly reported +3.3%; an isolated matching
10-sample rerun reported -9.7%, so it was classified as measurement noise in
an unchanged control path.
Rejected/no-benefit candidates: connector/TLS residual construction was left
unchanged because no benchmark evidence justified added complexity; no Cargo
or dependency changes were needed.
Docs-only closure SHA: `5c09b46` (the documentation/status commit).

## Completion criteria

- [x] All intended executable optimization work is frozen at one SHA.
- [x] Comparable before/after evidence is recorded.
- [x] No representative material regression is left unexplained.
- [x] Native Rust public API remains source compatible with the baseline.
- [x] ResponseBody public shape is unchanged.
- [x] Python native manifest/typing/API oracles have zero new drift.
- [x] HTTPX/HTTPX2 full compatibility is green with no new exception.
- [x] Streaming backpressure/cancellation/timeout tests are green.
- [x] Cookie and lazy-text behavioral tests are green.
- [x] FFI ABI/ownership tests are green.
- [x] Feature matrix, Tier 1, extended, package, security, and MSRV are green.
- [x] Downstream qualification was checked and truthfully skipped because its artifact was unavailable.
- [x] Exact-SHA qualification records point to the final executable freeze.
- [x] plans/README.md marks this program complete after evidence is recorded.

## Stop conditions

Do not close if an optimization changes a public API, weakens timeout/resource/security semantics, requires a new compatibility waiver, makes sync streaming unbounded, hides expired cookies, changes decoding behavior, or produces only non-reproducible benchmark noise while increasing complexity.
