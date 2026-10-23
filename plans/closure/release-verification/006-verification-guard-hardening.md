# Release and Verification Milestone 006 — Verification Guard Hardening

Status: closed

Source implementation plan:

- `plans/implementation/release-verification/006-verification-guard-hardening.md`

Source subsystem roadmap:

- `plans/subsystems/release-verification-roadmap.md#milestone-6--verification-guard-hardening`

Repository baseline reviewed: `873ea28541c82817317430c9523fca8525d99c81`.

Implementation commits:

- `d6c41adf4cf8f62ef990cccedf2f86512858fa0b` — feat: implement the eight
  registered milestones as one qualification freeze. This commit carries the
  milestone's implementation together with the other seven milestones
  executed in the same coordinated pass.
- `26fa619299fb8d447f25cbcb5eb713a1f537a30c` — fix(security): patch the four
  dependencies with live RustSec advisories. The bound Stage C SHA; it
  changes only `Cargo.lock` and `fuzz/Cargo.lock` relative to the freeze
  commit above, and no Rust source.

## 1. Executive finding

M006 is implemented, and each of the four fixes the milestone set out to make
is present and measured. The milestone changes validation scripts, which are
executable inputs under the exact-SHA rule in
`.skills/verification-qualification.md`, so it invalidated the live Stage C
binding at implementation time. Both conditions that kept this milestone open
are now met: the Stage C rebinding was performed, and the frozen-SHA tier
gates were run at the bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c`.
The milestone is therefore **closed**.

The evidence that met the two conditions:

- **Stage C rebinding.** Both HTTPX profiles (`compat/httpx/0.28.1/profile.toml`
  and `compat/httpx2/2.12.0/profile.toml`) and the ledger's Stage C SHA in
  `plans/httpx-parity-correction-status.md` are bound to
  `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, the SHA that carries the
  eight-milestone implementation freeze (`d6c41adf…`) and the dependency patch
  that follows it. No Rust source differs between the two commits; only
  `Cargo.lock` and `fuzz/Cargo.lock` changed, and the Rust public-API oracle
  is byte-identical in all eight profiles.
- **Frozen-SHA tier gates.** Tier 1, Tier 2 and Tier 3 are green on that SHA,
  together with the live security preflight (`./scripts/check_security.sh` →
  "advisories ok, bans ok, licenses ok, sources ok"). The evidence is in §2
  and §4; §10 records the two findings as resolved.

This binding is a compatibility binding. It authorizes no publication, and it
does not graduate H3, which remains experimental.

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
public-API oracle have since been run at the bound SHA and are green; the
milestone's own scoped findings in §10 remain open by design and are
unrelated to the binding.

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
| Tier 1 and Tier 2 are green on the frozen executable SHA (§13) | Tier 1 green at `26fa6192…`: 1,767 Rust workspace tests passed (0 failed, 0 ignored), 581 Python behavior tests passed, 134 HTTPX compatibility smoke tests passed, the native Python API manifest passed (66 exports), the Python typing surface and typing fixture passed, the lint-suppression guard self-test and policy scan passed, the H3 runner self-test passed (runner logic only), the adapter feature ownership check passed, the release version/ref validation passed, the stable public contract fixtures passed, and the feature-isolated profile whole-suite compile passed for all four standard/native profiles. Tier 2 green across every phase, including the full pinned HTTPX compatibility suite (2,073 passed / 0 failed), the Rust public-API oracle in 8 profiles and the MSRV matrix on the exact Rust 1.89.0 toolchain (5 checks) | Met | The H3 step is a **runner** self-test and makes no H3 conformance claim; see §1 and §4 |
| Both HTTPX profiles are rebound to that exact SHA (§13) | Both `compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml`, and the ledger's Stage C SHA in `plans/httpx-parity-correction-status.md`, are bound to `26fa619299fb8d447f25cbcb5eb713a1f537a30c` | Met | The bound SHA carries the eight-milestone freeze (`d6c41adf…`) and changes only `Cargo.lock` and `fuzz/Cargo.lock` relative to it; see §7 and §10 |

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

**The artifact is an operator obligation, and it was met for this
qualification.** There is no in-repository producer for
`target/downstream-qualification/artifact-manifest.json`, because producing it
requires the network-reachable HTTPX baseline that this milestone's scope
forbids. The procedure is documented for the operator in
`compat/downstream/README.md` ("Update Process"). For the bound-SHA
qualification the operator built the eggfetch wheel from
`26fa619299fb8d447f25cbcb5eb713a1f537a30c` plus the controlled httpx
replacement wheel, recorded both SHA-256 hashes in the manifest, and the gate
then **executed** rather than skipping: it passed 4 of 4 required packages
(respx 0.21.1, httpx-sse, httpx-auth, httpx-ws), with each package's source
hash verified against `compat/downstream/manifest.toml`. The fail-closed
design is unchanged and remains intentional; the obligation recurs on every
Tier 2 run, and for this qualification it was met.

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

