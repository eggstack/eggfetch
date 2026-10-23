# Core Transport and Request Policy Milestone 009 — Lean Path Single-Sourcing and Full-Suite Profile Coverage

Status: ready

Repository baseline: `873ea28541c82817317430c9523fca8525d99c81`

Source roadmap:

- `plans/subsystems/core-transport-policy-roadmap.md#milestone-9--lean-path-single-sourcing-and-full-suite-profile-coverage`

Long-term requirements:

- `plans/000-long-term-specification.md` §4.1, §5
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 items 2 and 4

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0002-pipeline-route-finalize.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`

Primary class: infrastructure

## 1. Objective

Remove the duplicated request-preparation logic between the lean request path
and the redirect path, and make the whole test suite compile under the lean
profile so that every supported profile can be gated as a whole.

Two related problems are in scope.

### 1.1 Three copies of the same preparation logic

`crates/eggfetch-core/src/pipeline/lean.rs` (121 lines) re-implements the
first-hop preparation that `crates/eggfetch-core/src/pipeline/redirect.rs`
already owns. The duplicated operations are:

| Operation | `lean.rs` | `redirect.rs` |
|---|---|---|
| default/request header merge | 53-58 | 318-323 |
| client/request timeout merge | 60-63 | 326 |
| hop request construction | 65-81 | `build_hop_request`, 76 |
| cookie-jar injection | 85-91 | 137 |
| auth resolve and apply | 93-104 | 146 |
| `Set-Cookie` jar update | 110-118 | 378 and 483 |

The cookie-injection and auth-resolution blocks therefore exist **three
times** in the tree, and the header merge, timeout merge and jar update
exist **twice**.

The duplication is not harmless. A change to how a request's headers,
timeouts, cookies or credentials are prepared must be made in two or three
places, and a missed copy is not a compile error — the exhaustive
`Request::into_parts()` destructuring guarantees that a *new field* is not
forgotten, but it does not guarantee that a *changed rule* is applied
consistently.

### 1.2 The lean profile cannot compile the full test suite

At the baseline, `cargo check -p eggfetch-core --no-default-features
--features standard-http1,tls-rustls --all-targets` fails to compile
thirteen test targets:

```
adverse_tests              custom_dialer_tests       direct_transport_tests
native_tower_service_tests network_stream_tests      redirect_strict_tests
request_failure_tests      resolved_route_cache_tests retry_integration
tls_integration            tls_response_completeness total_body_deadline_tests
transport_metrics_tests
```

The failures are unresolved imports for `RetryPolicy`, `Dialer` and
`RedirectPolicy`, and missing client methods such as `follow_redirects`,
`retry`, `resolved_addresses`, `uds_path` and `local_address` — all of
which are compiled out of the lean profile by design.

The consequence is that the lean profile can only be gated through the
narrow stable contract fixture. The whole suite can never be run under it,
so no lean-profile regression can be caught by a broad gate, and a new test
that accidentally depends on a non-lean feature cannot be detected.

## 2. Why this milestone is ready

- The duplication is confirmed and localized. There is no open design
  question about *what* the shared preparation must do; the two copies
  currently agree, and the milestone makes that agreement structural.
- ADR-0002's phase ownership already assigns first-hop assembly to
  `redirect` and request normalization to `prepare`. The lean path is an
  intentional exception to that ownership, and this milestone resolves the
  exception by moving the shared part into the module that already owns
  request normalization, without changing the phase order.
- The exhaustiveness property that ADR-0002 and `AGENTS.md` depend on is
  preserved by keeping the exhaustive `Request::into_parts()` destructuring
  of the **first-hop preparation** in exactly one place. The
  transport-preparation and subsequent-hop destructurings are separate
  stages, not copies of the first-hop preparation, and each remains
  exhaustive in its own right (§6.2).
- The lean profile exists for a recorded footprint reason. The decision to
  keep the lean path was recorded in
  `plans/high-level-policy-footprint-feature-boundary.md`, and this
  milestone preserves that outcome; it does not reopen it.
- The thirteen failing test targets fail for a mechanical reason — they
  reference APIs that the lean profile compiles out — so each can be gated
  at file level without behavioral change.
- M008 adds the lean behavioral gate. This milestone is what makes that
  gate extendable to the whole suite.

## 3. Current implementation evidence

At the baseline:

- `crates/eggfetch-core/src/pipeline/mod.rs` gates `lean` on
  `all(high-level-url, not(redirects))` and `redirect` on
  `all(high-level-url, redirects)`; `prepare` is ungated;
- `crates/eggfetch-core/src/pipeline/lean.rs` is 121 lines and contains the
  duplicated preparation blocks listed in section 1.1;
- `crates/eggfetch-core/src/pipeline/redirect.rs` is 851 lines and contains
  `build_hop_request` at line 76 plus a separate first-hop preparation
  beginning around line 290;
- `crates/eggfetch-core/src/pipeline/redirect.rs:483` contains a second
  `Set-Cookie` jar update, so the jar-update logic exists three times in the
  tree;
- `crates/eggfetch-core/src/pipeline/prepare.rs` is the ungated module that
  owns request normalization and pool acquisition;
- `plans/high-level-policy-footprint-feature-boundary.md` records the lean
  path as a deliberate binary-footprint trade-off;
- `crates/eggfetch-core/tests/lean_policy_tests.rs` (6 tests) and
  `lean_route_tests.rs` (10 tests) pass under the lean profile at the
  baseline;
- the thirteen test targets listed in section 1.2 fail to compile under the
  lean profile at the baseline.

## 4. Invariants that must not regress

- The exhaustive typed rebuild helpers remain exhaustive. A new request
  field must still fail to compile if it is not threaded through the
  preparation path.
- The **first-hop** preparation path must have exactly one exhaustive
  `Request::into_parts()` destructuring after this milestone, and both the
  lean path and the redirect path must use it. Two further exhaustive
  destructurings remain by this plan's own requirement (§6.2): the
  transport-preparation site in `prepare`, and the subsequent-hop site in
  `advance_redirect_hop`. They are different stages, not copies of the
  first-hop preparation, and each must stay exhaustive.
- The lean profile's binary-footprint outcome is preserved. The shared
  preparation must not pull `redirects`-gated code into the lean profile.
- The lean profile keeps its current behavior: 3xx responses are returned
  without following, one attempt per request, Bearer-only auth, body cap and
  total timeout enforced.
- The redirect path's behavior is unchanged, including the method-rewrite
  table, credential stripping and `Set-Cookie` handling.
- No public API change in any profile.
- Timeout ownership, pool admission, and body-lifecycle semantics are
  unchanged.
- The `ResponseBody` public variant shapes remain frozen.
- No new cargo dependency.

## 5. Scope

### In scope

- a shared, feature-independent preparation builder used by both the lean
  path and the redirect path;
- removal of the duplicated header merge, timeout merge, cookie injection,
  auth resolution and `Set-Cookie` update blocks;
- file-level gating of the thirteen test targets that do not compile under
  the lean profile;
- a gate that compiles the whole test suite under the lean profile;
- documentation of the lean profile's supported scope.

### Explicitly out of scope

- retiring the lean path or removing the `standard-http1` / `standard-http2`
  profiles;
- changing the lean profile's behavior, defaults, or footprint target;
- the redirect method-rewrite table's location, which is tracked by
  core-transport-policy M010;
- the profile-matrix coverage gaps tracked by core-transport-policy M008;
- the CLI and FFI protocol-capability work tracked separately by
  tls-proxy-protocols M005;
- publication, version bump, or release orchestration.

## 6. Required production changes

### 6.1 Extract one preparation builder

Create a single preparation builder in an **ungated** module —
`crates/eggfetch-core/src/pipeline/prepare.rs`, or a new ungated
`crates/eggfetch-core/src/pipeline/hop.rs` — that performs the shared work:

1. the exhaustive `Request::into_parts()` destructuring;
2. the default/request header merge;
3. the client/request timeout merge;
4. hop request construction, including every request field;
5. cookie-jar injection when no `Cookie` header is already present;
6. auth resolution and application;
7. `Set-Cookie` jar update from the response.

Both `lean.rs` and `redirect.rs` must call this builder. The lean path and
the redirect path must differ only in what they do *after* preparation:
the lean path sends one request and returns; the redirect path evaluates the
redirect decision and may build a further hop.

Design constraints:

- the builder must compile with only `high-level-url` and the lean
  feature set. It must not reference `redirects`-gated items;
- the cookie and auth blocks must remain `#[cfg(feature = "cookies")]`-gated
  in the same way they are today;
