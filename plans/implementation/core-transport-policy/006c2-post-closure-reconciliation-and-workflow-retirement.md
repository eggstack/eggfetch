# Core Transport Policy M006C2 — Post-closure reconciliation and one-off Windows workflow retirement

Status: ready

Repository baseline: `6904323c6435e5dfe3bb9a5a5ecbf8cf0b36645a`

Corrects / reconciles:

- `plans/closure/core-transport-policy/006c1-windows-qualification-and-downstream-closure.md`
- `plans/subsystems/core-transport-policy-roadmap.md`
- `plans/subsystems/release-verification-roadmap.md`
- `plans/registry.md`
- `plans/README.md`
- `docs/verification-policy.md` steady-state workflow budget

Related implementation history:

- M006 — Windows TLS response completeness investigation;
- M006C1 — native-Windows + downstream EggReplay qualification closure;
- one-off manual workflow `.github/workflows/windows-qualification.yml`.

Primary class: corrective / polish

## 1. Objective

Complete the administrative and verification-topology cleanup after M006C1.

M006C1 is technically and evidentially closed:

- EggFetch native-Windows matrix green, 19/19, run `36193582776`;
- EggReplay corrected 300 KiB Windows direct/Eggress/MITM proof green,
  run `36211265347`;
- Stage C remains bound to
  `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`;
- no EggFetch production transport change is required.

One temporary artifact remains: `.github/workflows/windows-qualification.yml`.
Its own header states it is a one-off M006C1 evidence surface and must be
removed when M006C1 closes.

The steady-state verification policy permits exactly one manual-dispatch
workflow: the PyPI release workflow. Keeping the M006C1 workflow after closure
would leave the repository outside its normal complexity budget.

This corrective removes that temporary surface, reconciles current planning
status, and records the milestone as fully returned to steady state.

## 2. Invariants

The corrective must preserve:

1. the M006 fixture-teardown technical verdict;
2. the M006C1 native-Windows and EggReplay evidence;
3. Stage C binding at
   `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`;
4. routine CI as the single automatic Ubuntu job;
5. PyPI as the single retained manual-dispatch workflow;
6. no EggFetch production/API/dependency/feature changes;
7. M001/M002 release procedures unchanged except for their temporary
   dependency on this reconciliation.

Do not delete historical Actions run references. Removing the workflow file
does not invalidate already-retained run/job evidence.

## 3. Required changes

### A. Remove the one-off qualification workflow

Delete:

`.github/workflows/windows-qualification.yml`

Do not merge its Windows job into routine CI.

Do not retain a dormant copy under another workflow filename.

The native-Windows proof remains available through the recorded Actions
run/job links in the M006C1 closure.

### B. Reconcile verification topology

After deletion, verify:

- `.github/workflows/ci.yml` remains the only push/PR workflow;
- `.github/workflows/pypi.yml` remains the only manual-dispatch workflow;
- no other workflow duplicates the retired Windows qualification surface;
- `docs/verification-policy.md` requires no semantic change.

Expected steady state:

```text
automatic workflows:       1
manual-dispatch workflows: 1 (PyPI only)
routine CI matrices:       0
```

If repository evidence differs, stop and reconcile the discrepancy before
closure.

### C. Reconcile the core-transport roadmap

Keep M006 and M006C1 closed as historical milestone evidence.

Add M006C2 as the final reconciliation corrective with status `ready` during
implementation and `closed` after verification.

After closure, the core-transport subsystem status should return to simply
`closed` with no active corrective.

Do not rewrite M006/M006C1 closure records.

### D. Compact the active registry

The registry is an active control surface, not a permanent duplicate of all
closure history.

After M006C2 closes:

- core transport status: `closed`;
- no dependency-ready M006/M006C1/M006C2 implementation work remains;
- M001 publication: `ready`;
- M002 Python 3.15 rehearsal: `ready`;
- retain concise links to the M006C1 closure where useful for recent history;
- remove stale language implying an active Windows qualification gate.

Do not remove the historical plan/closure files themselves.

### E. Reconcile release-verification status

During M006C2 implementation, M001 and M002 are blocked on this cleanup because
the repository currently exceeds its steady-state workflow budget.

After the one-off workflow is deleted and Tier 1 is green:

- M001 returns to `ready`;
- M002 returns to `ready`;
- Stage C remains unchanged;
- no new publication/rehearsal procedure is introduced.

### F. Reconcile planning README

Update the top status banner so it states:

- M006/M006C1/M006C2 are closed;
- Windows response-completeness work is fully reconciled;
- Stage C remains at the existing exact SHA;
- M001 and M002 are the current maintainer actions.

Avoid carrying transient run-by-run troubleshooting detail into the banner.

## 4. Stage C disposition

Deleting the one-off manual evidence workflow is a CI-surface cleanup, not an
EggFetch executable or Stage C qualification-input change.

Therefore the expected disposition is:

- no Stage C rebind;
- no Tier 2/Tier 3 rerun solely for this deletion;
- Stage C remains
  `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`.

If implementation changes any executable code, test fixture, validation
script, compatibility profile, or release validation input, stop and apply
ADR-0003 invalidation/requalification rules instead of treating the pass as
docs/CI cleanup.

## 5. Verification

Required:

```bash
./scripts/check.sh
git diff --check
```

Also inspect workflow topology directly and record:

```text
.github/workflows/ci.yml
.github/workflows/pypi.yml
```

as the only remaining workflow files unless unrelated approved workflows have
landed since the plan baseline.

Confirm the final push CI run is green.

No native-Windows rerun is required; its retained evidence is already the
M006C1 closure authority.

## 6. Closure record

Create:

`plans/closure/core-transport-policy/006c2-post-closure-reconciliation.md`

Record:

- implementation commit deleting the workflow;
- workflow topology before/after;
- Tier 1 result;
- final Actions run;
- Stage C unchanged rationale;
- registry/roadmap/release status;
- M001/M002 final readiness;
- confirmation that M006/M006C1 historical evidence remains intact.

## 7. Acceptance criteria

- [ ] `.github/workflows/windows-qualification.yml` is removed.
- [ ] routine automatic CI remains exactly one workflow/job surface.
- [ ] PyPI remains the only manual-dispatch workflow.
- [ ] verification-policy complexity budget is satisfied.
- [ ] no production/API/dependency/feature behavior changes.
- [ ] M006 and M006C1 closure evidence remains intact.
- [ ] Stage C remains
      `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`.
- [ ] core-transport roadmap has no active corrective after closure.
- [ ] active registry no longer treats M006-family work as executable.
- [ ] M001 and M002 are ready after reconciliation.
- [ ] `./scripts/check.sh` and `git diff --check` pass.
- [ ] final hosted CI is green.
- [ ] M006C2 closure record exists.

## 8. Non-goals

- No new Windows CI matrix.
- No changes to EggFetch transport code.
- No changes to EggReplay.
- No Stage C requalification unless scope unexpectedly changes.
- No publication or PyPI dispatch as part of this corrective.
- No Python 3.15 wheel rehearsal as part of this corrective.
- No archival relocation of M006/M006C1 files that would break existing
  evidence links.

## 9. Handoff

This should be a very small cleanup pass.

Expected implementation:

1. delete the one-off workflow;
2. run Tier 1;
3. reconcile registry/roadmap/release planning;
4. write closure;
5. confirm hosted CI green;
6. leave M001/M002 as the next operational work.

If anything beyond this is needed, stop and document why rather than expanding
M006C2 into another transport or release program.
