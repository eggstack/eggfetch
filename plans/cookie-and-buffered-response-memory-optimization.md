# Cookie and Buffered Response Memory Optimization

Planning baseline: 283d52cbf438abf5051527b1c764237d701d6fb3 (main, 2026-09-20; eggfetch 0.1.9)
Parent program: plans/performance-optimization-no-api-regression-program.md
Prerequisite: plans/performance-benchmark-baseline-and-guardrails.md

## Objective

Reduce lock contention and memory amplification in two high-level policy/adaptation paths: cookie reads and buffered Python responses. Preserve RFC/compatibility behavior exactly.

These changes are grouped because both optimize retained in-memory state rather than network transport. They should remain separate commits if implementation/testing makes review clearer.

## Part A — cookie expiry watermark

### Current issue

CookieJar::cookies_for_url, all_cookies, and get call expire_stale before reading. expire_stale unconditionally takes the jar write lock and scans every cookie with retain.

For the common case of no expired persistent cookie, read-heavy workloads therefore:

1. serialize behind a write lock;
2. perform O(n) expiry scanning;
3. release it;
4. reacquire a read lock;
5. scan again for matching cookies.

### Target design

Track the earliest relevant persistent expiry inside JarInner, for example next_expiry: Option<SystemTime>.

Read paths may skip write-locked pruning when the current time is strictly before next_expiry or when no expiry exists.

When expiry is due, enter the existing write-locked prune path and recompute the next earliest expiry from surviving cookies.

Mutations must keep the watermark correct:

- update_from_response insertion/replacement/removal;
- set;
- expired replacement removal;
- set_default_cookie;
- delete;
- clear;
- explicit expire_stale.

Correctness rule: the watermark may be conservatively early and cause an extra prune, but it must never be late in a way that lets an expired cookie become observable.

### Avoid lock upgrade races

Do not hold a read lock while waiting for the write lock.

A safe pattern is:

1. read-lock and check watermark;
2. if no prune due, continue the read operation under that lock where practical;
3. if prune due, release read lock, acquire write lock, re-check current time/watermark, prune/recompute, then perform/read after release as appropriate.

Alternatively encapsulate the operation in a helper that returns a valid read guard after ensuring expiry. Keep poisoning behavior consistent with current recovery policy.

### Preserve exact cookie semantics

Recent current-head cookie corrections are part of the baseline and must not regress:

- cookie names are case-sensitive;
- domains remain case-insensitive;
- RFC path ordering is longer path first then earlier creation;
- same-name cookies on multiple matching paths remain legal;
- deduplication key semantics remain unchanged;
- host-only/default/secure/path behavior remains unchanged;
- already-expired persistent cookies are not transiently observable.

### Secondary allocation cleanup

After the watermark change is benchmarked, inspect cookies_for_url temporaries:

- avoid cloning name/path merely to populate the local seen set if borrowed keys under the jar guard can express the identical case-sensitive dedup rule;
- build the final Cookie header without a Vec<String> plus join if a direct String builder is simpler and measurably better;
- change get(name, None, None) from collecting every match to finding at most two matches if this preserves ambiguity semantics.

Keep these subordinate to clarity; do not invent a cookie index or dependency for micro-allocation savings.

## Part B — lazy Python buffered response text

### Current issue

PyResponse stores content: Bytes and text: String. from_core_response_with_body eagerly decodes the entire body before returning the Python Response.

Large binary responses therefore retain both raw body bytes and a decoded text String even when callers only use content, iter_bytes, status/headers, or save the body elsewhere.

### Target design

Make decoded text a private lazy cache initialized on first text-dependent operation.

Candidate implementation:

- keep content as Bytes;
- retain the already-computed encoding/charset metadata;
- replace eager String with OnceLock<String> or another thread-safe/private cache compatible with PyO3 class requirements and PyResponse cloning;
- .text computes exactly once and returns the cached value thereafter;
- .json continues to parse from the same decoded text semantics it uses today;
- iter_text/iter_lines on buffered PyResponse use the same cached decoded representation unless changing that would alter current behavior.

Do not switch JSON parsing to bytes merely because it is faster; that can alter encoding behavior.

### Clone/history semantics

PyResponse derives Clone and redirect history stores PyResponse values.

Tests must prove:

- cloning an undecoded response does not change Python-visible behavior;
- whether the lazy cache is shared or independently initialized is purely private and does not cause inconsistent mutation;
- history entries remain metadata-only/empty-body as today;
- .text identity/caching does not expose a mutable object or lifetime issue.

### Python property compatibility

The Python-visible .text property must remain str with the same value and exception behavior. Internal PyO3 Rust return types may change only if the generated/native API surface and typing remain unchanged.

### Memory qualification

Use baseline large binary buffered responses.

Record:

- response construction elapsed time;
- RSS/peak memory immediately after construction without .text;
- first .text access cost and peak;
- repeated .text access cost;
- JSON/text control behavior.

Success is lower construction-time memory/CPU for callers that never request text, with no material repeated-access regression once cached.

## Tests

Cookie focused cases:

- no persistent cookies;
- persistent cookies with future expiry;
- expiry exactly/just past due using deterministic time seams where possible;
- replacing earliest-expiring cookie with a later expiry;
- deleting earliest expiry;
- expired Set-Cookie removing an existing cookie;
- clear;
- concurrent readers before expiry do not require a writer;
- concurrent read during expiry transition never emits stale data;
- existing RFC ordering/case/path tests remain green.

Python focused cases:

- UTF-8 valid/invalid fallback exactly matches baseline;
- explicit charset encodings;
- binary body with no text access;
- first/repeated text;
- json with kwargs;
- iter_text/iter_lines;
- response clone/history;
- content remains exact bytes.

## Non-goals

Do not implement a browser-grade cookie index, background expiry task/timer, lock-free cookie jar, alternate cookie crate, mutable Response.text, zero-copy Python str, JSON behavior change, or eager decoding heuristic based on Content-Type.

## Exit criteria

- [ ] Cookie read paths do not take a write lock/full prune when no expiry is due.
- [ ] next-expiry bookkeeping cannot hide an expired cookie.
- [ ] Current RFC/order/case semantics remain intact.
- [ ] Concurrent cookie read benchmarks improve or at minimum remove writer serialization without throughput regression.
- [ ] PyResponse does not decode/store text until text-dependent use.
- [ ] .text/.json/iter_text/iter_lines values remain identical.
- [ ] Large binary buffered response construction shows reduced memory amplification.
- [ ] Native Python manifest and compatibility tests remain unchanged/green.
- [ ] Tier 1 is green.
