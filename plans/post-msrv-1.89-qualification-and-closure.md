# Post-MSRV 1.89 Qualification and Closure

Status: **complete**  
Parent program: `msrv-1.89-migration-program.md`  
Date: 2026-09-15

## Purpose

Close the Rust 1.89 MSRV migration only after the manifest/resolver/validation
implementation is stable and one exact qualification-sensitive tree can be
frozen.

Although the intended MSRV migration changes build/package policy rather than
HTTP behavior, it changes `Cargo.toml`, Cargo resolver policy, validation
scripts, and potentially dependency resolution. Those inputs are not a
documentation-only descendant of the currently qualified tree. The existing
HTTPX 0.28.1 / HTTPX2 2.12.0 exact-SHA qualification must therefore be renewed
rather than implicitly carried forward.

## Closure Record

Closed on candidate executable SHA
`97e87e42c8f4d5659739e7b23ff9005a4ec1ae53` (parent
`a1ca8d9468b6227897110e26a4352f51ae05e73f`). Exact Rust 1.89.0 compiler and
Cargo proofs, the fresh-resolution audit, Tier 1, extended Tier 2, package
validation, API oracles, and the required three consecutive full pinned
compatibility runs passed. Each compatibility run collected 1,870 tests with
no failures. The prior `312cd440...` qualification is historical; both
compatibility profiles and the live ledger now bind to the candidate.

The committed `Cargo.lock` did not change, no direct dependency or feature
ownership changed, and Rust source edits were limited to stable-Clippy
compatibility corrections. A final docs/profile/ledger descendant is the only
post-freeze change. The exact toolchain was `rustc 1.89.0 (29483883e
2025-08-04)` with Cargo `1.89.0 (c24e10642 2025-06-23)`.

## Entry Criteria

Do not begin closure until all of the following are true:

- root workspace declares `rust-version = "1.89"`;
- all publishable members inherit the workspace Rust version;
- intended resolver policy is finalized;
- `rust-toolchain.toml` still uses stable;
- `tier2_msrv()` explicitly tests Rust 1.89.0 and no longer treats old-Cargo
  incompatibility as a permissible MSRV result;
- the implementation MSRV matrix passes;
- a disposable fresh-resolution audit has demonstrated a Rust-1.89-compatible
  graph;
- live documentation reflects the new floor; and
- there are no unrelated source/runtime changes mixed into the migration.

If any runtime code or dependency upgrade was required to make Rust 1.89 work,
expand the closure audit accordingly and record the exact reason before
freezing.

## Phase 1 — Freeze and Diff Audit

Create one clean candidate SHA after implementation. Record:

- candidate SHA;
- parent SHA;
- changed file list;
- whether `Cargo.lock` changed;
- whether any direct dependency requirement changed;
- whether any resolved dependency version/feature changed;
- whether Rust source files changed; and
- whether compatibility/runtime fixture files changed.

Classify every changed file into:

1. workspace/compiler/resolver metadata;
2. validation/release tooling;
3. live documentation;
4. compatibility qualification metadata; or
5. unexpected/runtime change.

The candidate is not acceptable if category 5 contains an unexplained change.

### Lockfile acceptance

Preferred state: no lockfile change.

If `Cargo.lock` changed:

- produce a dependency-version and feature diff;
- prove the change is required by the MSRV/resolver migration rather than an
  incidental `cargo update`;
- run the full runtime qualification as if this were a dependency update;
- call out changes in security-sensitive transport dependencies explicitly
  (`hyper`, `hyper-util`, `rustls`, `tokio-rustls`, `quinn`, `h3`, `h3-quinn`,
  `tokio`, PyO3).

Do not qualify a tree containing unexplained dependency churn.

## Phase 2 — Exact Rust 1.89 Proof

Run on the candidate SHA with **Rust 1.89.0**, not `stable` substituted for it.

Required compiler/metadata proof:

```sh
rustup run 1.89.0 cargo metadata --format-version 1 --no-deps
rustup run 1.89.0 cargo check --locked -p eggfetch-core --no-default-features
rustup run 1.89.0 cargo check --locked -p eggfetch-core --no-default-features --features http1
rustup run 1.89.0 cargo check --locked -p eggfetch-core --no-default-features --features http1,tls-rustls
rustup run 1.89.0 cargo check --locked -p eggfetch-core --all-features
rustup run 1.89.0 cargo check --locked --workspace --all-targets --all-features
```

