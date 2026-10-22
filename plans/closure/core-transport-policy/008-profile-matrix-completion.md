# Core Transport Milestone 008 — Profile Matrix Completion

Status: conditionally closed

Source implementation plan:

- `plans/implementation/core-transport-policy/008-profile-matrix-completion.md`

Source subsystem roadmap:

- `plans/subsystems/core-transport-policy-roadmap.md#milestone-8--profile-matrix-completion`

Repository baseline reviewed: `873ea28541c82817317430c9523fca8525d99c81`.

Implementation commits:

- None. The milestone is implemented in the working tree and is **not
  committed**; the maintainer reviews the working tree before any commit is
  made. No SHA is claimed for this milestone.

## 1. Executive finding

M008 is implemented and its own targeted gates are green, but it cannot be
marked `closed` under `plans/closure/README.md`: the milestone changes
validation scripts and adds two committed API snapshots, which are executable
inputs under the exact-SHA rule in `.skills/verification-qualification.md`.
That invalidates the live Stage C binding, and the Stage C rebinding plus a
full Tier 1 / Tier 2 / Tier 3 run on the frozen SHA were deliberately not
performed at this stage. The milestone is therefore **conditionally closed**,
with the condition, risk, and the exact future evidence stated in §10 and
§11.

The implementation itself is complete and is what the plan asked for:

- The contract matrix in `scripts/check.sh` now covers eight profiles instead
  of six. `standard-http2,tls-rustls` and `native-http2,tls-rustls` are
  compiled in isolation and their contract fixtures are executed.
- The lean profile's **behavioral** suites are now run in Tier 1, not only
  its compile-shape contracts. `lean_policy_tests` and `lean_route_tests`
  execute under `--no-default-features --features standard-http1,tls-rustls`.
- The Rust public-surface oracle now freezes eight profiles. Two new
  tool-generated snapshots were added; both were measured to be byte-identical
  to their H1 counterparts, which is the expected outcome now that the `http2`
  alias adds `high-level-url` / `logical-retry` / `redirects` / `basic-auth`
  rather than a distinct transport.

No Rust source, no cargo feature, no dependency, and no public engine surface
changed. The only executable changes are the two validation scripts and the
two new snapshot artifacts.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Contract matrix compiles `standard-http2` in isolation (§6.1) | `scripts/check.sh` invocation 5; 4 passed, 0 failed | Met | Same shape and bare `RUSTFLAGS=` prefix as the existing six |
| Contract matrix compiles `native-http2` in isolation (§6.1) | `scripts/check.sh` invocation 7; 4 passed, 0 failed | Met | `native-http2` is the profile `--all-features` masks |
| Lean profile's behavioral suites run in Tier 1 (§6.2) | `scripts/check.sh` invocations 9 and 10; 6 and 10 passed | Met | Not widened to `--all-targets`: 13 test targets do not compile under the lean profile (tracked by core-transport-policy M009) |
| Oracle freezes `standard-http2` (§6.3) | `compat/rust-public-api/standard-http2-tls-rustls.txt`, 135 926 bytes, tool-generated | Met | Byte-identical to `standard-http1-tls-rustls.txt` |
| Oracle freezes `native-http2` (§6.3) | `compat/rust-public-api/native-http2-tls-rustls.txt`, 119 215 bytes, tool-generated | Met | Byte-identical to `native-http1-tls-rustls.txt` |
| Snapshots are generated, not hand-written (§6.3) | Regenerated the two *existing* H1 snapshots with the identical invocation; both came back byte-identical to the committed artifacts, proving generator fidelity before the new files were produced | Met | Invocation: `cargo-public-api -p eggfetch-core -sss --color never <features>` under `RUSTUP_TOOLCHAIN=nightly-2026-05-07` |
| The H2-vs-H1 public-surface difference is measured, not assumed (§6.3) | `diff` of each new snapshot against its H1 counterpart is empty (exit 0) | Met | sha256 `standard-*` = `0ca3a536…`, `native-*` = `9a43ba5f…` |
| No existing snapshot changed (§6.3) | `git diff --stat -- compat/rust-public-api/` shows only `README.md` modified; both `.txt` files are new | Met | No snapshot regenerated |
| Eight-profile oracle is green (§10) | `python scripts/check_rust_public_api.py --skip-semver` → "Rust public API oracle passed (8 profiles)", exit 0 | Met | Run against the baseline tree with the deliverable scripts copied in |
| Semver half is green (§10) | Full oracle → 223 checks: 223 pass, 30 skip; "Summary no semver update required" | Met | Default profile only, as the script already did |
| Every profile's gate is documented (§6.4, §12) | `docs/architecture/feature-flags.md` "Gate coverage"; `docs/reference/feature-matrix.md` "Supported core profiles and gate coverage"; `docs/architecture/build-ci.md` Tier 1 rows; `compat/rust-public-api/README.md` "Profiles" | Met | No gate is claimed that does not exist |
| Tier 1 and Tier 2 green on the frozen executable SHA (§13) | Not run | **Not met** | Deliberately deferred at this stage; see §10 |
| Stage C rebinding to the frozen SHA (§13) | Not performed | **Not met** | The milestone changes validation scripts and adds API snapshots, which invalidates the live Stage C binding; see §10 |

