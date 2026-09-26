# Core Transport Policy M006C2 — Post-closure Reconciliation and Workflow Retirement

Status: closed

Source implementation plan:

- `plans/implementation/core-transport-policy/006c2-post-closure-reconciliation-and-workflow-retirement.md`

Source subsystem roadmaps:

- `plans/subsystems/core-transport-policy-roadmap.md`
- `plans/subsystems/release-verification-roadmap.md`

Repository baseline reviewed: `6904323c6435e5dfe3bb9a5a5ecbf8cf0b36645a`
(the plan's repository baseline; only plan-only descendants precede this
implementation).

## 1. Executive finding

M006C2 closes the administrative and verification-topology cleanup after
M006C1. The one-off M006C1 Windows qualification workflow
(`.github/workflows/windows-qualification.yml`) has been removed; routine
CI is back to a single automatic workflow (`ci.yml`) plus the single
manual-dispatch PyPI workflow (`pypi.yml`). The verification-policy
complexity budget is fully satisfied. Active planning is reconciled: the
core-transport subsystem returns to `closed`; M001 and M002 return to
`ready`; the registry and release-verification roadmap reflect the new
steady state. No EggFetch production, API, dependency, profile, or
qualification-input change was made. Stage C remains bound to
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`. No Tier 2 / Tier 3 rerun was
required.

## 2. Required-change evidence

| Requirement | Evidence | Result |
|---|---|---|
| Remove `.github/workflows/windows-qualification.yml` | `git rm` of the file in the M006C2 implementation commit | Removed; no dormant copy retained elsewhere |
| Routine automatic CI is exactly one workflow | `.github/workflows/ci.yml` remains the sole push/PR workflow | Met |
| PyPI remains the only manual-dispatch workflow | `.github/workflows/pypi.yml` remains the sole `workflow_dispatch` workflow | Met |
| Workflow topology matches verification-policy complexity budget | automatic workflows = 1, manual-dispatch workflows = 1 (PyPI only), routine CI matrices = 0 | Met |
| Tier 1 green on the implementation commit | `./scripts/check.sh` 2026-09-26: green (`EXIT=0`) | Met |
| `git diff --check` clean | `git diff --check` returns no output | Met |
| Stage C unchanged | `plans/httpx-parity-correction-status.md` still binds `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa` | Met |
| M006/M006C1 historical evidence intact | Both closure records (`plans/closure/core-transport-policy/006-windows-tls-response-completeness.md`, `plans/closure/core-transport-policy/006c1-windows-qualification-and-downstream-closure.md`) and the recorded Actions run/job URLs are untouched | Met |
| Core-transport roadmap at steady state | `plans/subsystems/core-transport-policy-roadmap.md` status updated to `closed`; M006/M006C1/M006C2 all closed in the milestone table | Met |
| Release-verification roadmap reflects unblocked M001/M002 | `plans/subsystems/release-verification-roadmap.md` status, dependency graph, milestone status, and §4 current state all reconciled | Met |
| Registry reflects steady state | `plans/registry.md` active subsystem roadmaps and dependency-ready tables both show M001/M002 ready; core-transport closed; no M006-family active work listed | Met |
| Planning README banner reconciled | `plans/README.md` status banner no longer references an active Windows qualification gate; M006/M006C1/M006C2 are closed and M001/M002 are the next operational actions | Met |

## 3. Workflow topology before/after

### Before

```text
.github/workflows/ci.yml                 — automatic (push/PR)
.github/workflows/pypi.yml               — manual-dispatch (PyPI)
.github/workflows/windows-qualification.yml — manual-dispatch (M006C1 one-off)
```

Complexity budget consumed: automatic workflows = 1, manual-dispatch
workflows = 2 (PyPI + M006C1 one-off). The second manual-dispatch workflow
was a documented exception against the M006 Windows regression history with
explicit maintainer approval; the plan header explicitly required its
removal on closure.

### After

```text
.github/workflows/ci.yml   — automatic (push/PR)
.github/workflows/pypi.yml — manual-dispatch (PyPI)
```

Complexity budget consumed: automatic workflows = 1, manual-dispatch
workflows = 1 (PyPI only), routine CI matrices = 0. The
`docs/verification-policy.md` budget is satisfied; no semantic change to
the policy is required.

## 4. Verification executed

- `./scripts/check.sh` — Tier 1, 2026-09-26: green. Includes formatting,
  lint-suppression policy, adapter feature ownership, release version
  validation, clippy, workspace Rust tests (excluding PyO3), stable Rust
  public-contract fixtures across six profiles, Python extension build
  via maturin, native Python API manifest (66 exports), Python typing
  surface + fixture checks, 578 ordinary Python behavior tests, HTTPX
  compatibility smoke kernel (134 tests across the two facades), and the
  Node binding prototype check (JS surface explicitly skipped — no
  built `eggfetch.node` artifact in the CI environment, matching the
  pre-existing policy skip rule).
- `git diff --check` — clean (no trailing-whitespace / indent warnings).
- No Tier 2 / Tier 3 / Stage C rerun was required: the only files
  touched were the workflow removal and four planning markdown files
  (registry, two roadmaps, README banner). No executable, test, fixture,
  validation script, compatibility profile, or release validation input
  changed.

Final Tier 1 output retained at `/tmp/opencode/m006c2-check.log`
(`All routine checks passed.`).

## 5. Stage C disposition

Stage C exact-SHA binding is unchanged:
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`. M006C2 removed a workflow
file and reconciled planning documents; it did not touch any EggFetch
executable, test, fixture, validation script, compatibility profile, or
release validation input. ADR-0003 invalidation/requalification
conditions did not fire; no rebind or requalification was required.

