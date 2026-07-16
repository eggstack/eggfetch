# Security Findings Tracker

This document tracks security findings from reviews, audits, fuzzing, and
external reports. Each finding has a severity, status, and resolution.

## Severity Levels

| Level | Description |
|-------|-------------|
| Critical | Immediate exploitation risk, data exfiltration possible |
| High | Exploitable with moderate effort, credential leakage possible |
| Medium | Requires specific conditions, defense-in-depth gap |
| Low | Best-practice improvement, minimal exploitation risk |
| Informational | Observation, no action required |

## Finding Status

| Status | Description |
|--------|-------------|
| Open | Not yet addressed |
| In Progress | Being worked on |
| Fixed | Resolved in code |
| Deferred | Intentionally postponed (with rationale) |
| Won't Fix | Not actionable (with rationale) |

---

## Findings

### F-001: ClientIdentity Debug leak

- **Severity**: High
- **Status**: Fixed
- **Date**: 2026-07-16
- **Component**: `eggfetch-core::tls::ClientIdentity`
- **Description**: `ClientIdentity` derived `Debug`, exposing `private_key_der: Vec<u8>` as raw bytes in any debug/logging output.
- **Impact**: Private key material leaked in logs, error messages, or debug formatting.
- **Resolution**: Replaced `#[derive(Debug)]` with custom `Debug` impl that shows `cert_count` and `key_label` only. Added regression test `client_identity_debug_no_private_key`.
- **Test**: `tls.rs::tests::client_identity_debug_no_private_key`

### F-002: SingleCertResolver Debug leak

- **Severity**: Medium
- **Status**: Fixed
- **Date**: 2026-07-16
- **Component**: `eggfetch-core::tls::SingleCertResolver`
- **Description**: `SingleCertResolver` derived `Debug`, exposing `private_key_der: Vec<u8>` in debug output.
- **Impact**: Same as F-001 but only reachable through internal resolver formatting.
- **Resolution**: Replaced `#[derive(Debug)]` with custom `Debug` impl that shows `cert_count` and `key_label` only.

### F-003: Proxy URL error messages may echo credentials

- **Severity**: Medium
- **Status**: Fixed
- **Date**: 2026-07-16
- **Component**: `eggfetch-core::proxy::parse_proxy_url`
- **Description**: Error messages from URL parsing could include the raw URL string, which might contain embedded credentials.
- **Impact**: Credentials in proxy URLs leaked in error messages.
- **Resolution**: Added `redact_url_string()` helper to strip credentials from URL strings. Applied to proxy URL error messages. Added 3 regression tests.

### F-004: Error messages unredacted across codebase

- **Severity**: Low
- **Status**: Deferred
- **Date**: 2026-07-16
- **Component**: `eggfetch-core::error`
- **Description**: Many error variants contain raw `String` values from underlying libraries (hyper, tokio, rustls). These strings could theoretically contain URLs with embedded credentials.
- **Impact**: Low — the `url` crate's `ParseError` does not echo the URL, and hyper/tokio errors don't typically include full URLs. The risk is primarily from future code changes.
- **Resolution**: Deferred. The centralized `redact` module provides the infrastructure. A systematic audit of every error path is a large effort for minimal near-term gain. Revisit before public stable release.

### F-005: GitHub Actions not pinned by SHA

- **Severity**: Medium
- **Status**: Fixed
- **Date**: 2026-07-16
- **Component**: `.github/workflows/*.yml`
- **Description**: All GitHub Actions used tag-based refs (`@v4`, `@v2`, `@stable`) which can be moved by upstream maintainers.
- **Impact**: Supply chain attack via tag mutation.
- **Resolution**: Pinned all actions to full-length commit SHAs with version comments. Updated process documented in AGENTS.md.

---

## Audit Log

| Date | Auditor | Scope | Result |
|------|---------|-------|--------|
| 2026-07-16 | AI Agent | Security hardening plan implementation | 4 findings fixed, 1 deferred |