## 3. Production implementation evidence

`scripts/check.sh`, function `tier1_rust_api_contracts` — the matrix now holds
eight contract invocations and two lean behavioral invocations:

- `RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features standard-http2,tls-rustls -- --test-threads=1`
- `RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features native-http2,tls-rustls -- --test-threads=1`
- `RUSTFLAGS= cargo test -p eggfetch-core --test lean_policy_tests --no-default-features --features standard-http1,tls-rustls -- --test-threads=1`
- `RUSTFLAGS= cargo test -p eggfetch-core --test lean_route_tests --no-default-features --features standard-http1,tls-rustls -- --test-threads=1`

The existing comment above the matrix, which explains that the bare
`RUSTFLAGS=` prefix overrides CI's `-D warnings` (profile-specific dead code),
was extended to cover all eight supported profiles.

`scripts/check_rust_public_api.py` — `PROFILES` gained
`("standard-http2-tls-rustls", ("--no-default-features", "--features", "standard-http2,tls-rustls"))`
and `("native-http2-tls-rustls", ("--no-default-features", "--features", "native-http2,tls-rustls"))`,
in the style of the existing entries. The in-script comment that described the
loop as "the stronger six-profile oracle" now reads "eight-profile oracle".

`compat/rust-public-api/` — two new files, both produced by `cargo-public-api`
under the pinned `nightly-2026-05-07`:

| Snapshot | Bytes |
|---|---|
| `default.txt` | 160 902 |
| `http1-tls-rustls.txt` | 160 902 |
| `http2-tls-rustls.txt` | 160 902 |
| `standard-http1-tls-rustls.txt` | 135 926 |
| `standard-http2-tls-rustls.txt` (new) | 135 926 |
| `native-http1-tls-rustls.txt` | 119 215 |
| `native-http2-tls-rustls.txt` (new) | 119 215 |
| `all-features.txt` | 199 384 |

The H1/H2 byte-identity is documented as a *measured* property of the current
source, not as a permanent invariant.

## 4. Verification executed

### Commands run

All cargo work was performed in a detached git worktree at the baseline
revision (`git worktree add --detach /tmp/eggfetch-oracle-wt 873ea285…`) with
the deliverable scripts copied in, so that no concurrently edited source could
contaminate the measurement. The worktree was removed afterwards;
`git worktree list` shows only the main tree. The contract matrix was executed
by extracting and running the **actual modified `tier1_rust_api_contracts`
function** from the worktree copy of `scripts/check.sh`, not by re-typing
commands.

```sh
# contract matrix + lean behavioural suites (10 invocations, all exit 0)
RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features http1,tls-rustls -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features http2,tls-rustls -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features standard-http1,tls-rustls -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features standard-http2,tls-rustls -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features native-http1,tls-rustls -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --no-default-features --features native-http2,tls-rustls -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts --all-features -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test lean_policy_tests --no-default-features --features standard-http1,tls-rustls -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test lean_route_tests --no-default-features --features standard-http1,tls-rustls -- --test-threads=1

# oracle
python scripts/check_rust_public_api.py --skip-semver
python scripts/check_rust_public_api.py

# snapshot fidelity and comparison
cmp compat/rust-public-api/standard-http2-tls-rustls.txt compat/rust-public-api/standard-http1-tls-rustls.txt
cmp compat/rust-public-api/native-http2-tls-rustls.txt  compat/rust-public-api/native-http1-tls-rustls.txt
git diff --stat -- compat/rust-public-api/
```

### Results (pass/fail/skip with reason; counts without concealment)

