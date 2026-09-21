# Post-Maintenance Documentation-State Polish Pass

Planning baseline: `e7371e3722dbbe61aa9004ef3c16f6b9fd866816` (`main`, 2026-09-21)
Qualified corrective freeze: `18c1f96c1cbf9d71aa480030b0f365c85267620b`
Qualified evidence descendant: `1f8daae4599361d6fb2acd2d8fbca6e38b4fc4fe`
Current documentation/index head at planning time: `e7371e3722dbbe61aa9004ef3c16f6b9fd866816`
Parent program: `plans/api-preserving-maintenance-and-interop-hardening-program.md`
Closure corrective: `plans/post-maintenance-closure-evidence-corrective-pass.md`
Reference contracts: `httpx==0.28.1`, `httpx2==2.12.0`
Normative verification policy: `docs/verification-policy.md`
Date: 2026-09-21

## Objective

Perform one documentation/state-only polish pass after the completed
API-preserving maintenance closure.

The implementation and qualification work are already complete. The live
machine-readable Stage C profiles, parity ledger, and residual-difference
record correctly bind HTTPX 0.28.1 and HTTPX2 2.12.0 to corrective freeze
`18c1f96c1cbf9d71aa480030b0f365c85267620b`, and current-head remote CI is
green.

Two remaining documentation defects prevent the closure state from being
fully self-consistent:

1. `docs/reference/compatibility.md` correctly names `18c1f96...` near its
   top-level qualification statement, but a later **Current qualification**
   block still incorrectly names the historical `bc4800ee...` freeze;
2. `plans/post-maintenance-closure-evidence-corrective-pass.md` still reads
   as an unexecuted handoff: acceptance boxes remain unchecked and no final
   execution/closure record was appended, even though `plans/README.md`
   already marks the corrective complete. The parent program addendum also
   retains the conditional wording that the program closes only after remote
   CI becomes green, while that CI condition is now satisfied.

This pass must make those records truthful without changing executable,
test, validation, profile, compatibility-policy, release, or qualification
state.

## Confirmed planning-time state

At planning time:

- current `main` is
  `e7371e3722dbbe61aa9004ef3c16f6b9fd866816`;
- corrective freeze
  `18c1f96c1cbf9d71aa480030b0f365c85267620b` contains the final
  test/validation changes;
- `1f8daae4599361d6fb2acd2d8fbca6e38b4fc4fe` contains only
  profile/ledger/plan/compatibility-documentation renewal after that freeze;
- `e7371e3722dbbe61aa9004ef3c16f6b9fd866816` is a later plan-index-only
  closure descendant;
- remote CI run `35624022656` passed on `1f8daae...`;
- remote CI run `35625522745` passed on current head `e7371e3...`;
- both live compatibility profiles record:
  - `stage = "stage-c-qualified"`;
  - `status = "qualified"`;
  - `qualification-sha = "18c1f96c1cbf9d71aa480030b0f365c85267620b"`;
  - `qualification-date = "2026-09-21"`;
  - `previous-qualification-sha = "bc4800ee9428f0fd11d7d0b914c489b444fe93fc"`;
- `plans/httpx-parity-correction-status.md` records `18c1f96...` as the
  current Stage C state;
- `docs/residual-differences.md` records `18c1f96...` as the current Stage C
  state;
- `docs/reference/compatibility.md` contains one stale later block that still
  calls `bc4800ee...` current;
- `plans/README.md` already marks the post-maintenance closure-evidence
  corrective and parent maintenance program complete;
- issue #24 remains open/fixed-but-unpublished;
- the separate Python 3.15 18-wheel rehearsal remains pending.

## Scope constraints

This is documentation/state polish only.

Allowed changes:

- `docs/reference/compatibility.md`;
- `plans/post-maintenance-closure-evidence-corrective-pass.md`;
- `plans/api-preserving-maintenance-and-interop-hardening-program.md`;
- `plans/README.md`;
- this plan file itself;
- other documentation-only files only if a direct contradiction with the same
  final closure state is discovered during the consistency sweep.

Do not modify:

- Rust, Python, JavaScript, C, or shell executable source;
- tests;
- validation/oracle scripts;
- Cargo/Python package metadata;
- feature/default wiring;
- CI workflows;
- compatibility allowlists;
- compatibility profile `qualification-sha` values;
- parity behavior;
- transport/TLS/proxy/timeout/decompression semantics;
- public API;
- release/tag/publication state;
- issue #24 state unless publication independently completed before this pass;
- Python 3.15 qualification state unless its separate rehearsal independently
  completed before this pass.

