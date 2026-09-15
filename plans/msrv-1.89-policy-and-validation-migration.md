# Rust 1.89 MSRV Policy and Validation Migration

Status: **ready for implementation**  
Parent program: `msrv-1.89-migration-program.md`  
Date: 2026-09-15

## Scope

Raise eggfetch's declared workspace MSRV from Rust 1.80 to Rust 1.89.0 and make
that floor mechanically verifiable across the published Rust workspace.

This plan owns the manifest, Cargo resolver, validation-script, package-metadata,
and live documentation changes needed for the migration. It does **not** own the
final exact-SHA compatibility requalification; that is handled by
`post-msrv-1.89-qualification-and-closure.md` after this implementation is
stable.

## Baseline Findings

### Workspace metadata

The root `Cargo.toml` is a virtual workspace with:

```toml
[workspace]
resolver = "2"

[workspace.package]
edition = "2021"
rust-version = "1.80"
```

The publishable crates (`eggfetch-core`, `eggfetch-cli`, `eggfetch-ffi`,
`eggfetch-python`, `eggfetch-node`) use `rust-version.workspace = true`.
`eggfetch-bench` is a workspace member but is not published.

### Toolchain policy

`rust-toolchain.toml` currently pins the **channel** to `stable` and requests
`rustfmt` + `clippy`. That is the correct development policy and must stay
unchanged. The repository should not force every contributor and CI invocation
to compile with the minimum compiler.

### Current MSRV validation weakness

`./scripts/check.sh extended` currently runs `tier2_msrv()`, which:

- labels the check `Rust 1.80`;
- skips when `rustup` is absent;
- skips when Rust 1.80 is not installed;
- checks only `eggfetch-core` with `http1,tls-rustls`; and
- explicitly converts certain old-Cargo/current-resolution incompatibilities
  into a recorded skip.

This was truthful as a temporary policy, but it no longer provides a verified
package contract.

### Verification policy

`docs/verification-policy.md`, `docs/architecture/build-ci.md`, and `AGENTS.md`
all describe the Rust-1.80 optional-skip behavior. `CONTRIBUTING.md` points to
Tier 2 as the release-time feature/MSRV validation authority. The release
process relies on local validation rather than adding more CI jobs; this
constraint must remain intact.

## Design Decisions

### 1. Use Rust 1.89 as one workspace-wide floor

Change only the authoritative workspace declaration:

```toml
[workspace.package]
rust-version = "1.89"
```

Do not duplicate `rust-version` values in member crates.

### 2. Keep Edition 2021

Do not conflate compiler support policy with an edition migration. No source
rewrite for Edition 2024 belongs in this plan.

### 3. Prefer resolver 3

Change the virtual workspace from:

```toml
resolver = "2"
```

to:

```toml
resolver = "3"
```

Rationale: with a Rust 1.89 floor, resolver 3 is supported and its
Rust-version-aware dependency selection is better aligned with maintaining an
MSRV than resolver 2's historical behavior.

Before committing the resolver change:

1. run the existing lockfile unchanged with `--locked`;
2. verify the lockfile remains accepted;
3. perform a separate disposable fresh-resolution exercise under Rust 1.89;
4. compare dependency versions/features against the committed graph; and
5. reject unrelated dependency churn from the implementation commit.

If resolver 3 exposes an actual package incompatibility, fix only the minimal
manifest/dependency constraint necessary and document why. Do not use the MSRV
migration as an excuse for broad dependency upgrades.

### 4. Make MSRV verification fail closed

Rewrite `tier2_msrv()` around one explicit toolchain constant, e.g.:

```sh
MSRV_TOOLCHAIN="1.89.0"
```

or an equivalent local constant that is not user-controlled in release
qualification.

Required behavior:

- if `rustup` is missing: fail with a clear prerequisite message;
- if Rust 1.89.0 is missing: fail with an installation command;
- if Cargo/rustc rejects the graph: fail and show the underlying log;
- remove the special Edition-2024/resolution skip branch;
- never pass `--ignore-rust-version`;
- never silently substitute latest stable for the MSRV compiler.

