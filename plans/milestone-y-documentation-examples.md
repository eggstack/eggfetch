# Milestone Y Plan: Documentation and Examples

## Objective

Create complete, versioned documentation for Rust, Python, and CLI users, with a tested compatibility matrix and practical examples. Documentation must describe actual behavior, limits, and security properties rather than aspirational parity.

## Scope

Produce:

- architecture and request-lifecycle docs
- Rust API guide and examples
- Python sync/async guide
- CLI reference
- timeout, streaming, redirects, cookies, auth, proxy, TLS, retry, compression, multipart, and protocol docs
- migration guides from requests and HTTPX
- feature/compatibility matrix
- cookbook and runnable examples
- troubleshooting and security guidance
- generated API documentation integration

## Information architecture

Suggested structure:

```text
docs/
  getting-started/
  python/
  rust/
  cli/
  concepts/
  cookbook/
  migration/
  reference/
  security/
```

Use one documentation generator only if it materially improves maintenance. Markdown checked in-tree remains the source of truth.

## Core concept docs

Required deep explanations:

- single async Rust engine and Python adapters
- client/request lifecycle and pipeline order
- pooling versus logical permits and protocol-specific behavior
- timeout scopes and deadlines
- body replayability
- buffered versus streamed APIs
- redirect credential/cookie policy
- decompression and decoded-body limits
- proxy transport limitations/reuse
- TLS trust and unsafe modes
- retry safety and idempotency

## Compatibility matrix

Track requests/HTTPX-inspired features as:

- supported and tested
- partially supported with documented differences
- planned
- intentionally unsupported

Include sync/async parity and Rust/Python/CLI coverage. Generate or validate this table from tests/config where practical to reduce drift.

## Migration guides

Provide focused examples:

- requests `Session` to `eggfetch.Client`
- HTTPX `Client`/`AsyncClient`
- timeouts
- streaming
- files/forms
- cookies/auth
- proxies/TLS
- exception mapping

Do not claim drop-in replacement. Call out differences explicitly.

## Cookbook examples

Runnable examples should cover:

- JSON REST calls
- large streaming download
- streaming upload and multipart file
- SSE-like line stream
- cookie session/login
- Basic/Bearer auth
- custom CA and mTLS
- proxy and NO_PROXY
- retry policy
- gzip/brotli/zstd responses
- concurrent async requests
- CLI scripting and NDJSON

Avoid examples requiring secrets or paid APIs. Third-party API examples should use environment variables and mocked alternatives where possible.

## Testing docs

Add doctests for Rust examples and a lightweight examples CI job. Python examples should be executable against local fixtures or syntax-checked. CLI examples should be covered by integration tests where feasible.

Add link checking and documentation build checks.

## API reference

Ensure rustdoc coverage for public Rust items. Python classes/functions need docstrings and generated reference pages if a Python doc tool is used. CLI `--help` is authoritative and may generate reference snippets/man pages.

## Security and troubleshooting

Document:

- verification disable risks
- credential visibility in command-line args
- redirect stripping
- decompression/resource limits
- proxy environment policy
- unsupported async backends
- common certificate/proxy/timeout errors

## Versioning

Documentation should be tied to releases/tags. Main-branch docs must be labeled development if published continuously.

## Acceptance criteria

- all public features have user-facing docs
- compatibility differences are explicit
- examples are runnable/tested
- Rust/Python/CLI references are complete
- link/doc checks run in CI
- documentation matches the release version and security policy