- the builder must not change the order of operations. In particular, cookie
  injection must still happen after the hop's headers are set and before the
  request is dispatched, and the `Set-Cookie` update must still happen
  after the response is received and before the response is returned;
- the builder must not introduce a second timeout owner or a second pool
  admission point.

### 6.2 Preserve exhaustiveness

After the extraction there must be exactly one exhaustive
`Request::into_parts()` destructuring in the **first-hop preparation** path,
and both the lean path and the redirect path must use it. A new request
field must therefore produce exactly one compile error when it is not
threaded through the first-hop preparation, not one per copy.

Two further exhaustive destructurings are not copies of the first-hop
preparation and are **not** removed by this milestone: the
transport-preparation destructuring in `prepare` (the stage that turns a
prepared hop into a `PreparedRequest`) and the subsequent-hop destructuring
in `advance_redirect_hop`. This section requires the latter to remain, and
the former is the same stage it always was. A newly added `RequestParts`
field must still fail to compile at each of them.

The `retry_request` and `advance_redirect_hop` typed rebuild helpers must
remain the only places a *subsequent* hop request is constructed.

### 6.3 Remove the duplicated blocks

The following must be deleted rather than kept as thin wrappers:

- the header merge in `lean.rs`;
- the timeout merge in `lean.rs`;
- the cookie injection in `lean.rs`;
- the auth resolution in `lean.rs`;
- the `Set-Cookie` update in `lean.rs`;
- the duplicate `Set-Cookie` update at `redirect.rs:483`, if the shared
  builder can serve both call sites.

