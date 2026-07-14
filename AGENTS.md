# Agent Guide

This file contains guidance for AI coding agents working in the eggfetch repository.

## Milestone Sequence

eggfetch follows a milestone-driven development sequence: A through Q. Each milestone is a handoff boundary. Before starting work, read `plans/ROADMAP.md` and the relevant milestone plan in `plans/`. The milestones are:

- A: Repository and workspace foundation (complete)
- B: Core request/response model and minimal HTTP engine (complete)
- C: Connection management (complete)
- D: Timeout system (complete)
- E: Streaming foundation (complete)
- F: Python sync API (complete)
- G: Python async API (complete)
- H: Response compatibility surface (complete)
- I: Request builder compatibility surface (complete)
- J: Redirect engine (complete)
- O: Cookie subsystem (complete)
- P: Authentication subsystem (complete)
- Q: Multipart and file uploads (complete)
- R: Response compression and decompression (complete)
- S: Proxy subsystem (complete)
- T: TLS configuration (complete)
- U: Retry and resilience policy (complete)
- V: HTTP/2 (complete)
- N: Semantic tightening and public-API stabilization (complete)
- Validation polish after Milestone N (complete before Q)
- Corrective integration after Milestone S (complete)
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

The validation pass also requires the core feature matrix, Python async
plugin, and wheel smoke path:

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
```

The supported CI matrix is Python 3.10–3.13 on Ubuntu, macOS, and Windows.
CI must install `pytest-asyncio`; `asyncio_mode = "auto"` alone does not
provide the plugin. Wheel checks build a wheel, install it into a clean
environment, and exercise both a local GET and a streaming response.

Streaming lifecycle policy: `StreamingResponse` starts live, a buffered read
owns the body until it reaches `buffered`, iterators transfer ownership to
`consumed`, and explicit/context-manager close is terminal. Closing a response
signals active readers and iterator producers. A response may outlive its
client because the core response owns its body lease; closing the client only
prevents new requests and does not invalidate an already returned response.

The workspace has ~598+ Rust tests and ~453+ Python tests covering construction, streaming, timeouts, pools, headers, integration scenarios, Python sync API, Python async API, response compatibility properties, redirect behavior, redirect replay, total timeout across redirects, sync/async API parity, cookie subsystem (parsing, matching, jar operations, client integration, Python API), authentication subsystem (Basic/Bearer auth, precedence, cross-origin credential stripping, Python auth classes), multipart (encoder, boundary, streaming, known-length, Python files= support), response decompression (gzip, deflate, brotli, zstd streaming decoders, Accept-Encoding negotiation, Content-Encoding parsing, header policy, feature-gated compilation, Python decompress kwarg), decoded-body resource limits (max size, decompression ratio), proxy subsystem (HTTP proxying, HTTPS CONNECT tunneling, proxy authentication, per-request/client proxy configuration, NO_PROXY bypass matching, proxy response parsing bounds (header size limits, line limits)), retry subsystem (policy construction, method safety, status codes, body replayability, backoff computation, Retry-After parsing, budget enforcement, Python Retry class), cross-feature integration tests (multipart through proxy, compressed responses through proxy, redirect/cookie/auth through proxy, cancellation, timeouts), true network streaming (sync/async `client.stream()`, `StreamingResponse`, chunk iteration, cancellation, pool lease lifecycle, split UTF-8, cross-chunk line delimiters, named exception types), and HTTP/2 (HttpVersionPolicy enum, ALPN negotiation, connector construction, h2 error taxonomy (GoAway, StreamReset, FlowControl, Protocol), forbidden header stripping, retry classification for REFUSED_STREAM, trailers documentation, pool concurrency model documentation, Python http2 option, Python h2 exception types).

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
