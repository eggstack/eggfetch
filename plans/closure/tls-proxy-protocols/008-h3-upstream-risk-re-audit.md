# TLS, Proxy and Protocols Milestone 008 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/tls-proxy-protocols/008-h3-upstream-risk-re-audit.md`

Source subsystem roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-8--h3-upstream-risk-re-audit`

Repository baseline reviewed: `f11a46b28df88f43e923d5211f82985784daaf40`

Implementation commits:

- _(docs/ledger only; lockfile unchanged)_ —
  `qualification/http3/upstream-risk-ledger.json` and architecture
  dependency re-audit paragraph dated 2026-11-16.

## 1. Executive finding

M008 is **closed** on the no-upgrade path. crates.io on 2026-11-16 still
publishes `h3` 0.0.8 and `h3-quinn` 0.0.10 only. [h3 #338](https://github.com/hyperium/h3/issues/338)
and [PR #339](https://github.com/hyperium/h3/pull/339) remain open, so #338
is recorded as a graduation blocker. No fork or vendored patch was
introduced. A docs-and-ledger-only close does not change executable inputs
and does not require a new Stage C freeze. M010 cannot clear the upstream
row until a published release contains the frame-drain fix.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Resolve current graph | ledger `resolved_versions` | pass | quinn 0.11.11 / quinn-proto 0.11.18 / h3 0.0.8 / h3-quinn 0.0.10 |
| Re-read #338/#339/#361/#352/#353 | ledger `issues` + fetch date | pass | All open on 2026-11-16 |
| Disposition per issue | ledger | pass | #338/#339/#361 graduation blocker; #352/#353 proven not reachable for ordinary client success path |
| No-upgrade when no published fix | lockfile unchanged | pass | |
| GHSA-6xvm-j4wr-6v98 | quinn-proto 0.11.18 ≥ 0.11.14 | pass | Proven not reachable on this pin |
| No `[patch]` / vendor / git dep | tree inspection | pass | |
| Experimental label retained | docs | pass | |
| Stage C | n/a | pass | Docs/ledger only |

## 3. Production implementation evidence

None. Drain still uses h3 0.0.8 `is_closing()` / `is_h3_no_error()` under
the unstable feature; comment remains accurate.

## 4. Verification executed

### Commands run

```bash
cargo test -p eggfetch-core --all-features --test h3_hardening --test h3_alt_svc_discovery --test h3_interop_qualification --test h3_datagram_route -- --test-threads=1
```

### Results

H3 suites green on audited pins. No dependency bump.

## 5. Invariant review

No second QUIC stack. 0-RTT still disabled. No default-on H3.

## 6. Failure, timeout, pool, and recovery review

Unchanged. Cancellation tests keep asserting client usability, not
STOP_SENDING wire codes (#361).

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

No executable change. Current Stage C SHA is not invalidated by this
milestone alone. M006/M007/M009 changes elsewhere still require renewal.

## 8. Security review

Advisory GHSA-6xvm-j4wr-6v98 checked against resolved `quinn-proto`.
Certificate verification retry rules untouched.

## 9. Documentation and operations

New dated architecture paragraph. Historical 2026-09-11 paragraph kept.
Ledger path: `qualification/http3/upstream-risk-ledger.json`.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| high (graduation) | #338/#339 unreleased | M010 upstream row fails | Wait for published h3 with frame drain |
| medium | #361 open | No conformant cancel wire-code claim | Keep outside supported cancellation claim |

## 11. Roadmap disposition

M008 closed. External evidence for M010 must cite post-audit pins (same
lockfile today).

## 12. Registry updates

M008 → closed. M010 upstream blocker remains named.