Routine Tier 1 CI remains unchanged and continues to use stable. The strict
MSRV compiler requirement belongs to Tier 2/release qualification.

## Required MSRV Validation Matrix

The existing one-profile check is insufficient because the same workspace
MSRV is published for every Rust crate.

At minimum, `tier2_msrv()` must run the following with Rust 1.89.0 and the
committed lockfile:

```sh
rustup run 1.89.0 cargo check --locked -p eggfetch-core --no-default-features
rustup run 1.89.0 cargo check --locked -p eggfetch-core --no-default-features --features http1
rustup run 1.89.0 cargo check --locked -p eggfetch-core --no-default-features --features http1,tls-rustls
rustup run 1.89.0 cargo check --locked -p eggfetch-core --all-features
rustup run 1.89.0 cargo check --locked --workspace --all-targets --all-features
```

The final command is the package-surface proof for all workspace members; the
focused core commands preserve minimal/default-ish feature-boundary evidence.

If `--all-targets --all-features` pulls in a target whose requirements are
strictly qualification-only and not part of a publishable crate contract,
exclude that target narrowly and document the reason. Do not weaken the gate by
falling back to the old single core profile.

### One-time migration proof

Before closure, additionally run under Rust 1.89.0:

```sh
cargo test -p eggfetch-core --all-features -- --test-threads=1
cargo test -p eggfetch-ffi --all-features
cargo test -p eggfetch-node --all-features
cargo check -p eggfetch-python --all-features
cargo check -p eggfetch-cli --all-features
```

These may be recorded as migration/closure evidence rather than repeated in
every routine Tier 2 invocation if runtime cost becomes disproportionate. The
persistent gate must still compile every publishable crate.

## Fresh-Resolution Audit

A committed lockfile proves the repository can build one known graph. It does
not prove the new resolver/MSRV policy can produce a compatible graph from
scratch.

Perform a one-time disposable fresh-resolution audit:

1. copy/checkout the migration commit into a clean temporary directory;
2. remove `Cargo.lock` only in that disposable copy;
3. run Rust 1.89.0 Cargo with the workspace's intended resolver policy to
   generate a new lockfile;
4. run the MSRV compile matrix against that fresh resolution;
5. record whether any direct/transitive package had to fall back to an older
   Rust-compatible version; and
6. compare the fresh graph with the committed graph.

Acceptance rules:

- a Rust-1.89-compatible fresh resolution must exist;
- no direct dependency may require a compiler above 1.89 under the supported
  feature sets;
- if a transitive dependency's newest release exceeds the floor but resolver 3
  selects a compatible release, record that as expected resolver behavior;
- do not commit the fresh lockfile merely because it is newer;
- if the committed lockfile itself contains a package incompatible with
  Rust 1.89, update only the minimum dependency set required and treat that as
  qualification-sensitive dependency work.

## Package Metadata and Release Contract

Verify with `cargo metadata` that every publishable member reports Rust 1.89.
Do not rely only on visual inspection of the root manifest.

During package validation, inspect at least the normalized packaged manifest for
`eggfetch-core` (and any other package artifact already materialized by the
existing packaging flow) to confirm `rust-version = "1.89"` survives workspace
inheritance/normalization into the published crate metadata.

Do not add a new workflow for this. Prefer a small assertion in existing local
validation/release tooling if mechanical enforcement is needed.

## Documentation Updates

Update current/live documentation that describes the supported compiler floor
or the old skip behavior. At minimum audit:

- `AGENTS.md`
- `CONTRIBUTING.md`
- `docs/verification-policy.md`
- `docs/architecture/build-ci.md`
- `.skills/rust-development.md`
- `.skills/release-process.md` if it mentions compiler support
- `README.md` / Rust guide / dependency policy if they state an MSRV
- `CHANGELOG.md` under `[Unreleased]`

Required documentation semantics:

- supported Rust floor is 1.89+;
- latest stable remains the normal development/CI toolchain;
- Tier 2 explicitly validates 1.89.0;
- missing MSRV tooling is a validation failure for a release/qualification
  environment, not a pass;
