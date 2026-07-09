# Dependency Policy

eggfetch follows a conservative dependency policy. Every dependency must have an explicit reason. The project trades breadth of features for correctness, auditability, and a small transitive tree.

## Current Posture

At Milestone B, eggfetch-core has the following direct dependencies:

- **bytes** -- efficient byte buffer types for request and response bodies.
- **http** -- standard HTTP types (`Method`, `StatusCode`, `HeaderMap`, `Uri`).
- **http-body** -- body trait abstraction.
- **http-body-util** -- body combinators for http-body.
- **hyper** -- HTTP/1.1 protocol implementation.
- **hyper-util** -- high-level client utilities built on hyper.
- **hyper-rustls** -- TLS integration via rustls.
- **rustls** -- memory-safe TLS implementation.
- **tokio** -- async runtime.
- **tokio-rustls** -- async TLS streams for tokio + rustls.
- **url** -- URI parsing and query string serialization.
- **thiserror** -- ergonomic error definitions.

These are small, well-audited crates with minimal transitive trees. They are the minimum required to build a working HTTPS client.

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
