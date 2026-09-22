# Private Architecture Qualification-State Corrective Pass

Planning baseline: `3addebd5680d460773a16215db90b71f7dfad5c0` (`main`, 2026-09-22 UTC)
Executable freeze to preserve: `d4979f1dac53f30f07900f54b01a88de06956c1c`
Parent program: `plans/api-preserving-private-architecture-containment-program.md`
Closure record: `plans/post-private-architecture-api-requalification-and-closure.md`
Normative verification policy: `docs/verification-policy.md`

## Objective

Correct the canonical qualification state after the completed
API-preserving private-architecture containment program.

The implementation and behavioral qualification are already complete on
executable freeze `d4979f1dac53f30f07900f54b01a88de06956c1c`. The freeze
passed Tier 1, extended, package, security, exact Rust 1.89.0/MSRV,
six-profile Rust public-API, native Python API/typing/behavior, HTTPX 0.28.1,
HTTPX2 2.12.0, FFI, CLI, and Node-Rust validation. Remote CI also passed on
the documentation descendants.

However, the live compatibility records were not rebound from the prior
maintenance freeze `18c1f96c1cbf9d71aa480030b0f365c85267620b` to the
new executable freeze. The repository therefore currently contains a
contradiction:

- the private-architecture closure plan says `d4979f1...` is the qualified
  executable freeze and records the compatibility runs on it;
- the live HTTPX/HTTPX2 Stage C profiles and canonical compatibility
  documentation still identify `18c1f96...` as the current Stage C
  qualification SHA.

This pass repairs evidence/state bookkeeping only. It must not change
executable source, tests, validation scripts, workflows, compatibility
behavior, feature graphs, dependency graphs, package contents, or API
snapshots.

A second small defect also remains: prerequisite/final acceptance checklists
still contain unchecked boxes even though their implementation/closure records
state that those criteria passed. This pass reconciles that plan state to the
already-recorded evidence.

## Scope

### 1. Rebind both live Stage C profiles to the qualified executable freeze

Update:

- `compat/httpx/0.28.1/profile.toml`
- `compat/httpx2/2.12.0/profile.toml`

Required state:

- `qualification-sha = "d4979f1dac53f30f07900f54b01a88de06956c1c"`;
- qualification date reflects the 2026-09-22 qualification;
- the previous live qualification SHA becomes
  `18c1f96c1cbf9d71aa480030b0f365c85267620b`;
- existing historical SHA fields remain historical and are not collapsed or
  rewritten unnecessarily;
- comments explain that the private-architecture campaign changed executable
  Rust/Python/CLI source while preserving behavior/API, so exact-SHA renewal
  was required;
- Stage designation remains Stage C;
- no allowed-difference classification changes;
- no API-manifest/snapshot regeneration.

Do not alter reference-version pins, Python-version claims, or Stage C target
semantics.

### 2. Renew the live parity/status ledger

Update `plans/httpx-parity-correction-status.md` with a new top/current
record stating that both facades are Stage C qualified on
`d4979f1dac53f30f07900f54b01a88de06956c1c`.

The record must include the existing qualification evidence already captured
by the closure plan, including at minimum:

- Tier 1 passed;
- extended passed;
- package passed;
- security passed;
- exact Rust 1.89.0/MSRV passed;
- six-profile Rust public API oracle passed without snapshot regeneration;
- native Python API/typing checks passed;
- native Python behavior suite passed;
- HTTPX 0.28.1 smoke/full compatibility passed;
- HTTPX2 2.12.0 qualification passed according to the same closure run;
- FFI/CLI/Node-Rust validation passed with the existing policy-defined Node JS
  artifact skip;
- no compatibility waiver or new residual difference was introduced.

The prior `18c1f96...` record remains in the ledger as historical evidence.

### 3. Reconcile canonical compatibility documentation

Update every document that presents a current/live Stage C qualification SHA,
including at minimum:

- `docs/reference/compatibility.md`;
- `docs/reference/compatibility-stage-decision.md`;
- `docs/residual-differences.md`.

Replace only current/live references to `18c1f96...` with the
`d4979f1...` freeze and identify the 2026-09-22 private-architecture
requalification.

Historical sections must retain their original SHAs and dates.

Perform a repository-wide consistency sweep for other files that use wording
such as:

- "Current qualification";
- "current Stage C";
- "Stage C qualified on frozen executable SHA";
- "qualification-sha";
- "live exact-SHA";
- "active qualification".

Any such current-state record must agree with `d4979f1...`. Historical
records must not be rewritten merely because they mention `18c1f96...`.

### 4. Reconcile private-architecture plan acceptance state

Update the completed plans so their checklists agree with their own
implementation/closure records.

At minimum inspect and reconcile:

- `plans/core-client-proxy-private-decomposition-second-pass.md`;
- `plans/python-streaming-and-cli-private-decomposition.md`;
- `plans/rust-surface-containment-and-experimental-adapter-hygiene.md`;
- `plans/post-private-architecture-api-requalification-and-closure.md`.

Change `[ ]` to `[x]` only when the criterion is directly supported by
the recorded implementation/qualification evidence.

If any criterion is not actually evidenced, leave it unchecked and reopen it
explicitly rather than asserting completion.

The parent program and `plans/README.md` should continue to distinguish:

- planning baseline;
- executable freeze `d4979f1...`;
- documentation/profile/ledger descendants;
- remote-CI descendant(s).

### 5. Register and close this corrective truthfully

Update `plans/README.md` at implementation start to register this as the
active corrective.

After the state repair is complete and verified, change the index entry to
Completed and record:

- the preserved executable freeze;
- the final documentation/profile descendant SHA;
- the ordinary remote CI run for that descendant;
- that no executable/test/validation/workflow/package source changed;
- that issue #24 publication and the Python 3.15 wheel rehearsal remain
  independent pending maintainer/release actions.

Do not reopen the parent architecture campaign's executable qualification.

## Evidence reuse policy

This corrective may reuse the qualification evidence already recorded for
`d4979f1...` because no executable/test/validation/qualification input is
allowed to change in this pass.

The pass must not claim a new executable qualification run merely because
metadata is corrected.

If implementation discovers that any executable, test, validation script,
workflow, compatibility fixture, API snapshot, package manifest, or other
qualification input must change, stop this corrective. Such a change
invalidates the preserved freeze and requires a new executable freeze plus the
full requalification procedure.

## Validation

Before edits:

1. record current `main` SHA;
2. confirm `d4979f1...` remains an ancestor of `main`;
3. compare `d4979f1...` to current `main` and confirm post-freeze changes
   are documentation/plan/evidence descendants only;
4. capture current values from both profile TOMLs and the three canonical
   compatibility documents.

After edits:

### Static consistency checks

- both profile TOMLs name exactly `d4979f1...`;
- their previous-qualification field names `18c1f96...`;
- the top/current parity-ledger record names `d4979f1...`;
- all current qualification blocks in canonical docs name `d4979f1...`;
- no current-state block still names `18c1f96...`;
- historical `18c1f96...` records remain intact where appropriate;
- the private-architecture plan checklists have no unexplained unchecked
  completion criteria.

### Diff classification

Compare the final corrective descendant against
`3addebd5680d460773a16215db90b71f7dfad5c0`.

Allowed paths are documentation/evidence/profile/plan files only.

The diff must contain none of:

- `crates/**`;
- `src/**`;
- `tests/**`;
- `scripts/**`;
- `.github/workflows/**`;
- `Cargo.toml` / `Cargo.lock`;
- `pyproject.toml`;
- `package.json`;
- API snapshot files;
- allowed-difference manifests;
- compatibility behavior fixtures.

If any prohibited path changes, stop and reclassify the work.

### Repository validation

Because this is state-only, run the repository's normal validation that checks
documentation/profile consistency as appropriate. At minimum run Tier 1 if
available in the handoff environment:

```sh
./scripts/check.sh
```

Do not rerun or rewrite the full compatibility evidence solely to change the
recorded SHA when the executable freeze and qualification inputs are unchanged.

After push, require ordinary remote CI to pass on the final
documentation/profile descendant.

## Acceptance criteria

- [ ] Both live compatibility profile TOMLs bind Stage C to
      `d4979f1dac53f30f07900f54b01a88de06956c1c`.
- [ ] Both profiles identify `18c1f96...` as the immediately previous
      qualification rather than current.
- [ ] The live parity/status ledger has a new current record for
      `d4979f1...` and preserves `18c1f96...` historically.
- [ ] `docs/reference/compatibility.md` presents `d4979f1...` as current.
- [ ] `docs/reference/compatibility-stage-decision.md` presents
      `d4979f1...` as current.
- [ ] `docs/residual-differences.md` presents `d4979f1...` as current.
- [ ] A repository-wide current-state sweep finds no stale live
      `18c1f96...` Stage C binding.
- [ ] Historical qualification records retain their original SHAs/dates.
- [ ] No API snapshot, allowed-difference set, residual classification, or
      compatibility behavior changes.
- [ ] Completed child/final plan checklists are reconciled to evidence, with
      no unsupported box checked.
- [ ] The final corrective diff is documentation/profile/plan/evidence only.
- [ ] The executable freeze remains `d4979f1...`; no new executable freeze is
      created.
- [ ] Tier 1/profile consistency checks pass.
- [ ] Ordinary remote CI passes on the final descendant.
- [ ] `plans/README.md` records the corrective as completed only after the
      above evidence exists.
- [ ] Issue #24 publication and Python 3.15 wheel rehearsal remain independent
      and truthfully pending.

## Non-goals

Do not:

- modify Rust/Python/CLI/FFI/Node executable source;
- modify tests or validation scripts;
- regenerate public API snapshots;
- change allowed differences or residual classifications;
- change feature/default/dependency graphs;
- rerun the architecture refactor;
- add or remove public API;
- mature Node;
- graduate HTTP/3;
- publish issue #24;
- perform the Python 3.15 wheel rehearsal.

## Stop conditions

Stop and open a new executable corrective if:

- canonical records cannot be rebound without changing executable or
  qualification inputs;
- review finds that the HTTPX/HTTPX2 tests recorded in the closure plan did
  not actually execute against `d4979f1...`;
- any acceptance criterion from the completed architecture plans is not
  supported by existing evidence;
- the current branch contains executable changes after `d4979f1...`;
- a profile/API/residual difference changed and therefore requires genuine
  requalification rather than bookkeeping repair.

The expected outcome is a small exact-SHA evidence repair, not another
implementation campaign.
