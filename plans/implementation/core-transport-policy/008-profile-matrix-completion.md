# Core Transport and Request Policy Milestone 008 — Profile Matrix Completion

Status: ready

Repository baseline: `873ea28541c82817317430c9523fca8525d99c81`

Source roadmap:

- `plans/subsystems/core-transport-policy-roadmap.md#milestone-8--profile-matrix-completion`

Long-term requirements:

- `plans/000-long-term-specification.md` §4.1, §5
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 items 2 and 4

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`

Primary class: invariant

## 1. Objective

Make the supported feature-profile set fully covered by the repository's
verification gates, so that no declared profile is compiled only as a side
effect of `--all-features`.

At the baseline, `eggfetch-core` declares two families of supported profiles:

| Family | H1 profile | H2 profile |
|---|---|---|
| standard (lean) | `standard-http1` | `standard-http2` |
| native (URL-less) | `native-http1` | `native-http2` |

The stable compile-contract matrix in `scripts/check.sh` and the Rust
public-surface oracle in `scripts/check_rust_public_api.py` each cover six
profiles:

- `default`
- `http1-tls-rustls`
- `http2-tls-rustls`
- `standard-http1-tls-rustls`
- `native-http1-tls-rustls`
- `all-features`

For both families, the H2 profile is missing. `standard-http2` and
`native-http2` are therefore never built in isolation by any script or CI
job, and their public surface is never frozen. A change that breaks the
`transport-http2` profile in combination with `advanced-routing` and without
`high-level-url` — the `native-http2` combination — would be invisible to
every gate, because `--all-features` masks the isolation defect.

A second gap in the same matrix: the lean profile's **behavioral** tests are
never run under the lean profile. `scripts/check.sh` runs only
`--test public_api_contracts` (compile shape) for `standard-http1`. The lean
profile's own behavioral suites exist and pass under the lean profile, but
no gate invokes them there, so the lean path's runtime behavior is unverified
in continuous integration.

## 2. Why this milestone is ready

- The gap is a coverage gap, not a design question. The supported profile
  set is already defined by `docs/architecture/feature-flags.md` and by the
  cargo feature graph; the gates simply do not cover all of it.
- Both missing profiles already compile. Verified at the baseline:
  `cargo check -p eggfetch-core --no-default-features --features native-http2,tls-rustls`
  and the same with `standard-http2` both succeed. The milestone therefore
  adds no new code paths, only new verification.
- The lean behavioral suites already pass under the lean profile. Verified at
  the baseline: `lean_policy_tests` 6/6 and `lean_route_tests` 10/10 under
  `--no-default-features --features standard-http1,tls-rustls`. The
  milestone adds the gate invocation, not new test logic.
- The oracle already supports a variable profile list; adding two entries and
  two snapshot files is an additive change to an existing mechanism.
- ADR-0005 containment is unaffected: no public surface is added, only
  frozen.

## 3. Current implementation evidence

At the baseline:

- `crates/eggfetch-core/Cargo.toml` declares `standard-http1`,
  `standard-http2`, `native-http1` and `native-http2` as supported profile
  aliases;
- `scripts/check.sh` lines 126-131 enumerate the six-profile contract
  matrix, using bare `RUSTFLAGS=` to neutralize the CI-wide `-D warnings`
  flag for profile-specific dead code;
- `scripts/check_rust_public_api.py` lines 18-31 define the same six oracle
  profiles against baseline `03ecba973010e2858bf16a2b5f84d51ce70adae4`;
- `compat/rust-public-api/` holds one snapshot per oracle profile:
  `default.txt`, `http1-tls-rustls.txt`, `http2-tls-rustls.txt`,
  `standard-http1-tls-rustls.txt`, `native-http1-tls-rustls.txt`,
  `all-features.txt`;
- the `standard-http1` snapshot (135,926 bytes) differs from the
  `native-http1` snapshot (119,215 bytes), proving the oracle does
  discriminate profile surfaces;
- `http1-tls-rustls.txt` and `http2-tls-rustls.txt` have identical size
  (160,902 bytes), so the `http2` alias shares the `http1` alias's public
  surface;
- `crates/eggfetch-core/tests/lean_policy_tests.rs` (6 tests) and
  `crates/eggfetch-core/tests/lean_route_tests.rs` (10 tests) are
  profile-portable and pass under both the lean profile and the full
  profile;
- no script or workflow invokes either lean suite under the lean profile;
- `cargo check -p eggfetch-core --no-default-features --features
  standard-http1,tls-rustls --all-targets` fails to compile thirteen test
  targets, which is tracked separately by core-transport-policy M009 and is
  not a prerequisite for this milestone.

## 4. Invariants that must not regress

- The public Rust surface of every already-covered profile is unchanged. This
  milestone must produce **no** public API delta in any profile.
- The oracle baseline SHA `03ecba973010e2858bf16a2b5f84d51ce70adae4` is not
  changed.
- Existing oracle snapshots are not regenerated to accept an accidental
  change. A snapshot that must change means the public surface changed, which
  is a defect for this milestone.
- The contract matrix continues to use bare `RUSTFLAGS=` so that
  profile-specific dead code does not fail the build.
- The `--all-features` profile remains in both gates.
- No public API is added, and no cargo feature is added, removed, or renamed.
- The stable contract fixture keeps its current structure; new profiles reuse
  the existing fixture rather than adding a parallel one.

## 5. Scope

### In scope

- add `native-http2,tls-rustls` to the stable compile-contract matrix;
- add `standard-http2,tls-rustls` to the stable compile-contract matrix;
- add the corresponding two oracle profiles and their frozen snapshots;
- run the lean profile's behavioral suites under the lean profile in the
  contract gate;
- document the now-complete supported-profile matrix.

### Explicitly out of scope

- making the whole test suite compile under the lean profile, which is
  tracked by core-transport-policy M009;
- any change to engine behavior, public API, or cargo features;
- the `transport-http2` ALPN branch structure, which is private and already
  reachable through the `http2` alias;
- the CLI and FFI protocol-capability work tracked separately by
  tls-proxy-protocols M005;
- the lint-guard and downstream-gate work tracked separately by
  release-verification M006;
- publication, version bump, or release orchestration.

## 6. Required production changes

### 6.1 Extend the stable compile-contract matrix

In `scripts/check.sh`, `tier1_rust_api_contracts` must gain two invocations
using the same shape as the existing ones, including the bare `RUSTFLAGS=`
prefix and the `--test-threads=1` flag:

```bash
RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features standard-http2,tls-rustls -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features native-http2,tls-rustls -- --test-threads=1
```

The existing comment explaining the bare `RUSTFLAGS=` prefix must be kept
accurate; it now covers eight profiles rather than six.

### 6.2 Run the lean behavioral suites under the lean profile

The lean profile's behavioral suites must be executed with the lean profile's
feature set, so that the lean request path is exercised rather than only
compiled:

```bash
RUSTFLAGS= cargo test -p eggfetch-core --test lean_policy_tests --no-default-features --features standard-http1,tls-rustls -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test lean_route_tests --no-default-features --features standard-http1,tls-rustls -- --test-threads=1
```

These must be added to the Tier 1 contract gate, not only to Tier 2, because
the lean path is a supported profile and its behavior is a Tier 1
responsibility.

If the lean suites cannot be run under the lean profile without the
whole-suite compile work tracked by M009, the gate must at minimum run the
two named test targets, which do compile and pass under the lean profile at
the baseline. The gate must not be widened to `--all-targets` under the lean
profile, which would require M009 first.

### 6.3 Extend the Rust public-surface oracle

`scripts/check_rust_public_api.py` must gain two profile entries:

```python
(
    "standard-http2-tls-rustls",
    ("--no-default-features", "--features", "standard-http2,tls-rustls"),
),
(
    "native-http2-tls-rustls",
    ("--no-default-features", "--features", "native-http2,tls-rustls"),
),
```

Two new snapshots must be generated and committed:

- `compat/rust-public-api/standard-http2-tls-rustls.txt`
- `compat/rust-public-api/native-http2-tls-rustls.txt`

Generation uses the pinned toolchain documented in
`compat/rust-public-api/README.md`:

```sh
RUSTUP_TOOLCHAIN=nightly-2026-05-07 \
  CARGO_PUBLIC_API=/path/to/cargo-public-api \
  CARGO_SEMVER_CHECKS=cargo-semver-checks \
  python scripts/check_rust_public_api.py
