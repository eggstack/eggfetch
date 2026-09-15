# Security Review Skill

Use this skill when performing security reviews or addressing security findings in eggfetch.

## Workflow

1. Read `docs/architecture/threat-model.md` for the threat model.
2. Read `docs/architecture/security-reviews.md` for existing review records.
3. Read `docs/architecture/security-findings.md` for tracked findings.
4. Read `docs/architecture/release-security-checklist.md` for the release checklist.
5. Read `SECURITY.md` for the vulnerability reporting policy.

## Key Security Properties

- `unsafe_code = "forbid"` workspace-wide (FFI/Node exceptions).
- All Debug/Display/error output redacts secrets via `eggfetch_core::redact`.
- Cross-origin redirects strip Authorization, Cookie, and Proxy-Authorization headers. The Python facade strips explicit Cookie headers on all redirects and regenerates from the jar.
- URL credentials (`user:pass@host`) are rejected.
- Decompression bombs limited by `max_decoded_body_size` and `max_decompression_ratio`.
- Multipart boundaries validated (no CR/LF injection). Filenames basename-only (no path traversal).
- httpx2 core hardening (`H2X-COMP/MP/PROXY/TLS-001`, `H2X-WS-003` for WS):
  multipart part headers validated before bytes (`try_header`), chained
  decoders capped (native 4 vs reference 5, intentionally stricter),
  decode failure closes/releases body/pool lease, proxy trust isolated
  (origin verify/CA/mTLS/SNI/version never influence proxy TLS), `NO_PROXY`
  CIDR malformed input never broadens bypass, `FunctionAuth` redirect/retry
  timing leaks no credentials, `Headers` merge preserves redaction.
- Proxy auth not forwarded to destination.
- Native proxy route pinning is split by hop: `Proxy::resolved_addresses()` pins
  the physical proxy peer while preserving the logical proxy URI/TLS identity,
  and `RequestBuilder::proxy_target_addresses()` separately pins the ultimate
  HTTPS CONNECT or local SOCKS5 destination. Direct `resolved_target` remains
  direct-only; pinned routes never fall back to DNS, SOCKS5H and plaintext
  forward-proxy target pinning fail closed before I/O, and snapshots survive
  retries and same-origin redirects but reject cross-origin reuse. SOCKS cache
  keys include both snapshots, hand-rolled HTTP proxy tunnels are not pooled,
  and typed candidate fallback is limited to CONNECT 502/504 rejection or
  local-SOCKS5 replies 0x03/0x04/0x05; no Python/HTTPX or Egress-policy
  dependency is introduced for these native controls.
- Cookie jar integrity maintained across redirects.
- Alt-Svc learns only from authenticated HTTPS (verified TLS, no proxy,
  hop-local origin); alternatives never change cookies/auth/Host policy;
  QUIC SNI stays the origin; oversized/malformed values fail closed.
- H3 fallback replays only pre-commit replayable bodies; one-shot bodies
  never duplicate; `Http3Only` never falls back.
- WebSocket handshake via the normal pipeline (proxy/SOCKS/TLS identical
  to requests); only 101 owns a writable stream; proxy headers never reach
  the origin; credentials redacted; max-message bounds fragments in total.
- SSE bounds event buffering via `max_event_size`; close/cancel releases
  the body/pool lease; no whole-response buffering.

## Severity Classification

Source of truth: `SECURITY.md` § "Vulnerability Response SLA".

| Severity | Criteria | Initial Response | Fix Target |
|----------|----------|-----------------|------------|
| Critical | RCE, credential exfiltration, TLS bypass | 24 hours | 7 days |
| High | Credential leakage, SSRF, decompression bomb | 48 hours | 14 days |
| Medium | Info disclosure, redirect issues, bypass | 1 week | 30 days |
| Low | Theoretical issues, minor leakage, DoS | 2 weeks | 90 days |

## Incident Contact

- Security reports: dbowman91@proton.me
- Response time: Acknowledgment within 48 hours

## Architecture References

- Threat model: `docs/architecture/threat-model.md`
- Security reviews: `docs/architecture/security-reviews.md`
- Security findings: `docs/architecture/security-findings.md`
- Incident runbook: `docs/architecture/incident-runbook.md`
- Release checklist: `docs/architecture/release-security-checklist.md`
