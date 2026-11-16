# TLS, Proxy and Protocols Milestone 012 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/tls-proxy-protocols/012-h3-qualification-security-hygiene.md`

Source subsystem roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-12--h3-qualification-security-hygiene`

Repository baseline reviewed: `8af32d7ca4f1140e266875798522f5da09c17850`

## 1. Executive finding

M012 is closed. The H3 upstream-risk ledger Quinn pins match workspace
`Cargo.lock` (0.11.12 / 0.11.19 / 0.5.16) with RUSTSEC-2026-0523 / 0524 /
0538 dispositions recorded; #338 remains a graduation blocker. Embedded
reqwest qualification locks were refreshed to the same Quinn floors.
Netem `scenario_id` is charset-validated before any shell use. Cert-bypass
help text remains opt-in / non-evidence. No production runtime change.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Ledger Quinn pins match workspace | `qualification/http3/upstream-risk-ledger.json` | pass | Before: 0.11.11/0.11.18/0.5.15 → After: 0.11.12/0.11.19/0.5.16 |
| Advisories for Quinn bump | ledger `advisories[]` | pass | 0523/0524/0538 + prior 0037 |
| #338 not waived | ledger issues + decision_summary | pass | still graduation blocker |
| Embed locks refreshed | `qualification/embedded/reqwest-{json,min}/Cargo.lock` | pass | same Quinn floors |
| Nested locks ≠ root scan | embed README + architecture note | pass | |
| scenario_id charset | `validate_scenario_id`; `test_scenario_id_charset_rejected_before_shell_use` | pass | |
| Cert-bypass opt-in warning | `h3_qualification.py --help` text | pass | |
| Runner self-test | `python scripts/test_h3_qualification.py` | pass | 15 tests |

## 3. Before/after ledger pin table

| Package | Before | After (workspace) |
|---|---|---|
| quinn | 0.11.11 | 0.11.12 |
| quinn-proto | 0.11.18 | 0.11.19 |
| quinn-udp | 0.5.15 | 0.5.16 |

## 4. Verification executed

```bash
python scripts/test_h3_qualification.py
./scripts/check.sh
./scripts/check_security.sh   # with Stage C
```

## 5–10. Reviews

No CI expansion. No production TLS bypass. rustix informational allow
unchanged. #338 disposition unchanged.

## 11. Closure recommendation

**Closed.** Paired with M011 on the shared exact-SHA Stage C freeze.