## 6. Registry and roadmap disposition

- `plans/subsystems/core-transport-policy-roadmap.md`: status banner
  changed from `active corrective — M006C2 post-closure reconciliation`
  to `closed — M006C2 post-closure reconciliation complete; subsystem
  returns to steady state`. §7 milestone entry for M006C2 changed from
  `Status: ready` to `Status: closed`, and the closure-record pointer
  is now linked. §11 "Completion definition" rewritten to record that
  M006/M006C1/M006C2 are all closed and M001/M002 are the next
  operational work. §12 milestone table row for M006C2 changed from
  `ready` to `closed` and the blocker column cleared.
- `plans/subsystems/release-verification-roadmap.md`: status banner
  changed from `active — M001/M002 blocked on M006C2 reconciliation` to
  `active — M001 publication and M002 wheel rehearsal ready
  post-M006C2`. §6 dependency-graph predecessor node updated from M006C1
  to M006C2 (closed). §7 milestone entries for M001/M002 updated their
  "M006C1 is closed" note to "M006C2 is closed". §12 milestone table
  rows for M001 and M002 changed from `blocked` to `ready` and the
  blocker column was cleared (M002 retains a dispatch-from-authoritative-
  Stage-C-SHA note, which is operational, not a blocker).
- `plans/registry.md`: rewritten as a compact active control surface.
  - Active subsystem roadmaps table now shows Core transport as
    `closed`, Release and verification as `active` (M001/M002 ready),
    all others unchanged.
  - Dependency-ready implementation plans table is reduced to M001 and
    M002, both `ready`.
  - Current execution order and dependency gates section has no active
    corrective gate.
  - Blocked / operational work table only contains the gated futures
    (H3 graduation, HTTPX 1.0 program).
  - Closure work and current control points section states all
    M006-family work is closed and links to the closure records.
- `plans/README.md`: the top status banner is reconciled — M006/M006C1/M006C2
  are stated as closed; M001 publication/tag/PyPI and M002 Python 3.15
  wheel rehearsal are stated as the next operational actions ready to
  dispatch from the live Stage C SHA. Transient run-by-run troubleshooting
  detail is not carried into the banner.

The historical plan and closure files for M006, M006C1, and earlier
milestones are untouched; no archival relocation was performed (per the
non-goal "no archival relocation of M006/M006C1 files that would break
existing evidence links"). Stage C binding language continues to live in
the ledger and compat profiles — never duplicated into the registry —
per ADR-0003 and the registry's own control-surface policy.

## 7. M001 / M002 final readiness

- M001 (`plans/implementation/release-verification/001-release-publication.md`):
  Status `ready`. Maintainer executes the leaf-order `cargo publish`
  (http-connect → core → cli → ffi → python → node), tags the release,
  and dispatches `.github/workflows/pypi.yml` with `publish=true`. Tier
  2 + Tier 3 + live `check_security.sh` confirmation from a trusted
  local environment are the required pre-publication gates.
- M002 (`plans/implementation/release-verification/002-python-315-wheel-rehearsal.md`):
  Status `ready`. Maintainer dispatches `.github/workflows/pypi.yml`
  with `publish=false` from the authoritative Stage C SHA; 19
  distributions are expected to assemble (18 wheels + 1 sdist).
  Python 3.15 support remains unclaimed until rehearsal evidence lands.

No new publication or rehearsal procedure is introduced by M006C2. The
existing maintainer-run procedures apply.

## 8. Unresolved findings

None.

## 9. Closure statement

M006C2 is **closed**. The one-off M006C1 Windows qualification workflow
has been retired; the verification-policy complexity budget is satisfied;
M006/M006C1 historical evidence remains intact; Stage C remains
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`. The core-transport
subsystem is back to steady state. M001 publication/tag/PyPI and M002
Python 3.15 wheel rehearsal are ready for maintainer execution against
the live Stage C freeze.