- Edition remains 2021;
- historical completed plans may retain Rust 1.80 references when those are
  descriptions of past evidence.

Run a repository-wide search for:

```text
1.80
Rust 1.80
MSRV
rust-version
resolver = "2"
```

Classify each hit as live policy, generated/package metadata, or historical
record before editing it.

## Lockfile / Dependency Rules

- Prefer **no** `Cargo.lock` change if the existing graph is valid on resolver 3
  and Rust 1.89.
- If a lock change is unavoidable, include an explicit before/after dependency
  graph review in the implementation record.
- Do not opportunistically upgrade `hyper`, `rustls`, PyO3, Quinn/h3, Tokio, or
  compression crates.
- Do not change feature ownership/defaults.
- Do not relax version requirements merely to make a test green.

## Compatibility / Runtime Boundaries

This work must not change:

- HTTP request/response semantics;
- routing, proxy, TLS, DNS, retry, redirect, timeout, pool, H1/H2/H3 behavior;
- Rust public API signatures;
- Python public API;
- C ABI or Node API;
- HTTPX/HTTPX2 behavior;
- default features;
- security policy.

If implementation requires a runtime/source change to compile on 1.89, stop and
record that as a discovered incompatibility. The expected migration should be
manifest/tooling/documentation-only plus any strictly necessary dependency
compatibility correction.

## Validation During Implementation

Run, in this order:

1. `cargo metadata --format-version 1 --no-deps` — confirm every publishable
   member reports Rust 1.89.
2. Rust 1.89 MSRV matrix from this plan.
3. Disposable fresh-resolution audit.
4. `./scripts/check.sh`.
5. `./scripts/check.sh extended` — must contain a real Rust 1.89 pass.
6. `./scripts/check.sh package` on a clean worktree after committing the
   implementation, if the package gate requires cleanliness.

Do not rebind compatibility profiles during intermediate implementation churn.
Freeze first; closure owns the exact-SHA qualification.

## Acceptance Criteria

### Manifest / resolver

- [ ] `[workspace.package].rust-version = "1.89"`.
- [ ] Every publishable crate still uses `rust-version.workspace = true`.
- [ ] Edition remains 2021.
- [ ] `rust-toolchain.toml` remains `stable`.
- [ ] Workspace resolver is 3, or a concrete documented blocker and equivalent
      Rust-version-aware strategy is recorded.

### Validation

- [ ] Tier 2 names and executes Rust 1.89.0.
- [ ] Missing `rustup`/1.89 toolchain fails with actionable instructions.
- [ ] No Edition-2024/old-Cargo skip branch remains.
- [ ] MSRV validation uses `--locked` for the committed-graph proof.
- [ ] Minimal core and all-feature core compile on 1.89.
- [ ] All publishable workspace crates compile on 1.89.
- [ ] One-time 1.89 tests/checks listed above pass.
- [ ] A disposable fresh dependency resolution succeeds on 1.89.

### Metadata / dependency graph

- [ ] `cargo metadata` reports Rust 1.89 for every publishable crate.
- [ ] Packaged crate metadata reports Rust 1.89.
- [ ] Existing lockfile remains unchanged, or every unavoidable change is
      reviewed and documented.
- [ ] No unrelated dependency upgrade lands.

### Documentation

- [ ] Live policy docs describe Rust 1.89+.
- [ ] Live docs no longer describe the Rust 1.80 resolver skip as current
      behavior.
- [ ] Changelog records the compiler-floor increase as a pre-1.0 breaking
      compatibility change.
- [ ] Historical evidence is not falsified by rewriting old records.

### Regression gates

- [ ] Tier 1 passes.
- [ ] Tier 2 passes with non-skipped MSRV evidence.
- [ ] Package validation passes.
- [ ] No runtime/public-API behavior change was introduced.

## Handoff Note

Once every acceptance item above is green, do not declare the migration closed.
Hand the resulting clean implementation to
`post-msrv-1.89-qualification-and-closure.md` for the exact-SHA compatibility
freeze, profile renewal, and final documentation/plan closure.
