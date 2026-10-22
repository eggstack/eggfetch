# Release and Verification Milestone 006 — Verification Guard Hardening

Status: conditionally closed

Source implementation plan:

- `plans/implementation/release-verification/006-verification-guard-hardening.md`

Source subsystem roadmap:

- `plans/subsystems/release-verification-roadmap.md#milestone-6--verification-guard-hardening`

Repository baseline reviewed: `873ea28541c82817317430c9523fca8525d99c81`.

Implementation commits:

- None. The milestone is implemented in the working tree and is **not
  committed**; the maintainer reviews the working tree before any commit is
  made. No SHA is claimed for this milestone.

## 1. Executive finding

M006 is implemented, and each of the four fixes the milestone set out to make
is present and measured. It cannot be marked `closed` under
`plans/closure/README.md`, for the same reason as the other milestones closed
in this pass: the milestone changes validation scripts, which are executable
inputs under the exact-SHA rule in `.skills/verification-qualification.md`.
That invalidates the live Stage C binding, and neither the Stage C rebinding
nor a full Tier 1 / Tier 2 / Tier 3 run on the frozen SHA was performed — the
maintainer deliberately deferred the full tier gates. The milestone is
therefore **conditionally closed**, with the condition, the risk, and the exact
future evidence stated in §10.

What the milestone delivered, in the four areas the plan named:

- **The lint-suppression guard now anchors on the attribute's lint list** and
  covers `expect` as well as `allow` and `deny`, so a blanket lint combined
  with other items or carrying a `reason = "…"` field is caught. It scans
  `fuzz/` as well as `crates/`, and it gained a `--self-test` mode that asserts
  the guard's verdict on **33 cases**. The self-test is proven to go red when
  the guard is broken.
- **The downstream Tier 2 step fails closed** instead of recording a skip. Its
  message names only artifacts that exist in the repository and states the
  operator obligation plainly. The consequence is stated honestly below: with
  the artifact absent, `./scripts/check.sh extended` can no longer complete on
  a clean checkout. That is the intended behavior for a release-blocking gate.
- **`tier2_feature_matrix` and `tier2_msrv` apply the bare `RUSTFLAGS=`
  treatment** to their profile-isolated checks only. The all-features checks,
  the workspace checks and clippy keep `-D warnings`.
- **Tier 1 runs the H3 qualification runner's self-test** and the lint guard's
  self-test. The H3 step is a *runner* self-test: it makes no network
  connection, needs no QUIC peer and no `http3` feature, and makes no H3
  conformance claim.

Nothing is claimed here that was not measured. The full Tier 1 / Tier 2 /
Tier 3 gates, the full workspace suite, the compat pytest suites and the Rust
public-API oracle were **not** run at the frozen SHA.

## 2. Requirement-to-evidence matrix

