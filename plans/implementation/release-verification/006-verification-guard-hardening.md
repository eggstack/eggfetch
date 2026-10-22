# Release and Verification Milestone 006 — Verification Guard Hardening

Status: ready

Repository baseline: `873ea28541c82817317430c9523fca8525d99c81`

Source roadmap:

- `plans/subsystems/release-verification-roadmap.md#milestone-6--verification-guard-hardening`

Long-term requirements:

- `plans/000-long-term-specification.md` §8
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 items 2 and 4

Applicable ADRs:

- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`
- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`

Primary class: invariant

## 1. Objective

Close four defects in the repository's own verification tooling, each of
which currently allows a real regression to pass unnoticed or causes a
release gate to report success while doing nothing.

### 1.1 The lint-suppression guard does not match the codebase's own idiom

`scripts/check_lint_suppressions.sh` rejects blanket lint suppressions. Its
patterns match only the bare form `#[allow(clippy::pedantic)]`. They do not
match:

- `#[allow(clippy::pedantic, reason = "…")]` — **the repository's own
  established idiom**, used 44 times in `crates/*/src`;
- `#[allow(warnings, reason = "…")]`;
- `#[allow(clippy::all, clippy::pedantic)]`;
- `#[expect(...)]` of any form.

The scan root is `$REPO_ROOT/crates/` with a `*.rs` filter, so `fuzz/` and
any `examples/` tree are not scanned.

The script has no self-test, so nothing asserts that it still rejects what it
is supposed to reject. A contributor who follows the codebase's own
convention — `reason = "…"` — walks straight past the guard.

### 1.2 A release-blocking gate silently skips

`scripts/check.sh` `tier2_downstream` runs the `compat/downstream/`
portfolio, which `docs/verification-policy.md` treats as release-blocking.
When the artifact manifest is absent it calls `record_skip` and returns `0`,
so the Tier 2 run reports "Extended validation passed with 1 optional
check(s) skipped" and exits successfully.

