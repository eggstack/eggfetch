# Core Transport Milestone 008 — Profile Matrix Completion

Status: closed

Source implementation plan:

- `plans/implementation/core-transport-policy/008-profile-matrix-completion.md`

Source subsystem roadmap:

- `plans/subsystems/core-transport-policy-roadmap.md#milestone-8--profile-matrix-completion`

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

M008 is implemented, its own targeted gates are green, and the milestone **is
closed**. The milestone changes validation scripts and adds two committed API
snapshots, which are executable inputs under the exact-SHA rule in
`.skills/verification-qualification.md`; that invalidated the live Stage C
binding at the time of implementation. The Stage C binding has since been
re-established and the frozen-SHA tier gates were run at the bound SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c`:

- **Tier 1** (`./scripts/check.sh`) is green — 1767 Rust workspace tests
  passed (0 failed, 0 ignored), 581 Python behaviour tests passed, 134 HTTPX
  compatibility smoke tests passed, the native Python API manifest (66
  exports), the Python typing surface and typing fixture, the
  lint-suppression guard self-test and policy scan, the H3 qualification-runner
  self-test (runner logic only), the adapter feature ownership check and the
  release version/ref validation all pass, the stable public contract fixtures
  pass, and the feature-isolated profile whole-suite compile passes for all
  four standard/native profiles.
- **Tier 2** (`./scripts/check.sh extended`) is green across every phase —
  the pinned HTTPX compatibility suite reports 2073 passed / 0 failed, the API
  manifest comparison is clean for both facades (74 symbols for httpx 0.28.1,
  76 symbols for httpx2 2.12.0; 0 unexplained, 0 stale), the Rust public-API
  oracle passes in eight profiles and `cargo-semver-checks` 0.49.0 reports
  "no semver update required" against the planning baseline
  `03ecba973010e2858bf16a2b5f84d51ce70adae4`, and the feature matrix, MSRV
  matrix on the exact Rust 1.89.0 toolchain, documentation, FFI, resource,
  lifecycle, soak, downstream behavioral-fixture, lossless-merge and Criterion
  phases all pass. The only skip is the policy-defined optional one: the Node
  JS surface, whose native `eggfetch.node` artifact is absent.
- **Tier 3** (`./scripts/check.sh package`) is green, and the live security
  preflight (`./scripts/check_security.sh`) reports
  `advisories ok, bans ok, licenses ok, sources ok`.

The two items that the previous closure pass left outstanding are therefore
recorded as resolved in §10. The rebinding establishes a compatibility
binding only: it authorizes no publication, and HTTP/3 remains experimental.

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
changed by this milestone. The only executable changes are the two validation
scripts and the two new snapshot artifacts.

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
| Tier 1 and Tier 2 green on the frozen executable SHA (§13) | `./scripts/check.sh`, `./scripts/check.sh extended` and `./scripts/check.sh package` on `26fa6192…` — green | Met | Run on the bound Stage C SHA; the Rust sources are byte-identical to the freeze commit `d6c41adf…`. See §10 |
| Stage C rebinding to the frozen SHA (§13) | Both HTTPX profiles and the ledger's Stage C SHA rebound to `26fa6192…` | Met | The binding that this milestone invalidated was re-established at the bound SHA, and the compat qualification was re-run against it; see §10 |

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

At the time of the milestone's own verification the repository-wide gates
were out of scope — `./scripts/check.sh` (Tier 1),
`./scripts/check.sh extended` (Tier 2) and `./scripts/check.sh package`
(Tier 3) — and the targeted equivalents listed above were the substitute
evidence for the milestone's own changes. The full gates were subsequently
run on the bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c` and are
green; see §10.

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

**Exact-SHA binding.** This milestone changed validation scripts
(`scripts/check.sh` and `scripts/check_rust_public_api.py`) and added API
snapshots (`compat/rust-public-api/*.txt`); all three are executable inputs
under `.skills/verification-qualification.md`, so the previous live Stage C
binding was invalidated by it. The binding has since been re-established at
the frozen SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, which carries
this milestone's changes in the freeze commit
`d6c41adf4cf8f62ef990cccedf2f86512858fa0b`. Both HTTPX profiles and the
ledger's Stage C SHA point at that SHA, and the frozen-SHA qualification is
green (§10). The rebinding establishes a compatibility binding only: it
authorizes no publication, and HTTP/3 remains experimental.

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
| Resolved | The live Stage C binding was invalidated by this milestone's changes, and the Stage C rebinding had not been performed at the time of implementation | The previous binding could not be cited by any publication | **Resolved 2026-10-22.** Both HTTPX profiles and the ledger's Stage C SHA were rebound to `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, and the frozen-SHA qualification was re-run |
| Resolved | Tier 1, Tier 2, and Tier 3 had not been run on the frozen SHA | The repository-wide gates were unproven for this tree | **Resolved 2026-10-22.** Tier 1, Tier 2, and Tier 3 are green on `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, together with the live security preflight |
| Resolved | 13 core test targets failed to compile under the lean profile | The lean gate covered only the two named behavioral targets; a lean-profile regression in an uncovered target stayed invisible | **Resolved by core-transport-policy M009** (lean path single-sourcing), which gated the thirteen targets on the feature each file's own content requires and added the Tier 1 whole-suite compile gate. The gate is not widened beyond M009's four feature-isolated profiles |
| Low | `env` on `PATH` in this environment ignores its arguments | Any `env VAR=… cmd` verification invocation silently no-ops and can produce false passes | Use the shell assignment prefix (`VAR=… cmd`); recorded here so the false-pass mode is not repeated |
| Low | Filesystem headroom was 98% used with ~22 GB free at the time of the milestone's own verification | Future full-workspace builds may run out of space | Closed for this pass: the frozen-SHA qualification ran with roughly 122 GB free and Tier 1 / Tier 2 / Tier 3 all completed. Reclaim space before any later full-workspace run if the host fills again |

## 11. Roadmap disposition

`plans/subsystems/core-transport-policy-roadmap.md` milestone 8 moves from
`conditionally closed` to `closed`, with §11 (completion definition) and §12
(milestone status) updated in the same pass.

The milestone's own scope is complete, and the cross-cutting release evidence
— Tier 1 / Tier 2 / Tier 3 on the frozen SHA and the Stage C rebinding — was
delivered on the bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c`. The
findings that are still open are listed in §10. The successor milestone
(M009, lean path single-sourcing) is not blocked by this one.

## 12. Registry updates

`plans/registry.md`:

- The core-transport-policy row records milestone 8 as closed.
- The implementation-plan table for M008 points at this closure record.
- The blocked / operational work section no longer records the Stage C
  rebinding as pending for the milestones closed in this pass: it was
  delivered on the bound SHA `26fa6192…`, together with the Tier 1 / Tier 2 /
  Tier 3 run.