The requirements are the acceptance criteria of §13 of the implementation plan.

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| The lint guard rejects every form in the §6.1 "rejected" table and permits every form in the "permitted" table (§13) | `bash scripts/check_lint_suppressions.sh --self-test` → exit 0, "OK: lint-suppression guard self-test passed (33 cases)" | Met | 33 cases: 23 rejection forms and 10 permitted forms |
| The guard's self-test runs in Tier 1 and fails when the guard is broken (§13) | `scripts/check.sh`, function `tier1_lint_suppression_self_test`, called from `run_tier1`; a scratch copy of the script with the `clippy::pedantic` entry replaced in its blanket map → self-test exit 1, "8 of 33 case(s) wrong" | Met | The perturbed copy was written outside the repository tree and discarded |
| The guard covers `expect` and scans `fuzz/` (§13) | `scripts/check_lint_suppressions.sh` scans `crates/` and `fuzz/`; `#[expect(clippy::pedantic)]` is a rejection case in the self-test | Met | The `eggfetch-ffi` and `eggfetch-node` exemptions are preserved |
| Every existing blanket suppression in the tree is resolved or individually justified, with the list recorded (§13, §6.1) | `bash scripts/check_lint_suppressions.sh` → exit 0, "OK: No forbidden broad lint suppressions found"; the baseline guard run against the same tree also reports no match | Met | **Zero** tree churn, and it is measured rather than assumed — see §4 |
| The downstream Tier 2 step does not report success while skipping, and its message references only what exists (§13, §6.2) | `tier2_downstream` extracted from `scripts/check.sh` and run in isolation with the manifest absent → exit 1, "FAIL: Downstream behavioral fixtures — release-blocking artifact manifest missing", no skip recorded | Met | `compat/downstream/README.md`, `compat/downstream/manifest.toml` and `scripts/run_downstream_compat.py` all exist; `.github/workflows/` contains only `ci.yml` and `pypi.yml` |
| `tier2_feature_matrix` and `tier2_msrv` pass with and without `RUSTFLAGS="-D warnings"` (§13, §6.3) | Before: with `RUSTFLAGS="-D warnings"` inherited, the profile-isolated checks fail (exit 101; 89 and 4 errors). After: with the bare `RUSTFLAGS=` prefix the script now uses, both exit 0. The all-features check with the flag kept exits 0 | Met | The MSRV leg was not exercised — it needs the exact Rust 1.89.0 toolchain and is a Tier 2 requirement |
| The all-features workspace check still fails on a warning (§13) | The all-features rows of `tier2_feature_matrix` and `tier2_msrv` carry no bare prefix, so the inherited flag still applies; a scratch crate with an unused import compiled with `RUSTFLAGS="-D warnings"` → exit 101, "`-D unused-imports` implied by `-D warnings`" | Met (mechanism) | The mechanism is demonstrated in a scratch crate, not by injecting a warning into the repository tree. The repository's own all-features build is warning-free today, so it exits 0 with the flag kept |
| The H3 runner self-test runs in Tier 1, passes, makes no network connection, and produces no H3 conformance claim (§13, §6.4) | `scripts/check.sh`, function `tier1_h3_runner_self_test`, placed first in `run_tier1` after the Python environment check; `.venv/bin/python scripts/test_h3_qualification.py` → exit 0, "Ran 5 tests in 0.002s … OK"; the same run under `unshare -rn` → exit 0 | Met | `unshare -rn` removes the network namespace, so the run proves no network connection. No `http3` feature is required |
| CI remains a single job on `ubuntu-latest` (§13) | `.github/workflows/ci.yml` has one job, `ci`, on `runs-on: ubuntu-latest`; `.github/workflows/` holds `ci.yml` and the pre-existing manually dispatched `pypi.yml` | Met | No workflow was added or changed by this milestone |
| Tier 1 and Tier 2 are green on the frozen executable SHA (§13) | Not run | **Not met** | Deliberately deferred; see §10 finding 2 |
| Both HTTPX profiles are rebound to that exact SHA (§13) | Not performed | **Not met** | The live binding is invalidated by this milestone; see §7 and §10 finding 1 |

## 3. Production implementation evidence

### 3.1 The lint-suppression guard

`scripts/check_lint_suppressions.sh` was rewritten. The old script matched six
*bare* attribute forms, so an attribute such as
`#[allow(clippy::pedantic, reason = "x")]` or
`#[allow(clippy::all, clippy::pedantic)]` passed it. The new scanner anchors
on the attribute's **lint list** rather than on the whole attribute body, so
the following are all rejected while specific lints remain permitted:

| Form | Verdict |
|---|---|
| `#[allow(clippy::pedantic)]` | rejected |
| `#[allow(clippy::pedantic, reason = "x")]` | rejected |
| `#[allow(warnings, reason = "x")]` | rejected |
| `#[allow(clippy::all, clippy::pedantic)]` | rejected |
| `#[expect(clippy::pedantic)]` | rejected |
| `#[allow(clippy::pedantic, reason = "x", clippy::nursery)]` | rejected |
| `#[allow(clippy::cast_precision_loss)]` | permitted |
| `#[allow(unused_mut)]` | permitted |
| `#[allow(clippy::pedantic)]` in `crates/eggfetch-ffi/` | permitted |
| `#[allow(clippy::pedantic)]` in `crates/eggfetch-node/` | permitted |

