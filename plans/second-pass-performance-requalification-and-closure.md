# Second-Pass Performance Requalification and Closure

Planning baseline: `b6bcc33aa1ad04dfa22af219b82601d3c1377743`
Parent program: `plans/second-pass-performance-ownership-optimization-program.md`
Normative verification policy: `docs/verification-policy.md`

## Objective

Freeze and qualify the final executable candidate for the second-pass ownership/performance campaign, compare it to the post-first-campaign baseline, prove that the public/API/compatibility surface did not regress, and close the program truthfully.

This plan owns final qualification. Child implementation plans should run focused tests and Tier 1 while developing but must not each create competing exact-SHA compatibility records.

## Part A — freeze one executable candidate

Identify the last commit that changes executable source/tests/bench harnesses for this campaign.

After the freeze, only documentation/evidence/profile/index closure may land without reopening executable qualification.

Record:

- planning baseline SHA;
- final executable SHA;
- rustc/cargo versions and exact Rust 1.89.0 MSRV result;
- Python version;
- OS/CPU;
- benchmark commands/profile/features.

## Part B — compare performance evidence

Rerun the exact comparable scenarios established in `second-pass-performance-benchmark-and-guardrails.md`.

Report separately:

- high-level request header construction at small/medium/large header counts;
- representative warm request control;
- native request header-heavy path;
- Python buffered/streaming header conversion;
- buffered iterator construction, time-to-first-item, full consumption, and RSS;
- async `aread()` large-body time/RSS;
- SOCKS/proxy controls if Plan 3 changed them;
- cookie mutation/body collection only if Plan 5 changed them.

Do not collapse results into one score.

Investigate any reproducible material regression in an unaffected representative workload. A microbenchmark win does not excuse worse warm-request, streaming, proxy, or resource behavior.

## Part C — native Rust API proof

Compare publishable Rust-facing crates to the planning baseline.

Explicitly prove:

- no removed/renamed public items;
- no changed public struct/enum field or variant shapes;
- `ResponseBody` exhaustive matches remain compatible;
- no tightened generic/trait bounds on public native request APIs;
- no feature/default removal;
- native frame/Tower/downstream fixtures compile.

Use a pinned one-time public-API/semver comparison tool or equivalent reproducible diff; do not add a permanent CI dependency solely for this campaign.

## Part D — Python and HTTPX compatibility proof

Run:

- native Python API manifest;
- Python typing surface/fixtures;
- ordinary native Python behavior;
- full HTTPX 0.28.1 compatibility;
- full HTTPX2 2.12.0 compatibility;
- both API oracles;
- lifecycle/shutdown/soak/resource controls relevant to streaming/body-state work.

No new allowed difference may be added to make an optimization pass.

Pay particular attention to:

- buffered iterator chunk/line semantics;
- UTF-8 multibyte chunking;
- response headers/cookies/charset metadata;
- `aread()` return type/content/cache;
- streaming close/cancel/consumed transitions.

## Part E — proxy/native/FFI/feature proof

Run focused and canonical checks covering:

- native frame-preserving body/trailer behavior;
- proxy/SOCKS route selection and reuse;
- CONNECT typed fallback and timeout classification;
- C FFI tests/ownership;
- no-default and documented feature profiles;
- docs/doctests;
- Node Rust tests and truthful optional JS skip;
- downstream compatibility when the existing artifact manifest is available.

Do not convert optional/missing artifacts into false PASS claims.

## Part F — repository gates

On the final executable SHA run current policy:

- `./scripts/check.sh`;
- `./scripts/check.sh extended`;
- `./scripts/check.sh package`;
- `./scripts/check_security.sh`;
- exact Rust 1.89.0 MSRV checks included by Tier 2.

Follow the live verification policy rather than copying repetition requirements from historical completed plans.

## Part G — exact-SHA compatibility records

If executable source changed after the currently recorded Stage C freeze, renew the live compatibility profiles/ledger to the final executable SHA only after complete qualification.

Update only the canonical records identified by AGENTS.md/current policy. Do not scatter hardcoded qualification SHAs into unrelated documentation.

## Part H — documentation closure

After qualification:

- update the parent program status;
- update child implementation records with measured outcomes/rejected candidates;
- update `plans/README.md`;
- update `docs/architecture/benchmarks.md` only for stable reproduction guidance/results worth retaining;
- update architecture prose only where private ownership implementation descriptions materially changed.

Do not publish a release merely because this campaign closes unless separately requested.

## Completion criteria

