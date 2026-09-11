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

The workspace sets `unsafe_code = "forbid"`. The sole exceptions are the
`eggfetch-ffi` and `eggfetch-node` crates, which override it to `"allow"`
for their checked FFI/N-API boundaries. Do not add new `unsafe` without
explicit discussion and a strong justification.

## Documentation

The workspace sets `missing_docs = "warn"`. Public items (structs, enums, traits, functions, modules) should have doc comments. This is enforced by both the lint and `.clippy.toml` (`missing-docs-in-crate-items = true`).

For new public types, write a brief doc comment that describes the type's purpose and behavior. Reference the relevant architecture doc if applicable:

```rust
/// Phase-aware timeout configuration for HTTP requests.
///
/// See `docs/architecture/core-timeout-pool.md` for implementation details.
```

The goal is to make the gap between current state and final implementation obvious to future readers.

## Testing

Tests live next to the code they cover, using `#[cfg(test)] mod tests` blocks within the same file. Integration tests live in `crates/eggfetch-core/tests/` (loopback fixtures only, no public internet). Run the full suite single-threaded (resource-stabilization tests measure process RSS and go flaky under concurrency):

```sh
cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1
```

Prefer small, focused tests that exercise one behavior. Test counts change
with every commit; the live qualification evidence (Rust/Python/compat/FFI
counts for the qualified SHA) is recorded in
`plans/httpx-parity-correction-status.md`. As the project grows, tests should cover protocol correctness, edge cases, and error paths.

### Python tests

Python tests require an active venv (Python 3.10+, maturin, pytest, pytest-asyncio) and a fresh extension build — stale `.so` files cause confusing failures:

```sh
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop -m crates/eggfetch-python/Cargo.toml
python -m pytest crates/eggfetch-python/tests/ -q --ignore=crates/eggfetch-python/tests/compat
```

Run differential tests against the pinned HTTPX references (Tier 2):

```sh
EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
```

### Validation pass

The full validation pass is `./scripts/check.sh` (Tier 1) plus `./scripts/check.sh extended` (Tier 2) before release. Tier 2 runs the feature-gated subsets from `scripts/check.sh` (`tier2_feature_matrix` + `tier2_feature_tests`); see `docs/architecture/feature-flags.md` for the exact matrix. Manual extras (not Tier 2 gates):

```sh
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

Current `eggfetch-core` feature declarations (see `crates/eggfetch-core/Cargo.toml` for the exact dependency mapping):

```toml
default = ["http1", "tls-rustls"]
# http1/http2/http3, tls-rustls, json (reserved), compression-gzip/brotli/zstd/deflate,
# cookies, proxy, multipart, tracing (optional), test-util (internal)
```

`cookies`, `proxy`, and `multipart` are opt-in in core. CLI enables cookies/multipart/proxy; Python enables http2/http3/cookies/multipart/proxy plus all compressions. `http3` is experimental. `json` is reserved (Python delivers JSON via `json.dumps()`); `tracing` is an optional structured-logging gate; `test-util` enables `tokio/test-util` for deterministic time testing.
See `docs/architecture/feature-flags.md` for details.

## Compatibility Expectations

The Rust API stays idiomatic. Do not shape the Rust API to mirror Python conventions. The Rust `Client` should feel like a natural async Rust HTTP client, not a port of `httpx`.

The Python sync API must block on the async Rust engine and release the GIL during blocking operations. The Python async API targets asyncio only. Trio/AnyIO remain out of scope (see `docs/reference/compatibility.md`).

## No Duplicate Networking

All network I/O goes through eggfetch-core. There must not be a second synchronous networking implementation in the Python crate, the CLI crate, or anywhere else. Synchronous Python adapters block on the async engine. This is a hard architectural invariant.

If you find yourself writing HTTP logic outside of eggfetch-core, stop and refactor.

## Working Context

All milestones (A through Z) are complete. The workspace is in production-maintenance mode. Before starting work, read `plans/ROADMAP.md` for the full project history and any planned future work. Make the workspace build green before adding new functionality. Run `./scripts/check.sh` before committing. See `docs/architecture/overview.md` for the crate layout and architecture deep-dive index.

## CI

Actions run on pushes and pull requests to `main` as a fast regression safety net. CI repeats the same `./scripts/check.sh` command on Ubuntu. It is not a release authority. Verify locally before committing.