`expect` coverage was added because `#[expect(clippy::pedantic)]` is a blanket
suppression with the same effect as `allow`. The scan root was extended from
`crates/` to `crates/` and `fuzz/`. The `eggfetch-ffi` and `eggfetch-node`
exemptions for the pedantic rule are preserved.

**`--self-test` mode.** The self-test builds, in a temporary directory, one
attribute of each form the guard must reject and each form it must permit, and
asserts the verdict for every case — 33 cases in total. It follows the pattern
already used by `scripts/check_native_python_api.py` and
`scripts/check_python_typing_surface.py`. It runs in well under two seconds
and does not modify the working tree.

### 3.2 The downstream gate fails closed

`tier2_downstream` previously called `record_skip` and returned 0 when
`target/downstream-qualification/artifact-manifest.json` was absent, so Tier 2
reported success while skipping a release-blocking gate. It now calls `fail`.
The skip message also pointed at a downstream qualification workflow that does
**not** exist in the repository — `.github/workflows/` contains only `ci.yml`
and `pypi.yml`, and the producing workflow was deleted.

The **fail-closed** option was chosen and the reason is recorded here: the
portfolio needs a network-reachable HTTPX baseline with pinned reference
responses, which cannot be produced from a clean checkout offline, so a
repository-owned producer would require the network dependency that this
milestone's scope forbids (the plan's §14 stop condition names exactly this
case). The new message names only artifacts that exist in the repository —
`compat/downstream/manifest.toml`, `compat/downstream/README.md` and
`scripts/run_downstream_compat.py` — and states the operator obligation
plainly.

**The consequence, stated plainly:** with the artifact manifest absent,
`./scripts/check.sh extended` can no longer complete on a clean checkout. That
is the intended behavior for a release-blocking gate. It is not softened into
a skip, and no substitute evidence is offered in its place.

### 3.3 The Tier 2 `RUSTFLAGS` treatment

`scripts/check.sh` now reads:

```sh
tier2_feature_matrix() {
    # … Bare RUSTFLAGS= overrides CI's -D warnings for the profile-isolated
    # checks only; the all-features check below keeps the flag.
    RUSTFLAGS= cargo check -p eggfetch-core --no-default-features
    RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features http1
    RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
    RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,tls-native-roots
    cargo check -p eggfetch-core --all-features
}
```

and, in `tier2_msrv`, the three profile-isolated checks carry the same bare
prefix while the two all-features / workspace checks keep the flag:

```sh
RUSTFLAGS= rustup run "$msrv_toolchain" cargo check $check_args   # profile-isolated
rustup run "$msrv_toolchain" cargo check $check_args             # all-features, workspace
```

Both sites carry a comment with the same reason as the existing
`tier1_rust_api_contracts` matrix: the lean and native profiles retain private
helpers for other supported profiles, and CI's global `-D warnings` turns
profile-specific dead-code warnings into errors. The exemption is limited to
the profile-isolated checks; the all-features checks, the workspace checks and
clippy keep `-D warnings`.

### 3.4 The Tier 1 self-tests

`scripts/check.sh` gained two Tier 1 steps:

- `tier1_h3_runner_self_test` — runs `scripts/test_h3_qualification.py`. It is
  placed **first** in `run_tier1`, after the Python environment check, so a
  broken runner is caught in under a second before the tier spends time on
  cargo work. It is a **runner** self-test: it makes no network connection, it
  requires no QUIC peer and no `http3` feature, and it makes **no H3
  conformance claim**. H3 remains experimental and is not a Tier 1 gate.
