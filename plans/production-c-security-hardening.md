# Production Track C Plan: Security Hardening

## Objective

Establish a formal security-hardening program for dependencies, TLS, redirects, auth, cookies, proxies, decompression, multipart, retries, protocol handling, Python bindings, CLI, and release artifacts.

## Scope

Implement:

- cargo-audit and cargo-deny
- dependency allow/deny and license policy
- secret-redaction audit
- TLS configuration review
- redirect credential/cookie leakage review
- proxy/auth boundary review
- decompression/resource-exhaustion policy
- multipart filename/header/path safety review
- retry amplification and replay review
- protocol downgrade/fallback review
- Python/CLI unsafe-option review
- vulnerability reporting and response process

## Dependency security

Add CI for:

- known advisories
- duplicate/version review
- banned crates/features
- license compliance
- source restrictions

Pin critical GitHub Actions and document update process. Generate SBOMs for releases if practical.

## Threat model

Document assets and boundaries:

- credentials, cookies, client keys
- destination/proxy trust boundaries
- redirect and retry transitions
- compressed/untrusted body processing
- local file uploads/download paths
- FFI/runtime boundaries

Define supported attacker capabilities and explicit non-goals.

## Security reviews

### TLS

Review root fallback, custom CA replacement, unsafe verification bypass, mTLS key handling, SNI/ALPN, downgrade behavior, direct versus CONNECT parity.

### Redirect/auth/cookies

Test cross-origin, cross-scheme, port changes, chained redirects, user headers, client defaults, retries, and proxy routes for leakage.

### Proxy

Review CONNECT parsing, response bounds, auth separation, NO_PROXY matching, DNS behavior, environment policy, TLS interception assumptions, cancellation, and socket cleanup.

### Bodies

Review content-length conflicts, multipart injection/path handling, decoded-body limits, nested compression, buffering thresholds, and partial-body reuse.

### Retry

Prevent amplification, unsafe-method replay, deadline extension, duplicate side effects, and secret leakage across attempts.

## Secret redaction

Audit all Debug/Display/repr/error/log/CLI output for:

- Authorization
- Proxy-Authorization
- cookies
- bearer tokens/passwords
- URL userinfo
- private keys
- sensitive paths where appropriate

Add centralized redaction helpers and regression tests.

## Static/dynamic tooling

Use clippy, rustdoc warnings, cargo-geiger as informational input, sanitizers where supported, fuzzing outputs, and dependency scanners. Do not treat a tool score as a substitute for review.

## Security process

Update SECURITY.md with private reporting instructions, supported versions, response targets, embargo policy, CVE/GHSA process, and coordinated disclosure expectations.

Create a release security checklist and incident runbook.

## External review

After feature stabilization, commission or solicit focused review of TLS, redirect/auth/cookie pipeline, proxy transport, compression limits, and Python streaming lifecycle.

## Acceptance criteria

- security CI checks are active
- threat model and review records exist
- secret leakage tests cover all output paths
- unsafe modes are explicit
- vulnerability response process is documented
- no unresolved high-severity findings remain before public stable release