The implementer must confirm, after the change, that no operation listed in
section 6.1 exists in more than one place in `crates/eggfetch-core/src`.

### 6.4 Make the whole test suite compile under the lean profile

Each of the thirteen failing test targets must be gated at file level so
that it compiles to an empty test binary when the lean profile compiles out
the APIs it exercises. The gate must be a file-level `#![cfg(...)]`
attribute expressing the feature requirement of that file's content, not a
blanket profile check.

For example, a target that exercises `RetryPolicy` must require
`logical-retry`; a target that exercises `RedirectPolicy` must require
`redirects`; a target that exercises the custom dialer must require
`advanced-routing`.

The gate must be per-file and precise. A blanket "skip under lean" gate
would hide real compile regressions in those files under the default
profile.

Each gated file must still compile and pass under the default profile, and
under `--all-features`, unchanged.

### 6.5 Gate the whole suite under the lean profile

Once the thirteen targets compile under the lean profile, the Tier 1 gate
must compile the whole test suite under the lean profile, so that a future
test that accidentally depends on a non-lean feature is caught:

```bash
RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --all-targets
```

This must use the same bare `RUSTFLAGS=` treatment as the existing contract
matrix, for the reason recorded in `scripts/check.sh`.

This step depends on the coverage added by core-transport-policy M008 and
must not be added before it, so that the gate sequence is coherent.

## 7. Ordered work packages

1. Add failing evidence: record the current duplication and the current
   thirteen-target compile failure so the change is measurable.