- `tier1_lint_suppression_self_test` — runs
  `scripts/check_lint_suppressions.sh --self-test`.

### 3.5 Documentation

`docs/verification-policy.md`, `docs/architecture/build-ci.md`,
`docs/architecture/overview.md` and `.skills/verification-qualification.md`
were updated for the downstream gate's behavior, the `-D warnings` exemption
scope, the lint guard's coverage and the new self-tests. `AGENTS.md` was
updated by the main agent to describe the corrected guard, the `fuzz/` scan
root, the new self-tests, the H3 runner self-test and the downstream gate's
fail-closed behavior.

## 4. Verification executed

### Commands run

The working tree is shared with another agent that was editing
`crates/eggfetch-core/src/**` during this closure pass. The lint-guard, script
and Python measurements below do not depend on that source tree; the cargo
measurements are pinned to the tree fingerprint given.

```sh
# lint guard
bash scripts/check_lint_suppressions.sh --self-test
bash scripts/check_lint_suppressions.sh

# perturbed guard: a scratch copy with one blanket-lint entry broken
cp scripts/check_lint_suppressions.sh /tmp/perturbed.sh
sed -i 's/BLANKET\["clippy::pedantic"\] = 1/BLANKET["clippy::pedantic_broken"] = 1/' /tmp/perturbed.sh
bash /tmp/perturbed.sh --self-test

# baseline guard, run against the same tree from a sandbox with the repo root resolved
git show 873ea285…:scripts/check_lint_suppressions.sh > /tmp/oldguard/scripts/check_lint_suppressions.sh
ln -s <repo>/crates /tmp/oldguard/crates && ln -s <repo>/fuzz /tmp/oldguard/fuzz
bash /tmp/oldguard/scripts/check_lint_suppressions.sh

# before / after of the -D warnings treatment
RUSTFLAGS="-D warnings" cargo check -p eggfetch-core --no-default-features
RUSTFLAGS="-D warnings" cargo check -p eggfetch-core --no-default-features --features http1
RUSTFLAGS="-D warnings" bash -c 'RUSTFLAGS= cargo check -p eggfetch-core --no-default-features'
RUSTFLAGS="-D warnings" bash -c 'RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features http1'
RUSTFLAGS="-D warnings" bash -c 'cargo check -p eggfetch-core --all-features'

# downstream gate, exercised in isolation with the manifest absent
awk '/^tier2_downstream\(\) \{/,/^\}/' scripts/check.sh > /tmp/t2d.sh
bash -c '… define info/warn/fail …; source /tmp/t2d.sh; tier2_downstream'

# H3 runner self-test
.venv/bin/python scripts/test_h3_qualification.py
unshare -rn .venv/bin/python scripts/test_h3_qualification.py

# tree-wide suppressions
grep -rn "#\!\[expect(" crates/ fuzz/          # zero matches
grep -rn "clippy::pedantic" crates/eggfetch-ffi crates/eggfetch-node   # zero matches
```

### Results (pass/fail/skip with reason; counts without concealment)

