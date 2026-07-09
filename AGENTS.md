# Agent Guide

This file contains guidance for AI coding agents working in the eggfetch repository.

## Milestone Sequence

eggfetch follows a milestone-driven development sequence: A through M. Each milestone is a handoff boundary. Before starting work, read `plans/ROADMAP.md` and the relevant milestone plan in `plans/`. The milestones are:

- A: Repository and workspace foundation (complete)
- B: Core request/response model and minimal HTTP engine (complete)
- C: Connection management (current)
- C: Connection management
- D: Timeout system
- E: Streaming foundation
- F: Python sync API
- G: Python async API
- H: Response compatibility surface
- I: Request builder compatibility surface
- J: Redirect engine
- K: CLI
- L: Correctness and differential testing
- M: Documentation and public MVP preparation

Do not skip ahead. A clean baseline matters more than an early partial implementation.

## Crate Boundary Invariant

eggfetch-core owns all HTTP behavior. The CLI and Python crates are thin adapters.

- eggfetch-core must not depend on PyO3, clap, or CLI argument parsing.
- eggfetch-cli and eggfetch-python must not contain independent HTTP behavior (no direct use of hyper, tokio TCP, or any networking crate beyond what eggfetch-core exposes).
- All network I/O goes through eggfetch-core. There is exactly one networking implementation.

## One Networking Implementation

There must not be a parallel synchronous networking path. The Python sync API blocks on the async Rust engine and releases the GIL. The CLI delegates to eggfetch-core. If you find yourself writing HTTP logic outside of eggfetch-core, stop and refactor.

## Lint Policy

The workspace enables pedantic clippy with `module_name_repetitions = "allow"` and `must_use_candidate = "allow"`. The `unsafe_code` lint is set to `forbid` workspace-wide. The `missing_docs` lint is set to `warn`.

- Do not disable pedantic lints to make code compile.
- Do not add `#[allow(unsafe_code)]`. There is no path to `unsafe` without explicit discussion.
- If a lint is genuinely wrong for a specific case, justify the suppression with a comment explaining why.

## Doc Policy

Public items need doc comments. The `.clippy.toml` sets `missing-docs-in-crate-items = true`. For skeletal types, use a brief doc comment that states which milestone fills in the real implementation:

```rust
/// Body placeholder.
///
/// Real body semantics (buffered vs streaming, ownership rules) land in
/// Milestone E.
```

## Tests

Prefer small, focused tests colocated with the module under test. Use `#[cfg(test)] mod tests` blocks within the same file. Run the full suite with:

```sh
cargo test --workspace --all-features
```

The current tests are skeletal and verify compilation and basic construction.

## Working Style

- Make the workspace build green before adding new functionality.
- Run `cargo fmt --all` before committing.
- Run `cargo clippy --workspace --all-targets --all-features -- -D warnings` before committing.
- Do not bypass CI to land changes.
- Keep commits scoped to a single logical change.

## Safety

Do not add `unsafe`. The workspace uses `unsafe_code = "forbid"`. If you think you need `unsafe`, stop and ask.

## Commits

Do not commit without an explicit user request. When committing, keep the message scoped and descriptive of the change.
