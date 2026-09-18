# Total Deadline Public-API Compatibility and Closure Corrective Pass

Planning baseline: `dd52f8c4a8d54fbaa403a7995a41bf60b235f7ac` (`main`, 2026-09-18)
Parent plan: `plans/total-deadline-response-body-lifecycle-corrective.md`
Original pre-corrective API baseline: `60a6e2b384519507e04cf296ffd484388e872e47`
Status: active follow-up corrective — required before the parent timeout line can close or downstream consumers are told to rely on the release

## Objective

Keep the behavioral fix from `dd52f8c4` — one native `Timeout.total` deadline from logical request start through response-body EOF/trailers — while correcting the public Rust API regression introduced by that implementation, strengthening weak proofs, adding the missing retry-to-final-body aggregate-deadline regression, and completing the qualification/release closure required by the parent plan.

This is a narrow corrective. Do not reopen the timeout model, recent linked-footprint feature work, proxy architecture, HTTPX semantics, or unrelated response-body design.

Desired end state:

1. `Timeout.total` retains the corrected post-header/body semantics from `dd52f8c4`.
2. The public `ResponseBody` enum has the same externally destructurable variant field shape as eggfetch-core 0.1.6 / baseline `60a6e2b3`.
3. Tests can actually distinguish the desired deadline behavior from restarted/headers-only alternatives.
4. The parent plan receives truthful closure evidence on one final executable SHA.
5. A coordinated patch is published before downstream handoff is marked complete.

## Triggering findings

### Public ResponseBody shape regressed

Before `dd52f8c4` the public variants were:

```rust
ResponseBody::Streaming {
    stream,
    lease,
}

ResponseBody::EncodedStreaming {
    stream,
    lease,
    content_encoding,
    limit,
}
```

The implementation added public `read_timeout` and `total_deadline` fields to both variants.

Because `ResponseBody` is public and not `#[non_exhaustive]`, downstream Rust code may destructure these variants exhaustively. Adding fields makes previously valid source fail with E0027. That contradicts the parent plan's explicit no-public-API-change requirement and must not ship in a patch release.

### Redirect proof is too permissive

The current `redirect_final_body_uses_remaining_original_budget` test permits:

```text
elapsed < total + 1200 ms
```

with a 500 ms total and roughly 200 ms first-hop delay. A broken implementation that grants a fresh 500 ms total after final response headers can still fit inside this bound.

### Native pool-release assertion is tautological

The current native lease regression ends with:

```rust
assert!(second.is_ok() || second.is_err());
```

The outer Tokio timeout proves the future returned, but this assertion proves nothing about the actual second request result.

### Retry -> final body aggregate deadline is still unproved

The repository already has deterministic retry integration fixtures, so the parent plan's retry proof should now be explicit instead of remaining an unrecorded omission.

### Release closure remains incomplete

Routine push CI is green for `dd52f8c4`, but the parent plan remains active. Extended/package/security, exact-SHA HTTPX/HTTPX2 renewal, baseline evidence, and coordinated publication remain unrecorded on a final corrected executable SHA.

## Part A — Add a compile-time public-shape regression first

Before production edits, add an integration test compiled as an external crate:

```text
crates/eggfetch-core/tests/response_body_public_shape.rs
```

It should accept a `ResponseBody` and exhaustively match the published 0.1.6/pre-corrective shapes without `..`:

```rust
fn consume_shape(body: ResponseBody) {
    match body {
        ResponseBody::Buffered { bytes } => {
            let _ = bytes;
        }
        ResponseBody::Streaming { stream, lease } => {
            let _ = (stream, lease);
        }
        ResponseBody::EncodedStreaming {
            stream,
            lease,
            content_encoding,
            limit,
        } => {
            let _ = (stream, lease, content_encoding, limit);
        }
        ResponseBody::Consumed => {}
    }
}
```

Record three states:

1. `60a6e2b3`: this pattern compiles.
2. `dd52f8c4`: the same pattern is red because the two timeout fields were added.
3. Final corrected implementation: green again.

Do not add `trybuild` or another dependency. A normal integration test already compiles as an external crate.

Acceptance:

- [ ] The compatibility test is demonstrably red on `dd52f8c4`.
- [ ] The identical source pattern is green against the pre-corrective API and final correction.
- [ ] No `#[non_exhaustive]` shortcut is used; adding it would itself break existing exhaustive matches.