| # | Invocation | Result |
|---|---|---|
| 1 | `bash scripts/check_lint_suppressions.sh --self-test` | exit 0 — "OK: lint-suppression guard self-test passed (33 cases)." |
| 2 | `bash scripts/check_lint_suppressions.sh` | exit 0 — "OK: No forbidden broad lint suppressions found." over `crates/` and `fuzz/` |
| 3 | Perturbed guard, `--self-test` | **exit 1** — "Lint-suppression guard self-test FAILED: 8 of 33 case(s) wrong", naming `#[allow(clippy::pedantic)]`, `#[allow(clippy::pedantic, reason = "x")]`, `#[expect(clippy::pedantic)]`, `#[deny(clippy::pedantic)]`, `#![allow(clippy::pedantic)]` and three more |
| 4 | Baseline guard (from `873ea285…`) run against the current tree | exit 0 — "OK: No forbidden broad lint suppressions found." **Same verdict as the corrected guard: zero tree churn, measured rather than assumed** |
| 5 | Baseline guard vs `#[allow(clippy::pedantic, reason = "x")]` in a scratch crate | exit 0 — **false pass**: the attribute is not detected |
| 6 | Corrected guard vs the same file | **exit 1** — "ERROR: Found forbidden broad lint suppressions: crates/demo/src/lib.rs:2:1: allow clippy::pedantic — blanket pedantic suppression" |
| 7 | `RUSTFLAGS="-D warnings" cargo check -p eggfetch-core --no-default-features` (before state) | **exit 101**, 89 errors |
| 8 | `RUSTFLAGS="-D warnings" cargo check -p eggfetch-core --no-default-features --features http1` (before state) | **exit 101**, 4 errors |
| 9 | Same two commands with the bare `RUSTFLAGS=` prefix the script now uses (after state) | exit 0 and exit 0 |
| 10 | `RUSTFLAGS="-D warnings" bash -c 'cargo check -p eggfetch-core --all-features'` (flag kept) | exit 0 — the all-features profile is warning-free today, and the flag is still in force |
| 11 | Scratch crate with an unused import, `RUSTFLAGS="-D warnings"` | **exit 101** — "`-D unused-imports` implied by `-D warnings`": the mechanism that makes the all-features rows still fail on a warning |
| 12 | `tier2_downstream`, isolated, manifest absent | **exit 1** — "FAIL: Downstream behavioral fixtures — release-blocking artifact manifest missing", followed by the full operator message; **no skip was recorded** |
| 13 | `.venv/bin/python scripts/test_h3_qualification.py` | exit 0 — "Ran 5 tests in 0.002s … OK" |
| 14 | `unshare -rn .venv/bin/python scripts/test_h3_qualification.py` | exit 0 — "Ran 5 tests in 0.001s … OK" with no network namespace available |
| 15 | `grep -rn "#![expect(" crates/ fuzz/` | zero matches — the tree contains no `#[expect(...)]` attribute at all |
| 16 | `grep -rn "clippy::pedantic" crates/eggfetch-ffi crates/eggfetch-node` | zero matches — the FFI / Node pedantic exemptions are currently unused |
| 17 | `.github/workflows/ci.yml` | one job, `ci`, `runs-on: ubuntu-latest` |

The 89 and 4 errors in rows 7 and 8 are, by kind, entirely the
profile-specific category the bare prefix exists for: 11 × "associated
function `new` is never used", 2 × "unused import: `std::sync::Arc`", 2 ×
"associated function `from_policy` is never used", 1 × "variant `Adapter` is
never constructed", and the remainder unused imports of `Error`, `Result`,
`Timeout`, `TimeoutPhase`, `Duration`, `OriginKey`, `PoolGuard`,
`NativeRequestBody`, `crate::timeout::Timeout` and
`crate::stream::write_timeout_stream`; the `--features http1` case adds "method
`is_https` is never used". Under the bare prefix the identical set is reported
as warnings, not errors. The exemption masks profile-specific dead code and
nothing else.

The plan's §1.3 baseline table recorded 86 and 3 errors for the same two
commands at the baseline SHA; the counts above are larger because they were
measured against the current working tree, which carries other milestones'
changes. The before/after comparison that matters is the one within a single
tree, and that is what rows 7–10 show.

Not run, with reason:

- `./scripts/check.sh` (Tier 1), `./scripts/check.sh extended` (Tier 2),
  `./scripts/check.sh package` (Tier 3) — deliberately deferred by the
  maintainer. The targeted equivalents in the table above are the substitute
  evidence. Note that `./scripts/check.sh extended` would now stop at the
  downstream gate (§3.2), by design.
- The full workspace test suite (`cargo test --workspace --exclude
  eggfetch-python --all-features`) — same reason.
