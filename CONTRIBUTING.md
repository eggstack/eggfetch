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

CI enforces a lint-suppression policy via `scripts/check_lint_suppressions.sh` that rejects forbidden blanket suppressions (`allow(warnings)`, `allow(clippy::all)`, `allow(clippy::pedantic)`). Use specific lint names in all `#[allow]` attributes.

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

Prefer small, focused tests that exercise one behavior. The workspace has ~750+ Rust tests, ~463+ Python tests, and ~40+ Node.js/FFI tests covering construction, streaming, timeouts, pools, headers, integration scenarios, sync/async API parity, redirect replay, total timeout across redirects, response decoding, cookie subsystem, authentication subsystem, multipart uploads, decompression, proxy tunneling, retry policies, and true network streaming via `client.stream()`. As the project grows, tests should cover protocol correctness, edge cases, and error paths.

### Python tests

Python tests require the wheel to be built first:

```sh
cd crates/eggfetch-python
maturin develop
python -m pytest -p pytest_asyncio
```

Run differential tests against pinned versions of `requests` and `HTTPX`:

```sh
python -m pytest tests/test_differential.py -p pytest_asyncio
```

### Validation pass

The full validation pass (used before release) runs feature-gated compilation and test subsets:

```sh
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --all-features
cargo test -p eggfetch-core --all-features
cd crates/eggfetch-python
maturin develop
python -m pytest -p pytest_asyncio
maturin build
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-brotli
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-zstd
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
```

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

Current `eggfetch-core` feature declarations:

```toml
default = ["http1", "tls-rustls"]
http1 = []
http2 = []
http3 = []
tls-rustls = []
json = []
compression-gzip = []
compression-brotli = []
compression-zstd = []
compression-deflate = []
cookies = []
proxy = []
multipart = []
tracing = []
test-util = []
```

All feature flags are implemented. `cookies`, `proxy`, and `multipart` are
opt-in in core and enabled by the Python binding and CLI. `http3` is
experimental. `tracing` and `json` are reserved stubs with no gated code
yet. See `docs/architecture/feature-flags.md` for details.

## Compatibility Expectations

The Rust API stays idiomatic. Do not shape the Rust API to mirror Python conventions. The Rust `Client` should feel like a natural async Rust HTTP client, not a port of `httpx`.

The Python sync API must block on the async Rust engine and release the GIL during blocking operations. The Python async API targets asyncio first. Trio/AnyIO support is a later goal, not an MVP requirement.

## No Duplicate Networking

All network I/O goes through eggfetch-core. There must not be a second synchronous networking implementation in the Python crate, the CLI crate, or anywhere else. Synchronous Python adapters block on the async engine. This is a hard architectural invariant.

If you find yourself writing HTTP logic outside of eggfetch-core, stop and refactor.

## Milestone Context

eggfetch follows a milestone-driven development sequence (A through Z). Before starting work, read `plans/ROADMAP.md` and the relevant milestone plan in `plans/`. Each milestone is a handoff boundary: finish one before starting the next. A clean baseline matters more than an early partial implementation.

Make the workspace build green before adding new functionality. Format before committing. Do not bypass CI to land changes.

## CI and branch protection

Actions run on pushes and pull requests to `main`. The required visible checks
are the Rust formatting, Rust clippy, Rust build/tests, and Rust documentation
jobs, plus the Python matrix and wheel-smoke matrix. A
maintainer enabling branch protection should require all three job families,
require branches to be up to date before merge, and disallow force pushes to
`main`. The wheel smoke job is intentionally separate from source-build tests
so packaging regressions cannot hide behind a passing test matrix.