```

The new snapshots must be **generated from the current baseline**, not copied
from an H1 profile. The implementer must record what the new snapshots show
relative to their H1 counterparts. In particular, it must be determined and
recorded whether `native-http2-tls-rustls.txt` differs from
`native-http1-tls-rustls.txt`, and whether
`standard-http2-tls-rustls.txt` differs from
`standard-http1-tls-rustls.txt`. The H2 alias's public surface is expected to
be equivalent to the H1 alias's, because the alias adds
`high-level-url`, `logical-retry`, `redirects` and `basic-auth` rather than a
transport, but this must be measured rather than assumed.

If a new snapshot differs from its H1 counterpart in a way that reflects a
genuine public-surface difference between H1 and H2 profiles, that difference
is the expected outcome and must be recorded in the closure evidence. If it
reveals an accidental difference, that is a defect and must be fixed inside
this milestone.

The existing snapshots must not be regenerated to accept a change. If an
existing snapshot needs to change, the public surface changed and this
milestone must stop.

### 6.4 Document the complete matrix

`docs/architecture/feature-flags.md` and
`docs/reference/feature-matrix.md` must state the complete supported-profile
set and, for each profile, the gate that covers it. The documentation must
not claim coverage that the gates do not provide.

## 7. Ordered work packages

1. Add the two contract-matrix invocations to `scripts/check.sh`.
2. Add the two lean behavioral invocations to the Tier 1 contract gate.
3. Verify all eight contract invocations pass locally.
4. Add the two oracle profile entries to `scripts/check_rust_public_api.py`.
5. Generate the two new snapshots with the pinned toolchain and compare them
   against their H1 counterparts.
6. Record the comparison result; fix any accidental difference found.
7. Run the full oracle, including the semver half, on the default profile.
8. Update the feature-flag and feature-matrix documentation.
9. Run Tier 1, then Tier 2.
10. Renew the exact-SHA Stage C binding, because `scripts/check.sh` and the
    oracle are executable inputs.
11. Write `plans/closure/core-transport-policy/008-profile-matrix-completion.md`.

## 8. Failure, cancellation, timeout, and pool semantics

- This milestone changes no runtime behavior. No timeout, pool, cancellation,
  retry, redirect, or body-lifecycle semantics change.
- Adding gate invocations must not change test concurrency. Every added
  invocation must keep `--test-threads=1`, because RSS-stabilization tests
  are flaky under concurrency.
- The bare `RUSTFLAGS=` prefix must be retained on every added invocation.
  Omitting it would make the lean and native profiles fail the gate on
  profile-specific dead code, which is the reason the prefix exists.
- The gate must not be widened to `--all-targets` under the lean profile.

## 9. Compatibility and migration

- No runtime behavior change. No public API change. No cargo feature change.
- No downstream migration is implied.
- The oracle profile count changes from six to eight. The oracle's
  documentation and the `compat/rust-public-api/README.md` description of
  "six profiles" must be updated.
- `scripts/check.sh` and the oracle script are executable inputs, so the
  exact-SHA Stage C binding must be renewed under
  `.skills/verification-qualification.md`.
- No crate version bump is implied.

## 10. Required tests

### Contract matrix

- all eight contract invocations pass;
- `standard-http2,tls-rustls` and `native-http2,tls-rustls` compile the
  stable contract fixture;
- the existing six invocations are unchanged.

### Lean behavioral gate

- `lean_policy_tests` passes under `standard-http1,tls-rustls`;
- `lean_route_tests` passes under `standard-http1,tls-rustls`;
- both suites still pass under the default profile, so the gate does not
  narrow them.

### Oracle

- the oracle runs eight profiles and passes;
- the two new snapshots are present and committed;
- the comparison of each new snapshot against its H1 counterpart is recorded;
- the semver half still passes on the default profile;
- no existing snapshot changed.

### Regression

- Tier 1 and Tier 2 are green;
- the workspace test suite is unchanged and green.

## 11. Required verification commands

Follow `docs/verification-policy.md` and
`.skills/verification-qualification.md`.

Focused first:

```bash
RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features standard-http2,tls-rustls -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features native-http2,tls-rustls -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test lean_policy_tests --no-default-features --features standard-http1,tls-rustls -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test lean_route_tests --no-default-features --features standard-http1,tls-rustls -- --test-threads=1
```

Then the oracle, which needs the pinned toolchain and tools:

```bash
rustup toolchain install 1.89.0 --profile minimal
rustup toolchain install nightly-2026-05-07 --profile minimal
RUSTUP_TOOLCHAIN=nightly-2026-05-07 \
  CARGO_PUBLIC_API=/path/to/cargo-public-api \
  CARGO_SEMVER_CHECKS=cargo-semver-checks \
  python scripts/check_rust_public_api.py