## Part B — Restore the exact public ResponseBody shape

The corrected public enum must return to the pre-corrective field shape:

```rust
pub enum ResponseBody {
    Buffered { bytes: Bytes },
    Streaming {
        stream: BoxBytesStream,
        lease: Option<...>,
    },
    EncodedStreaming {
        stream: BoxBytesStream,
        lease: Option<...>,
        content_encoding: String,
        limit: DecompressionLimit,
    },
    Consumed,
}
```

No additional public fields or variants may carry timeout state.

Do not solve this by:

- marking the enum non-exhaustive;
- adding a new timeout-bearing public variant;
- changing the public types of `stream`, `lease`, `content_encoding`, or `limit`;
- changing the public `BoxBytesStream` alias;
- hiding the regression behind a breaking version bump;
- weakening `Timeout.total` back to headers-only semantics.

## Part C — Move timeout metadata behind private lifecycle ownership

Read/total metadata still needs to survive until the caller selects and polls raw or decoded body consumption. Move that state behind a crate-private lifecycle owner rather than public enum fields.

### Preferred direction

Prefer an existing private lifecycle owner instead of a global registry.

A reasonable design is to extend private state behind the response's existing lease:

- `PoolGuard` is already a public type with private fields and already owns the streaming response lifetime;
- add a small crate-private response-lifecycle policy/state behind that owner, or use an equivalently private existing owner;
- initialize read timeout + absolute `ResponseDeadline` before the guard is placed behind the existing lease `Arc`;
- when `ResponseBody::take_stream()` selects raw or decoded consumption, read that private policy from the lease and construct the existing final `BodyTimeoutStream` outside the selected stream;
- retain current lease release behavior on EOF/error/timeout/drop.

An alternative private carrier is acceptable only if it preserves all of these:

- zero public variant/type-shape change;
- no global side table or pointer-keyed registry;
- no second response/body engine;
- no duplicated raw-vs-decoded timeout implementations;
- no Tokio `Sleep` carried across runtimes;
- final selected raw and decoded streams remain covered;
- manually constructed `ResponseBody::streaming()` values retain no implicit client timeout.

If extending `PoolGuard`, keep permit semantics conceptually separate with a small private response-lifecycle sub-struct. Do not scatter timeout fields through semaphore logic.

Acceptance:

- [ ] `ResponseBody` matches the old public field shape exactly.
- [ ] Timeout metadata is entirely private.
- [ ] `BodyTimeoutStream` remains the authoritative high-level response timeout implementation.
- [ ] Raw and decoded compressed modes retain final-boundary timeout enforcement.
- [ ] No dependency, public API, MSRV, or feature change is introduced.

## Part D — Preserve the dd52 behavioral fix

Restoring API shape is not sufficient. Retain the behavioral coverage introduced by `dd52f8c4`:

- headers before total, body after total -> `TimeoutPhase::Total`;
- post-first-chunk stall with no read timeout -> `Total`;
- continuous progress under read inactivity but beyond total -> `Total`;
- delayed first body poll after total -> `Total` before accepting ready inner data;
- shorter read -> `Read`;
- shorter total -> `Total`;
- `bytes()`;
- `bytes_stream()`;
- `text()` and JSON when enabled;
- raw compressed streaming;
- decoded compressed buffered + streaming paths;
- trailers delayed beyond total;
- high-level pool-lease release;
- native frame total/read precedence and delayed first poll;
- lean `standard-http1` total-body behavior.

Do not weaken `BodyTimeoutStream` tests to make the API fix easier.

## Part E — Strengthen redirect remaining-budget proof

Replace the broad elapsed assertion with a discrimination window that fails if a fresh total is assigned after the redirect.

Robust structure:

1. configure total around 1–1.5 seconds;
2. first hop consumes most of it;
3. final response sends headers immediately then stalls well beyond both possible deadlines;
4. after final headers, wrap body consumption in a test-only timeout that is comfortably longer than the true remaining budget but comfortably shorter than a newly restarted full total;
5. require `resp.bytes()` to return `TimeoutPhase::Total` inside that window.

Example numbers, not mandatory:

```text
total = 1500 ms
first-hop delay ~= 1000 ms
remaining original budget ~= 500 ms
external discrimination timeout ~= 900 ms
fresh restarted total ~= 1500 ms after final headers
```

Acceptance:

- [ ] The test fails if the final response gets a fresh total.
- [ ] CI jitter tolerance remains generous.
- [ ] The result must be `TimeoutPhase::Total`, not merely the external test timeout.

## Part F — Add retry -> final body remaining-budget proof

Use `crates/eggfetch-core/tests/retry_integration.rs` rather than a new retry harness.

Create a deterministic case where:

1. attempt 1 consumes a material fraction of `Timeout.total` and ends in an explicitly retryable status/failure;
2. retry policy allows attempt 2 with deterministic/small backoff;
3. attempt 2 returns successful headers before the original total expires;
4. attempt 2's body stalls or trickles;
5. body consumption crosses the original logical deadline.

Require:

- at least two observed attempts;
- final headers were returned successfully;
- final body fails with `TimeoutPhase::Total`;
- a discrimination window proves it received only the remaining original budget, not a fresh total.

Do not test Hyper's hidden canceled-idle retry here. This is the explicit eggfetch logical retry path.

Acceptance:

- [ ] Attempt count is asserted.
- [ ] Final body uses remaining original total.
- [ ] Existing retry/backoff/replayability behavior remains unchanged.

## Part G — Fix the native pool-lease proof

Replace the tautological assertion in `native_total_timeout_releases_pool_lease`.

Require:

1. first native response owns the only same-origin logical permit;
2. its body terminates with `TimeoutPhase::Total`;
3. the second same-origin native request is admitted after release;
4. it reaches response headers within a generous outer timeout;
5. the result is explicitly successful and has the expected status, normally 200.

Suggested shape:

```rust
let response = tokio::time::timeout(..., second_request)
    .await
    .expect("second request admitted after total timeout")
    .expect("second request should reach response headers");
assert_eq!(response.status(), StatusCode::OK);
```

Acceptance:

- [ ] No `is_ok() || is_err()` tautology remains.
- [ ] Successful second-request admission/headers are actually proven.

## Part H — Rationalize duplicate read-timeout code

`stream/read_timeout.rs` is no longer the production high-level response timeout owner; production uses `BodyTimeoutStream`.

Audit whether `ReadTimeoutStream` still has a current production or unique test purpose.

Preferred outcome:

- migrate useful focused read tests to `BodyTimeoutStream`;
- remove `read_timeout.rs` and the dead-code compatibility helper when no production caller remains.

If retained, record the concrete reason and ensure it cannot silently become a second production path.

Do not expand this into a general stream-module refactor.

## Part I — Record baseline evidence, not comments

### Behavioral baseline

Reproduce at least the core headers-before-total/body-after-total failure against `60a6e2b3` in a disposable worktree or equivalent isolated checkout using the regression test.

Record:

```text
baseline SHA:
test source/patch:
command:
observed baseline result:
corrected result:
```

### Public API regression

Record:

- `60a6e2b3` exact destructuring pattern: green;
- `dd52f8c4` exact destructuring pattern: red;
- final corrective SHA: green.

Do not modify historical commits.

## Part J — Focused validation

Run at minimum:

```sh
cargo test -p eggfetch-core --all-features --test response_body_public_shape -- --test-threads=1
cargo test -p eggfetch-core --all-features --test total_body_deadline_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test retry_integration -- --test-threads=1
cargo test -p eggfetch-core --all-features --test native_http_body_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test native_tower_service_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test trailer_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test pool_tests -- --test-threads=1
cargo test -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --test lean_route_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test proxy_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test h3_hardening -- --test-threads=1
cargo test -p eggfetch-core --all-features --test h3_alt_svc_discovery -- --test-threads=1
```

Run the external native-body fixture:

```sh
cargo run --manifest-path qualification/native-http-body-tls/Cargo.toml
```

Before the executable commit:

```sh
./scripts/check.sh
```

Do not add a CI workflow or matrix.

## Part K — Final executable freeze and parent-plan closure

The corrective changes executable core code again, so `dd52f8c4` is not the final qualification freeze.

After focused tests pass:

1. freeze one clean executable/test SHA;
2. make no further executable/test/build changes while qualifying;
3. run:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
./scripts/check_security.sh
```

4. renew exact-SHA HTTPX 0.28.1 evidence;
5. renew exact-SHA HTTPX2 2.12.0 evidence;
6. bind live compatibility profiles/ledger only to that final SHA;
7. append closure records to this pass and the parent timeout plan;
8. update `plans/README.md` from active to complete only after required evidence exists.

HTTPX timeout semantics must remain unchanged; the compatibility facades do not gain native `Timeout.total`.

Any executable/test/build change after the freeze invalidates it.

## Part L — Coordinated patch release and downstream handoff

This timeout line is not complete for downstream consumers until the corrected code is published through the normal coordinated release process.

If no intervening coordinated release changes the sequence, this is expected to be the next patch after 0.1.6. Use the actual next coordinated version rather than hard-coding 0.1.7 if repository state changes.

Record:

```text
final executable freeze SHA:
coordinated version:
crates.io eggfetch-core publication:
other coordinated crates publication status:
public ResponseBody shape proof:
post-header total proof:
redirect remaining-budget proof:
retry remaining-budget proof:
native lease proof:
Tier 1:
extended:
package:
security:
HTTPX 0.28.1:
HTTPX2 2.12.0:
downstream handoff status:
```

Do not instruct downstreams to depend on an unpublished git-only fix when their migration requires crates.io.

## Files expected to change

Likely executable/test scope:

- `crates/eggfetch-core/src/body.rs`;
- the private lifecycle owner chosen for timeout metadata, likely `crates/eggfetch-core/src/pool.rs` or an equivalent existing private owner;
- `crates/eggfetch-core/src/pipeline/finalize.rs`;
- `crates/eggfetch-core/src/stream/body_timeout.rs`;
- `crates/eggfetch-core/src/stream/mod.rs`;
- possibly removal of `crates/eggfetch-core/src/stream/read_timeout.rs`;
- `crates/eggfetch-core/tests/response_body_public_shape.rs`;
- `crates/eggfetch-core/tests/total_body_deadline_tests.rs`;
- `crates/eggfetch-core/tests/retry_integration.rs`.

Closure/documentation scope:

- parent timeout plan;
- this corrective pass;
- `plans/README.md`;
- architecture docs only if internal ownership description changes;
- compatibility profiles/ledger after exact-SHA qualification;
- normal release metadata/changelog.

Unexpected dependency, public API, FFI, Node, HTTPX, feature graph, MSRV, release-workflow, proxy-routing, or H3-policy changes are scope-expansion signals.

## Explicit non-goals

Do not expand this pass into:

- redesigning `ResponseBody` as a new public opaque type;
- marking it non-exhaustive;
- changing published variant field names/types;
- adding public timeout metadata accessors;
- changing `Timeout` or `TimeoutPhase`;
- changing HTTPX timeout semantics;
- proxy route/cache redesign;
- retry-policy redesign beyond the regression fixture;
- decompression or body-limit redesign;
- linked-footprint tuning;
- lean-proxy/feature-boundary work;
- HTTP/3 graduation;
- new dependencies;
- new CI/release automation.

## Completion criteria

This corrective and its parent are ready to close only when:

- [ ] The exact 0.1.6/pre-corrective `ResponseBody` variant field shapes are restored.
- [ ] An external integration test destructures those shapes without `..`.
- [ ] That test is recorded green on `60a6e2b3`, red on `dd52f8c4`, and green on the final corrective.
- [ ] No non-exhaustive escape hatch or semver-breaking public replacement is introduced.
- [ ] Timeout metadata is carried entirely behind private lifecycle state.
- [ ] `BodyTimeoutStream` remains the authoritative high-level response timeout mechanism.
- [ ] The behavioral timeout matrix from `dd52f8c4` remains green.
- [ ] Raw and decoded compressed final streams still enforce total.
- [ ] Redirect proof categorically distinguishes remaining-original-budget behavior from a restarted total.
- [ ] A logical retry -> successful headers -> slow final body regression proves remaining-original-budget behavior.
- [ ] Native total timeout releases the permit and the second native request is explicitly successful.
- [ ] Duplicate `ReadTimeoutStream` ownership is removed or explicitly justified.
- [ ] Behavioral baseline-red evidence is recorded.
- [ ] Lean standard route, full/default, proxy, deterministic H3, trailer, pool, native frame, and native service regressions pass.
- [ ] No dependency, feature, MSRV, public API, Python, CLI, FFI, Node, or HTTPX semantic change is introduced.
- [ ] The external native HTTP-body/TLS fixture passes.
- [ ] Tier 1 passes on the final executable SHA.
- [ ] Extended, package, security, and MSRV gates pass as required.
- [ ] HTTPX 0.28.1 and HTTPX2 2.12.0 exact-SHA evidence is renewed on that same freeze.
- [ ] Parent plan and plan index contain truthful closure evidence.
- [ ] A coordinated crates.io patch containing the correction is published before downstream handoff is marked complete.

## Closure record template

Append before marking complete:

```text
Parent planning baseline: 60a6e2b384519507e04cf296ffd484388e872e47
Follow-up planning baseline: dd52f8c4a8d54fbaa403a7995a41bf60b235f7ac

