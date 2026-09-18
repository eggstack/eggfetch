# Total Deadline Final Proof, Qualification, and Release Closure

Planning baseline: `2c68b44176b3a189e1f1fdc6586d8678a0945284` (`main`, 2026-09-18)
Parent plans:
- `plans/total-deadline-response-body-lifecycle-corrective.md`
- `plans/total-deadline-response-body-api-compatibility-corrective-pass.md`

Status: active final closure pass

## Objective

Close the total-deadline corrective line without reopening its architecture.

The behavioral implementation from `dd52f8c4` and the public-API compatibility correction from `2c68b441` are now structurally correct. The remaining work is narrow:

1. strengthen the native pool-lease regression so it proves timeout terminalization releases the permit before body drop;
2. freeze the resulting executable/test SHA;
3. record the required historical baseline proofs;
4. run the repository's release qualification on that exact SHA;
5. renew exact-SHA HTTPX 0.28.1 and HTTPX2 2.12.0 evidence;
6. fill closure records for this pass and both parent plans;
7. publish the coordinated patch release before downstream handoff is declared complete.

No new timeout behavior, public API, feature design, dependency work, or transport architecture belongs in this pass.

## Current state at planning baseline

At `2c68b441`:

- `ResponseBody` public variant field shape matches the pre-corrective / 0.1.6 API again.
- timeout metadata is private behind the response lease lifecycle.
- `BodyTimeoutStream` is the single authoritative high-level response timeout wrapper.
- the duplicate standalone `ReadTimeoutStream` has been removed.
- raw and decoded compressed final streams retain total/read enforcement.
- redirect remaining-budget proof now uses a discrimination window.
- logical retry -> successful headers -> slow final body remaining-budget proof exists.
- native second-request assertion now checks successful response headers/status.
- routine push CI run 610 passed.

One proof weakness remains: `native_total_timeout_releases_pool_lease` drops the timed-out native body before issuing the second request. That allows the test to pass if ordinary Drop, rather than the timeout terminal path, releases the permit. The production implementation currently appears to release on timeout, but the regression should prove that property directly.

Release/qualification closure also remains intentionally open.

## Part A — Correct the native lease-release proof

Modify only the native lease regression in:

```text
crates/eggfetch-core/tests/total_body_deadline_tests.rs
```

### Required test semantics

The test must keep the timed-out `NativeResponseBody` alive while the second same-origin request is admitted.

Required sequence:

1. configure `max_in_flight_requests(1)`;
2. start the first native request and obtain its response body;
3. poll that body until it terminates with `TimeoutPhase::Total`;
4. do **not** drop the body;
5. issue the second same-origin native request while the timed-out body value still exists;
6. require the second request to reach expected response headers/status within a generous outer test timeout;
7. only then drop the first timed-out body.

The key assertion is:

```text
timeout terminal state itself released the logical permit
```

not:

```text
dropping the timed-out body released the logical permit
```

A suitable shape is:

```rust
assert!(saw_total, "first native body must hit Total");

// Keep `body` alive here.
let response = tokio::time::timeout(
    Duration::from_secs(3),
    client.execute_http_body_default(request2),
)
.await
.expect("second request admitted after native Total")
.expect("second request should reach response headers");

assert_eq!(response.status(), http::StatusCode::OK);

// Only now may the first timed-out body be dropped.
drop(body);
```

If the existing fixture cannot serve the second response promptly enough while preserving deterministic behavior, adjust only the fixture timing/connection handling. Do not alter production lease logic unless the strengthened test exposes a real failure.

### Acceptance

- [ ] First body reports `TimeoutPhase::Total`.
- [ ] First body remains alive during second request admission.
- [ ] Second request succeeds with expected status.
- [ ] The regression fails if permit release occurs only in Drop.
- [ ] No production code changes are needed unless this stronger proof actually exposes a defect.

## Part B — Freeze the executable/test candidate

Once Part A is green, run focused tests and create one final executable/test commit.

This SHA becomes the candidate freeze.

After that SHA:

- do not change Rust source;
- do not change tests;
- do not change Cargo manifests/lockfiles;
- do not change build scripts;
- do not change qualification tooling;
- do not change compatibility fixtures.

Documentation-only closure commits may follow after successful qualification, but any executable/test/build change invalidates the freeze and requires qualification to restart.

Record:

```text
final executable/test freeze SHA:
parent behavioral implementation: dd52f8c4a8d54fbaa403a7995a41bf60b235f7ac
API compatibility correction: 2c68b44176b3a189e1f1fdc6586d8678a0945284
```

## Part C — Focused regression validation before release gates

Run the focused matrix against the candidate freeze:

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

Then:

```sh
cargo run --manifest-path qualification/native-http-body-tls/Cargo.toml
./scripts/check.sh
```

All Rust workspace tests must continue to use `--test-threads=1` where required by repository policy.

Do not add new CI automation.

## Part D — Record the historical public-API proof

The child corrective explicitly requires three-state source-compatibility evidence.

