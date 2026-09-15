# Rust 1.89 MSRV Migration Program

Status: **ready for implementation**  
Date: 2026-09-15  
Target repository: `eggstack/eggfetch`  
Target MSRV: **Rust 1.89.0**

## Objective

Raise eggfetch's declared and actually verified minimum supported Rust version
from Rust 1.80 to Rust 1.89.0, align the workspace with the current eggstack
Rust baseline, and remove the existing state where the repository advertises
an MSRV that its own current Cargo dependency graph cannot reliably validate.

This is a build/tooling/package-contract program. It must not alter HTTP
behavior, public Rust/Python APIs, feature semantics, dependency ownership, or
runtime defaults merely to justify the compiler-floor change.

## Current State and Trigger

The root workspace currently declares:

```toml
[workspace]
resolver = "2"

[workspace.package]
edition = "2021"
rust-version = "1.80"
```

All publishable workspace crates inherit `rust-version.workspace = true`, so
there is one authoritative package-level Rust floor today.

The current extended validation has a special Rust 1.80 path that:

- checks only a narrow `eggfetch-core` feature profile;
- skips when Rust 1.80 is not installed; and
- also skips when Rust 1.80's Cargo cannot parse the current crates.io
  resolution (for example, dependencies using manifest/edition features newer
  than that Cargo understands).

The normative verification docs correctly say this skip is not a pass, but the
result is still a mismatch between the declared package contract and the
repository's ability to prove it.

Normal development and CI are intentionally pinned to the `stable` channel via
`rust-toolchain.toml` / the stable GitHub Actions toolchain. That should remain
unchanged: MSRV is a lower compatibility bound, not the preferred development
compiler.

## Program Decisions

1. **Set the workspace MSRV to exactly `1.89`.**
   - Use `rust-version = "1.89"` in `[workspace.package]`.
   - All publishable crates continue to inherit that value.
   - Do not add crate-specific Rust versions.

2. **Keep Rust Edition 2021.**
   - The MSRV bump does not imply an edition migration.
   - Edition 2024 is explicitly out of scope.

3. **Keep the repository toolchain channel on `stable`.**
   - Do not change `rust-toolchain.toml` from `channel = "stable"` to 1.89.
   - Latest stable remains the normal formatter/linter/development compiler.
   - Rust 1.89.0 is invoked explicitly by the MSRV validation path.

4. **Adopt Cargo resolver 3 with the MSRV bump, subject to a clean dependency-resolution audit.**
   - Rust 1.89 is new enough to support resolver 3.
   - Resolver 3 makes Rust-version-aware dependency fallback the workspace
     default and is directly aligned with maintaining a declared compiler
     floor.
   - Keep Edition 2021; resolver 3 is an independent workspace choice.
   - Do not accept unrelated dependency upgrades as incidental fallout. If the
     existing `Cargo.lock` remains valid, keep it. If the resolver transition
     requires a lockfile change, inspect and document the complete graph diff.

5. **Replace the current best-effort MSRV check with a real Rust 1.89 gate in Tier 2.**
   - The gate must fail when Rust 1.89.0 is unavailable instead of recording a
     successful/neutral skip.
   - Error output should state the exact installation command, e.g.
     `rustup toolchain install 1.89.0 --profile minimal`.
   - Remove the Rust-1.80-specific Edition-2024/Cargo-resolution skip branch.
   - Do not add a routine CI matrix or a second automatic workflow.

6. **Verify the whole published Rust surface, not only one core feature set.**
   The shared workspace MSRV applies to `eggfetch-core`, `eggfetch-cli`,
   `eggfetch-ffi`, `eggfetch-python`, and `eggfetch-node`. The MSRV validation
   must therefore compile every publishable crate at least once and must also
   cover core's minimal and all-feature shapes.

7. **Treat the MSRV change as qualification-sensitive.**
   `Cargo.toml`, resolver behavior, validation scripts, and potentially the
   resolved dependency graph are not documentation-only changes. After the
   implementation lands, perform one clean exact-SHA repository compatibility
   qualification and rebind the live HTTPX/HTTPX2 qualification records.

## Work Breakdown

Execute these plans in order:

1. `msrv-1.89-policy-and-validation-migration.md`
   - change workspace metadata/resolver policy;
   - replace the Rust 1.80 validation path with an exact 1.89.0 gate;
   - prove all publishable crates and representative core feature profiles
     compile on 1.89;
   - audit fresh dependency resolution;
   - update all live/normative MSRV documentation.

2. `post-msrv-1.89-qualification-and-closure.md`
   - freeze one executable/qualification-sensitive SHA;
   - run Tier 1, Tier 2, package validation, API oracles, and exact-SHA
     compatibility qualification;
   - inspect dependency/lock/package metadata;
   - update live compatibility profiles/ledger and close the program.

## Explicit Non-Goals

- No Rust Edition 2024 migration.
- No runtime feature work.
- No HTTP protocol changes.
- No Python API changes.
- No FFI or Node API changes.
- No dependency upgrade campaign unless a dependency is demonstrably
  incompatible with Rust 1.89 or resolver 3.
- No new CI matrix, scheduled workflow, or publication automation.
- No attempt to make historical completed plan records pretend they were
  written against Rust 1.89; historical Rust 1.80 references may remain when
  they describe past evidence.

## Program Acceptance Criteria

The program is complete only when all of the following are true:

- [ ] Root `Cargo.toml` declares `rust-version = "1.89"`.
- [ ] All publishable workspace crates inherit the workspace Rust version.
- [ ] The workspace uses resolver 3, or the implementation records a concrete
      technical reason resolver 3 could not be adopted and demonstrates an
      equivalent Rust-version-aware dependency-resolution strategy.
- [ ] `rust-toolchain.toml` still targets `stable`.
- [ ] Tier 2 invokes Rust **1.89.0** explicitly and cannot convert compiler or
      Cargo incompatibility into an MSRV pass.
- [ ] The MSRV gate checks every publishable crate plus core minimal/all-feature
      profiles.
- [ ] A clean/fresh dependency resolution under Rust 1.89 succeeds using the
      intended resolver policy.
- [ ] Any `Cargo.lock` change is intentional, fully reviewed, and contains no
      unrelated dependency churn.
- [ ] Current/live docs no longer claim Rust 1.80 is supported.
- [ ] Historical records are left truthful rather than rewritten.
- [ ] Tier 1 passes.
- [ ] Tier 2 passes with a real, non-skipped Rust 1.89 result.
- [ ] Package validation passes.
- [ ] Exact-SHA HTTPX 0.28.1 and HTTPX2 2.12.0 qualification is renewed on the
      post-migration freeze.
- [ ] No HTTP/runtime/API behavior change is required to complete the program.