Public API regression:
60a6 exact-shape compile:
dd52 exact-shape red:
final exact-shape compile:

Behavioral baseline-red reproduction:
Implementation commit(s):
Final executable freeze SHA:
Coordinated published version:

Immediate headers/body total:
Post-first-chunk stall:
Continuous progress aggregate total:
Delayed first poll:
Read wins:
Total wins:
Raw/decoded compressed:
Trailers:
High-level lease release:
Lean standard-route:

Redirect discrimination proof:
Retry remaining-budget proof:

Native frame total/read:
Native delayed-first-poll:
Native lease release + second-request success:
NativeHttpService:

Decoded-body-limit regression:
ResponseBody public-shape integration test:
Duplicate read-timeout disposition:

Proxy focused:
H3 deterministic:
Native HTTP body/TLS fixture:

Tier 1:
Extended:
Package:
Security:
MSRV:
HTTPX 0.28.1:
HTTPX2 2.12.0:
Remote CI:

Dependency/feature/public-API delta:
Known limitations:
Downstream handoff:
Documentation-only descendant SHA:
```

Missing evidence remains missing. Do not convert an unrun gate, unavailable fixture, or unpublished patch into a pass.

## Exit criterion

The total-deadline corrective is genuinely closed when eggfetch preserves its published Rust response-body source shape, retains the corrected request-lifecycle total deadline through all returned body modes, has discriminating redirect/retry/lease proofs, is requalified on one exact executable SHA, and the corrected coordinated patch is available from crates.io for downstream consumers.

## Closure record (2026-09-18; closed by `total-deadline-final-proof-qualification-release-closure.md`)

```text
Parent planning baseline: 60a6e2b384519507e04cf296ffd484388e872e47
Follow-up planning baseline: dd52f8c4a8d54fbaa403a7995a41bf60b235f7ac

Public API regression:
60a6 exact-shape compile: green (probe 1/1).
dd52 exact-shape red: E0027 on Streaming/EncodedStreaming, exit 101.
final exact-shape compile: green (response_body_public_shape 1/1).

Behavioral baseline-red reproduction: __closure_baseline_red_probe red on
  60a6e2b3 (Ok(b"") after ~2 s), green on final freeze (Total, ~0.33 s).
Implementation commit(s): 2c68b441 (shape restoration) + 82f3f38 (lease proof).
Final executable freeze SHA: 82f3f38631b44a9a5c5ec5b40790e5015aeb40f8
Coordinated published version: (see final closure plan Part K.)

Immediate headers/body total: green.
Post-first-chunk stall: green.
Continuous progress aggregate total: green.
Delayed first poll: green.
Read wins: green.
Total wins: green.
Raw/decoded compressed: green.
Trailers: green.
High-level lease release: green.
Lean standard-route: green.

Redirect discrimination proof: green.
Retry remaining-budget proof: green.

Native frame total/read: green.
Native delayed-first-poll: green.
Native lease release + second-request success: green — timed-out body kept alive,
  second request 200 OK within 3 s, drop only after.
NativeHttpService: green.

Decoded-body-limit regression: green (no duplicate API).
ResponseBody public-shape integration test: green.
Duplicate read-timeout disposition: ReadTimeoutStream removed; BodyTimeoutStream
  is the single high-level owner.

Proxy focused: green (54/54).
H3 deterministic: green.
Native HTTP body/TLS fixture: green.

Tier 1: passed. Extended: passed (incl. MSRV 1.89.0).
Package: passed. Security: passed. MSRV: passed.
HTTPX 0.28.1: 71/0/0/0; 3× 1871 passed.
HTTPX2 2.12.0: 79/0/0/0; same 3× runs.
Remote CI: (recorded after push.)

Dependency/feature/public-API delta: none vs 0.1.6.
Known limitations: H3 experimental; Node experimental prototype.
Downstream handoff: (named published version in final plan.)
Documentation-only descendant SHA: (closure commit; see plans/README.md.)
```
