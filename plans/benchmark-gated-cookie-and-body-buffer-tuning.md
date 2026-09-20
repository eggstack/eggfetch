# Benchmark-Gated Cookie and Body Buffer Tuning

Planning baseline: `b6bcc33aa1ad04dfa22af219b82601d3c1377743`
Parent program: `plans/second-pass-performance-ownership-optimization-program.md`
Prerequisite: `plans/second-pass-performance-benchmark-and-guardrails.md`

## Objective

Evaluate two lower-priority residual performance candidates and implement only the portions with reproducible benefit and simple correctness arguments:

1. incremental CookieJar earliest-expiry watermark maintenance on mutation;
2. conservative capacity hints when collecting safe, known decoded response bodies.

This plan is intentionally allowed to close with no executable changes.

## Part A — cookie mutation evidence

The first performance campaign introduced `JarInner.next_expiry`, which avoids the prior read-side full-map write-locked prune. Mutation paths such as individual set/delete currently recompute the next expiry by scanning all cookies.

Use the Plan 1 mutation benchmarks to determine whether this is material at realistic large-jar sizes.

Do not optimize session-cookie/read-heavy cases merely because a synthetic 10k-cookie writer loop exists.

## Part B — incremental earliest-expiry maintenance, only if justified

If evidence is material, implement the smallest conservative algorithm.

Permitted shape:

- insertion whose persistent expiry is earlier than the current minimum updates the watermark in O(1);
- insertion/replacement not affecting the minimum leaves it unchanged;
- deletion/replacement of the current minimum may trigger a full recomputation;
- equal-minimum expiries may conservatively recompute rather than adding complex indexing/count state;
- bulk update may continue to mutate then recompute once.

Correctness is more important than eliminating every scan.

Required cookie tests:

- session cookies never affect next expiry;
- replacing the unique minimum;
- deleting the unique minimum;
- equal minimum expiries;
- inserting already-expired cookies;
- expiry pruning still never emits an expired cookie;
- cookie ordering/deduplication unchanged.

Do not add a background timer/task, heap/index, or additional synchronization structure unless evidence overwhelmingly requires it; that would need a new plan.

## Part C — decoded body collection capacity evidence

`ResponseBody::bytes()` collects streaming bodies into a fresh `BytesMut`. Determine whether a safe decoded-length hint can be threaded privately into collection without changing the frozen public `ResponseBody` variant shapes.

Any hint must satisfy all of:

- it describes decoded/uncompressed body bytes, not compressed wire length;
- it is bounded by a conservative internal reservation cap;
- it never bypasses `max_decoded_body_size`;
- absence/false metadata behaves exactly as today;
- allocation failure/malicious metadata cannot become a denial-of-service amplification.

A simple private helper that accepts an optional capacity hint is preferable to storing new public fields.

## Part D — stop if the hint requires architectural distortion

Do not:

- add fields to public ResponseBody variants;
- trust compressed `Content-Length` as decoded length;
- reserve arbitrary attacker-declared sizes;
- duplicate body-collection loops in adapters;
- bypass timeout/decompression/lease wrappers;
- add a new buffering abstraction for a small microbenchmark gain.

If the only clean path would violate those constraints, record the body-capacity candidate as rejected.

## Performance acceptance

Cookie mutation:

- require reproducible improvement at large-jar mutation workloads;
- ordinary cookie lookup must remain non-regressed;
- synchronization behavior must remain simple.

Body collection:

- require reduced reallocations/RSS or reproducible large-body collection improvement under identical limits;
- unknown-length and compressed controls must not regress materially.

## Acceptance criteria

- [x] Plan 1 evidence is reviewed before executable changes.
- [x] Cookie mutation optimization is implemented only if evidence is material.
- [x] Earliest-expiry correctness is fully tested if mutation logic changes.
- [x] Body capacity hint is explicitly rejected because decoded-length provenance is unavailable.
- [x] ResponseBody public variants remain unchanged.
- [x] No timeout/decompression/decoded-size/lease semantic change.
- [x] No new dependency/background task/index structure.
- [x] Rejected/no-benefit candidates are explicitly recorded.
- [x] Tier 1 is green if executable code changes.

## Execution record — 2026-09-20

The cookie candidate was implemented as a small private expiry-watermark
optimization. Insert, replacement, and deletion update the watermark
incrementally; bulk update/expiry paths recompute once. Focused tests cover
equal minima, expired insertions, visibility, and mutation correctness. The
ten-sample mutation controls at 10/1,000/10,000 cookies measured
approximately 2.11 us, 200 us, and 2.15 ms with no material benchmark delta;
the change was retained because it removes repeated full-jar pruning work
without changing lookup or synchronization semantics.

The decoded-body capacity candidate was explicitly rejected. The safe
collection boundary cannot prove a decoded length, and wire `Content-Length`
is not a valid capacity hint for compressed responses. `ResponseBody` remains
unchanged, so no speculative reservation, limit bypass, or duplicate buffering
loop was introduced. Tier 1 and extended validation passed.