Required migration confidence runs:

```sh
rustup run 1.89.0 cargo test --locked -p eggfetch-core --all-features -- --test-threads=1
rustup run 1.89.0 cargo test --locked -p eggfetch-ffi --all-features
rustup run 1.89.0 cargo test --locked -p eggfetch-node --all-features
rustup run 1.89.0 cargo check --locked -p eggfetch-python --all-features
rustup run 1.89.0 cargo check --locked -p eggfetch-cli --all-features
```

Record the exact `rustc --version --verbose` and `cargo --version --verbose`
outputs used for evidence.

Acceptance:

- every publishable crate reports `rust_version = 1.89` in metadata;
- no command uses `--ignore-rust-version`;
- no MSRV command is skipped;
- no target silently compiles with stable instead of 1.89; and
- failures are fixed at the narrowest responsible layer rather than hidden by
  weakening the matrix.

## Phase 3 — Fresh Resolution Proof

Repeat the disposable fresh-resolution exercise from the implementation plan
against the frozen candidate:

1. clean temporary checkout/copy;
2. remove only the temporary `Cargo.lock`;
3. generate a fresh graph using Rust 1.89.0 and the candidate's resolver policy;
4. run the core minimal + all-feature checks and the workspace all-feature
   check;
5. capture the resulting dependency graph for the closure record.

The fresh graph is evidence only unless the committed lockfile is invalid.
Do not replace a reviewed committed lockfile simply because fresh resolution
selects newer packages.

## Phase 4 — Repository Validation

On the same candidate tree, run the canonical repository gates:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Requirements:

- Tier 1 fully passes subject only to already-documented non-product optional
  Node JS artifact behavior;
- Tier 2 records a **real Rust 1.89 MSRV pass**;
- an absent Rust 1.89 toolchain is a closure blocker, not a skip;
- package validation uses a clean worktree;
- `eggfetch-core` publish dry-run succeeds;
- dependent package-structure/version checks succeed;
- Python wheel build/smoke and package-content validation succeed.

Inspect packaged Cargo metadata and confirm the published normalized manifest
contains the intended Rust 1.89 floor.

## Phase 5 — API and Compatibility Oracles

Even though no API change is intended, run the existing API oracles on the
frozen candidate:

- HTTPX 0.28.1 API manifest comparison;
- HTTPX2 2.12.0 API manifest comparison;
- lossless merge tests;
- ordinary native Python behavior tests; and
- any repository script currently used to assert generated/reference API
  consistency.

There must be no new allowed-difference entry justified solely by the MSRV
migration.

## Phase 6 — Exact-SHA HTTPX / HTTPX2 Requalification

Run the repository's existing full pinned compatibility procedure on the exact
candidate SHA.

Follow the established closure convention:

- run the complete compatibility suite successfully;
- run the required repeated exact-SHA passes (currently three consecutive full
  runs where the live ledger requires them);
- regenerate/compare API manifests rather than hand-editing generated data;
- update both compatibility profiles to the candidate qualification SHA/date;
- mark the previous qualification SHA historical; and
- update `plans/httpx-parity-correction-status.md` with the new freeze and MSRV
  qualification result.

Do not claim the prior `312cd440...` executable qualification remains current
after the manifest/resolver/validation migration.

## Phase 7 — Dependency / Feature Invariance Audit

Verify the MSRV migration did not alter product behavior indirectly.

Required checks:

- root/default core features unchanged;
- every explicit feature-to-dependency mapping unchanged unless a documented
  Rust-1.89 compatibility correction required otherwise;
- `cargo tree -e features` (or repository-equivalent dependency evidence)
  compared before/after for default core and all-feature core;
- Python feature set unchanged;
- CLI feature set unchanged;
- FFI/Node dependency direction unchanged;
- no new dependency added solely to perform MSRV validation.

If resolver 3 changes a selected package on fresh resolution but not the
committed lockfile, record that as fresh-resolution behavior rather than a
runtime dependency change in the frozen product graph.

