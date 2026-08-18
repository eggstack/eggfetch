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

### F-006: SSLContext CA-count heuristic for default-trust detection

- **Severity**: High
- **Status**: Fixed
- **Date**: 2026-08-18
- **Component**: `eggfetch-python::compat::httpx::_ssl_context::context_to_eggfetch_kwargs`
- **Description**: The translation layer compared the loaded CA count against the system/certifi trust store (with a tolerance) and silently treated similar-cardinality stores as `verify=True`.  This caused two custom CA stores with identical cardinalities but different content to be misclassified, allowing a connection that the caller explicitly configured a custom trust anchor for to be trusted against the wrong anchor.
- **Impact**: A caller supplying a custom CA bundle whose count resembled the default trust would, in some cases, see the wrong set of trust anchors used for the handshake.  This is a defense-in-depth gap: explicit user intent (narrow custom CA) could be silently widened by the heuristic.
- **Resolution**: Removed the CA-count comparison.  Translation now always carries the actual DER bytes when the loaded CA list is non-empty, and uses `verify=True` only when the context is empty and represents a default-trust helper context.  Two CA stores with identical cardinalities but different content produce different `verify` kwargs.  Added construction-fingerprint detection so that a helper-created context whose live state has been mutated (e.g. via `load_verify_locations`, `set_minimum_version`, `set_ciphers`) is reclassified from the live snapshot.  Added regression tests in `test_corrective_01_tls_and_proxy_trust_safety.py` and `test_corrective_01_tls_network_proof.py`.
- **Test**: `test_corrective_01_tls_and_proxy_trust_safety::TestRepresentabilityMatrix`, `TestConstructionFingerprint`; `test_corrective_01_tls_network_proof::TestTranslationDeterminismOverTheWire`, `TestFingerprintMutationOverTheWire`.

### F-007: Proxy endpoint TLS configuration fell back to origin TLS

- **Severity**: High
- **Status**: Fixed
- **Date**: 2026-08-18
- **Component**: `eggfetch-core::pipeline::send_with_proxy`
- **Description**: When the proxy was HTTPS and no explicit `proxy_tls_config` was set, the pipeline fell back to the origin `tls_config`.  This caused the origin's `verify=False`, custom CA bundle, mTLS client identity, SNI override, and TLS version policy to also apply to the proxy endpoint handshake.  An attacker who could MITM the proxy connection could rely on the origin's permissive settings (e.g. `verify=False`) to bypass proxy endpoint verification.
- **Impact**: Origin TLS policy leaked to the proxy endpoint.  In a typical scenario where the origin uses `verify=False` for development, the proxy endpoint would also skip certificate verification, allowing a network attacker to intercept and rewrite proxy traffic.  Conversely, a custom origin CA would be used to verify the proxy, causing spurious failures.
- **Resolution**: Removed the origin TLS fallback for the proxy leg.  `proxy_tls_config` is now sourced exclusively from `proxy_config.proxy_tls_config()`.  If the proxy URL is HTTPS and the caller did not supply a proxy SSL context, the proxy endpoint is verified using rustls' default trust anchors (system roots) and rejects certificates issued by any custom CA set on the origin.  Added regression tests in `test_corrective_01_tls_network_proof.py::TestProxyTrustDomainIsolation`.
- **Test**: `test_corrective_01_tls_network_proof::TestProxyTrustDomainIsolation`.

### F-008: Proxy headers and `Headers` debug output leak credentials

- **Severity**: Medium
- **Status**: Fixed
- **Date**: 2026-08-18
- **Component**: `eggfetch-core::headers::Headers`, `eggfetch-python::compat::httpx::_proxy::Proxy.__repr__`
- **Description**: `Headers::Debug` and the Python compat `Proxy.__repr__` showed the raw values of `authorization`, `proxy-authorization`, `cookie`, and `set-cookie` headers.  These surfaces are diagnostic (debug logging, error context, panic messages) and must not echo credentials.
- **Impact**: Credentials in proxy/auth headers could be captured in logs, error messages, or diagnostic dumps.  The ordinary iteration API (`iter`, `get`, `get_all`, `keys`) remained unredacted because protocol code must observe the raw values.
- **Resolution**: Replaced the derived `Debug` on `Headers` with a manual implementation that redacts the four sensitive names to `<redacted>`.  Updated `Proxy.__repr__` to redact the same four names.  Protocol iterators are unchanged.  Added regression tests in `test_corrective_01_tls_and_proxy_trust_safety.py::TestProxyHeaderRedaction` and `test_corrective_01_tls_network_proof.py::TestProxyHeaderRedactionAcrossSurfaces`.
- **Test**: `eggfetch-core::headers::tests`, `test_corrective_01_tls_and_proxy_trust_safety::TestProxyHeaderRedaction`, `test_corrective_01_tls_network_proof::TestProxyHeaderRedactionAcrossSurfaces`.

---

## Audit Log

| Date | Auditor | Scope | Result |
|------|---------|-------|--------|
| 2026-07-16 | AI Agent | Security hardening plan implementation | 4 findings fixed, 1 deferred |