If any executable/test/validation/profile-semantic change appears necessary,
stop this polish pass and open a separately attributable corrective instead.

## Part 1 — repair the stale compatibility-reference block

Update `docs/reference/compatibility.md`.

The later **Current qualification** block must agree with the already-correct
top-level qualification statement and live profiles.

Required current state:

- HTTPX 0.28.1: Stage C qualified;
- HTTPX2 2.12.0: Stage C qualified;
- executable/test/tooling freeze:
  `18c1f96c1cbf9d71aa480030b0f365c85267620b`;
- qualification date: 2026-09-21;
- qualification reason/context: maintenance closure-evidence corrective after
  the API-preserving maintenance campaign;
- `bc4800ee9428f0fd11d7d0b914c489b444fe93fc` is historical;
- `df2549f7c64ebfccde61ed36fef785d39e83b38d` is intermediate maintenance
  implementation evidence, not a live Stage C binding;
- the live ledger remains
  `plans/httpx-parity-correction-status.md`.

Do not alter surrounding compatibility claims, allowed differences, feature
coverage, HTTPX2 surface description, H3/Node maturity, or Python-version
claims merely to rewrite the SHA block.

### Acceptance

- [ ] No block in `docs/reference/compatibility.md` calls `bc4800ee...`
      the current qualification.
- [ ] Every current-qualification statement in the file names `18c1f96...`.
- [ ] Historical references to `bc4800ee...` remain clearly historical.
- [ ] No compatibility contract text changes beyond closure-state truth.

## Part 2 — close the corrective plan record itself

Update
`plans/post-maintenance-closure-evidence-corrective-pass.md`.

Do not rewrite the handoff requirements. Preserve them as the implementation
record and append a final execution/closure section that records what actually
landed.

The execution record must include:

- planning baseline
  `5ced637af7479b95b6454697e638c7a3759d8670`;
- original maintenance implementation freeze
  `df2549f7c64ebfccde61ed36fef785d39e83b38d`;
- final corrective test/tooling freeze
  `18c1f96c1cbf9d71aa480030b0f365c85267620b`;
- evidence/profile/docs descendant
  `1f8daae4599361d6fb2acd2d8fbca6e38b4fc4fe`;
- final index-only closure descendant
  `e7371e3722dbbe61aa9004ef3c16f6b9fd866816` at the start of this polish
  pass;
- the relational Python negative-evidence result;
- the malformed SSLContext private-contract negative-evidence result;
- six-profile Rust exact public API result;
- Rust semver result;
- Tier 1, extended, package, security, exact MSRV, compatibility/API-oracle
  results as already recorded;
- policy-defined Node/downstream skips as skips;
- remote CI run `35624022656` on `1f8daae...`;
- remote CI run `35625522745` on `e7371e3...`;
- canonical Stage C renewal to `18c1f96...`;
- explicit statement that no public API/capability/feature/default/
  compatibility-waiver change occurred;
- issue #24 and Python 3.15 remain separate pending work.

Mark the plan's acceptance checkboxes complete only where the recorded
evidence actually satisfies them. Do not fabricate evidence to make every box
green; any unsupported box must remain unchecked with a short explanation.

### Acceptance

- [ ] The corrective plan contains a final execution/closure record.
- [ ] Its checkbox state agrees with the execution record.
- [ ] The record distinguishes test/tooling freeze from later docs-only heads.
- [ ] Remote CI closure is explicitly recorded.
- [ ] No statement implies `1f8daae...` or `e7371e3...` is the executable
      qualification freeze.

## Part 3 — finalize the parent-program wording

Update
`plans/api-preserving-maintenance-and-interop-hardening-program.md`.

The current corrective addendum ends with a conditional statement that the
program closes only after the corrective's remote CI descendant is green.
That condition is now satisfied.

Append or minimally amend the addendum to state:

- remote CI passed;
- final live Stage C binding is `18c1f96...`;
- the closure-evidence corrective is complete;
- the parent maintenance program is fully closed;
- issue #24 publication and Python 3.15 wheel rehearsal remain outside this
  program and still pending independently.

Do not replace the historical original closure text at the top of the parent;
the addendum exists specifically to preserve the history of the first closure
and subsequent correction.

### Acceptance

- [ ] The addendum no longer leaves closure conditional on a future CI event.
- [ ] The parent explicitly records the completed corrective and live freeze.
- [ ] Historical `df2549f7...` implementation evidence remains intact.
- [ ] No unrelated historical plan text is rewritten.

## Part 4 — run a narrow closure-state consistency sweep