Using disposable worktrees or equivalent isolated historical checkouts, record the same exact external integration pattern against:

### D1. Pre-corrective baseline

SHA:

```text
60a6e2b384519507e04cf296ffd484388e872e47
```

Expected result: exact exhaustive `ResponseBody` destructuring compiles.

### D2. Regressed implementation

SHA:

```text
dd52f8c4a8d54fbaa403a7995a41bf60b235f7ac
```

Expected result: compile failure because `Streaming` and `EncodedStreaming` have added `read_timeout` / `total_deadline` fields.

Record the actual compiler error summary, normally E0027.

### D3. Final freeze

Expected result: the same integration source compiles with no `..` wildcard.

Record for each state:

```text
SHA:
test source:
command:
exit status:
relevant output:
```

Do not alter historical commits.

## Part E — Record the historical behavioral baseline-red proof

The original timeout corrective required deterministic evidence that the old implementation failed post-header total enforcement.

Against `60a6e2b3`, reproduce at least:

```text
response headers arrive before Timeout.total
response body completion/stall exceeds Timeout.total
old behavior fails to report TimeoutPhase::Total at the body boundary
```

Use the current regression fixture/test logic applied in a disposable historical checkout.

Record:

```text
baseline SHA:
test source/patch:
command:
observed baseline behavior:
final-freeze behavior:
```

The evidence need only prove the original defect once. Do not attempt to backport the full modern test suite onto the historical tree if unnecessary.

## Part F — Release qualification on the exact freeze