| # | Invocation | Result |
|---|---|---|
| 1 | `public_api_contracts` default | 7 passed, 0 failed |
| 2 | `public_api_contracts` http1,tls-rustls | 7 passed, 0 failed |
| 3 | `public_api_contracts` http2,tls-rustls | 7 passed, 0 failed |
| 4 | `public_api_contracts` standard-http1,tls-rustls | 4 passed, 0 failed |
| 5 | `public_api_contracts` standard-http2,tls-rustls | 4 passed, 0 failed |
| 6 | `public_api_contracts` native-http1,tls-rustls | 4 passed, 0 failed |
| 7 | `public_api_contracts` native-http2,tls-rustls | 4 passed, 0 failed |
| 8 | `public_api_contracts` --all-features | 9 passed, 0 failed |
| 9 | `lean_policy_tests` standard-http1,tls-rustls | 6 passed, 0 failed |
| 10 | `lean_route_tests` standard-http1,tls-rustls | 10 passed, 0 failed |

`unused import` warnings in the lean and native profiles are the reason the bare
`RUSTFLAGS=` prefix is required; CI's `-D warnings` would fail those profiles.

| Check | Result |
|---|---|
| Oracle, 8 profiles, `--skip-semver` | passed, exit 0 — "Rust public API oracle passed (8 profiles)" |
| Oracle, 8 profiles + semver | passed, exit 0 — "Checked 223 checks: 223 pass, 30 skip"; "Summary no semver update required" |
| `diff standard-http2` vs `standard-http1` | empty, exit 0 |
| `diff native-http2` vs `native-http1` | empty, exit 0 |
| Existing snapshots changed | none — only `README.md` modified; both `.txt` files are new |
| Generator fidelity (regenerated H1 snapshots vs committed) | byte-identical, 135 926 / 119 215 bytes |

Not run, with reason: `./scripts/check.sh` (Tier 1), `./scripts/check.sh
extended` (Tier 2), and `./scripts/check.sh package` (Tier 3) — deliberately
deferred at this stage per the maintainer's instruction to keep verification
scoped. The targeted equivalents listed above are the substitute evidence.

## 5. Invariant review

- **No Rust source changed.** `git diff --stat` over `crates/` is empty for
  this milestone's scope.
- **No cargo feature added, removed, or renamed.** The two new profiles use
  the existing `standard-http2` and `native-http2` aliases.
- **No public engine surface changed.** The oracle is the authority and it
  reports no difference against the baseline `03ecba97…`.
- **No dependency added.** `Cargo.lock` unchanged.
- **The `RUSTFLAGS=` rationale is preserved.** The bare prefix is retained on
  every invocation, including the two new ones, so the lean and native
  profiles are not failed by CI's `-D warnings`.
- **`--test-threads=1` is preserved** on every invocation, per the repository
  rule that workspace Rust tests are never parallelized.
- **The lean gate is not widened.** Only the two named behavioral targets are
  executed under the lean profile; `--all-targets` is deliberately not used,
  because 13 test targets do not compile there and widening the gate would
  make it unrunnable. That gap is tracked by core-transport-policy M009, not
  here.

## 6. Failure, timeout, pool, and recovery review

Not applicable. This milestone adds no request path, no transport, no timeout,
no pool behaviour, and no recovery logic. It adds gates and frozen snapshots.

One operational hazard is recorded because it can silently invalidate future
verification in this environment: `env` on `PATH` resolves to
`/home/sugarwookie/.local/bin/env`, a shim that ignores its arguments and only
prepends `PATH`. An invocation of the form `env RUSTFLAGS= cargo …` therefore
succeeds without running `cargo` at all. The first matrix run in this
milestone reported ten false passes for exactly this reason; every result in
§4 comes from re-runs using the shell assignment prefix (`RUSTFLAGS= cargo …`),
which is also how `scripts/check.sh` is written. Any future verification must
use the shell prefix form. This is an environment property, not a repository
defect, and it is recorded so that a future reader does not repeat the false
pass.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

Supported core profiles and their gates after this milestone:

| Profile | Contract fixtures | Behavioral suite | Public-surface snapshot |
|---|---|---|---|
| default | Tier 1 | yes | `default.txt` |
| http1 + tls-rustls | Tier 1 | yes | `http1-tls-rustls.txt` |
| http2 + tls-rustls | Tier 1 | yes | `http2-tls-rustls.txt` |
| standard-http1 + tls-rustls | Tier 1 | yes (`lean_policy_tests`, `lean_route_tests`) | `standard-http1-tls-rustls.txt` |
| standard-http2 + tls-rustls | Tier 1 (new) | yes | `standard-http2-tls-rustls.txt` (new) |
| native-http1 + tls-rustls | Tier 1 | yes | `native-http1-tls-rustls.txt` |
| native-http2 + tls-rustls | Tier 1 (new) | yes | `native-http2-tls-rustls.txt` (new) |
| all features | Tier 1 | yes | `all-features.txt` |

