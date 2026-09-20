# Second-Pass Performance Closure Corrective Pass

Planning baseline: `8201f35e4746cfaca23ae7f975b701a8488bb88b`
Affected executable freeze: `bc4800ee9428f0fd11d7d0b914c489b444fe93fc`
Parent program: `plans/second-pass-performance-ownership-optimization-program.md`
Prior closure record: `plans/second-pass-performance-requalification-and-closure.md`
Normative verification policy: `docs/verification-policy.md`
Date: 2026-09-20

## Objective

Correct three narrow defects discovered during post-closure review of the second-pass performance campaign:

1. restore exact buffered Python `iter_lines()` semantics for a final standalone carriage return;
2. finish the native `http::Uri` ownership optimization that the native/proxy plan currently records as complete even though the private resolver still clones the URI;
3. reconcile the cookie-mutation implementation with the campaign's evidence gate instead of retaining an unproven optimization after recording "no material change."

This corrective must remain narrow. It does not reopen the successful request-HeaderMap ownership work, standard-route URL ownership, native HeaderMap/body decomposition, SOCKS ownership, Python HeaderMap transfer, lazy buffered byte/text iteration, async `aread()` copy removal, proxy behavior, timeout semantics, or public API surfaces.

Because item 1 is a real behavior regression and items 2-3 affect the truth of the existing closure record, the current second-pass closure should be treated as provisionally superseded until this corrective is implemented and requalified.

## Finding 1 — buffered iter_lines drops a valid standalone trailing CR

### Current behavior

The new lazy `PyResponseLinesIterator::__next__` finds `\n` manually and then strips a preceding `\r` whenever `end > start` and the byte before `end` is `b'\r'`.

That correctly handles CRLF, but it also strips a final standalone carriage return when there is no following LF.

Example:

- input text: `"foo\r"`
- historical implementation: `decoded_text().lines()` -> `["foo\r"]`
- current lazy implementation: `["foo"]`

The old implementation delegated to Rust `str::lines()`; compatibility therefore requires reproducing its semantics exactly, not a generalized newline-normalization rule.

### Required correction

Change the lazy lines iterator so `\r` is stripped only when it is the carriage-return half of an actual `\r\n` delimiter.

A straightforward implementation is:

- search for the next `\n`;
- when `\n` is found, yield the range before it and strip one immediately preceding `\r`;
- when no `\n` remains, yield the remainder verbatim, including any trailing standalone `\r`;
- preserve the existing no-synthetic-trailing-empty-line behavior of `str::lines()`.

Do not replace the lazy iterator with eager list materialization.

### Required regression matrix

Add focused buffered-response tests for at least:

- `"foo\r\nbar"` -> `["foo", "bar"]`;
- `"foo\r"` -> `["foo\r"]`;
- `"foo\n"` -> `["foo"]`;
- `"\r\n"` -> `[""]`;
- `"\r"` -> `["\r"]`;
- empty string -> `[]`;
- multiple CRLF lines with a final partial line;
- repeated independent iterators over the same response.

Where practical, compare the iterator output directly against Rust/Python fixture expectations derived from the previous `str::lines()` behavior rather than duplicating the new algorithm in test code.

## Finding 2 — native URI ownership is still clone-based

### Current behavior

`send_native_http_body()` correctly consumes the caller-owned `http::Request<B>` with `request.into_parts()` and moves method, version, HeaderMap, and body.

However, it then calls:

`resolve_native_request_uri(&request_parts.uri, &transport_hints)`

The helper still accepts `&http::Uri` and:

- returns `Ok(uri.clone())` when no target override exists;
- uses `uri.clone().into_parts()` when a target override exists.

This means the plan checkbox and execution record claiming that the no-target native URI "moves through private plumbing" are not accurate.

### Required correction

Change the private native URI resolver to consume `http::Uri` by value:

`resolve_native_request_uri(uri: http::Uri, hints: &TransportHints) -> Result<http::Uri>`

Then:

