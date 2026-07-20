# Release Security Checklist

This checklist must be completed before any release of eggfetch. Each item has a clear pass/fail criterion. A release must not proceed until all items pass.

## CI and Tooling

- [ ] All CI checks pass (test suite, clippy, fmt)
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` produces zero warnings
- [ ] `cargo fmt --all` produces no changes (all code is formatted)
- [ ] `cargo test --workspace --all-features` passes with no failures
- [ ] `Required CI Gate` job passes with `if: always()` and fail-closed evaluation
- [ ] `scripts/evaluate_ci_gate.py` tests pass: `python -m pytest scripts/test_evaluate_ci_gate.py -v`

## Dependency Audit

- [ ] `cargo-deny` passes with no advisory violations
- [ ] `cargo-audit` reports no known vulnerabilities in the dependency tree
- [ ] No new dependencies have been added without explicit documentation in `docs/architecture/dependency-policy.md`
- [ ] License audit passes (no GPL-incompatible licenses in the dependency tree)

## Fuzz and Property Testing

- [ ] No high-severity findings from fuzz targets (review `fuzz/artifacts/` for new crash inputs)
- [ ] All fuzz targets build successfully: `cd fuzz && cargo +nightly fuzz build`
- [ ] Property tests pass: `cargo test -p eggfetch-core --all-features`
- [ ] New fuzz targets added for any new parsing or state-machine code

## Secret Redaction

- [ ] Secret redaction tests pass for all output paths (Rust Debug/Display, Python repr, CLI output)
- [ ] `BasicAuth` and `BearerAuth` Debug/Display show `<redacted>` for credentials
- [ ] `ProxyAuth` Debug/Display redacts the password
- [ ] `Response` debug output replaces `Authorization`, `Proxy-Authorization`, `Cookie`, and `Set-Cookie` values with `<redacted>`
- [ ] URL debug output strips userinfo, query strings, and fragments
- [ ] Python `Client.__repr__` prints `[UNSAFE: TLS verification disabled]` when `verify=False`
- [ ] CLI secret redaction tests pass (no credentials in output, error messages, or verbose logging)

## TLS Configuration

- [ ] TLS configuration review completed (trust store hierarchy, verification toggle, version policy)
- [ ] `danger_accept_invalid_certs(true)` requires explicit opt-in (no default or config-file bypass)
- [ ] Custom CA bundle correctly replaces all default roots (native + packaged)
- [ ] Client certificate private key material excluded from debug output, error messages, and repr
- [ ] Encrypted PEM private keys rejected at construction time
- [ ] TLS 1.2 minimum enforced (no SSLv3, TLS 1.0, TLS 1.1 support)

## Redirect and Authentication

- [ ] Cross-origin redirect stripping tests pass (`Authorization`, `Proxy-Authorization`, `Cookie`, `Set-Cookie` stripped)
- [ ] Same-origin redirect header preservation tests pass
- [ ] Port changes correctly treated as cross-origin
- [ ] Client-level auth not reapplied on cross-origin redirects
- [ ] URL credential rejection tests pass (userinfo in URL produces error)
- [ ] Redirect history is metadata-only (no body content stored)

## Proxy Security

- [ ] Proxy authentication boundary tests pass (`Proxy-Authorization` not forwarded to destination)
- [ ] CONNECT tunnel treated as transparent byte stream (no TLS interception)
- [ ] Proxy credential URL rejection tests pass (redacted error on `http://user:pass@proxy/`)
- [ ] NO_PROXY matching tests pass (exact, domain suffix, wildcard, port-specific)
- [ ] Proxy response parsing bounds tests pass (header size limits, line limits)

## Decompression and Body Limits

- [ ] Decompression resource limits tested (`max_decoded_body_size`, `max_decompression_ratio`)
- [ ] Limits enforced during streaming (not just on buffered reads)
- [ ] Multipart path safety tests pass (basename-only filenames, no directory traversal)
- [ ] Multipart boundary validation tests pass (no CR/LF injection)
- [ ] Content-Length handling tests pass (server-trusted, no double-counting)

## Retry and Resilience

- [ ] Retry amplification tests pass (deadline not extended across retries)
- [ ] Idempotent-only default retry policy tests pass (GET, HEAD, OPTIONS, PUT, DELETE)
- [ ] Body replayability checks pass (streaming bodies not retried)
- [ ] Retry budget enforcement tests pass (max retries, backoff)

## Python Bindings

- [ ] Python binding repr/redaction tests pass
- [ ] Python sync/async API parity tests pass
- [ ] Python `files=` multipart path safety tests pass
- [ ] Python `verify=False` repr warning tests pass
- [ ] Python `NOAUTH` sentinel tests pass
- [ ] Python GIL release during streaming tests pass

## CLI

- [ ] CLI secret redaction tests pass
- [ ] CLI exit code tests pass (correct codes for each error class)
- [ ] CLI `--no-verify` flag correctly disables TLS verification
- [ ] CLI `--proxy` correctly configures proxy
- [ ] CLI `--auth` and `--bearer` correctly configure authentication
- [ ] CLI `--download` filename derivation tests pass (Content-Disposition, path, deduplication)

## Documentation

- [ ] Changelog updated with security-relevant changes
- [ ] `SECURITY.md` updated with new supported version (if applicable)
- [ ] `docs/security/guidelines.md` updated with any new security-relevant behavior
- [ ] `docs/architecture/threat-model.md` reflects current attack surface
- [ ] `docs/architecture/security-reviews.md` updated for any new subsystems

## Repository Hygiene

- [ ] No secrets or keys committed to repository (check `git log --all --diff-filter=A` for sensitive files)
- [ ] `.gitignore` covers private keys, certificates, and credential files
- [ ] No hardcoded test credentials in source code (use environment variables or test fixtures)
- [ ] SBOM generated (if tooling available): `cargo deny list --format json > sbom.json`

## Release Process

- [ ] Release branch created from main
- [ ] Version bumped in `Cargo.toml` files
- [ ] Release notes drafted with security-relevant changes highlighted
- [ ] Advisory drafted (if applicable) with affected versions, fixed version, severity, and mitigation
- [ ] CVE/GHSA requested (if applicable) via GitHub Security Advisory
- [ ] Reporter notified (if applicable) with 7-day advance notice before public disclosure
- [ ] Dry-run workflow_dispatch with `candidate_sha` and `dry_run=true` completes successfully
- [ ] `verify-no-side-effects` job confirms no publishing or repository mutations in dry run
- [ ] Evidence manifest (`release-evidence.json`) generated and reports overall pass

## Release Artifacts

- [ ] Python wheels built for all declared platforms and Python versions
- [ ] CLI binaries built for all declared platforms
- [ ] SHA256 checksums generated for all binary artifacts
- [ ] Wheel and CLI artifacts smoke-tested in clean environments
- [ ] crates.io packages pass `cargo package --list` verification
- [ ] GitHub Release created with release notes from CHANGELOG
- [ ] Post-release install tests pass from crates.io and PyPI