- [x] One final executable SHA is identified.
- [x] Comparable before/after performance evidence is recorded.
- [x] No representative material regression remains unexplained.
- [x] Native Rust public API is source-compatible with the planning baseline.
- [x] ResponseBody public shape is unchanged.
- [x] Python native manifest and typing have zero new drift.
- [x] HTTPX/HTTPX2 API oracles and full compatibility have zero new unexplained differences.
- [x] Buffered iterator/aread/header-transfer behavior is fully green.
- [x] Native/proxy ownership changes retain framing/routing/fallback/timeout semantics.
- [x] FFI, features, docs, lifecycle/resource/soak and applicable downstream controls are green or truthfully skipped per policy.
- [x] Tier 1, extended, package, security, and exact MSRV gates are green.
- [x] Exact-SHA qualification records point to the final executable freeze when renewal is required.
- [x] plans/README.md marks the program complete only after evidence is recorded.

## Stop conditions

Do not close if an optimization changes a public API, changes Python return types or iterator semantics, weakens timeout/resource/security behavior, introduces a new compatibility waiver, changes proxy fallback/routing identity, alters native body framing, or has only non-reproducible performance evidence while increasing complexity.

## Execution record — final executable freeze 2026-09-20

Final executable SHA: `1153d40c9a8a3bb380e63add1dee7601469d91f3`. Planning
baseline: `b6bcc33aa1ad04dfa22af219b82601d3c1377743`; pre-change benchmark
baseline after the requested rebase: `96d5681d6ac7e31f17b200f7feda8849de5f29b5`.
Environment: Linux x86_64, CPython 3.12.3 in `.venv`, rustc/cargo 1.98.1
for the primary run, with the exact Rust 1.89.0 MSRV checks included by the
extended gate. The qualification harness used the optimized Criterion
profile and `python scripts/performance_benchmark.py --repeats 3`; comparable
controls and outcomes are recorded in the benchmark and child-plan records.

The final candidate passed `./scripts/check.sh`, `./scripts/check.sh
extended`, `./scripts/check.sh package`, and `./scripts/check_security.sh`.
The clean package gate passed crate dry-runs, internal dependency validation,
the release wheel build/content/smoke checks, and installed-wheel typing
smoke. Security passed cargo-deny advisories/bans/licenses/sources and the
RustSec cargo-audit scan; cargo-deny retained only the existing duplicate
`getrandom` lockfile warning. Node JavaScript was skipped because the native
artifact is absent, and downstream qualification was skipped because its
artifact manifest is absent, both as required by current policy.

Native API/typing checks stayed at 66 exports, 32 exception bases, and 24
reviewed member contracts. The full pinned HTTPX 0.28.1/HTTPX2 2.12.0 suite,
API oracles, FFI, feature matrix, lifecycle/resource, soak, docs, and
frame/proxy/native controls passed without a new unexplained difference.
`ResponseBody` public variants, Python return types, proxy fallback/routing,
timeouts, framing/trailers, and public exports remain unchanged. The live
Stage C profiles, ledger, and canonical compatibility pages now point to this
final executable SHA; subsequent changes are documentation-only closure.

## Execution record — closure corrective executable freeze 2026-09-20

Final executable SHA: `bc4800ee9428f0fd11d7d0b914c489b444fe93fc`. Planning
baseline: `b6bcc33aa1ad04dfa22af219b82601d3c1377743`; pre-change benchmark
baseline after the requested rebase: `96d5681d6ac7e31f17b200f7feda8849de5f29b5`.
This corrective supersedes the prior executable freeze
`1153d40c9a8a3bb380e63add1dee7601469d91f3`, which remains historical.

The corrective restored the prior `str::lines()` behavior for a standalone
final carriage return while retaining CRLF trimming, moved the private native
URI resolver's owned `http::Uri` instead of cloning it, and added focused
cookie mutation benchmarks/tests. The retained cookie expiry watermark is
supported by the focused prebuilt-jar control: the old watermark-free path
measured approximately 185 ns, 3.43 us, and 40.7 us for 10, 1,000, and
10,000-cookie replacement mutations, while the candidate measured
approximately 220 ns, 227 ns, and 208 ns. The small-jar setup cost is accepted
because the large-jar scan is removed; equal-minimum replacement and minimum
deletion recomputation remain covered. A decoded-body capacity hint was
rejected because safe length provenance was not available at the collection
boundary.

Focused URI, cookie, and Python response tests passed; the native Python
manifest remained at 66 exports and the typing surface at 66 runtime exports,
32 exception bases, and 24 reviewed member contracts. Tier 1, extended
(including exact Rust 1.89.0 MSRV), package, and live security preflight all
passed. The full pinned HTTPX 0.28.1/HTTPX2 2.12.0 compatibility suites,
API oracles, FFI, feature, lifecycle/resource, soak, docs, and native/proxy
controls passed without new unexplained differences. Node JavaScript remains
truthfully skipped because its native artifact is absent, and downstream
qualification remains skipped because its artifact manifest is absent.