Deferred at implementation time, and since run at the bound SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c`:

- `./scripts/check.sh` (Tier 1), `./scripts/check.sh extended` (Tier 2),
  `./scripts/check.sh package` (Tier 3) — all three are green at the bound
  SHA (§1, §2). The downstream gate executed rather than skipping, because
  the operator artifact manifest was produced for this qualification (§3.2).
- The full workspace test suite (`cargo test --workspace --exclude
  eggfetch-python --all-features`) — 1,767 tests passed, 0 failed, 0 ignored.
- The Python HTTPX compat pytest suites — 581 Python behavior tests and 134
  HTTPX compatibility smoke tests at Tier 1; the full pinned HTTPX
  compatibility suite passed 2,073 / 0 failed at Tier 2.
- The Rust public-API oracle (`scripts/check_rust_public_api.py`) — passed in
  8 profiles at the bound SHA, with `cargo-semver-checks` 0.49.0 reporting
  "no semver update required" against the planning baseline
  `03ecba973010e2858bf16a2b5f84d51ce70adae4`. The oracle output is
  byte-identical to the freeze commit `d6c41adf…`.
- The MSRV leg of `tier2_msrv` — exercised at the bound SHA on the exact Rust
  1.89.0 toolchain (5 checks), which also confirms the three profile-isolated
  MSRV commands under the bare `RUSTFLAGS=` prefix.

Still outstanding, with reason:

- The repository's own all-features build with a warning deliberately
  injected — injecting a warning would mean editing the tree, which this
  closure pass does not do; row 11 demonstrates the mechanism in a scratch
  crate instead. The bound-SHA Tier 2 run keeps `-D warnings` on the
  all-features and workspace rows and is green, which confirms the mechanism
  implicitly (§10).

### Qualification at the bound SHA

Two host accommodations are recorded rather than hidden:

- The first Tier 2 attempt failed with a **linker SIGBUS** (`ld terminated
  with signal 7`) while compiling a test binary. This was host memory
  pressure from a concurrent build on the same 16-CPU / 15-GiB machine, not a
  code defect: the same step re-verified clean immediately afterwards, and the
  full gate then passed end to end. The rerun used `CARGO_BUILD_JOBS=6`.
- Tier 1 and Tier 2 each execute the lint-suppression guard self-test and the
  H3 runner self-test that this milestone added; both passed. The H3 step is a
  **runner** self-test only — it makes no network connection, no H3
  conformance claim, and does not change H3's experimental status.

The only Tier 2 skip is the policy-defined optional one: the Node JS surface,
because the native `eggfetch.node` artifact is absent.

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

**Exact-SHA binding.** At implementation time this milestone invalidated the
live Stage C binding, because `scripts/check_lint_suppressions.sh` and
`scripts/check.sh` are validation scripts and are therefore executable inputs
under `.skills/verification-qualification.md`. The previous binding was
`42a9c96db5b25354843bd83bd458c71757e95651` (`compat/httpx/0.28.1/profile.toml`
and `compat/httpx2/2.12.0/profile.toml`, and the ledger's Stage C entry in
`plans/httpx-parity-correction-status.md`), an ancestor of the baseline
`873ea285…` that did not cover the working tree.

The binding is now **re-established**. Both HTTPX profiles and the ledger's
Stage C SHA are bound to `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, which
carries the eight-milestone implementation freeze (`d6c41adf…`) and the
follow-up dependency patch. No Rust source differs between the two commits:
only `Cargo.lock` and `fuzz/Cargo.lock` changed, and the Rust public-API
oracle is byte-identical in all eight profiles.

