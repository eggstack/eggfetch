# Dependency Policy

eggfetch follows a conservative dependency policy. Every dependency must have an explicit reason. The project trades breadth of features for correctness, auditability, and a small transitive tree.

## Current Posture

At Milestone A, eggfetch-core has zero external dependencies. The workspace compiles with only the Rust standard library. This is intentional: the skeleton exists to establish crate boundaries and lint configuration before networking code arrives.

## Expected Early Dependencies

When Milestone B begins (core request/response model and minimal HTTP engine), eggfetch-core will gain:

- **bytes** -- efficient byte buffer types for request and response bodies.
- **http** -- standard HTTP types (`Method`, `StatusCode`, `HeaderMap`, `Uri`).
- **url** -- URI parsing and query string serialization.
- **thiserror** or a handwritten error type -- ergonomic error definitions.

These are small, well-audited crates with minimal transitive trees. They are the minimum required to model HTTP requests and responses correctly.

## Expected Milestone B/C Dependencies

When connection management and the HTTP engine land:

- **tokio** -- async runtime. Required by hyper and the core async model.
- **hyper** -- HTTP/1.1 and HTTP/2 protocol implementation.
- **hyper-util** -- high-level client utilities built on hyper.
- **http-body-util** -- body combinators for http-body.
- **rustls** -- TLS implementation. Preferred over native TLS for portability and auditability.
- **tokio-rustls** -- async TLS integration for tokio + rustls.

These are the standard building blocks for a Rust HTTP client. They are widely used, actively maintained, and have small dependency trees relative to their functionality.

## Optional Later Dependencies

Features that are not core to HTTP/1.1 client behavior are optional and feature-gated:

- **pyo3**, **pyo3-async-runtimes** -- Python bindings (eggfetch-python crate only).
- **clap** -- CLI argument parsing (eggfetch-cli crate only).
- **serde**, **serde_json** -- JSON serialization (behind the `json` feature).
- **compression crates** -- gzip, brotli, zstd (behind `compression-*` features).
- **cookie** -- cookie jar support (behind the `cookies` feature).
- **tracing** -- structured logging (behind the `tracing` feature).

These dependencies stay optional. They do not enter `default` features without discussion.

## Selection Criteria

When evaluating a new dependency:

1. **Explicit reason required.** Every dependency must solve a real problem. "It might be useful" is not a reason.
2. **Prefer Rustls over native TLS.** Rustls is memory-safe, portable, and has a smaller audit surface than OpenSSL or platform-native TLS.
3. **Minimize transitive trees.** A convenience crate that pulls in 30 transitive dependencies needs a strong justification. Prefer direct, focused crates.
4. **Avoid proc-macro-heavy crates.** Proc macros slow compilation and expand the attack surface. Use them only when they materially improve correctness or maintainability (e.g., thiserror for error definitions).
5. **Keep features non-default.** Optional behavior behind feature flags. Users pay only for what they use.

## Audit Tools

The project plans to use:

- **cargo-deny** for license compliance, advisory database checks, and duplicate dependency detection.
- **cargo-audit** for known vulnerability scanning against the RustSec advisory database.

These tools are not yet wired into CI. Adding them is part of the pre-release hardening work. Every dependency in the tree must have an explicit reason documented in code or review.
