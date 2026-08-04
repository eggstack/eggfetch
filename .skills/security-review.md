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
- Proxy auth not forwarded to destination.
- Cookie jar integrity maintained across redirects.

## Severity Classification

| Severity | Criteria | Response Time |
|----------|----------|---------------|
| Critical | RCE, credential exfiltration, TLS bypass | Fix within 7 days |
| High | Credential leakage, SSRF, decompression bomb | Fix within 14 days |
| Medium | Info disclosure, redirect issues, bypass | Fix within 30 days |
| Low | Theoretical issues, minor leakage, DoS | Fix within 60 days |

## Incident Contact

- Security reports: dbowman91@proton.me
- Response time: Acknowledgment within 48 hours

## Architecture References

- Threat model: `docs/architecture/threat-model.md`
- Security reviews: `docs/architecture/security-reviews.md`
- Security findings: `docs/architecture/security-findings.md`
- Incident runbook: `docs/architecture/incident-runbook.md`
- Release checklist: `docs/architecture/release-security-checklist.md`
