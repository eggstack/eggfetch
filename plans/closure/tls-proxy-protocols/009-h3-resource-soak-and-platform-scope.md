# TLS, Proxy and Protocols Milestone 009 — Closure Status

Status: conditionally closed

Source implementation plan:

- `plans/implementation/tls-proxy-protocols/009-h3-resource-soak-and-platform-scope.md`

Source subsystem roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-9--h3-resource-soak-and-platform-scope`

Repository baseline reviewed: `f11a46b28df88f43e923d5211f82985784daaf40`

Implementation commits:

- _(uncommitted at closure drafting)_ — soak tests in
  `h3_interop_qualification.rs` and platform table in
  `docs/architecture/core-tls-proxy-protocols.md`.

## 1. Executive finding

M009 is **conditionally closed**. Deterministic soak counts (1 000 reuse,
50 fail/reconnect, 50+30 cancel/partial-drop, 100 client drop) are
enforced in-tree. Darwin arm64 filled one platform-table row as local
developer evidence only — not product support. Linux RSS/fd envelope is
implemented and skipped on macOS. Stage C renewal is required on the
freeze that commits these tests. HTTP/3 remains experimental.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| 1 000 sequential reuse | `thousand_sequential_requests_on_reused_h3_connection` | pass | |
| Extended fail/reconnect | `fifty_repeated_fail_reconnect_cycles_do_not_grow_state_soak` | pass | 50 cycles |
| Cancel + partial drop storm | `extended_cancellation_and_partial_drop_storm_leaves_client_usable` | pass | |
| Construction/drop loop | `hundred_client_construction_drop_loops_do_not_leak_soak` | pass | 100 |
| Diagnostics ≤ 64, redacted | `assert_h3_diagnostics_bounded_and_redacted` | pass | |
| Platform table | architecture doc | pass | Darwin filled; Linux/Windows not executed |
| Stage C | — | **conditional** | New Rust tests |

## 3. Production implementation evidence

None expected. No public metrics API added. No `ResponseBody` change.

## 4. Verification executed

### Commands run

```bash
cargo test -p eggfetch-core --all-features --test h3_interop_qualification thousand_sequential -- --test-threads=1
cargo test -p eggfetch-core --all-features --test h3_interop_qualification soak -- --test-threads=1
cargo test -p eggfetch-core --all-features --test h3_interop_qualification extended_cancellation -- --test-threads=1
```

### Results

All new soak filters pass on Darwin arm64. Numeric RSS/fd skipped (no
`/proc`). Short controls still pass.

## 5. Invariant review

Cache and diagnostics remain capped at 64. No exact RSS equality.
Experimental label retained.

## 6. Failure, timeout, pool, and recovery review

Uses existing H3 timeout/cancel paths. Fail/reconnect does not poison a
different origin.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

Tests behind `http3` only. Stage C required on freeze.

## 8. Security review

Diagnostics redaction asserted. No new secret surfaces.

## 9. Documentation and operations

Platform table and soak counts documented under Resource and Platform
Scope. Graduation section not rewritten by this milestone.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| medium | Stage C unbound | Publication | Freeze + Tier 2/3 + HTTPX |
| info | Only Darwin row filled | Support wording limits | M010 may add rows when executed |

## 11. Roadmap disposition

Soak + table ready for M010 citation. Additional OS rows remain M010
operational evidence.

## 12. Registry updates

M009 → conditionally closed.