- no target override: return `Ok(uri)`;
- target override: validate/parse the target, call `uri.into_parts()`, replace only `path_and_query`, then rebuild the URI.

Update `send_native_http_body()` to pass `request_parts.uri` by value after every validation/routing fact that needs to borrow the original URI has already been derived.

Do not change the public native request API, introduce `url::Url`, or alter target-smuggling validation.

### Required regression proof

Cover:

- no-target native request;
- target path/query override;
- `*` target where currently supported;
- duplicate request headers;
- body data frames and trailers;
- explicit port/proxy routing facts derived before the URI move;
- resolved-target/SNI routing;
- invalid UTF-8/forbidden target bytes retain the same error classification.

The implementation should make the ownership property obvious from source; do not add a fragile timing threshold merely to prove removal of one cheap URI clone.

## Finding 3 — cookie implementation conflicts with its evidence gate

### Current record

`benchmark-gated-cookie-and-body-buffer-tuning.md` states:

- cookie mutation optimization is permitted only if the benchmark evidence is material;
- candidates with added complexity but only measurement noise should be rejected.

Its execution record then states that 10/1,000/10,000-cookie controls showed "no material change," but the incremental expiry-watermark implementation was retained.

The code itself is conservative, but the implementation/closure record currently violates the stated decision rule.

### Corrective decision procedure

Do not resolve this by simply weakening the historical acceptance criterion.

First add or run a focused mutation microbenchmark that isolates the work the implementation is intended to remove. The existing mixed mutation control may hide the full-map recomputation behind cookie construction/parsing or bulk paths.

Measure repeated *single-cookie* operations against an equivalent baseline/reference behavior at representative jar sizes, including:

- insertion of a persistent cookie whose expiry does not become the minimum;
- replacement of a non-minimum persistent cookie;
- insertion of a new earlier minimum;
- replacement/deletion of the current unique minimum, which legitimately requires recomputation;
- equal-minimum expiries;
- session-cookie insertion.

Use at least small/medium/large jars (for example 10/1,000/10,000 entries) and enough repetitions to distinguish noise.

### Retain-or-revert rule

Retain the incremental watermark implementation only if the focused evidence shows a reproducible material benefit for the non-minimum single-mutation cases it is designed to optimize, while lookup and minimum-invalidating cases remain non-regressed.

If focused evidence still shows no material benefit:

- revert the incremental mutation logic to the simpler pre-campaign recomputation behavior;
- keep the already-qualified read-side expiry watermark optimization from the first performance campaign;
- record the second-pass cookie mutation candidate as rejected.

Do not add a heap, timer, secondary expiry index, background task, or extra synchronization structure in this corrective.

### Correctness matrix if retained

If the optimization is retained, prove:

- session cookies do not alter `next_expiry`;
- inserting a later expiry leaves the current minimum unchanged;
- inserting an earlier expiry updates the minimum;
- replacing/deleting the unique minimum recomputes correctly;
- equal minima remain correct when one is removed/replaced;
- already-expired `set()` removes an existing matching cookie without transient visibility;
- bulk `update_from_response()` still recomputes once and remains semantically unchanged;
- `cookies_for_url()`, `all_cookies()`, and `get()` never expose expired entries.

## Part 4 — closure and evidence repair

After implementing the chosen fixes, update the affected plan records so they describe what actually landed.

At minimum:

- `python-buffered-response-and-adapter-ownership-optimization.md`: record the lone-CR semantic corrective;
- `native-and-proxy-ownership-cleanup.md`: record that native URI ownership is now truly move-based;
- `benchmark-gated-cookie-and-body-buffer-tuning.md`: record focused evidence and the retain/revert decision;
- `second-pass-performance-requalification-and-closure.md`: supersede the prior executable freeze with the corrective executable SHA if any executable source/test/benchmark changes land;
- `second-pass-performance-ownership-optimization-program.md`: update final status only after qualification;
- `plans/README.md`: mark this corrective complete only after remote CI closure.

