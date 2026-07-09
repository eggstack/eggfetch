# Milestone A Plan: Repository and Workspace Foundation

## Objective

Establish the eggfetch repository as a clean, auditable Rust workspace prepared for a long-lived HTTP client project. This milestone should create the structural foundation without prematurely implementing the networking engine.

The output should be a repository that is easy for future agents and maintainers to navigate, validate, lint, document, and extend.

## Scope

Milestone A includes:

- Cargo workspace creation
- crate boundary decisions
- baseline package metadata
- feature flag policy
- MSRV policy
- formatting and linting configuration
- CI workflow
- dependency audit posture
- initial README
- initial architecture documentation
- contribution and security notes

Milestone A does not include:

- full HTTP request execution
- Python bindings
- CLI behavior
- connection pooling
- timeout implementation
- streaming implementation

## Proposed workspace layout

Create the following structure:

```text
Cargo.toml
README.md
LICENSE
SECURITY.md
CONTRIBUTING.md
rust-toolchain.toml
rustfmt.toml
.clippy.toml
.github/workflows/ci.yml
crates/
  eggfetch-core/
    Cargo.toml
    src/
      lib.rs
      error.rs
      client.rs
      request.rs
      response.rs
      body.rs
      headers.rs
      timeout.rs
      config.rs
  eggfetch-cli/
    Cargo.toml
    src/
      main.rs
  eggfetch-python/
    Cargo.toml
    pyproject.toml
    src/
      lib.rs
plans/
```

The crate names should remain stable unless there is a strong reason to rename them later.

## Root Cargo workspace

Create a root `Cargo.toml` using resolver version 2.

Recommended shape:

```toml
[workspace]
resolver = "2"
members = [
    "crates/eggfetch-core",
    "crates/eggfetch-cli",
    "crates/eggfetch-python",
]

[workspace.package]
edition = "2021"
rust-version = "1.80"
license = "MIT OR Apache-2.0"
repository = "https://github.com/eggstack/eggfetch"
readme = "README.md"

[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"
must_use_candidate = "allow"
```

The exact MSRV can be adjusted, but it should be explicit. Prefer a stable compiler version that comfortably supports the selected dependency versions.

## Crate responsibilities

### eggfetch-core

`eggfetch-core` is the only crate that owns HTTP behavior.

It should eventually contain:

- async client
- request builder
- response type
- body model
- timeout model
- connection management
- TLS integration
- redirect engine
- cookie handling
- proxy handling
- error taxonomy

It must not depend on PyO3 or CLI argument parsing.

### eggfetch-cli

`eggfetch-cli` is a thin binary around `eggfetch-core`.

It should eventually contain:

- argument parsing
- terminal output formatting
- exit code mapping
- body/header display
- machine-readable output

It must not contain independent HTTP behavior.

### eggfetch-python

`eggfetch-python` exposes Python bindings over `eggfetch-core`.

It should eventually contain:

- PyO3 class wrappers
- sync client runtime adapter
- asyncio integration
- Python exception mapping
- Python package metadata

It must not duplicate request execution logic.

## Dependency policy

Start with very small dependencies. The repository may initially compile with almost no external crates beyond workspace scaffolding. When networking begins in Milestone B, use dependencies deliberately.

Expected early dependencies:

- `bytes`
- `http`
- `url`
- `thiserror` or a handwritten error type

Expected Milestone B/C dependencies:

- `tokio`
- `hyper`
- `hyper-util`
- `http-body-util`
- `rustls`
- `tokio-rustls`

Expected optional later dependencies:

- `pyo3`
- `pyo3-async-runtimes` or equivalent adapter if justified
- `clap`
- `serde`
- `serde_json`
- compression crates
- cookie crate
- tracing crates

Dependency rules:

- Every dependency needs an explicit reason.
- Prefer Rustls over native TLS for auditability and portability.
- Keep optional features out of default features unless they are essential.
- Avoid large transitive trees for convenience features.
- Avoid proc-macro heavy dependencies unless they materially improve correctness or maintainability.

## Feature flag policy

Define feature intent early, even if not all features exist yet.

Potential feature layout for `eggfetch-core`:

```toml
[features]
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

Avoid enabling advanced features by default before the core is stable.

## CI plan

Create `.github/workflows/ci.yml` with jobs for:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo doc --workspace --all-features --no-deps`

If the repository is initially skeletal, CI should still run and pass.

Later hardening can add:

- cargo-deny
- cargo-audit
- MSRV check
- Python package build
- maturin build
- CLI smoke test

## Initial Rust modules

Create stub modules in `eggfetch-core` so the architecture is visible:

```rust
pub mod body;
pub mod client;
pub mod config;
pub mod error;
pub mod headers;
pub mod request;
pub mod response;
pub mod timeout;

pub use client::Client;
pub use error::{Error, Result};
pub use request::{Method, Request, RequestBuilder};
pub use response::Response;
```

Each module should have placeholder types with doc comments. These should compile but do minimal work.

## Initial API sketch

The initial Rust API should point toward this shape:

```rust
let client = eggfetch_core::Client::new();
let response = client.get("https://example.com").send().await?;
```

Do not overfit Python semantics into the Rust API. Rust should remain idiomatic.

## Documentation tasks

### README.md

The README should include:

- project purpose
- current status: early development
- architecture summary
- non-goals for MVP
- example target APIs
- repository layout
- development commands

### SECURITY.md

Include:

- how to report vulnerabilities
- supported versions policy placeholder
- dependency audit posture
- statement that networking/TLS code is security-sensitive

### CONTRIBUTING.md

Include:

- formatting/linting expectations
- dependency policy
- testing expectations
- compatibility expectations
- no duplicate sync networking implementation rule

## Acceptance criteria

Milestone A is complete when:

- the workspace builds with `cargo check --workspace`
- formatting passes
- clippy passes
- tests pass, even if only skeletal
- docs build
- root README explains the project direction
- crate boundaries are clear
- dependency and feature policies are documented
- CI exists and is expected to pass

## Risks

The main risk is prematurely adding implementation detail before the architecture settles. Keep this pass structural and conservative.

Another risk is exposing Python or CLI constraints too early inside `eggfetch-core`. Prevent this by keeping `eggfetch-core` independent of PyO3 and CLI dependencies.

## Handoff notes

The next implementer should start Milestone B only after the skeletal workspace is green. If there is friction in CI, rust-toolchain, lints, or docs, fix that first. A clean baseline matters more than an early partial HTTP request.
