# Security Policy

## Reporting Vulnerabilities

If you discover a security vulnerability in eggfetch, please report it privately. Do **not** open a public GitHub issue.

Email: **dbowman91@proton.me**

PGP encryption is preferred but not required. Please include:

- A description of the vulnerability and its potential impact.
- Steps to reproduce or a proof-of-concept.
- The affected component (e.g. core crate, Python bindings, CLI).
- Any suggested remediation, if available.

You will receive an acknowledgement within **48 hours** of your report. If you do not receive an acknowledgement within that window, follow up on a non-security GitHub issue referencing your original report.

Please do not disclose the vulnerability publicly until a fix has been released and you have been notified.

## Supported Versions

Only the most recent tagged release is considered supported. No backports or point releases are planned until a formal release is cut. Once tagged releases begin, only the most recent minor release line will receive security updates.

## Security Update Process

Security fixes land on `main` as a single, scoped commit that includes:

1. The fix itself.
2. A regression test that exercises the vulnerable code path.
3. A changelog entry under a `Security` section.

After the fix is merged, a new release (or release candidate) is tagged promptly. The fix will be included in the next scheduled release if one is imminent; otherwise an out-of-cycle release is cut.

Versioning follows SemVer. Security patches that do not change the public API will bump the patch component. Fixes that alter public API semantics will bump the minor or major component as appropriate.

## Vulnerability Response SLA

| Severity | Initial Response | Fix Target |
|----------|-----------------|------------|
| Critical | 24 hours | 7 days |
| High | 48 hours | 14 days |
| Medium | 1 week | 30 days |
| Low | 2 weeks | 90 days |

Severity is assessed using CVSS v3.1 where applicable, with adjustments for the context of an HTTP client library (e.g. a TLS bypass is Critical; a denial-of-service in debug logging is Low).

If a fix cannot meet the target, the maintainers will communicate a revised timeline to the reporter before the deadline expires.

## Embargo Policy

Vulnerabilities are treated as confidential until a fix is released. The standard embargo period is **90 days** from the date of the initial report, after which the maintainer may disclose the vulnerability publicly.

The reporter will receive **7 days' advance notice** before any public disclosure. If the 90-day deadline passes without a release, the maintainer will coordinate disclosure with the reporter regardless.

## CVE / GHSA Process

When a security fix is released, the maintainer will:

1. Create a **GitHub Security Advisory** via the repository's Security Advisories feature.
2. Assign a severity rating and affected version range.
3. Document the impact, affected components, and recommended mitigation.
4. Request a **CVE identifier** through GitHub's CVE Numbering Authority (CNA) process or MITRE if GitHub CNA coverage does not apply.

The advisory will be published at the same time as (or shortly after) the fix commit lands on `main`.

## Dependency Audit

`cargo-deny` and `cargo-audit` are wired into CI and run on every push. The advisory database is checked against all workspace dependencies, including transitive ones. Deny rules enforce:

- No known vulnerable crates (RUSTSEC advisories).
- No unmaintained or yanked crates without an explicit exception.
- License compliance for the workspace.

If a new advisory affects a dependency, the maintainer will either upgrade the dependency, patch it, or remove the affected functionality. Temporary exceptions require a documented justification and a tracked issue for resolution.

## Networking and TLS

eggfetch is an HTTP client engine. The core crate performs TLS negotiation, DNS resolution, and parsing of untrusted network input. A vulnerability in this code path can compromise confidentiality, integrity, or availability of connections made through the library.

The project **prefers Rustls** over native TLS (OpenSSL, Secure Transport, SChannel) for portability and because Rustls has a smaller, memory-safe attack surface. All TLS, DNS, and body-parsing code is subject to review before release. TLS configuration (cipher suites, protocol versions, certificate validation) is reviewed periodically and restricted to secure defaults.

## Secret Redaction

All `Debug`, `Display`, error, and log output from eggfetch redacts sensitive credentials. This includes:

- HTTP `Authorization` headers (Basic, Bearer, and other schemes).
- HTTP `Cookie` headers.
- Proxy authentication credentials.
- URLs with embedded userinfo.

Credential redaction is enforced by regression tests in the core crate. Any new code path that formats or logs request/response data must include redaction and a corresponding test case.

## MSRV and Supply Chain

The minimum supported Rust version (MSRV) is **1.80**. This is a conservative pin that avoids pulling in unstable compiler features and reduces the attack surface of the build toolchain.

The toolchain is pinned via `rust-toolchain.toml` on the stable channel. CI uses the same pinned version to ensure reproducible builds.

Dependency verification:

- Lockfile (`Cargo.lock`) is committed and enforced in CI.
- Dependencies are fetched from crates.io; no vendored dependencies at this stage.
- `cargo-deny` enforces advisory checks, license restrictions, and source allowlists.

## Security Hardening

The following security measures are active in the eggfetch repository:

- **cargo-deny** in CI: advisory database checks, license enforcement, dependency source allowlists.
- **cargo-audit** in CI: RUSTSEC advisory scanning on every push.
- **CI security checks**: lint, typecheck, and test suite run on pushes and pull requests.
- **Threat model**: documented in `docs/security/`, covers the five trust boundaries (local app, eggfetch core, remote server, network, dependency ecosystem).
- **Security reviews**: TLS configuration, redirect/auth/cookie handling, proxy tunneling, body streaming, retry policy, Python bindings, and CLI are reviewed for misuse and injection vectors.
- **Credential redaction**: regression-tested across Debug, Display, error, and log output.
- **Safe Rust only**: `unsafe_code` is set to `forbid` workspace-wide; no unsafe code is permitted without explicit discussion and justification.

## Scope of Security Reviews

Security reviews cover the following areas:

- **TLS and certificate handling**: Rustls integration, certificate validation, SNI, ALPN negotiation, cipher suite selection.
- **Redirect, authentication, and cookie handling**: cross-origin credential stripping, redirect loops, cookie jar integrity, SameSite and Secure attributes.
- **Proxy subsystem**: HTTP proxying, HTTPS CONNECT tunneling, proxy authentication, NO_PROXY bypass rules, proxy response parsing bounds.
- **Body handling**: streaming decompression, Content-Length enforcement, chunked transfer decoding, decoded-body resource limits.
- **Retry and resilience**: method safety classification, status code triggers, backoff computation, Retry-After parsing, budget enforcement.
- **Python bindings (FFI)**: GIL handling, pointer safety, string encoding, error propagation, null pointer safety.
- **CLI**: argument parsing, secret redaction, base64 encoding, file upload handling, shell injection vectors.

## Contact

For security reports, contact: **dbowman91@proton.me**
