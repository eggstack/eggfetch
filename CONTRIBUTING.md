# Contributing to eggfetch

Thank you for your interest in contributing to eggfetch. This document covers the project's expectations for code style, linting, dependencies, testing, and architectural boundaries.

## Formatting

All Rust code must be formatted with rustfmt using the project's `rustfmt.toml`:

```sh
cargo fmt --all
```

Key settings: `max_width = 100`, `use_small_heuristics = "default"`. The project uses the 2021 edition. Run `cargo fmt --all` before committing; CI will check formatting.

## Linting

Clippy runs with pedantic lints enabled:

```sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The workspace `Cargo.toml` enables `pedantic = { level = "warn", priority = -1 }` with `module_name_repetitions = "allow"` and `must_use_candidate = "allow"`. The `.clippy.toml` sets `missing-docs-in-crate-items = true` and `avoid-breaking-exported-api = false`.

Do not disable pedantic lints to make code compile. If a lint is genuinely incorrect or unhelpful for a specific case, justify the suppression with a comment explaining why and seek reviewer approval.

## Unsafe

The workspace sets `unsafe_code = "forbid"`. There is no `#[allow(unsafe_code)]` anywhere in the project. Any new use of `unsafe` requires a strong justification, a separate decision, and likely a dedicated review. Do not add `unsafe` without explicit discussion.

## Documentation

The workspace sets `missing_docs = "warn"`. Public items (structs, enums, traits, functions, modules) should have doc comments. This is enforced by both the lint and `.clippy.toml` (`missing-docs-in-crate-items = true`).

For skeletal types in Milestone A, use a brief doc comment like:

```rust
/// Timeout configuration placeholder.
///
/// Phase-aware timeouts (connect, pool, write, read, total) land in
/// Milestone D.
```

The goal is to make the gap between current state and final implementation obvious to future readers.

## Testing

Tests live next to the code they cover, using `#[cfg(test)] mod tests` blocks within the same file. Run the full suite with:

```sh
cargo test --workspace --all-features
```

Prefer small, focused tests that exercise one behavior. The workspace has ~160 tests covering construction, streaming, timeouts, pools, headers, and integration scenarios. As the project grows, tests should cover protocol correctness, edge cases, and error paths.

## Dependencies

Every new dependency must have an explicit reason stated in the PR or commit. The project follows these rules:

- Prefer Rustls over native TLS for auditability and portability.
- Keep optional features out of `default` unless they are essential for a minimal HTTP client.
- Avoid proc-macro-heavy dependencies unless they materially improve correctness or maintainability.
- Minimize transitive dependency trees. A convenience crate that pulls in a large tree needs a strong justification.
- Feature-gate capabilities that are not core to HTTP/1.1 client behavior (compression, cookies, proxy, tracing, JSON).

See `docs/architecture/dependency-policy.md` for the full dependency policy.

## Feature Flags

Do not add a feature flag just to silence a clippy lint or to opt into behavior that should be unconditional. Feature flags exist to let users pay only for what they use. Do not enable optional behavior in `default` without discussion.

Current `eggfetch-core` features:

```toml
default = ["http1", "tls-rustls"]
http1 = []
http2 = []
tls-rustls = []
json = []
compression-gzip = []
compression-brotli = []
compression-zstd = []
cookies = []
proxy = []
tracing = []
```

The `http1` and `tls-rustls` features are implemented and wired to real dependency features (default). The remaining features represent intended capability, not current behavior. See `docs/architecture/feature-flags.md` for details.

## Compatibility Expectations

The Rust API stays idiomatic. Do not shape the Rust API to mirror Python conventions. The Rust `Client` should feel like a natural async Rust HTTP client, not a port of `httpx`.

The Python sync API must block on the async Rust engine and release the GIL during blocking operations. The Python async API targets asyncio first. Trio/AnyIO support is a later goal, not an MVP requirement.

## No Duplicate Networking

All network I/O goes through eggfetch-core. There must not be a second synchronous networking implementation in the Python crate, the CLI crate, or anywhere else. Synchronous Python adapters block on the async engine. This is a hard architectural invariant.

If you find yourself writing HTTP logic outside of eggfetch-core, stop and refactor.

## Milestone Context

eggfetch follows a milestone-driven development sequence (A through M). Before starting work, read `plans/ROADMAP.md` and the relevant milestone plan in `plans/`. Each milestone is a handoff boundary: finish one before starting the next. A clean baseline matters more than an early partial implementation.

Make the workspace build green before adding new functionality. Format before committing. Do not bypass CI to land changes.
