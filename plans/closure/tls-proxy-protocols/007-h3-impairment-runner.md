# TLS, Proxy and Protocols Milestone 007 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/tls-proxy-protocols/007-h3-impairment-runner.md`

Source subsystem roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-7--h3-impairment-runner`

Repository baseline reviewed: `f11a46b28df88f43e923d5211f82985784daaf40`

Implementation commits:

- _(uncommitted at closure drafting)_ —
  `crates/eggfetch-core/tests/h3_impairment_scenarios.rs`,
  `scripts/h3_impairment_netem.py`, contract tests in
  `scripts/test_h3_qualification.py`, docs + matrix pointer.

## 1. Executive finding

M007 is **closed**. The coordinator now has a real Linux netem runner and a
Rust scenario target for all 14 matrix ids. On Darwin arm64 (planning host)
privileged execution is unavailable; the runner records `unsupported` rather
than pass. That is honest and does **not** satisfy M010's impairment row.
Exact-SHA Stage C is rebound to `6b571028767dde4b0a78f36e6ecef525c6d68482`.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Runner callable as `--runner scripts/h3_impairment_netem.py` | script + coordinator contract | pass | |
| 14 scenario tests named as matrix ids | `h3_impairment_scenarios.rs` | pass | Unselected → skip |
| Contract self-test socket-free | `test_h3_qualification.py` | pass | Wired into Tier 1 path |
| Privileged Linux full matrix | not executed | **not executed** | Host: Darwin arm64; missing netns/tc privileges |
| Quinn peer impairment ≠ independent interop | this record | pass | Explicit |
| Stage C | ledger | **pass** | Rebound to `6b571028767dde4b0a78f36e6ecef525c6d68482` |

## 3. Production implementation evidence

None. Impairment stays outside `eggfetch-core`. No production feature flag.

## 4. Verification executed

### Commands run

```bash
python3 scripts/test_h3_qualification.py
cargo test -p eggfetch-core --all-features --test h3_impairment_scenarios -- --test-threads=1
cargo test -p eggfetch-core --all-features --test h3_hardening -- --test-threads=1
python3 scripts/h3_impairment_netem.py packet_loss_low /tmp/h3-imp-packet_loss_low.json
```

### Results

- Contract + unselected scenario cargo tests: pass
- macOS runner: `unsupported` with reason naming Darwin / missing capability
- Privileged Linux coordinator JSON: **not executed**

## 5. Invariant review

`Http3Only` never falls back in blackhole scenarios. Auto fallback only
pre-commit for replayable bodies. Core does not learn netem.

## 6. Failure, timeout, pool, and recovery review

Scenarios that name deadlines use Timeout budgets that fail closed on hang.
Cancellation uses existing drop/abort paths.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

`http3`-gated test only. Stage C rebound. macOS `unsupported` is not a
client failure.

## 8. Security review

Certificate material stays in test fixtures. Runner does not disable
production certificate verification.

## 9. Documentation and operations

Architecture doc documents runner invocation, privileges, 1%/20% loss
rates, and that `unsupported` ≠ graduation evidence.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| high (for M010) | Privileged Linux matrix not run | M010 impairment row open | Operator Linux netem run |
| — | Stage C rebound | Closed | Exact-SHA binding `6b571028767dde4b0a78f36e6ecef525c6d68482` |

## 11. Roadmap disposition

Harness landed. M010 impairment evidence still open until Linux pass.

## 12. Registry updates

M007 → closed. Note Darwin unsupported vs Linux pending for graduation.