- The Python HTTPX compat pytest suites — same reason; they are also network
  dependent.
- The Rust public-API oracle (`scripts/check_rust_public_api.py`) — requires
  `cargo-public-api 0.52.0`, `cargo-semver-checks 0.49.0` and the pinned
  `nightly-2026-05-07`, and must be run at the frozen SHA.
- The MSRV leg of `tier2_msrv` — requires the exact Rust 1.89.0 toolchain
  (`rustup toolchain install 1.89.0 --profile minimal`) and is a Tier 2
  requirement. The three MSRV profile-isolated commands carry the same bare
  prefix as the four that were exercised, but they were not run.
- The repository's own all-features build with a warning deliberately
  injected — injecting a warning would mean editing the tree, which this
  closure pass does not do; row 11 demonstrates the mechanism in a scratch
  crate instead.

Every environment-variable assignment used the shell prefix
(`RUSTFLAGS="-D warnings" bash -c '…'`). The `env` binary on this machine is a
shim that ignores its arguments, so `env RUSTFLAGS=… cmd` would silently
produce a false pass; this hazard is recorded in the M008 closure record and
was avoided here.

## 5. Invariant review

- **No Rust source changed.** This milestone touches `scripts/` and `docs/`
  only. `git diff --stat -- crates/` shows no change attributable to it.
- **No cargo feature, dependency or manifest changed.** `Cargo.lock` is
  untouched.
- **CI remains a single job on `ubuntu-latest`.** No workflow, matrix or
  scheduled job was added; the downstream gate does not require one, which was
  an explicit constraint of the plan (§11, §14).
- **The `-D warnings` exemption is scoped.** It applies to the profile-isolated
  checks in `tier2_feature_matrix` and `tier2_msrv` only. The all-features
  check, the workspace check, clippy and every Tier 1 step keep the flag.
- **The exemption masks only profile-specific dead code.** Rows 7–11 in §4
  establish this: every error that the bare prefix suppresses is a
  never-used-item or unused-import warning in a profile that legitimately
  compiles the item out, and the flag still turns a warning into an error
  everywhere else.
- **The FFI and Node exemptions are preserved.** The guard still permits
  `clippy::pedantic` under `crates/eggfetch-ffi/` and `crates/eggfetch-node/`.
  Both exemptions are currently unused (§4 row 16), which is a property of the
  tree, not of the guard.
- **The lint policy is not weakened.** The corrected guard is strictly
  stricter than the one it replaces: it rejects more forms, covers `expect`,
  and scans more of the tree. It introduced no tree churn, so no existing
  suppression needed converting or justifying.
- **No release gate was softened.** The downstream step changed from
  fail-open to fail-closed. That is the opposite of a softening, and its cost
  is stated in §3.2 and §10.

## 6. Failure, timeout, pool, and recovery review

Not applicable. This milestone adds no request path, no transport, no timeout,
no pool behavior and no recovery logic. It changes three validation scripts
and their documentation.

The downstream gate's change is a failure-path change and is worth stating
precely: a Tier 2 run on a checkout without the artifact manifest now stops
with a non-zero exit and an operator-actionable message, where it previously
continued and reported success. The failure is reported through the script's
existing `fail` helper, which prints the message and exits non-zero; the
`record_skip` call is gone from that step.

One operational hazard is carried forward because it can silently invalidate
future verification in this environment: `env` on `PATH` resolves to
`/home/sugarwookie/.local/bin/env`, a shim that ignores its arguments and only
prepends `PATH`. An invocation of the form `env RUSTFLAGS="-D warnings" cargo …`
therefore succeeds without running `cargo` at all and produces a false pass.
Every result in §4 comes from the shell prefix form, which is also how
`scripts/check.sh` is written.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

This milestone does not change any cargo feature, any supported profile or any
public API surface. The profile matrix in
`docs/architecture/feature-flags.md` and the snapshot set in
`compat/rust-public-api/` are unaffected by it.