The skip message instructs the operator to "run the downstream qualification
workflow first". **No such workflow exists.** `.github/workflows/` contains
only `ci.yml` and `pypi.yml`. The producing workflow was `qualification.yml`,
deleted on 2026-07-28 in commit `9bf81f46` ("ci: simplify verification and
make release manual"). Nothing in the repository generates
`target/downstream-qualification/artifact-manifest.json`; the only
references to that path are consumers.

The effect is that the downstream portfolio is a release gate that no-ops on
any checkout that does not already carry a manually-produced artifact.

### 1.3 Tier 2's feature matrix and MSRV gate fail in a CI-like environment

`ci.yml` sets `RUSTFLAGS: -D warnings` for the whole job. The Tier 1
contract matrix deliberately uses bare `RUSTFLAGS=` and carries a comment
explaining why: the lean and native profiles retain private helpers for other
supported profiles, and the CI-wide flag turns those profile-specific dead
code warnings into errors.

`tier2_feature_matrix` and `tier2_msrv` run `cargo check` without that
guard. Measured at the baseline with `RUSTFLAGS="-D warnings"`:

| Command | Result |
|---|---|
| `cargo check -p eggfetch-core --no-default-features` | **exit 101, 86 errors** |
| `cargo check -p eggfetch-core --no-default-features --features http1` | **exit 101, 3 errors** |
| `cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls` | exit 0 |
| `cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,tls-native-roots` | exit 0 |
| `cargo check -p eggfetch-core --all-features` | exit 0 |

So `./scripts/check.sh extended` passes in a plain shell but fails in an
environment that sets the flag CI sets. The tier's outcome depends on the
caller's environment rather than on the code.

### 1.4 The H3 qualification tooling never runs in any tier

`scripts/h3_qualification.py`, `scripts/h3_impairment.py` and
`scripts/test_h3_qualification.py` are not referenced by `scripts/check.sh`
at all — `grep -c h3 scripts/check.sh` returns `0`. The runner's own
self-test passes standalone (5 tests, OK) but is invoked by nothing.

This milestone adds the self-test to Tier 1. It does **not** claim H3
graduation, does not run any network-dependent H3 qualification, and does
not change H3's experimental status.

## 2. Why this milestone is ready

- Each defect is a mechanical property of the tooling, confirmed by
  measurement rather than by inspection alone.
- The lint-guard fix is a pattern change plus a self-test; it changes no
  production code.
- The downstream-gate fix is a decision about fail-open versus fail-closed,
  and the repository's policy already requires Tier 2 to "fail, never
  skip" for the toolchain gate. Extending the same rule to a
  release-blocking portfolio is consistent with the existing policy rather
  than a new policy.
- The `-D warnings` fix restores the guard the Tier 1 matrix already
  documents and applies, so it reduces rather than adds policy.
- The H3 self-test already passes, so adding it to Tier 1 introduces no new
  failure mode.
- No dependency of this milestone is blocked. It does not depend on the H3
  graduation program, the HTTPX 1.0 program, or Node maturation.

## 3. Current implementation evidence

At the baseline:

- `scripts/check_lint_suppressions.sh` scans
  `$REPO_ROOT/crates/` with `-g "*.rs"` / `--include="*.rs"` and matches six
  bare attribute forms;
- the repository uses `reason = "…"` on 44 `allow`/`expect` attributes in
  `crates/*/src`, so the guard's patterns do not match the dominant local
  idiom;
- the script has no `--self-test` mode and no test file;
- `scripts/check.sh` `tier2_downstream` calls `record_skip` and returns `0`
  when `target/downstream-qualification/artifact-manifest.json` is absent;
- the skip message names a "downstream qualification workflow" that does not
  exist in `.github/workflows/`;
- git history shows `.github/workflows/qualification.yml` added and later
  deleted, with the deletion in `9bf81f46`;
- `grep -rn "downstream-qualification" scripts/ compat/ docs/` finds only
  consumers, never a producer;
- `scripts/check.sh` `tier2_feature_matrix` and `tier2_msrv` invoke
  `cargo check` without a bare `RUSTFLAGS=` prefix;
- `.github/workflows/ci.yml` sets `RUSTFLAGS: -D warnings` globally;
- `grep -c h3 scripts/check.sh` returns `0`;
- `python scripts/test_h3_qualification.py` runs 5 tests and passes.

## 4. Invariants that must not regress

- Tier 1 remains the required pre-commit gate and remains a single CI job
  on `ubuntu-latest`.
- Tier 2 remains required before release and fails rather than skips when
  its hard requirements are unmet.
- Tier 3 remains required before publish.
- The lint-suppression policy is unchanged in substance: blanket
  `allow`/`deny(warnings, clippy::all/pedantic/nursery/restriction)` forms
  remain rejected, and `eggfetch-ffi` and `eggfetch-node` remain exempt from
  the pedantic rule as the workspace policy allows.
- The `-D warnings` posture is unchanged: warnings are errors in the
  all-features workspace checks and in clippy. Only the profile-isolated
  checks are exempted, exactly as the Tier 1 contract matrix already
  documents.
- No new automatic workflow, matrix, or evidence schema is introduced.
- The H3 self-test added to Tier 1 must remain a self-test: it must not
  require network access, a QUIC peer, or the `http3` feature.
- H3 remains experimental. Nothing in this milestone changes H3's status,
  blockers, or graduation criteria.

## 5. Scope

### In scope

- the lint-suppression guard's patterns, its `expect` coverage, and its scan
  root;
- a self-test for the lint-suppression guard that asserts it *rejects* the
  forms it must reject;
- the downstream gate's fail-open behavior and its reference to a
  non-existent workflow;
- the `RUSTFLAGS` treatment of the Tier 2 feature matrix and MSRV gate;
- the H3 qualification runner's self-test in Tier 1.

### Explicitly out of scope

- adding any new CI job, matrix, or scheduled workflow;
- restoring a permanent qualification workflow;
- running any network-dependent H3 qualification;
- H3 graduation, impairment evidence, or interop evidence;
- the profile-matrix coverage gaps tracked by core-transport-policy M008;
- the whole-suite lean-profile coverage tracked by core-transport-policy
  M009;
- the plan and documentation hygiene tracked by release-verification M007;
- publication, version bump, or release orchestration.

## 6. Required production changes

### 6.1 Lint-suppression guard

The guard's patterns must match an `allow`, `deny` or `expect` attribute
whose lint list contains a blanket lint, **regardless of any additional
items** in the attribute, including a `reason = "…"` field.

The match must be anchored on the lint list rather than on the whole
attribute body, so that a blanket lint combined with other lints is still
rejected. A blanket lint inside a multi-lint attribute is exactly the case
the current patterns miss.

The guard must additionally cover `expect`, which it currently does not
check at all. `#[expect(clippy::pedantic)]` is a blanket suppression with the
same effect as `allow` and must be treated identically.

The scan root must include the `fuzz/` tree, which contains Rust source that
is compiled and fuzzed. The `eggfetch-ffi` and `eggfetch-node` exemptions
must continue to apply.

The guard must gain a self-test mode, following the pattern already used by
`scripts/check_native_python_api.py` and
`scripts/check_python_typing_surface.py`, which both provide `--self-test`.
The self-test must construct, in a temporary directory, one attribute of each
form the guard must reject and one of each form it must permit, and assert
the guard's verdict for every case. It must cover at minimum:

| Form | Expected verdict |
|---|---|
| `#[allow(clippy::pedantic)]` | rejected |
| `#[allow(clippy::pedantic, reason = "x")]` | rejected |
| `#[allow(warnings, reason = "x")]` | rejected |
| `#[allow(clippy::all, clippy::pedantic)]` | rejected |
| `#[expect(clippy::pedantic)]` | rejected |
| `#[allow(clippy::pedantic, reason = "x", clippy::nursery)]` | rejected |
| `#[allow(clippy::cast_precision_loss)]` | permitted |
| `#[allow(clippy::pedantic)]` inside `crates/eggfetch-ffi/` | permitted |
| `#[allow(clippy::pedantic)]` inside `crates/eggfetch-node/` | permitted |
| `#[allow(unused_mut)]` | permitted |

The self-test must run in Tier 1 and must be fast.

The implementer must additionally record, in the closure evidence, how many
existing suppressions in `crates/*/src` the corrected patterns newly match.
If the corrected guard rejects existing suppressions in the tree, the
implementer must either convert those suppressions to a permitted form or
record each one with a justification. A blanket suppression that the
repository's own policy forbids must not be grandfathered by this
milestone.

### 6.2 Downstream gate must not silently skip

The downstream portfolio is release-blocking per `docs/verification-policy.md`.
The Tier 2 gate must not report success while skipping it.

Two acceptable implementations:

- **Fail closed.** A missing artifact manifest is a Tier 2 failure with an
  actionable message describing exactly how to produce the manifest. Tier 2
  then cannot complete on a checkout that lacks the artifact, which is the
  correct behavior for a release-blocking gate.
- **Promote the artifact to a required input.** Provide a repository-owned
  producer for the artifact manifest, so the gate can run it from a clean
  checkout.

The implementer must choose one and record the choice. The current
fail-open behavior with a message naming a workflow that does not exist is
not acceptable under either option.

If the fail-closed option is chosen, the error message must name the
producer that exists, or state plainly that the artifact must be supplied
by the operator, and must not refer to a workflow that is not in the
repository.

The implementer must also confirm what the downstream portfolio requires to
run, and whether it needs network access. If it needs network access, that
fact must be stated in the gate's output rather than being discovered as a
failure.

### 6.3 Tier 2 `RUSTFLAGS` treatment

`tier2_feature_matrix` and `tier2_msrv` must apply the same bare `RUSTFLAGS=`
treatment that `tier1_rust_api_contracts` already applies, with a comment
carrying the same reason.

The exemption is limited to the profile-isolated checks. The all-features
checks, the workspace checks and clippy must keep `-D warnings`.

The implementer must verify that the Tier 2 feature matrix passes both with
and without `RUSTFLAGS="-D warnings"` set in the environment.

### 6.4 H3 qualification self-test in Tier 1

Tier 1 must run the H3 qualification runner's self-test:

```bash
"$PYTHON_BIN" scripts/test_h3_qualification.py
```

The invocation must not require network access, a QUIC peer, the `http3`
feature, or any external artifact. It must be placed in the Tier 1 sequence
where it runs early enough to catch a broken runner before the rest of the
tier spends time, and after the Python environment check.

The Tier 1 output must make it clear that this is a **runner self-test**, not
an H3 qualification result. The tier must not print or imply any H3
conformance claim.

## 7. Ordered work packages

1. Add the `--self-test` mode to `scripts/check_lint_suppressions.sh`, with
   the verdict table from section 6.1.
2. Correct the guard's patterns to match blanket lints in any attribute
   position, cover `expect`, and extend the scan root to `fuzz/`.
3. Run the corrected guard against the tree; record every newly matched
   suppression and resolve each one.
4. Wire the lint guard's self-test into Tier 1.
5. Choose and implement the downstream gate's behavior, and update the
   skip/fail message to reference something that exists.
6. Add the bare `RUSTFLAGS=` treatment to `tier2_feature_matrix` and
   `tier2_msrv`, with the explanatory comment.
7. Verify Tier 2's feature matrix passes with and without
   `RUSTFLAGS="-D warnings"`.
8. Add the H3 qualification self-test to Tier 1.
9. Update `docs/verification-policy.md`, `docs/architecture/build-ci.md` and
   `.skills/verification-qualification.md` to describe the corrected gates.
10. Run Tier 1 and Tier 2.
11. Renew the exact-SHA Stage C binding, because the validation scripts are
    executable inputs.
12. Write `plans/closure/release-verification/006-verification-guard-hardening.md`.

## 8. Failure, cancellation, timeout, and pool semantics

- This milestone changes no production runtime behavior. No timeout, pool,
  cancellation, retry, redirect or body-lifecycle semantics change.
- The Tier 1 gate must remain fast enough to be a pre-commit gate. The
  added self-tests must be sub-second.
- The H3 self-test must not open a socket, resolve a name, or attempt a QUIC
  handshake. It must be a pure unit self-test of the runner's logic.
- A failing gate must fail the tier, not merely warn.
- The lint-guard self-test must not modify the working tree.

## 9. Compatibility and migration

- No public API change and no cargo feature change.
- No downstream migration is implied.
- Tier 2's outcome becomes environment-independent: it passes or fails based
  on the code, not on the caller's `RUSTFLAGS`.
- Tier 2's downstream step becomes explicit: it either runs or it fails with
  an actionable message.
- The lint guard may newly match existing suppressions in the tree. Those
  must be resolved or individually justified; the closure evidence records
  each one.
- `scripts/check.sh` and `scripts/check_lint_suppressions.sh` are executable
  inputs, so the exact-SHA Stage C binding must be renewed under
  `.skills/verification-qualification.md`.
- No crate version bump is implied.

## 10. Required tests

### Lint guard self-test

- every row of the section 6.1 verdict table is asserted;
- the self-test exits non-zero when the guard is broken, proven by
  temporarily perturbing a pattern in a scratch copy;
- the self-test does not modify the working tree;
- the self-test runs in under two seconds.

### Lint guard against the tree

- the corrected guard's output over `crates/` and `fuzz/` is recorded;
- every newly matched blanket suppression is resolved or individually
  justified in the closure evidence;
- `eggfetch-ffi` and `eggfetch-node` remain exempt from the pedantic rule;
- no new blanket suppression is introduced.

### Downstream gate

- with the artifact manifest absent, the Tier 2 downstream step either fails
  with an actionable message or runs from a clean checkout; it does not
  report success while skipping;
- the message references only things that exist in the repository;
- with the artifact manifest present, the downstream step runs as before.

### Tier 2 environment independence

- `tier2_feature_matrix` passes with `RUSTFLAGS="-D warnings"` set;
- `tier2_feature_matrix` passes with `RUSTFLAGS` unset;
- `tier2_msrv` passes with `RUSTFLAGS="-D warnings"` set;
- the all-features workspace check still fails on a warning, proving the
  exemption did not widen.

### H3 self-test

- the H3 runner self-test runs in Tier 1 and passes;
- it makes no network connection;
- the Tier 1 output contains no H3 conformance claim.

## 11. Required verification commands

Follow `docs/verification-policy.md` and
`.skills/verification-qualification.md`.

Focused first:

```bash
python scripts/check_lint_suppressions.sh --self-test
python scripts/check_lint_suppressions.sh
python scripts/test_h3_qualification.py
RUSTFLAGS="-D warnings" bash -c 'source scripts/check.sh' 2>/dev/null || true
```

Then:

```bash
./scripts/check.sh
./scripts/check.sh extended
```

Tier 2 requires the exact Rust 1.89.0 toolchain and fails rather than skips.

Do not create a new automatic workflow or a permanent network-dependent CI
job. The downstream gate must not require one.

## 12. Documentation updates

Implementation must update:

- `docs/verification-policy.md` (the downstream gate's behavior, the
  `-D warnings` exemption scope, and the lint guard's coverage);
- `docs/architecture/build-ci.md` (the Tier 1 and Tier 2 step lists);
- `.skills/verification-qualification.md` (the focused equivalents and the
  new self-tests);
- `docs/architecture/overview.md` if the H3 tooling's tier placement is
  described there;
- `plans/registry.md`, the subsystem roadmap and `plans/README.md` at
  closure.

## 13. Acceptance criteria

M006 closes only when:

- the lint guard rejects every form in the section 6.1 "rejected" table and
  permits every form in the "permitted" table;
- the lint guard's self-test runs in Tier 1 and fails when the guard is
  broken;
- the guard covers `expect` and scans `fuzz/`;
- every existing blanket suppression in the tree is resolved or individually
  justified, with the list recorded;
- the downstream Tier 2 step does not report success while skipping, and its
  message references only what exists;
- `tier2_feature_matrix` and `tier2_msrv` pass with and without
  `RUSTFLAGS="-D warnings"`;
- the all-features workspace check still fails on a warning;
- the H3 runner self-test runs in Tier 1, passes, makes no network
  connection, and produces no H3 conformance claim;
- CI remains a single job on `ubuntu-latest`;
- Tier 1 and Tier 2 are green on the frozen executable SHA;
- both HTTPX profiles are rebound to that exact SHA.

## 14. Stop conditions

Stop and revise this plan rather than improvising if:

- correcting the lint guard's patterns would reject a large number of
  existing suppressions that cannot be individually justified, in which case
  the guard's policy needs a maintainer decision before the pattern change
  lands;
- the downstream portfolio cannot be made to run from a clean checkout
  without a network dependency, in which case the fail-closed option must be
  chosen and the message must state the operator obligation plainly;
- adding the bare `RUSTFLAGS=` treatment would mask a real error in a
  profile-isolated check rather than only profile-specific dead code;
- the H3 self-test requires network access, a QUIC peer, or the `http3`
  feature, in which case it does not belong in Tier 1;
- any change would require a new CI job, matrix, or scheduled workflow.

## 15. Closure evidence required

Create `plans/closure/release-verification/006-verification-guard-hardening.md` containing:

- implementation commit(s) and the final frozen SHA;
- the lint guard's verdict-table self-test output;
- the lint guard's full-tree output before and after the pattern change, and
  the disposition of every newly matched suppression;
- the chosen downstream gate behavior and the before/after Tier 2 output
  with and without the artifact manifest;
- the Tier 2 feature matrix output with and without `RUSTFLAGS="-D
  warnings"`;
- evidence that the all-features check still fails on a warning;
- the Tier 1 output showing the H3 self-test running;
- the exact Tier 1 and Tier 2 outcomes;
- renewed HTTPX Stage C binding evidence for both profiles;
- known limitations, including that the downstream portfolio may still
  require an operator-supplied artifact.

## 16. Handoff notes

Each of the four fixes is small. The value is that they turn four
"currently passing" states into states that mean something.

The lint guard's self-test must assert the guard *rejects* things. A
self-test that only asserts the guard accepts the tree would pass with a
broken guard.

The downstream gate decision is the only one with a policy flavor. Record
the choice and its reason; the default expectation is that a
release-blocking gate fails rather than skips.

Do not let the `-D warnings` exemption spread beyond the profile-isolated
checks. The all-features workspace check and clippy must keep treating
warnings as errors.

Adding the H3 self-test to Tier 1 must not be described as H3 qualification.
H3 remains experimental and its graduation criteria are unchanged.