Before finalizing the polish, inspect the current documentation/state files
that are authoritative or directly affected:

- `plans/README.md`;
- `plans/api-preserving-maintenance-and-interop-hardening-program.md`;
- `plans/post-maintenance-api-requalification-and-state-closure.md`;
- `plans/post-maintenance-closure-evidence-corrective-pass.md`;
- `plans/httpx-parity-correction-status.md`;
- `compat/httpx/0.28.1/profile.toml`;
- `compat/httpx2/2.12.0/profile.toml`;
- `docs/residual-differences.md`;
- `docs/reference/compatibility.md`;
- `docs/verification-policy.md` only to verify terminology/policy, not to
  change policy.

Verify that all live/current references agree on:

- Stage C freeze `18c1f96...`;
- documentation descendants are not executable freezes;
- HTTP/3 remains experimental;
- Node remains experimental;
- no new compatibility waiver exists;
- issue #24 is fixed/qualified but publication-pending;
- Python 3.15 wheel rehearsal remains pending.

Historical sections may retain older SHAs when clearly labeled historical.
Do not mass-rewrite historical evidence.

### Acceptance

- [ ] All live/current Stage C references agree on `18c1f96...`.
- [ ] Older SHAs appear only in historical/intermediate context.
- [ ] No current wording contradicts the machine-readable profiles.
- [ ] Issue #24 and Python 3.15 state remain truthful.

## Part 5 — plan index registration and closure

At implementation start, `plans/README.md` should register this plan as an
active documentation-state polish pass.

After Parts 1–4 are complete:

- mark this plan Completed;
- retain the parent maintenance program as complete;
- do not reopen the closure-evidence corrective;
- record the final docs-only commit/head;
- state explicitly that the qualified executable/test/tooling freeze remains
  `18c1f96...`;
- retain issue #24 and Python 3.15 as the only independent pending items
  relevant to these records.

## Verification

Because this pass is documentation/state only, do not rerun the full
qualification suite unless implementation unexpectedly changes a
qualification input.

Required deterministic checks:

1. confirm the diff from
   `18c1f96c1cbf9d71aa480030b0f365c85267620b` to the final polish head
   contains no executable, test, validation, workflow, manifest, or build
   changes beyond the already-qualified corrective;
2. confirm both profile `qualification-sha` values remain exactly
   `18c1f96c1cbf9d71aa480030b0f365c85267620b`;
3. confirm no current qualification statement in directly affected docs names
   `bc4800ee...`;
4. confirm the live ledger still names `18c1f96...`;
5. run formatting/lint checks applicable to Markdown/TOML only if the
   repository already provides them;
6. push the final docs-only descendant and require ordinary GitHub CI to pass.

Routine CI on the docs-only descendant is closure evidence; it does not create
a new executable qualification freeze.

## Final acceptance criteria

- [ ] The stale `docs/reference/compatibility.md` Current qualification block
      names `18c1f96...`, not `bc4800ee...`.
- [ ] The closure-evidence corrective plan has an execution/closure record.
- [ ] The closure-evidence corrective plan's checkboxes accurately reflect
      completed evidence.
- [ ] The parent maintenance addendum records that remote CI succeeded and the
      program is fully closed.
- [ ] Both compatibility profiles remain bound to `18c1f96...`.
- [ ] The parity ledger and residual-difference doc remain bound to
      `18c1f96...`.
- [ ] Documentation descendants are clearly separated from the qualified
      freeze.
- [ ] No executable/test/validation/build/workflow/manifest change is part of
      this polish.
- [ ] No compatibility waiver, API change, feature/default change, H3
      graduation, or Node maturation occurs.
- [ ] Issue #24 remains fixed-but-unpublished unless publication independently
      completes.
- [ ] Python 3.15 remains pending unless its separate rehearsal independently
      completes.
- [ ] Final ordinary remote CI is green.
- [ ] `plans/README.md` marks this polish pass complete only after that CI
      result.

## Stop conditions

Stop this documentation-state pass and open separate work if:

- any executable/test/validation/build/workflow change is required;
- the two machine-readable profiles disagree with one another;
- the live parity ledger disagrees with the profiles in a way requiring
  requalification;
- correcting the documentation would require changing a compatibility claim
  rather than merely its evidence SHA/state wording;
- issue #24 publication or Python 3.15 qualification becomes necessary for
  closure;
- a new API, feature/default, compatibility waiver, H3 maturity, or Node
  maturity change is proposed.

The successful result is a repository whose current documentation and plan
state tell exactly the same story as its already-qualified machine-readable
evidence, with no new qualification campaign.