**Exact-SHA binding.** The live Stage C binding is **invalidated** by this
milestone. The live binding is `42a9c96db5b25354843bd83bd458c71757e95651`
(`compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml`,
and the ledger's Stage C entry in
`plans/httpx-parity-correction-status.md`), which is an ancestor of the
baseline `873ea285…` but does not cover the working tree.
`scripts/check_lint_suppressions.sh` and `scripts/check.sh` are validation
scripts and are executable inputs under
`.skills/verification-qualification.md`. Until the Stage C binding is
re-established at a frozen SHA that includes these changes, no publication may
cite the previous binding. No SHA is claimed here because the milestone is not
committed.

**Downstream portfolio requirements.** The gate's message now states, rather
than leaving it to be discovered as a failure, that the downstream portfolio
needs network access: the isolated runner creates a virtual environment,
downloads the pinned downstream packages and their wheels from PyPI and
verifies each package's source hash against `compat/downstream/manifest.toml`,
while the package test suites themselves run with outbound network disabled.

## 8. Security review

No security-relevant runtime behavior changed. The milestone touches
validation scripts only; no request path, transport, credential, trust store,
certificate handling, redaction or TLS configuration was touched.

Two security-adjacent observations are recorded as findings rather than
blockers:

- The corrected lint guard is **stricter** than the one it replaces. It
  rejects `expect` forms and blanket lints in multi-lint attributes that
  previously passed, so a future blanket suppression that would have slipped
  through the old guard is now caught. The measurement in §4 row 4 shows the
  stricter guard introduces no churn in the current tree, so nothing had to be
  rewritten.
- The downstream gate's fail-closed behavior means a Tier 2 run can no longer
  pass by omission. That is the intended direction: a release-blocking gate
  that reports success while skipping is a weaker gate than one that stops.

## 9. Documentation and operations

| File | Change |
|---|---|
| `docs/verification-policy.md` | The downstream gate's fail-closed behavior and the operator obligation; the scope of the `-D warnings` exemption; the lint guard's coverage (`expect`, `fuzz/`) and its self-test |
| `docs/architecture/build-ci.md` | The Tier 1 and Tier 2 step lists, including the two new Tier 1 self-tests and the downstream gate's new behavior |
| `docs/architecture/overview.md` | The H3 tooling's tier placement is described as a runner self-test, with no conformance claim |
| `.skills/verification-qualification.md` | The focused equivalents and the new self-tests |
| `AGENTS.md` | Updated by the main agent: the corrected guard, the `fuzz/` scan root, the new self-tests, the H3 runner self-test, and the downstream gate's fail-closed behavior |

Operations:

- Tier 1 gains two steps. The H3 runner self-test runs first and costs under a
  second; the lint guard's self-test builds a small temporary tree and is
  likewise fast. Neither needs the network, a credential or an external
  artifact, and neither modifies the working tree.
- Tier 2's downstream step now stops a run instead of continuing it. On a
  checkout without `target/downstream-qualification/artifact-manifest.json`,
  Tier 2 will not complete. That is the chosen behavior and is documented in
  the gate's own output.
- Tier 2's profile-isolated checks now override an inherited `-D warnings`
  with the bare `RUSTFLAGS=` prefix, matching the Tier 1 contract matrix. The
  all-features and workspace checks are unaffected.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| High | The live Stage C binding (`42a9c96d…`) is invalidated by this milestone's validation-script changes, and the Stage C rebinding was deliberately not performed | No publication may cite the previous Stage C binding; `compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml` still point at the superseded SHA | Rebind both HTTPX profiles and the ledger's Stage C SHA in `plans/httpx-parity-correction-status.md` to the frozen commit that includes this milestone, then re-run the compat qualification |