```

Then:

```bash
./scripts/check.sh
./scripts/check.sh extended
cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

Tier 2 requires the exact Rust 1.89.0 toolchain and fails rather than skips.
Do not create a new automatic workflow or a permanent CI job.

## 12. Documentation updates

Implementation must update:

- `docs/architecture/feature-flags.md` (the supported-profile matrix and the
  gate that covers each profile);
- `docs/reference/feature-matrix.md`;
- `compat/rust-public-api/README.md` (the profile count and the new
  snapshots);
- `docs/architecture/build-ci.md` (the Tier 1 contract gate contents);
- `plans/registry.md`, the subsystem roadmap and `plans/README.md` at
  closure.

## 13. Acceptance criteria

M008 closes only when:

- all eight supported profiles are compiled by the stable contract matrix;
- both missing profiles are frozen in the Rust public-surface oracle, with
  committed snapshots;
- the lean profile's behavioral suites run under the lean profile in the
  Tier 1 gate;
- the snapshot comparison against the H1 counterparts is recorded in the
  closure evidence;
- no existing oracle snapshot changed;
- the public API oracle reports zero unexplained and zero stale differences
  for all eight profiles;
- the documentation states the complete matrix and the gate covering each
  profile;
- Tier 1 and Tier 2 are green on the frozen executable SHA;
- both HTTPX profiles are rebound to that exact SHA.