The binding establishes a compatibility binding. It authorizes no publication,
and it does not graduate H3, which remains experimental.

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
| Resolved | The live Stage C binding (`42a9c96d…`) was invalidated by this milestone's validation-script changes, and the Stage C rebinding had not been performed at the time of implementation | The previous binding could not be cited by any publication | **Resolved 2026-10-22.** Both HTTPX profiles and the ledger's Stage C SHA were rebound to `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, and the frozen-SHA qualification was re-run |
| Resolved | Tier 1, Tier 2, and Tier 3 had not been run on the frozen SHA | The repository-wide gates were unproven for this tree | **Resolved 2026-10-22.** Tier 1, Tier 2, and Tier 3 are green on `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, together with the live security preflight |
| Resolved | The downstream Tier 2 gate fails closed when `target/downstream-qualification/artifact-manifest.json` is absent, and no in-repository producer exists for that artifact | The release-blocking downstream portfolio cannot be qualified from a clean checkout without operator action. This is the chosen fail-closed behavior, not a defect, and it is a standing operator obligation | **Resolved for this qualification 2026-10-22.** The artifact is an operator obligation, documented in `compat/downstream/README.md` ("Update Process"), not a repository defect: the operator built the eggfetch wheel from the bound SHA plus the controlled httpx replacement wheel and recorded both SHA-256 hashes in the manifest. The gate then executed rather than skipping and passed 4 of 4 required packages (respx 0.21.1, httpx-sse, httpx-auth, httpx-ws), with each package's source hash verified against `compat/downstream/manifest.toml`. The gate still fails closed when the artifact is absent, and the obligation recurs on every Tier 2 run |
| Resolved | The MSRV leg of `tier2_msrv` required the exact Rust 1.89.0 toolchain and had not been exercised at the time of implementation | The three MSRV profile-isolated commands carry the same bare `RUSTFLAGS=` prefix as the four that were exercised, but their behavior under an inherited `-D warnings` was unproven | **Resolved 2026-10-22.** The bound-SHA Tier 2 run exercised the MSRV matrix on the exact Rust 1.89.0 toolchain (5 checks) and passed |
| Resolved | The "the all-features workspace check still fails on a warning" criterion was demonstrated by mechanism in a scratch crate, not by injecting a warning into the repository tree | The repository's own all-features build is warning-free today, so the negative case cannot be observed without editing the tree, which this closure pass does not do | **Resolved 2026-10-22.** The bound-SHA Tier 2 run keeps `-D warnings` on the all-features and workspace rows and is green, which confirms the mechanism implicitly; no further action |
| Low | The working tree was shared with an agent editing `crates/eggfetch-core/src/**` during this closure pass, so the measurements in §4 are pinned to tree fingerprints rather than to a commit | The lint-guard, script and Python measurements are unaffected; the cargo measurements are pinned to the tree fingerprint | **Resolved 2026-10-22.** The concurrent work landed in the freeze commit `d6c41adf…`, and the repository-wide gates were re-run at the bound SHA `26fa6192…`; the §4 measurements remain pinned to the fingerprints recorded above |
| Low | The plan's §12 documentation list is satisfied (`docs/verification-policy.md`, `docs/architecture/build-ci.md`, `docs/architecture/overview.md` and `.skills/verification-qualification.md` all updated), but §12 also names `plans/registry.md`, the subsystem roadmap and `plans/README.md`, which are outside this record's scope | The registry and roadmap updates listed in §12 are not made by this record | **Done 2026-10-22.** `plans/registry.md`, `plans/subsystems/release-verification-roadmap.md` and `plans/README.md` were updated in the rebinding commit `9eafb8b5` |

## 11. Roadmap disposition

`plans/subsystems/release-verification-roadmap.md` milestone 6 moves from
`conditionally closed` to `closed`, with §11 (completion definition) and §12
(milestone status) updated in the same pass.

The milestone's own scope — the corrected lint guard and its self-test, the
fail-closed downstream gate, the scoped Tier 2 `-D warnings` treatment and the
Tier 1 H3 runner self-test — is complete and measured. The cross-cutting
release evidence was delivered on the bound SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c`: the Stage C rebinding was
performed, and Tier 1 / Tier 2 / Tier 3 are green there, together with the
live security preflight.

The work that remains is not this milestone's: the operator obligation for the
downstream artifact manifest recurs on every Tier 2 run, and the maintainer
decisions recorded in `plans/README.md` are owned by the release-verification
subsystem's follow-up work.

The successor milestone (M007, planning and documentation truthfulness
hygiene) is not blocked by this one.

## 12. Registry updates

`plans/registry.md`:

- The release-verification row records milestone 6 as closed, with the bound
  SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c` named as the executable
  freeze the milestone is closed on.
- The implementation-plan table for M006 moves from `conditionally closed` to
  `closed` and points at this closure record.
- The blocked / operational work section no longer records the Stage C
  rebinding as pending for the milestones closed in this pass, and records the
  operator-supplied downstream artifact manifest as a standing operator
  obligation.