Run the repository's normative release gates on the final executable/test freeze:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
./scripts/check_security.sh
```

Record exact command results and relevant tool versions.

Extended validation must include the repository's required Rust 1.89 MSRV proof.

Do not substitute routine GitHub push CI for these release gates.

If a gate fails because of the timeout corrective, fix it, produce a new freeze, and restart Part F.

If a gate is unavailable for an environmental reason, record it as unavailable/blocked. Do not mark it passing.

## Part G — Renew exact-SHA compatibility evidence

The current HTTPX/HTTPX2 qualification records are historical because executable source/tests changed after their prior freeze.

Renew both compatibility profiles on the same final executable SHA:

- HTTPX 0.28.1;
- HTTPX2 2.12.0.

Follow the repository's existing compatibility qualification procedures and exact-SHA rules.

Required outcomes:

- zero unexplained/stale/resolved-active oracle differences;
- required full pinned compatibility runs pass according to current repository policy;
- live compatibility ledger/profile records bind to the final executable SHA;
- no HTTPX facade gains native `Timeout.total` semantics.

Do not edit the recorded SHA until qualification has actually passed.

## Part H — Remote CI confirmation

After the final executable/test commit is pushed, require the normal repository GitHub Actions CI run to complete successfully on that exact SHA.

Record:

```text
workflow:
run number/id:
head SHA:
conclusion:
```

A green local Tier 1 run and green remote CI are separate evidence.

## Part I — Complete all three closure records

Update:

- `plans/total-deadline-response-body-lifecycle-corrective.md`;
- `plans/total-deadline-response-body-api-compatibility-corrective-pass.md`;
- this plan.

Each record must point to the same final executable/test freeze and accurately list:

- historical baseline-red proof;
- public API three-state compile proof;
- behavioral timeout regressions;
- redirect discrimination proof;
- retry discrimination proof;
- strengthened native lease-release proof;
- raw/decoded compressed behavior;
- trailers;
- lean profile;
- proxy focused tests;
- deterministic H3 tests;
- native body/TLS fixture;
- Tier 1;
- extended;
- package;
- security;
- MSRV;
- HTTPX 0.28.1;
- HTTPX2 2.12.0;
- remote CI;
- dependency/public API/feature delta;
- known limitations.

Missing evidence must remain explicitly missing until executed.

## Part J — Update the plan index truthfully

Only after Parts A-I are complete, update `plans/README.md`:

- change the timeout corrective line from active to completed;
- identify the final freeze SHA;
- identify the public API restoration;
- state that total now spans response-body EOF/trailers;
- state that read remains first-poll/per-chunk inactivity;
- reference redirect/retry/native lease proofs;
- reference exact-SHA compatibility renewal;
- identify the coordinated release version once published.

If qualification is complete but publication has not occurred, keep downstream handoff explicitly pending publication.

## Part K — Coordinated patch release

Publish through the repository's normal coordinated manual release process.

Do not hard-code a version in advance. If 0.1.6 remains the latest coordinated crate version and no intervening release occurs, the natural candidate is the next patch; use the actual repository state at release time.

Follow the documented publish order and release process.

Record:

```text
coordinated version:
eggfetch-http-connect:
eggfetch-core:
eggfetch-cli:
eggfetch-ffi:
eggfetch-python:
eggfetch-node:
GitHub release/tag if used:
crates.io visibility verified:
```

The corrected `eggfetch-core` crate must be available from crates.io before downstream migration is considered complete.

Do not tell Eggsact or another downstream to rely on a git-only correction when the handoff contract requires a published crate.

## Part L — Downstream handoff note

After publication, record the exact minimum downstream-relevant facts:

- corrected published `eggfetch-core` version;
- `Timeout.total` now covers response body EOF/trailers;
- `max_decoded_body_size` remains the correct hard bound for unknown/false `Content-Length` metadata bodies;
- public `ResponseBody` field shape remains compatible with 0.1.6;
- no new HTTPX total semantic;
- no new dependency/MSRV/feature requirements beyond the published release notes.

Do not include downstream Eggsact code changes in this repository pass.

## Expected file changes

Executable/test change expected before freeze:

- `crates/eggfetch-core/tests/total_body_deadline_tests.rs` only, unless the stronger lease test exposes a genuine production defect.

Closure/qualification changes after freeze may include:

- `plans/total-deadline-response-body-lifecycle-corrective.md`;
- `plans/total-deadline-response-body-api-compatibility-corrective-pass.md`;
- this plan;
- `plans/README.md`;
- compatibility profile/ledger records;
- changelog/version/release metadata required by the normal coordinated release.

Unexpected production Rust changes after Part A are a scope-expansion signal and invalidate the candidate freeze.

## Explicit non-goals

Do not use this pass for:

- new timeout behavior;
- changing `Timeout` or `TimeoutPhase`;
- changing `ResponseBody` public shape;
- new public APIs;
- proxy/cache redesign;
- retry-policy redesign;
- decompression changes;
- body-limit changes;
- footprint tuning;
- feature graph work;
- HTTP/3 graduation;
- new dependencies;
- MSRV changes;
- Python/FFI/Node behavior changes;
- CI automation changes;
- downstream Eggsact implementation.

## Completion criteria

This final closure pass is complete only when:

- [ ] The native lease test proves timeout terminalization releases the permit while the timed-out body remains alive.
- [ ] No unintended production behavior change was required, or any discovered defect is explicitly corrected and requalified.
- [ ] One final executable/test freeze SHA is recorded.
- [ ] The public `ResponseBody` exact-shape integration test passes on the final freeze.
- [ ] Historical public API evidence is recorded: `60a6e2b3` green, `dd52f8c4` red, final freeze green.
- [ ] Historical behavioral baseline-red evidence is recorded against `60a6e2b3`.
- [ ] Redirect remaining-budget discrimination proof passes.
- [ ] Retry remaining-budget discrimination proof passes.
- [ ] Native read/total precedence and delayed-first-poll proofs pass.
- [ ] High-level and native lease-release proofs pass.
- [ ] Raw/decoded compressed timeout tests pass.
- [ ] Trailer timeout tests pass.
- [ ] Lean standard-route test passes.
- [ ] Proxy focused tests pass.
- [ ] Deterministic H3 tests pass.
- [ ] Native HTTP-body/TLS fixture passes.
- [ ] `./scripts/check.sh` passes.
- [ ] `./scripts/check.sh extended` passes.
- [ ] `./scripts/check.sh package` passes.
- [ ] `./scripts/check_security.sh` passes.
- [ ] Rust 1.89 MSRV validation passes.
- [ ] HTTPX 0.28.1 exact-SHA evidence is renewed on the final freeze.
- [ ] HTTPX2 2.12.0 exact-SHA evidence is renewed on the same freeze.
- [ ] Remote CI passes on the final executable/test SHA.
- [ ] All parent/child closure records are filled truthfully.
- [ ] Plan index marks the line complete only after required evidence exists.
- [ ] A coordinated crates.io patch containing the correction is published.
- [ ] Downstream handoff names the actual published version.

## Closure record template

```text
Planning baseline: 2c68b44176b3a189e1f1fdc6586d8678a0945284
Final executable/test freeze SHA:
Documentation-only descendant SHA:
Coordinated published version:

Native lease proof correction:
Timed-out body retained during second request:
Second request status:

Public API shape evidence:
60a6e2b3:
dd52f8c4:
final freeze:

Behavioral baseline-red:
60a6e2b3:
final freeze:

Immediate headers/body total:
Post-first-chunk stall:
Continuous trickle aggregate total:
Delayed first poll:
Read-vs-total precedence:
Raw compressed:
Decoded compressed:
Trailers:
High-level lease release:
Native lease release:
Redirect discrimination:
Retry discrimination:
Lean standard route:
Proxy focused:
H3 deterministic:
Native HTTP-body/TLS fixture:

Tier 1:
Extended:
Package:
Security:
MSRV:
HTTPX 0.28.1:
HTTPX2 2.12.0:
Remote CI:

Dependency delta:
Feature delta:
Public API delta:
Known limitations:

Release:
crates.io verification:
Downstream handoff:
```

## Exit criterion

The timeout corrective line is closed only when the native lease proof is categorical, one final executable SHA has passed all release and compatibility qualification, closure records are complete, and the corrected coordinated patch is publicly available for downstream consumers.
