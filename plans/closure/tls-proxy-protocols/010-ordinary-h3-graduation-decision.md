# TLS, Proxy and Protocols Milestone 010 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/tls-proxy-protocols/010-ordinary-h3-graduation-decision.md`

Source subsystem roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-10--ordinary-h3-graduation-decision`

Repository baseline reviewed: `f11a46b28df88f43e923d5211f82985784daaf40`

Implementation commits:

- _(documentation decision)_ — dated graduation section in
  `docs/architecture/core-tls-proxy-protocols.md`;
  `qualification/http3/public-origin-ledger.json` shortfall record.
  No production code change.

## 1. Executive finding

M010 is **closed with experimental retained**. Predecessors M006–M009
landed their harnesses, ledger, soak, and platform table. The ordinary
HTTP/3 gate does **not** pass: independent non-Quinn servers, privileged
impairment, public origins, and upstream #338 remain open. A retained
experimental label is the successful close for this evidence state.
`Auto { allow_http3: false }` stays the default. Datagram routing and
advanced QUIC features stay experimental/unimplemented. Long-term
specification documents (`plans/000`–`002`) were not edited.

Recommendation: **closed with experimental retained**.

## 2. Requirement-to-evidence matrix

| Gate row | Evidence | Result | Notes |
|---|---|---|---|
| Deterministic hardening / Alt-Svc / lifecycle / soak | M006–M009 suites on this tree | pass (local) | Darwin arm64 |
| Two independent non-Quinn servers | private manifest run | **fail / missing** | Not supplied |
| Independent drain / reset / graceful close | control URLs on non-Quinn peer | **fail / missing** | Not supplied |
| Public ledger ≥ 3 successes | `qualification/http3/public-origin-ledger.json` | **shortfall** | 0 of 3 |
| Impairment matrix all-pass | privileged Linux netem | **not executed** | Darwin → unsupported |
| Upstream no corrupting ordinary-response issue | `qualification/http3/upstream-risk-ledger.json` | **blocker** | #338/#339 open |
| #361 fixed or outside cancel claim | ledger + docs | pass (scoped out) | Wire-code claim excluded |
| Platform table matches support wording | M009 table | pass | Only Darwin developer row; no support claim |
| Tier 1 | `./scripts/check.sh` | pending on freeze | Executable inputs from M006/M007/M009 |
| Stage C | docs-only decision | unchanged by M010 | Predecessors still need renewal |

## 3. Production implementation evidence

None. Documentation decision only.

## 4. Verification executed

### Commands run

```bash
python3 scripts/test_h3_qualification.py
cargo test -p eggfetch-core --all-features --test h3_hardening --test h3_alt_svc_discovery --test h3_interop_qualification --test h3_datagram_route --test h3_impairment_scenarios -- --test-threads=1
```

Independent-server and privileged impairment commands were **not** run
(no private non-Quinn manifests; no Linux netns on this host).

### Results

Deterministic suites green locally. Gate operational rows open as named
above.

## 5. Invariant review

Missing evidence not treated as pass. No second H3 stack. No default
`allow_http3: true`. CLI/FFI defaults unchanged. ADR-0008 fail-closed
semantics unchanged.

## 6. Failure, timeout, pool, and recovery review

Unchanged. No product defect found that required a corrective plan stop
beyond the already-recorded upstream blocker.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

Docs-only decision keeps the current Stage C binding for this milestone.
Executable changes from M006/M007/M009 still require their conditional
Stage C renewal and must not be cited as Stage C green until rebound.

## 8. Security review

No new trust or redaction surface. Public-origin ledger contains no host
names (empty shortfall).

## 9. Documentation and operations

New dated section
`### Production Graduation Decision (2026-11-16, M010): experimental retained`.
Feature-flag / README / compatibility tables were **not** rewritten to
claim support. Historical September evidence files were not overwritten.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| high | Independent servers absent | Blocks support label | Operator private manifest + two stacks |
| high | #338 open | Blocks support label | Published h3 with #339 |
| high | Impairment not passed on Linux | Blocks support label | Privileged netem run |
| high | Public origins shortfall | Blocks support label | Three unrelated CDN/operator checks |
| medium | M006/M007/M009 Stage C | Publication of this tree | Freeze + Tier 2/3 + HTTPX |

## 11. Roadmap disposition

Ordinary H3 graduation remains a gated future. Re-open a new decision
milestone only when the open rows above can be filled on one SHA after
an M008-style audit of the pins under test. Do not amend `plans/000`
until a future passing gate.

## 12. Registry updates

M010 → closed (experimental retained). Gated-future line stays: H3
experimental until a later passing decision. Remove M010 from blocked
implementation plans; record open operational blockers under Operational
blockers.