| High | Tier 1, Tier 2 and Tier 3 were not run on the frozen SHA (deliberately deferred), and neither was the Rust public-API oracle or the full workspace suite | The milestone's own gates are green, but the repository-wide gates and the public-surface oracle are unproven for this tree. In particular the **new** Tier 1 steps have not been observed running inside a real Tier 1 pass | Run `./scripts/check.sh`, `./scripts/check.sh extended` and `./scripts/check.sh package` on the frozen SHA, plus `scripts/check_rust_public_api.py`, before release |
| Medium | With the artifact manifest absent, `./scripts/check.sh extended` cannot complete on a clean checkout, and no in-repo producer exists for `target/downstream-qualification/artifact-manifest.json` | The release-blocking downstream portfolio cannot be qualified from a clean checkout without operator action. This is the chosen fail-closed behavior, not a defect, but it is a standing operator obligation | The operator must build the eggfetch wheel and the controlled httpx replacement wheel, write their paths and SHA-256 hashes into the manifest, and re-run Tier 2. The procedure is documented in `compat/downstream/README.md` ("Update Process") |
| Medium | The MSRV leg of `tier2_msrv` was not exercised — it requires the exact Rust 1.89.0 toolchain, which is a Tier 2 requirement | The three MSRV profile-isolated commands carry the same bare `RUSTFLAGS=` prefix as the four that were exercised, but their behavior under an inherited `-D warnings` is unproven | Install `rustup toolchain install 1.89.0 --profile minimal` and run `./scripts/check.sh extended` on the frozen SHA |
| Medium | The "the all-features workspace check still fails on a warning" criterion is demonstrated by mechanism in a scratch crate, not by injecting a warning into the repository tree | The repository's own all-features build is warning-free today, so the negative case cannot be observed without editing the tree, which this closure pass does not do | Confirmed implicitly by the frozen-SHA Tier 2 run, which keeps `-D warnings` on the all-features and workspace rows; no further action beyond that run |
| Low | The working tree is shared with an agent editing `crates/eggfetch-core/src/**` during this closure pass, so no measurement here isolates this milestone from other in-flight work | The lint-guard, script and Python measurements are unaffected; the cargo measurements are pinned to the tree fingerprint | Re-run the §4 command set against the frozen commit after the concurrent work lands |
| Low | The plan's §12 documentation list is satisfied (`docs/verification-policy.md`, `docs/architecture/build-ci.md`, `docs/architecture/overview.md` and `.skills/verification-qualification.md` all updated), but §12 also names `plans/registry.md`, the subsystem roadmap and `plans/README.md`, which are outside this record's scope | The registry and roadmap updates listed in §12 are not made by this record | The main agent must update `plans/registry.md`, `plans/subsystems/release-verification-roadmap.md` and `plans/README.md` |

## 11. Roadmap disposition

`plans/subsystems/release-verification-roadmap.md` milestone 6 moves from
`ready` to `conditionally closed`, with §11 (completion definition) and §12
(milestone status) updated in the same pass.

The milestone's own scope — the corrected lint guard and its self-test, the
fail-closed downstream gate, the scoped Tier 2 `-D warnings` treatment and the
Tier 1 H3 runner self-test — is complete and measured. What remains is the
cross-cutting release evidence: Tier 1 / Tier 2 / Tier 3 on the frozen SHA and
the Stage C rebinding, which belong to the release-verification subsystem's
release gates rather than to this milestone, plus the standing operator
obligation for the downstream manifest.

The successor milestone (M007, planning and documentation truthfulness
hygiene) is not blocked by this one.

## 12. Registry updates

`plans/registry.md`:

- The release-verification row records milestone 6 as conditionally closed,
  with the Stage C rebinding and the frozen-SHA tier gate run named as the
  outstanding condition.
- The implementation-plan table for M006 points at this closure record.
- The blocked / operational work section records the Stage C rebinding as
  pending for every milestone closed in this pass, not for M006 alone, and
  records the operator-supplied downstream artifact manifest as a standing
  operator obligation.