2. Extract the shared preparation builder into an ungated module.
3. Move the lean path onto the shared builder.
4. Move the redirect path's first-hop preparation onto the shared builder.
5. Remove the duplicated blocks and the duplicate `Set-Cookie` update.
6. Verify exactly one exhaustive destructuring remains, and that adding a
   hypothetical request field produces exactly one compile error.
7. Gate the thirteen test targets at file level.
8. Verify the whole suite compiles under the lean profile.
9. Add the whole-suite lean compile check to the Tier 1 gate.
10. Verify the lean profile's behavioral suites still pass under the lean
    profile and under the default profile.
11. Update the lean-profile documentation.
12. Run Tier 1, then Tier 2.
13. Renew the exact-SHA Stage C binding, because the engine and the
    validation script are executable inputs.
14. Write `plans/closure/core-transport-policy/009-status.md`.

## 8. Failure, cancellation, timeout, and pool semantics

- The shared builder must not change when the hop's timeout is computed. The
  client timeout merged with the request timeout remains the hop's timeout,
  and `Timeout.total` remains monotonic and owned by the outer dispatch.
- The builder must not introduce a second pool admission point or a second
  deadline owner. The hop is prepared before dispatch exactly as today.
- The `Set-Cookie` update must continue to happen after the response is
  received and before the response is returned to the caller, so that a
  caller observing `client.cookies` immediately after a response sees the
  same state as today.
- Cookie injection must continue to be skipped when the hop already carries a
  `Cookie` header.
- Auth resolution must continue to observe the hop's own auth state and the
  client's configured auth, and the `auth_disabled` flag must keep its
  meaning.
- No cancellation, backpressure, or streaming behavior changes.

## 9. Compatibility and migration

- No public API change in any profile.
- No behavior change. The lean path and the redirect path must produce
  byte-identical requests and identical responses before and after.
- The only observable difference may be in the lean profile's binary
  footprint, which must not increase. If the shared builder increases the
  lean profile's footprint beyond the recorded target, that is a stop
  condition.
- Feature-profile effects: the thirteen gated test files compile to empty
  test binaries under the lean profile. Under every other profile they are
  unchanged.
- The engine and the validation script are executable inputs, so the
  exact-SHA Stage C binding must be renewed under
  `.skills/verification-qualification.md`.
- No crate version bump is implied.

## 10. Required tests

### Exhaustiveness

- exactly one exhaustive `Request::into_parts()` destructuring exists in the
  **first-hop preparation** path, and both the lean path and the redirect
  path use it; the transport-preparation site in `prepare` and the
  subsequent-hop site in `advance_redirect_hop` remain exhaustive and are
  listed separately in the closure evidence;
- adding a field to the request type produces exactly one compile error in
  the first-hop preparation path (proved by a temporary local experiment, not
  a committed change), and one at each of the two remaining sites.

### Equivalence

- the lean path's prepared hop is identical to the redirect path's first
  hop for the same request, asserted by a fixture that captures the
  outgoing request from both paths and compares method, URL, headers,
  timeout, body, auth and transport hints;
- the `Set-Cookie` jar update produces the same jar state from both paths.

### Profile coverage

- the whole test suite compiles under `standard-http1,tls-rustls`;
- all thirteen previously failing targets compile under the lean profile;
- each of those targets still compiles and passes under the default profile
  and under `--all-features`;
- `lean_policy_tests` and `lean_route_tests` pass under the lean profile;
- the same two suites pass under the default profile, unchanged.

### Regression

- the redirect path's existing suites are unchanged and green;
- the workspace test suite is green;
- Tier 1 and Tier 2 are green;
- the public API oracle is unchanged for every profile.

## 11. Required verification commands

Follow `docs/verification-policy.md` and
`.skills/verification-qualification.md`.

Focused first:

```bash
RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --all-targets
RUSTFLAGS= cargo test -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --test lean_policy_tests -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --test lean_route_tests -- --test-threads=1
cargo test -p eggfetch-core --test redirect_policy_tests -- --test-threads=1
```

Then:

```bash
cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1
./scripts/check.sh
./scripts/check.sh extended
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

Never parallelize Rust workspace tests; `--test-threads=1` is required.

Do not create a new automatic workflow or a permanent CI job.

## 12. Documentation updates

Implementation must update:

- `docs/architecture/core-engine.md` (the pipeline phase ownership, and the
  lean path's relationship to the redirect path);
- `docs/architecture/core-timeout-pool.md` if the preparation builder's
  timeout ownership description changes;
- `docs/architecture/feature-flags.md` (the lean profile's supported scope
  and its test coverage);
- `docs/architecture/core-auth-redirect-retry.md` (the single preparation
  builder);
- `plans/registry.md`, the subsystem roadmap and `plans/README.md` at
  closure.

## 13. Acceptance criteria

M009 closes only when:

- exactly one exhaustive `Request::into_parts()` destructuring exists in the
  **first-hop preparation** path, with `prepare_single_request` and
  `advance_redirect_hop` remaining as the two transport- and subsequent-hop
  sites, each exhaustive in its own right (§6.2);
- the header merge, timeout merge, cookie injection, auth resolution and
  `Set-Cookie` update each exist in exactly one place;
- the lean path and the redirect path produce identical prepared hops for the
  same request;
- the lean profile's behavior is unchanged: 3xx not followed, one attempt,
  Bearer-only auth, body cap and total timeout enforced;
- the whole test suite compiles under `standard-http1,tls-rustls`;
- all thirteen previously failing targets compile under the lean profile and
  still pass under the default profile;
- the Tier 1 gate compiles the whole suite under the lean profile;
- the lean profile's binary footprint does not increase;
- no public API changed in any profile;
- Tier 1 and Tier 2 are green on the frozen executable SHA;
- both HTTPX profiles are rebound to that exact SHA.

## 14. Stop conditions

Stop and revise this plan rather than improvising if:

- the shared builder cannot be compiled in the lean profile without pulling
  in `redirects`-gated code, in which case the lean path must keep its own
  preparation and the milestone must be re-scoped to the test-target gating
  only;
- the shared builder measurably increases the lean profile's binary
  footprint beyond the recorded target;
- the file-level gating of the thirteen test targets would require a
  behavioral change inside any of them;
- a request field would fail to be exhaustive after the extraction, that is,
  if a new field can be added without a compile error;
- the redirect path's observable behavior would change in any way.

## 15. Closure evidence required

Create `plans/closure/core-transport-policy/009-lean-path-single-sourcing.md` containing:

- implementation commit(s) and the final frozen SHA;
- the before/after duplication count, per operation;
- the equivalence fixture output proving both paths prepare identical hops;
- the exhaustiveness proof;
- the before/after lean-profile compile-failure list;
- the lean-profile binary footprint before and after;
- the exact Tier 1 and Tier 2 outcomes;
- renewed HTTPX Stage C binding evidence for both profiles;
- the public API delta (expected: none);
- known limitations, including that the redirect method-rewrite table
  remains separate and is tracked by M010.

## 16. Handoff notes

This milestone is a consolidation, not a redesign. The observable behavior
of both paths must be identical before and after; the closure evidence
should be able to show that by direct comparison of the prepared hop.

Do not reopen the decision to keep the lean path. It was recorded in
`plans/high-level-policy-footprint-feature-boundary.md` for a footprint
reason, and this milestone preserves that outcome while removing the
duplication it caused.

The file-level gating of the thirteen test targets must be precise. A
blanket profile check would make those files silently empty under the lean
profile in a way that hides real regressions under the default profile.

M010 depends on this milestone: consolidating the duplicated preparation
blocks first makes the remaining duplication clusters easier to reason
about.