## 14. Stop conditions

Stop and revise this plan rather than improvising if:

- generating a new snapshot requires changing an existing snapshot, which
  would mean the public surface changed;
- the oracle cannot be run because the pinned nightly toolchain or
  `cargo-public-api` is unavailable, in which case the snapshot must not be
  hand-written;
- running the lean behavioral suites under the lean profile turns out to
  require the whole-suite compile work tracked by M009, in which case the
  two named targets must still be run and the dependency must be recorded;
- a new profile turns out to need a different feature combination than the
  documented alias, which would mean the supported-profile matrix itself is
  wrong and the documentation must be corrected first.

## 15. Closure evidence required

Create `plans/closure/core-transport-policy/008-profile-matrix-completion.md` containing:

- implementation commit(s) and the final frozen SHA;
- the eight-profile contract matrix output;
- the eight-profile oracle output, including the semver half;
- the two new snapshots and the recorded comparison against their H1
  counterparts;
- the lean behavioral gate output under the lean profile;
- evidence that no existing snapshot changed;
- the public API delta (expected: none);
- the exact Tier 1 and Tier 2 outcomes;
- renewed HTTPX Stage C binding evidence for both profiles;
- known limitations, including that the whole test suite still does not
  compile under the lean profile.

## 16. Handoff notes

This milestone adds verification, not capability. If a new profile's
snapshot reveals an accidental public-surface difference between the H1 and
H2 profiles, that is a real finding and must be fixed here rather than
deferred.

Do not regenerate existing snapshots. The point of the oracle is to freeze
the surface; regenerating it to match a change defeats the gate.

The lean behavioral gate must run under the lean profile. Running the lean
suites under the default profile exercises the redirect path, not the lean
path, and would make the gate meaningless.

M009 depends on this milestone's lean coverage, and M009 in turn unblocks
running the whole suite under the lean profile. Keep them in that order.
