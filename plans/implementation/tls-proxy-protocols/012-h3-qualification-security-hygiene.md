# TLS, Proxy, and Protocols Milestone 012 — H3 Qualification Security Hygiene

Status: closed

Repository baseline: `8af32d7ca4f1140e266875798522f5da09c17850`

Closure: `plans/closure/tls-proxy-protocols/012-h3-qualification-security-hygiene.md`

Source roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-12--h3-qualification-security-hygiene`

Long-term requirements:

- `plans/000-long-term-specification.md` §3, §6
- `plans/001-terminology-and-domain-model.md` §10–§11
- `plans/002-long-term-roadmap.md` Phase 4 item 2

Applicable ADRs:

- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`
- `plans/adrs/ADR-0007-caller-owned-h3-datagram-routing.md`

Primary class: infrastructure

## 1. Objective

Align H3 qualification artifacts and operator-facing harnesses with the
post-Quinn-advisory lockfile and with safe-by-default qualification
hygiene, closing Medium/Low findings from the post-M010 security audit
that are not production dial-path defects (those are M011).

Deliverables:

1. Refresh `qualification/http3/upstream-risk-ledger.json` so resolved Quinn
   pins and advisory reachability match workspace `Cargo.lock` after the
   RUSTSEC-2026-0523 / 0524 / 0538 bump (and any later same-semver patches).
2. Refresh or clearly mark
   `qualification/embedded/reqwest-json/Cargo.lock` and
   `qualification/embedded/reqwest-min/Cargo.lock` so security evidence
   never cites pre-bump Quinn as current.
3. Harden qualification-only shell/cert surfaces: validate `scenario_id`
   before shell interpolation in the netem runner; keep cert-bypass
   opt-in and reinforce “not public evidence” in runner help and architecture
   docs.

Upstream h3 #338 remains a graduation blocker (M008/M010); this milestone
does not attempt a workaround.

## 2. Why this milestone is ready

The Stage C freeze that cleared Quinn advisories updated workspace
`Cargo.lock` but left the M008 risk ledger and embedded qualification
lockfiles on older Quinn versions. Operators reading those files can draw
wrong advisory conclusions. Netem and interop harnesses are qualification
only; small hygiene fixes reduce misuse risk without changing product
behavior.

## 3. Current implementation evidence

- Ledger `resolved_versions` still lists quinn **0.11.11** /
  quinn-proto **0.11.18** / quinn-udp **0.5.15** while workspace lock has
  **0.11.12** / **0.11.19** / **0.5.16**.
- Embedded reqwest qualification locks still resolve pre-bump Quinn.
- `scripts/h3_impairment_netem.py` embeds `scenario_id` in `bash -lc` after
  matrix validation in `main()`, but a direct call with untrusted argv is
  unsafe.
- `EGGFETCH_H3_INTEROP_DANGER_ACCEPT_INVALID_CERTS` /
  `--danger-accept-invalid-certs` are documented opt-in; reinforce
  boundaries only.

## 4. Invariants that must not regress

- Qualification fixtures stay non–Tier-1 gates (runner self-test only in
  Tier 1).
- No new CI job, matrix, or evidence schema.
- No production TLS bypass; danger flags remain off by default.
- M008 no-upgrade decision for h3 0.0.8 / #338 unchanged unless a published
  fix appears (that would be a new M008-style re-audit, not this milestone).
- `scripts/check_security.sh` continues to scan the workspace root lock;
  document that embedded locks are separate evidence trees.

## 5. Scope

### In scope

- Ledger pin/advisory refresh (and `git_revision` / notes for the bump).
- Embedded lock refresh via the existing embed qualification update path,
  or an explicit “stale / not security-scanned” banner in the embed README
  if a full refresh is deferred — prefer refresh.
- Netem `scenario_id` allowlist before any shell use (reject metacharacters
  even on direct invocation).
- Doc/help reinforcement for cert-bypass and operator-supplied interop URLs.

### Explicitly out of scope

- M011 dial-error / recv-trust production fixes.
- Expanding `check_security.sh` to scan every nested lock without an
  explicit verification-policy change (MAY add a note or optional path;
  MUST NOT silently expand CI scope).
- Publishing or graduating H3.
- Changing rustix / RUSTSEC-2026-0486 policy (informational allow stays).

## 6. Required production changes

None expected in `eggfetch-core` runtime. Changes are under
`qualification/`, `scripts/h3_*.py`, and documentation. If a script change
is treated as an executable input under the exact-SHA rule, renew Stage C.

## 7. Ordered work packages

1. Diff ledger `resolved_versions` + advisories against current
   `Cargo.lock`; rewrite ledger fields; record that Quinn bump does not
   clear #338.
2. Refresh embedded reqwest locks (or document non-authority clearly).
3. Fail closed on `scenario_id` charset before `bash -lc`.
4. Audit help strings / architecture notes for danger-accept-invalid-certs.
5. Self-tests for runner validation; Tier 1 runner self-test still green.

## 8. Failure, cancellation, timeout, and pool semantics

N/A (qualification tooling).

## 9. Compatibility and migration (incl. feature-profile effects)

No facade or feature-profile change. Consumers of the ledger JSON must
tolerate added advisory entries for the Quinn bump.

## 10. Required tests

- `python scripts/test_h3_qualification.py` (and any new validation cases
  for scenario_id).
- Optional: focused netem argparse unit checks without requiring netem.

## 11. Required verification commands

```bash
python scripts/test_h3_qualification.py
./scripts/check.sh
./scripts/check_security.sh
# If scripts/ or lockfiles under qualification affect executable-input
# policy for Stage C, renew exact-SHA binding after freeze.
```

## 12. Documentation updates

- Ledger + embed README notes.
- Short note in `docs/architecture/core-tls-proxy-protocols.md` or
  qualification README that nested locks are not covered by root
  `check_security.sh` unless refreshed for a named evidence run.

## 13. Acceptance criteria

- Ledger Quinn pins match workspace lock (or explicitly document a newer
  intentional pin with advisory disposition).
- Embedded locks refreshed or marked non-authoritative for advisory claims.
- Untrusted `scenario_id` cannot reach the shell.
- Cert-bypass remains opt-in with explicit non-evidence warning.

## 14. Stop conditions

- Pressure to enable danger-accept-invalid-certs in CI or Tier 1.
- Attempts to waive #338 via ledger wording.

## 15. Closure evidence required

- Before/after ledger pin table.
- Commands run and outcomes.
- Closure record under `plans/closure/tls-proxy-protocols/`.

## 16. Handoff notes

Pair with M011 when scheduling; prefer M012 first if only docs/locks/scripts
are needed for an upcoming security citation. #338 stays on the gated-future
list until a published h3 release lands.