If the corrective changes executable source or executable qualification inputs, renew the live Stage C HTTPX/HTTPX2 exact-SHA records to the new executable freeze according to the repository's existing exact-SHA rule. Do not leave them pointing to `1153d40c...` after executable changes.

## Verification

During implementation run focused tests first, then the canonical repository gates on the final executable freeze.

Required focused checks:

- buffered iterator compatibility tests including standalone trailing CR;
- native HTTP body/frame/trailer/duplicate-header/target tests;
- cookie expiry-watermark unit tests;
- focused cookie mutation benchmark/control if deciding retain vs revert.

Required final gates:

- `./scripts/check.sh`;
- `./scripts/check.sh extended`;
- `./scripts/check.sh package`;
- `./scripts/check_security.sh`;
- exact Rust 1.89.0 gate through the current extended policy;
- native Python API manifest and typing surface;
- full pinned HTTPX 0.28.1 and HTTPX2 2.12.0 compatibility/oracles;
- applicable FFI, feature, lifecycle/resource, proxy/native, docs and soak checks already owned by the canonical scripts.

Remote CI must be green on the final descendant. Optional Node JS/downstream artifacts must remain truthful skips when absent.

## API and semantic invariants

- No public Rust API signature/type/variant/feature/default change.
- `ResponseBody` public variant shapes remain frozen.
- No Python public export, signature, return-type or exception change.
- The private lazy iterator classes remain private implementation details.
- `iter_lines()` matches the historical buffered-response line semantics exactly.
- Native request target validation/routing/error taxonomy remains unchanged.
- Native body framing/trailers remain frame-preserving.
- Proxy routing/fallback/auth/deadline/cache identity remains unchanged.
- No new dependency or unsafe code.
- No routine CI timing threshold.
- No reopening of HTTP/3 or Node maturation.

## Acceptance criteria

- [x] A regression test proves buffered `iter_lines()` preserves a final standalone `\r`.
- [x] CRLF, LF, empty, partial-final-line and repeated-iterator cases match historical behavior.
- [x] `resolve_native_request_uri` consumes `http::Uri` by value.
- [x] The native no-target path returns the owned URI without cloning it.
- [x] The target-override path consumes URI parts without cloning the whole URI.
- [x] Native target validation, routing, framing, trailers and duplicate headers remain green.
- [x] Focused single-cookie mutation evidence isolates the claimed optimization.
- [x] Incremental cookie mutation logic is retained only with reproducible material evidence; otherwise it is reverted.
- [x] Cookie expiry correctness remains green under insertion/replacement/deletion/equal-minimum/session/expired cases.
- [x] A new final executable freeze is recorded if executable inputs change.
- [x] Native Rust public API is unchanged.
- [x] Python native manifest/typing have zero new drift.
- [x] HTTPX/HTTPX2 have zero new unexplained compatibility drift.
- [x] Tier 1, extended, package, security and exact MSRV gates are green.
- [x] Remote CI is green on the final closure descendant.
- [x] All affected plan/index/profile/ledger records truthfully reference the final executable SHA.

## Stop conditions

Stop and split new work rather than expanding this corrective if any proposed fix requires a public API change, new production dependency, new synchronization/index structure for cookies, unsafe Python buffer handling, proxy redesign, redirect/retry redesign, or unrelated performance tuning.

## Execution record — corrective complete 2026-09-20

Executable corrective freeze: `bc4800ee9428f0fd11d7d0b914c489b444fe93fc`.
The standalone-final-CR regression, native URI move, and focused cookie
mutation evidence were completed as specified. The watermark was retained for
reproducible large-jar non-minimum replacement improvement; equal-minimum and
minimum-invalidating cases retain recomputation coverage. The decoded-body
capacity candidate was rejected because the collection boundary lacks safe
decoded-length provenance.

Tier 1, extended (including exact Rust 1.89.0 MSRV), package, security,
full HTTPX/HTTPX2 compatibility/oracle, FFI, feature, lifecycle/resource,
soak, docs, native, and proxy controls passed. Node JavaScript and downstream
qualification remain truthful skips because their artifacts are absent.