**Exact-SHA binding.** The live Stage C binding is **invalidated** by this
milestone. `scripts/check.sh` and `scripts/check_rust_public_api.py` are
validation scripts and `compat/rust-public-api/*.txt` are API snapshots; all
three are executable inputs under `.skills/verification-qualification.md`.
Until the Stage C binding is re-established at a frozen SHA that includes
these changes, no publication may cite the previous binding. No SHA is
claimed here because the milestone is not committed.

**Documentation consistency.** Three stale "six profiles" statements outside
this milestone's original scope were corrected in the same pass, because the
milestone's own documentation would otherwise contradict them:
`AGENTS.md` (contract-fixture profile count and oracle profile count) and
`docs/architecture/overview.md` (the Tier 1 contract description, now also
naming the two lean behavioral gates).

## 8. Security review

No security-relevant behaviour changed. The added gates increase coverage of
the feature matrix that the `native-http2` isolation gap had left unchecked.
No trust store, certificate handling, credential, redaction, proxy, cookie, or
TLS configuration was touched.

One security-adjacent observation is recorded as a finding rather than a
blocker: the pre-existing gap this milestone closes meant that a change
breaking the `transport-http2` + `advanced-routing` profile *without*
`high-level-url` would have passed every gate, including `--all-features`.
That is now closed for `native-http2`; the same class of gap for other
feature combinations is bounded by the profile set documented in §7.

## 9. Documentation and operations

| File | Change |
|---|---|
| `docs/architecture/feature-flags.md` | New "Gate coverage" section: the eight supported profiles with the gate that covers each; the manual-check list no longer lists the now-gated lean commands |
| `docs/reference/feature-matrix.md` | New "Supported core profiles and gate coverage" section with the same table |
| `docs/architecture/build-ci.md` | Tier 1 "Rust API contracts" row updated from six profiles to eight with the full list; new "Rust lean behavioral contracts" row |
| `compat/rust-public-api/README.md` | New "Profiles" section: the eight-profile table, the two new snapshots, and the measured H1/H2 byte-identity |
| `AGENTS.md` | Contract-fixture profile count and oracle profile count updated to eight; the lean behavioral commands added to the focused-command list |
| `docs/architecture/overview.md` | Tier 1 contract description updated to the eight-profile matrix and the two lean behavioral gates |

Operations: `scripts/check.sh` Tier 1 runtime grows by two contract
invocations and two lean behavioral invocations, all of which compile
`eggfetch-core` under a non-default feature set. No new external dependency,
no network access, and no credential is required.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| High | The live Stage C binding is invalidated by this milestone's validation-script and API-snapshot changes, and the Stage C rebinding was deliberately not performed at this stage | No publication may cite the previous Stage C binding; the compat profiles still point at the superseded SHA | Rebind both HTTPX profiles and the ledger's Stage C SHA to the frozen commit that includes this milestone, then re-run the compat qualification |
| High | Tier 1, Tier 2, and Tier 3 were not run on the frozen SHA (deliberately deferred) | The milestone's own gates are green, but the repository-wide gates are unproven for this tree | Run `./scripts/check.sh`, `./scripts/check.sh extended`, and `./scripts/check.sh package` on the frozen SHA before release |
| Low | 13 core test targets still fail to compile under the lean profile | The lean gate covers only the two named behavioral targets; a lean-profile regression in an uncovered target stays invisible | Track and fix in core-transport-policy M009 (lean path single-sourcing); do not widen the gate until that milestone lands |
| Low | `env` on `PATH` in this environment ignores its arguments | Any `env VAR=… cmd` verification invocation silently no-ops and can produce false passes | Use the shell assignment prefix (`VAR=… cmd`); recorded here so the false-pass mode is not repeated |
| Low | Filesystem at 98% with ~22 GB free | Future full-workspace builds may run out of space | Reclaim space before Tier 2 / Tier 3 |

## 11. Roadmap disposition

`plans/subsystems/core-transport-policy-roadmap.md` milestone 8 moves from
`ready` to `conditionally closed`, with §11 (completion definition) and §12
(milestone status) updated in the same pass.

The milestone's own scope is complete. What remains is the cross-cutting
release evidence — Tier 1 / Tier 2 / Tier 3 on the frozen SHA and the Stage C
rebinding — which belongs to the release-verification subsystem, not to this
milestone. The successor milestone (M009, lean path single-sourcing) is not
blocked by this one.

## 12. Registry updates

`plans/registry.md`:

- The core-transport-policy row records milestone 8 as conditionally closed,
  with the Stage C rebinding named as the outstanding condition.
- The implementation-plan table for M008 points at this closure record.
- The blocked / operational work section records the Stage C rebinding as
  pending for all four milestones closed in this pass, not for M008 alone.
