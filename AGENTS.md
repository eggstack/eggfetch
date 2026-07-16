# Agent Guide

This file contains guidance for AI coding agents working in the eggfetch repository.

## Milestone Sequence

eggfetch follows a milestone-driven development sequence: A through Y. Each milestone is a handoff boundary. Before starting work, read `plans/ROADMAP.md` and the relevant milestone plan in `plans/`. The milestones are:

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
- W: HTTP/3 (complete, experimental)
- N: Semantic tightening and public-API stabilization (complete)
- Validation polish after Milestone N (complete before Q)
- Corrective integration after Milestone S (complete)
- K: CLI (complete)
- Y: Documentation and examples (complete)
- L: Correctness and differential testing
- M: Documentation and public MVP preparation
- Z: Additional bindings and frameworks (complete)

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

User-facing documentation lives in `docs/`:

| Directory | Content |
|-----------|---------|
| `docs/getting-started/` | Installation and quickstart |
| `docs/concepts/` | Core concept explanations |
| `docs/rust/` | Rust API guide |
| `docs/python/` | Python API guide |
| `docs/cli/` | CLI reference |
| `docs/migration/` | Migration guides (requests, HTTPX) |
| `docs/cookbook/` | Practical examples |
| `docs/reference/` | Compatibility matrix, feature matrix, errors |
| `docs/security/` | Security guidelines and troubleshooting |
| `docs/architecture/` | Internal architecture documentation |
| `docs/ffi/` | C ABI guide, architecture, and surface audit |
| `docs/releases/` | Release process and compatibility policy |

When adding new features, update the relevant docs alongside the code changes.

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
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
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

The workspace has ~750+ Rust tests, ~463+ Python tests, and ~40+ Node.js/FFI tests covering construction, streaming, timeouts, pools, headers, integration scenarios, Python sync API, Python async API, response compatibility properties, redirect behavior, redirect replay, total timeout across redirects, sync/async API parity, cookie subsystem (parsing, matching, jar operations, client integration, Python API), authentication subsystem (Basic/Bearer auth, precedence, cross-origin credential stripping, Python auth classes), multipart (encoder, boundary, streaming, known-length, Python files= support), response decompression (gzip, deflate, brotli, zstd streaming decoders, Accept-Encoding negotiation, Content-Encoding parsing, header policy, feature-gated compilation, Python decompress kwarg), decoded-body resource limits (max size, decompression ratio), proxy subsystem (HTTP proxying, HTTPS CONNECT tunneling, proxy authentication, per-request/client proxy configuration, NO_PROXY bypass matching, proxy response parsing bounds (header size limits, line limits)), retry subsystem (policy construction, method safety, status codes, body replayability, backoff computation, Retry-After parsing, budget enforcement, Python Retry class), cross-feature integration tests (multipart through proxy, compressed responses through proxy, redirect/cookie/auth through proxy, cancellation, timeouts), true network streaming (sync/async `client.stream()`, `StreamingResponse`, chunk iteration, cancellation, pool lease lifecycle, split UTF-8, cross-chunk line delimiters, named exception types), HTTP/2 (HttpVersionPolicy enum, ALPN negotiation, connector construction, h2 error taxonomy (GoAway, StreamReset, FlowControl, Protocol), forbidden header stripping, retry classification for REFUSED_STREAM, trailers documentation, pool concurrency model documentation, Python http2 option, Python h2 exception types), FFI (ClientBuilder configuration, per-request overrides, streaming body, cancellation, error management, null safety, local server integration, streaming chunked responses), CLI (19 unit tests for argument parsing, method detection, exit codes, header/query/form/file parsing, version strings, secret redaction, base64 encoding, follow logic, and 56 subprocess integration tests using a local TCP test server covering GET/POST/JSON/form/body-file, headers, query params, JSON/NDJSON output, base64 binary body, include/headers-only/no-body, output files, no-clobber, download mode, basic/bearer auth, env var auth, redirect follow/no-follow, connect timeout, max redirects, status code checking, chunked responses, large downloads, shell completions, form urlencoded, file upload, cookies, verbose output, JSON errors field, max body size, and error exit codes), and FFI (11 tests for C ABI handle lifecycle, null safety, request building, response handling, error management, convenience methods, string management, and a local server integration test), redaction (centralized secret redaction, header sanitization, URL sanitization, regression tests).