## Phase 8 — Documentation and Historical Truth Pass

After the exact executable/qualification-sensitive SHA is frozen and validated,
make only the documentation/profile/ledger edits needed to close the program.

Current/live docs should consistently state:

- Rust 1.89+ is supported;
- Edition remains 2021;
- normal development/CI uses stable;
- Tier 2 explicitly validates Rust 1.89.0;
- the old Rust-1.80 Cargo/Edition-2024 skip is no longer current policy.

Do **not** rewrite old completed plan evidence that truthfully records Rust 1.80
or the former skip. Historical evidence must remain historical.

Update this plan and `msrv-1.89-migration-program.md` with:

- implementation SHA;
- frozen qualification SHA;
- whether `Cargo.lock` changed;
- exact Rust/Cargo 1.89 versions used;
- Tier 1/2/package outcomes;
- compatibility run counts/results;
- profile/ledger renewal status; and
- any remaining optional local skips unrelated to MSRV.

A final documentation-only descendant is allowed after the freeze. Audit that
all post-freeze changes are documentation/profile/ledger-only before calling
the candidate binding current.

## Optional External Qualification

The MSRV change does not by itself require HTTP/3 external interop,
native-body/TLS provider, or Tonic fixture requalification because those are
behavior/consumer qualification programs rather than compiler-floor gates.

Run the relevant external fixture anyway if either of these occurred:

- a dependency version/feature changed in the frozen graph; or
- Rust source changed to accommodate 1.89.

If no runtime/dependency graph change occurred, the ordinary exact-SHA
repository/compatibility closure is sufficient.

## Closure Acceptance Criteria

### Compiler contract

- [x] Candidate compiles with exact Rust 1.89.0.
- [x] All publishable crates report Rust 1.89 in Cargo metadata.
- [x] Minimal core profile compiles on 1.89.
- [x] Core all-features compiles/tests on 1.89.
- [x] Workspace all-target/all-feature check passes on 1.89.
- [x] FFI/Node 1.89 tests and Python/CLI 1.89 checks pass.
- [x] No MSRV check is skipped or run with `--ignore-rust-version`.

### Resolution / packaging

- [x] Fresh Rust-1.89 dependency resolution succeeds.
- [x] Committed lockfile is unchanged or every change is required/reviewed.
- [x] No unrelated dependency upgrade is present.
- [x] Packaged crate metadata carries `rust-version = "1.89"`.
- [x] `cargo publish --dry-run -p eggfetch-core` succeeds.

### Repository quality

- [x] Tier 1 passes.
- [x] Tier 2 passes with real MSRV evidence.
- [x] Package validation passes.
- [x] Feature/dependency ownership is unchanged.
- [x] No product/runtime/API semantic change was introduced solely for MSRV.

### Compatibility

- [x] HTTPX 0.28.1 API oracle passes.
- [x] HTTPX2 2.12.0 API oracle passes.
- [x] Full pinned compatibility suite passes on the exact candidate SHA.
- [x] Required consecutive exact-SHA runs pass.
- [x] Both compatibility profiles are rebound to the new qualification SHA.
- [x] Live parity/status ledger records the new freeze.

### Documentation / closure

- [x] All live compiler-support docs say Rust 1.89+.
- [x] Stable remains the normal repository toolchain.
- [x] Edition remains 2021.
- [x] Historical Rust 1.80 records remain truthful.
- [x] Parent program and this plan record final SHAs/evidence.
- [x] Any post-freeze descendant is proven documentation/profile/ledger-only.

## Final Outcome Required

The closure statement should be materially equivalent to:

> eggfetch now declares and verifies Rust 1.89 as its workspace MSRV. All
> publishable Rust crates compile on the exact 1.89 toolchain, the current
> dependency graph and a fresh Rust-version-aware resolution are compatible,
> package metadata carries the new floor, and the exact post-migration tree has
> passed the repository and HTTPX/HTTPX2 qualification gates. Stable remains
> the normal development toolchain and the project remains on Edition 2021.

Do not use that wording until every acceptance criterion above is supported by
actual evidence from the frozen candidate.
