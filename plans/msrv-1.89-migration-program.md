# Rust 1.89 MSRV Migration Program

Status: **complete**  
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

## Closure Record

Implemented in candidate commit `97e87e42c8f4d5659739e7b23ff9005a4ec1ae53`
(parent `a1ca8d9468b6227897110e26a4352f51ae05e73f`). The workspace now uses
MSRV 1.89 and Cargo resolver 3 while retaining Edition 2021 and the stable
development toolchain. The committed `Cargo.lock` and dependency/feature
ownership are unchanged; the only Rust source changes are stable-Clippy
compatibility corrections with no runtime or public-API behavior change.

Exact toolchain evidence: `rustc 1.89.0 (29483883e 2025-08-04)` and
`cargo 1.89.0 (c24e10642 2025-06-23)`. The exact MSRV matrix, one-time crate
proofs, Tier 1, extended Tier 2, and package validation all passed. A
disposable Rust-1.89 fresh-resolution audit locked 282 packages and passed the
minimal core, all-feature core, and workspace all-target/all-feature checks;
that evidence lockfile was not committed. The packaged core manifest retained
`rust-version = "1.89"`.

The post-migration executable candidate passed the HTTPX 0.28.1 and httpx2
2.12.0 API oracles and three consecutive full pinned compatibility runs,
collecting 1,870 tests per run with no failures. Both profiles and the live
ledger are renewed to the candidate SHA. The only remaining local skips are
the repository's documented unrelated optional Node artifact/downstream
artifact checks; the MSRV check is a required non-skipped pass.

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

- [x] Root `Cargo.toml` declares `rust-version = "1.89"`.
- [x] All publishable workspace crates inherit the workspace Rust version.
- [x] The workspace uses resolver 3, or the implementation records a concrete
      technical reason resolver 3 could not be adopted and demonstrates an
      equivalent Rust-version-aware dependency-resolution strategy.
- [x] `rust-toolchain.toml` still targets `stable`.
- [x] Tier 2 invokes Rust **1.89.0** explicitly and cannot convert compiler or
      Cargo incompatibility into an MSRV pass.
- [x] The MSRV gate checks every publishable crate plus core minimal/all-feature
      profiles.
- [x] A clean/fresh dependency resolution under Rust 1.89 succeeds using the
      intended resolver policy.
- [x] Any `Cargo.lock` change is intentional, fully reviewed, and contains no
      unrelated dependency churn.
- [x] Current/live docs no longer claim Rust 1.80 is supported.
- [x] Historical records are left truthful rather than rewritten.
- [x] Tier 1 passes.
- [x] Tier 2 passes with a real, non-skipped Rust 1.89 result.
- [x] Package validation passes.
- [x] Exact-SHA HTTPX 0.28.1 and HTTPX2 2.12.0 qualification is renewed on the
      post-migration freeze.
- [x] No HTTP/runtime/API behavior change is required to complete the program.