## Benchmarks

The benchmark suite lives in `crates/eggfetch-bench/` and uses Criterion. Run with:

```sh
cargo bench -p eggfetch-bench                     # all suites
cargo bench -p eggfetch-bench --bench microbench   # microbenchmarks only
cargo bench -p eggfetch-bench --bench e2e          # end-to-end only
cargo bench -p eggfetch-bench --bench resources    # resource tests only
```

## Fuzzing and Property Testing

The fuzzing and property testing infrastructure targets high-risk parsers, state machines, and codec boundaries.

**cargo-fuzz targets** live in `fuzz/fuzz_targets/`. Each target exercises a specific subsystem with structured random input via libFuzzer:

| Target | Subsystem |
|--------|-----------|
| `fuzz_headers` | Header parsing and validation |
| `fuzz_cookie` | Cookie parsing, matching, and jar operations |
| `fuzz_redirect` | Redirect policy and replay logic |
| `fuzz_multipart` | Multipart encoder boundary and streaming |
| `fuzz_compression` | Gzip, deflate, brotli, zstd decompression |
| `fuzz_proxy` | Proxy configuration and NO_PROXY matching |
| `fuzz_proxy_response` | Proxy CONNECT response parsing from raw bytes |
| `fuzz_timeout` | Timeout state machine and scheduling |
| `fuzz_retry` | Retry policy, backoff, and Retry-After parsing |
| `fuzz_tls` | TLS configuration and SNI handling |
| `fuzz_url` | URL parsing and normalization |

**Proptest property tests** live colocated in `eggfetch-core` modules. They verify round-trip invariants (parse then serialize produces equivalent output) and state-machine correctness for cookies, redirects, retries, and decompression.

Run fuzz targets:

```sh
cd fuzz && cargo +nightly fuzz run <target>
```

Build all fuzz targets:

```sh
cd fuzz && cargo +nightly fuzz build
```

Run property tests:

```sh
cargo test -p eggfetch-core --all-features
```

Nightly Rust is required for cargo-fuzz (sanitizer coverage flags). Property tests run on stable.

## Security

The project enforces security through CI automation and code conventions:

- `deny.toml` configures `cargo-deny` for advisory, license, ban, and source checks.
- `.github/workflows/security.yml` runs `cargo-deny` and `cargo-audit` on every push and PR.
- `.github/workflows/ci.yml` runs the full test matrix, clippy, and fmt checks.
- All `Debug`/`Display`/error output must redact secrets. The centralized `redact` module (`eggfetch_core::redact`) provides `redact_headers()`, `redact_url()`, and `SENSITIVE_HEADERS`.
- The `unsafe_code` lint is `forbid` workspace-wide (eggfetch-ffi is the sole exception).
- See `SECURITY.md` for vulnerability reporting. See `docs/architecture/threat-model.md` for the threat model.
- See `docs/architecture/release-security-checklist.md` for the pre-release checklist.
- See `docs/architecture/incident-runbook.md` for the incident response process.

## Release Engineering

eggfetch uses coordinated versioning across all publishable crates (core, CLI, Python, FFI, Node). All crates share the same version number. The bench and fuzz crates are not published.

Release workflow:
- Triggered by version tags (`v*`) or manual dispatch
- Runs full CI matrix before any publishing
- Builds Python wheels for Linux (x86_64/aarch64), macOS (x86_64/aarch64), Windows (x86_64) across Python 3.10–3.13
- Builds CLI binaries for supported platforms with checksums
- Smoke-tests all artifacts before publishing
- Requires environment approval for crates.io and PyPI publishing
- Creates GitHub Release with binaries and release notes
- Runs post-release install verification

Publishing order: eggfetch-core → eggfetch-cli → eggfetch-ffi → eggfetch-python → eggfetch-node (crates.io index propagation requires waits between publishes).

See `docs/releases/process.md` for the full release process and `docs/releases/compatibility-policy.md` for versioning and compatibility policies.

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
