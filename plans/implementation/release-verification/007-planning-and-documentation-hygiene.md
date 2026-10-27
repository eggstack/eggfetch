# Release and Verification Milestone 007 — Planning and Documentation Truthfulness Hygiene

Status: ready

Repository baseline: `873ea28541c82817317430c9523fca8525d99c81`

Source roadmap:

- `plans/subsystems/release-verification-roadmap.md#milestone-7--planning-and-documentation-truthfulness-hygiene`

Long-term requirements:

- `plans/000-long-term-specification.md` §8
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 items 2 and 4

Applicable ADRs:

- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`

Primary class: polish

## 1. Objective

Make the repository's planning and qualification records state what is
actually true, so that a reader can rely on them.

Six items are in scope. Each is a truthfulness defect: a record that claims
something the repository does not do, or that leaves a reader unable to tell
whether a condition was met.

1. **Plan acceptance criteria are not evaluated.** 83 plan files under
   `plans/` carry 2,197 unchecked `- [ ]` boxes. The files whose own record
   says the work was executed are the sharpest problem: a file can carry a
   completion record and still leave its criteria unticked, so a reader
   cannot tell "met" from "assumed". The sharpest example is
   `plans/core-private-module-decomposition.md`: ten acceptance criteria at
   lines 188-197 remain unticked while line 205 reads
   `## Execution record — complete (2026-09-21)`. That file is cited as a
   prerequisite by `plans/connect-wire-overlap-conformance.md`.

2. **`AGENTS.md` points at a section that does not exist.** `AGENTS.md` line 4
   says the "pending maintainer actions" are at the top of `plans/README.md`.
   `grep -ci 'pending maintainer' plans/README.md` returns `0`. The section
   was removed without updating the pointer.

3. **`fuzz/Cargo.lock` records a stale core version.**
   `fuzz/Cargo.lock` pins `eggfetch-core 0.1.0` while the crate is at
   `0.2.2`. The fuzz tree has its own lock file, outside the root lock, and
   no script validates it.

4. **The compat profiles accumulate a historical log in comments.**
   `compat/httpx/0.28.1/profile.toml` is 207 lines, of which 161 are
   comments retaining the historical SHA log — one full 40-character SHA
   string and sixteen distinct abbreviated 8-character SHA prefixes, the
   latter repeated across the comment block. Only 41 lines are
   machine-readable content once blank lines are excluded; 46 non-comment
   lines remain if blanks are counted. The same pattern applies to the
   httpx2 profile.

5. **Two near-identical httpx replacement packages.**
   `compat/httpx-shim/` and `compat/httpx-controlled-replacement/` contain
   functionally identical 14-line `httpx/__init__.py` modules. Only the
   controlled-replacement sets `__eggfetch_shim__ = True`, which is what
   `scripts/run_isolated_downstream.py` requires. The shim is referenced
   only by `docs/architecture/overview.md` and by plan history; no script or
   workflow uses it.

6. **Lint suppressions lack justification.** 189 `allow`/`expect` attributes
   exist in `crates/*/src`. 44 use Rust's `reason =` field. The remainder
   rely on an adjacent comment, and a measurable minority carry no
   justification at all.

## 2. Why this milestone is ready

- Every item is a record-keeping correction. None changes runtime behavior,
  public API, cargo features, or a documented contract.
- Item 1 is the highest-value correction: an acceptance criterion that is
  never evaluated cannot distinguish "met" from "assumed", and a plan cited
  as a prerequisite is only as trustworthy as its own criteria.
- Item 2 is required for the planning system's own visibility rule to
  work: `plans/003-planning-process.md` requires pending maintainer actions
  to stay visible at the top of `plans/README.md` and in the registry.
- Item 3 is a one-line version correction with a concrete failure mode: a
  stale fuzz lock can resolve dependencies differently from the root lock.
- Item 4 conflicts with the repository's own rule that the live SHA lives in
  the canonical qualification records and nowhere else. Comments retaining
  sixteen abbreviated historical SHA prefixes and one full SHA in a
  machine-read file are exactly the duplication that rule forbids.
- Item 5 is a bounded consolidation: the qualification path uses only the
  controlled-replacement, and the shim's `__eggfetch_shim__` marker is the
  only functional difference.
- Item 6 has a mechanical resolution, and release-verification M006 corrects
  the guard that should have caught it, so this milestone supplies the
  missing justifications.

Per `plans/003-planning-process.md` §5, polish follows correctness closure.
This milestone is scheduled after the correctness milestones in the execution
order, and does not block them.

## 3. Current implementation evidence

At the baseline:

- 83 files under `plans/` contain unchecked `- [ ]` boxes, 2,197 in total;
- `plans/core-private-module-decomposition.md` has ten unticked acceptance
  criteria at lines 188-197 and `## Execution record — complete (2026-09-21)`
  at line 205;
- `plans/connect-wire-overlap-conformance.md` line 5 reads
  `Prerequisite: plans/core-private-module-decomposition.md`;
- `AGENTS.md` line 4 names the pending maintainer actions section;
  `plans/README.md` contains no such section;
- `fuzz/Cargo.lock` records `eggfetch-core 0.1.0`;
  `crates/eggfetch-core/Cargo.toml` records `0.2.2`;
- `compat/httpx/0.28.1/profile.toml` is 207 lines with 161 comment lines and
  41 non-blank machine-readable lines; those comments retain one distinct
  40-character SHA string and sixteen distinct abbreviated 8-character SHA
  prefixes;
- `compat/httpx-shim/httpx/__init__.py` and
  `compat/httpx-controlled-replacement/httpx/__init__.py` differ only in
  their docstrings and the `__eggfetch_shim__` marker;
- `grep -rn 'httpx-shim' scripts/ .github/` returns nothing;
- `scripts/run_isolated_downstream.py` requires `httpx.__eggfetch_shim__`;
- `scripts/run_downstream_compat.py` selects the
  `httpx-controlled-replacement` artifact type;
- 189 `allow`/`expect` attributes exist in `crates/*/src`; 44 carry a
  `reason =` field.

## 4. Invariants that must not regress

- The canonical long-term documents `plans/000-` through `plans/003-` are not
  modified by this milestone. They are historical direction, not operational
  records.
- The live SHA remains recorded only in the canonical qualification records:
  `plans/httpx-parity-correction-status.md`,
  `compat/httpx/0.28.1/profile.toml` and
  `compat/httpx2/2.12.0/profile.toml`, plus the three documents named in
  `.skills/verification-qualification.md`.
- The machine-readable content of both compat profiles is unchanged. Only
  the comment history is affected.
- The controlled-replacement package keeps its publish guard
  (`check_no_publish.py`) and its identity verification
  (`verify_identity.py`).
- The downstream qualification path keeps working and keeps requiring
  `__eggfetch_shim__`.
- No `allow`/`expect` attribute is removed without its replacement being
  correct; the count of blanket suppressions must not increase.
- `unsafe_code = "forbid"` remains workspace-wide apart from the existing
  FFI and Node overrides.
- No runtime behavior changes.

## 5. Scope

### In scope

- evaluation and disposition of the unticked plan acceptance criteria;
- restoration of the pending-maintainer-actions section in `plans/README.md`,
  and the `AGENTS.md` pointer;
- refreshing `fuzz/Cargo.lock`;
- moving the compat profiles' historical SHA comment history out of the
  machine-read files;
- consolidating the two httpx replacement packages;
- adding `reason =` justifications to the unjustified lint suppressions.

### Explicitly out of scope

- moving the 249 flat legacy plan files into `plans/archive/`, which is a
  separate bulk reorganization;
- the CLI's `run()` function (586 lines at
  `crates/eggfetch-cli/src/main.rs:28-613`, 509 of them non-blank and
  non-comment), which is deferred because the CLI
  adapter has no owning subsystem roadmap and needs a maintainer ownership
  decision first;
- the `tracing` feature's lack of a test or an adapter consumer, which is a
  capability decision rather than hygiene;
- the Node adapter's zero automated tests and the FFI's missing
  capabilities, both of which are registered gated futures;
- the `compat/*/profile.toml` machine-readable content;
- any change to the exact-SHA qualification binding, the live ledger, or the
  published public identities;
- any code change beyond lint-suppression justifications.

## 6. Required production changes

### 6.1 Evaluate the unticked acceptance criteria

For each plan file with unticked acceptance criteria, the implementer must
classify every unticked box as one of:

- **met** — the condition holds; tick the box and, where the evidence is not
  obvious, cite the evidence inline;
- **not met, superseded** — the condition was overtaken by a later plan;
  tick the box and cite the superseding plan;
- **not met, deferred** — the condition is still open; move it to an
  explicit, dated open-items section in the same file and leave the original
  box unticked with a pointer to that section;
- **not met, never applicable** — the condition does not apply to what was
  actually built; replace it with a note stating why, in the file's own
  words.

The result must not be a mass tick. A file whose execution record says
"complete" must not retain unticked boxes that describe unmet conditions.

`plans/core-private-module-decomposition.md` must be handled first and
individually, because `plans/connect-wire-overlap-conformance.md` cites it
as a prerequisite. If any of its ten conditions is genuinely unmet, that
prerequisite citation must be corrected in the same change.

A plan file that records an execution history must not be rewritten to hide
history. The disposition is recorded alongside the existing record.

### 6.2 Restore the pending maintainer actions section

`plans/README.md` must regain a section at its top listing the actions
currently pending a maintainer decision, each with an owner and a
pointer to the plan or record that needs the decision.

The section must reflect reality: the decisions currently pending are the
ADR acceptances for ADR-0008 and ADR-0009, the execution-order choice among
the newly registered milestones, and any decision the new plans explicitly
defer to a maintainer.

`AGENTS.md` line 4 must then be accurate. If the section is restored,
`AGENTS.md` needs no change; if the section is not restored, `AGENTS.md` must
be corrected. The implementer must choose one and make both consistent.

### 6.3 Refresh the fuzz lock

`fuzz/Cargo.lock` must be regenerated so that it records the current
`eggfetch-core` version and a dependency set consistent with the root
`Cargo.lock`.

The implementer must confirm that the fuzz tree still builds against the
refreshed lock, and must record whether the refresh changed any fuzz
dependency version. A version change in a fuzz dependency must be called out
explicitly, because the fuzz tree is not covered by the root lock's review.

The build is not implied by the lock refresh. Two pre-existing API drifts in
the fuzz harness — `fuzz/fuzz_targets/fuzz_headers.rs` and
`fuzz/fuzz_targets/fuzz_multipart.rs` — blocked the build against the
refreshed lock and had to be fixed before the lock could be used. Those
fixes are the main agent's edits, not the milestone's; see §17.

### 6.4 Clean the compat profiles

The historical SHA log in the comments of
`compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml`
must be removed from the machine-read files. The history belongs in the
canonical ledger, `plans/httpx-parity-correction-status.md`, which is the
record that already carries the binding history.

The machine-readable content of both profiles must be byte-identical before
and after the change. The implementer must prove this by diffing the
non-comment content. The count to quote is the **non-blank** non-comment
count — 41 lines per profile at the baseline — and not the count that
includes blank lines, so that no phantom change in machine-readable content
is implied. The baseline comment block of each profile holds one full
40-character SHA string and sixteen distinct abbreviated 8-character SHA
prefixes; no SHA string of either shape remains in the comments afterwards.

Each profile must retain a short comment stating where the live binding and
its history live, so that a reader of the profile knows where to look.

### 6.5 Consolidate the httpx replacement packages

The two packages must be reconciled. The acceptable resolutions are:

- **one package.** Keep the controlled-replacement, whose distribution name
  is `httpx`, and delete the shim, updating
  `docs/architecture/overview.md` and any plan that cites the shim. This is
  only acceptable if the shim's user-facing purpose — providing
  `import httpx` in an environment where the real httpx cannot be modified —
  is served by the controlled-replacement's distribution.
- **two packages, one source.** Extract the shared `httpx/__init__.py` body
  into a single module that both packages re-export, so the runtime code
  exists once, and add `__eggfetch_shim__ = True` to both so the
  qualification path's identity check works regardless of which package is
  installed.

The chosen resolution must keep the controlled-replacement's publish guard
and identity verification intact, and must keep the downstream qualification
path working.

### 6.6 Justify the lint suppressions

Every `allow`/`expect` attribute in `crates/*/src` that does not already
carry a `reason =` field and has no adjacent explanatory comment must gain
one.

Rust's `reason = "…"` field is the preferred form because it is
self-documenting and travels with the attribute. The implementer must use
it where the lint accepts it, and use an adjacent comment only where the
lint does not support a `reason` field.

The justification must state **why the suppression is correct**, not
restate the lint name. A suppression whose justification cannot be written
is a signal that the code should change instead; the implementer must record
such cases rather than writing a vacuous justification.

This work is coupled to release-verification M006, which corrects the guard
that should have caught these. M006's guard correction must land first or
together, so that the newly corrected guard is green when this milestone
completes.

## 7. Ordered work packages

1. Inventory the unticked acceptance criteria by file and by count.
2. Handle `plans/core-private-module-decomposition.md` individually, and
   correct its prerequisite citation if needed.
3. Classify and dispose of the remaining unticked criteria.
4. Restore the pending maintainer actions section in `plans/README.md`, and
   make `AGENTS.md` consistent.
5. Regenerate `fuzz/Cargo.lock` and verify the fuzz tree still builds.
6. Remove the historical SHA comments from both compat profiles and prove
   the machine-readable content is unchanged.
7. Reconcile the two httpx replacement packages.
8. Add `reason =` justifications to the unjustified lint suppressions.
9. Update `docs/architecture/overview.md` for the shim consolidation.
10. Run Tier 1 and Tier 2.
11. Renew the exact-SHA Stage C binding if any executable input changed.
12. Write `plans/closure/release-verification/007-planning-and-documentation-hygiene.md`.

## 8. Failure, cancellation, timeout, and pool semantics

- This milestone changes no runtime behavior. No timeout, pool,
  cancellation, retry, redirect or body-lifecycle semantics change.
- The fuzz lock refresh must not change fuzz behavior beyond the dependency
  versions it records; any change in a fuzz target's behavior must be
  reported.
- The lint-suppression justification pass must not change what any
  suppression permits. The lint set of every attribute must be identical
  before and after; only the justification is added.
- The compat profile cleanup must not change the machine-readable content,
  and therefore must not change the qualification binding.

## 9. Compatibility and migration

- No public API change, no cargo feature change, no runtime change.
- No downstream migration is implied.
- The live Stage C binding is unaffected, because the machine-readable
  profile content is unchanged and the ledger is unchanged.
- `fuzz/Cargo.lock` is not a published artifact; refreshing it does not
  affect consumers.
- Removing or consolidating `compat/httpx-shim/` affects only a local
  convenience distribution. If it is published separately, the implementer
  must confirm with a maintainer before deleting it, and record that
  confirmation.
- `plans/` and `docs/` changes are documentation-only and do not invalidate
  the exact-SHA binding. The lint-suppression changes are executable inputs
  and do invalidate it.

## 10. Required tests

### Acceptance criteria disposition

- every unticked box is classified, and the counts per class are recorded;
- no plan file carries an execution record marked complete while retaining
  unticked boxes that describe unmet conditions;
- `plans/core-private-module-decomposition.md` has no unticked acceptance
  criteria, or each remaining one points to an explicit dated open-items
  section;
- `plans/connect-wire-overlap-conformance.md`'s prerequisite citation is
  accurate.

### Pending maintainer actions

- `plans/README.md` has the section, and it lists the decisions actually
  pending;
- `AGENTS.md` line 4 is accurate about where that section lives.

### Fuzz lock

- `fuzz/Cargo.lock` records `eggfetch-core 0.2.2`;
- the fuzz tree builds against the refreshed lock;
- any fuzz dependency version change is recorded.

### Compat profiles

- the non-comment content of both profiles is byte-identical before and
  after;
- no historical SHA remains in either profile's comments;
- both profiles still name where the live binding and its history live.

### Replacement packages

- exactly one or two packages exist, per the chosen resolution;
- the controlled-replacement's publish guard still passes;
- its identity verification still passes;
- the downstream qualification path still resolves `httpx` to the
  eggfetch-backed shim and still finds `__eggfetch_shim__`.

### Lint suppressions

- every `allow`/`expect` attribute in `crates/*/src` carries a `reason =`
  field or an adjacent justification;
- the lint set of every attribute is unchanged;
- the corrected lint guard from release-verification M006 is green.

### Regression

- Tier 1 and Tier 2 are green;
- the workspace test suite is green.

## 11. Required verification commands

Follow `docs/verification-policy.md` and
`.skills/verification-qualification.md`.

Focused first:

```bash
grep -rn '^\s*- \[ \]' plans/ --include='*.md' | wc -l
grep -ci 'pending maintainer' plans/README.md
grep -A2 'name = "eggfetch-core"' fuzz/Cargo.lock
python scripts/check_lint_suppressions.sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Then:

```bash
cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1
cargo build -p eggfetch-fuzz --all-targets 2>/dev/null || cargo check --manifest-path fuzz/Cargo.toml --all-targets
./scripts/check.sh
./scripts/check.sh extended
```

Never parallelize Rust workspace tests; `--test-threads=1` is required.

## 12. Documentation updates

Implementation must update:

- `plans/README.md` (the restored pending maintainer actions section, and
  the new plans' index if the file lists them);
- `AGENTS.md` only if the pending-maintainer-actions pointer is not restored
  as described;
- `docs/architecture/overview.md` (the compat directory description, after
  the shim consolidation);
- `compat/httpx/0.28.1/profile.toml` and
  `compat/httpx2/2.12.0/profile.toml` (comment cleanup only);
- `plans/registry.md`, the subsystem roadmap and `plans/README.md` at
  closure.

## 13. Acceptance criteria

M007 closes only when:

- every unticked acceptance criterion in `plans/` is classified and
  dispositioned, with the per-class counts recorded;
- no plan file carries a complete execution record while retaining unticked
  boxes describing unmet conditions;
- `plans/core-private-module-decomposition.md` is individually resolved and
  its prerequisite citation in `plans/connect-wire-overlap-conformance.md`
  is accurate;
- `plans/README.md` carries the pending maintainer actions section and
  `AGENTS.md` is accurate about it;
- `fuzz/Cargo.lock` records `eggfetch-core 0.2.2` and the fuzz tree builds;
- the machine-readable content of both compat profiles is byte-identical
  before and after, and neither retains a historical SHA in its comments;
- the httpx replacement packages are reconciled, and the controlled
  replacement's publish guard and identity verification still pass;
- every `allow`/`expect` attribute in `crates/*/src` carries a justification,
  with no lint set changed;
- Tier 1 and Tier 2 are green;
- both HTTPX profiles remain bound to the recorded live SHA.

## 14. Stop conditions

Stop and revise this plan rather than improvising if:

- evaluating a plan's acceptance criteria would require rewriting its
  execution history, in which case the file must instead be annotated as
  superseded;
- the fuzz lock refresh would change a fuzz target's behavior, in which case
  that dependency change must be handled as its own change;
- removing the compat profiles' SHA comments would lose history that the
  canonical ledger does not hold, in which case the history must be moved
  into the ledger first;
- deleting `compat/httpx-shim/` would affect a published distribution
  without maintainer confirmation;
- a lint suppression cannot be justified, in which case the code must
  change instead and that change belongs in its own plan;
- this milestone would require modifying `plans/000-` through `plans/003-`.

## 15. Closure evidence required

Create `plans/closure/release-verification/007-planning-and-documentation-hygiene.md` containing:

- implementation commit(s) and the final frozen SHA;
- the acceptance-criteria disposition table, per file and per class;
- the evidence for the `core-private-module-decomposition` resolution;
- the fuzz lock diff and the fuzz build result;
- the profile-content diff proving the machine-readable content is
  unchanged;
- the replacement-package reconciliation, with the publish guard and
  identity verification output;
- the lint-suppression justification inventory;
- the exact Tier 1 and Tier 2 outcomes;
- renewed HTTPX Stage C binding evidence if any executable input changed;
- known limitations, including the deferred items this milestone explicitly
  does not address.

## 16. Handoff notes

Do not mass-tick the acceptance criteria. A mass tick converts a visible
unknown into an invisible false claim, which is worse than the state this
milestone starts from.

The plan files are historical records. Where a condition was overtaken,
record the supersession rather than rewriting the original plan.

This milestone is polish. It must not delay the correctness milestones
registered alongside it, and the execution order in `plans/registry.md`
reflects that.

The deferred items in section 5 are deferred on purpose. The CLI
decomposition needs an ownership decision that no current subsystem roadmap
provides; the `tracing` feature needs a capability decision; the Node and
FFI surface gaps are registered gated futures. Each is recorded as deferred
rather than quietly dropped.

## 17. Implementation outcome — §6.1 through §6.5 (2026-10-22)

This section records what was implemented for §6.1–§6.5. §6.6 (lint-suppression
`reason =` justifications) is owned by a separate change and is **not** covered
here. The closure evidence for this milestone is still to be written by the
maintainer in `plans/closure/release-verification/007-planning-and-documentation-hygiene.md`.

### §6.1 — unticked acceptance criteria

Baseline measurement at the start of this change: **2,267** unticked `- [ ]`
boxes across **88** files under `plans/`. After this change, **1,257** unticked
boxes remain, in **50** files, and every one of them is either a deferred open
item with a pointer or a frozen never-executed record.

Tiers and counts (the three rows sum back to the 2,267-box baseline):

| tier | files | boxes | treatment |
| --- | --- | --- | --- |
| Tier 1 — execution/completion record (in the file, or in a linked sibling status record that the file names) | 39 | 1,010 | individual disposition |
| Tier 2 — no execution record at all | 45 | 1,195 | dated "never executed" note, boxes left unticked |
| Deferred (open items) | 5 | 62 | dated open-items section, boxes left unticked with a pointer |

The tiering is the implementer's rule-based classification, not a scan a reader
can reproduce. The completion markers in this corpus are not uniform —
`## Execution record`, `## Execution update`, `## Implementation status`,
`## Status`, `## Status: …`, `## Closure record (executed …)` and a
front-matter `Status:` line all occur — so a scan for one heading shape
finds materially fewer files than the rule does. The implementer reports
**31** of the 83 legacy flat files under the in-file marker rule and **40**
of the 83 under the broader rule that also accepts a linked sibling status
record the plan names; those linked-sibling records include
`plans/httpx-drop-in-phase-{0,1,2,3,4,5}-status.md` and
`plans/httpx-drop-in-final-native-qualification-and-evidence-closure-status.md`.
The two Tier-2 taxonomy plans (`006-windows-tls-…`, `006c2-…`) are counted in
Tier 2: their `Status: ready` means no execution happened. The marker list
is not published, so the per-file tier assignment is not independently
re-derivable; the closure record's §10 finding records the same limitation.

Of the 83 legacy flat files, **40** carry an execution record under that
broader rule and **43** carry none and are Tier 2 (one file —
`plans/httpx-drop-in-phase-1-production-semantics.md` — is partly deferred
and therefore appears in two tiers).

What a reader can rely on is the measured outcome, which is independently
confirmed: **1,010** boxes newly ticked, **1,257** boxes left unticked
across **50** files, and **0** acceptance violations — every remaining
unticked box carries a dated disposition note or a dated open-items pointer.

Per-class box counts across all 2,267 baseline boxes:

- **met** — 760
- **not met, superseded** — 250 (the three drop-in corrective/phase-6 plans
  whose own status records were marked SUPERSEDED, plus
  `001b-0-2-1-coordinated-publication.md`, which was superseded before execution)
- **not met, deferred** — 1,257 (1,195 in the Tier-2 never-executed files, plus
  the 62 named open items below)
- **not met, never applicable** — 0

The five deferred files (62 boxes still unticked) are
`plans/http3-independent-interop-and-impairment-qualification.md` (26),
`plans/http3-upstream-risk-resource-and-observability-hardening.md` (14),
`plans/httpx-drop-in-phase-1-production-semantics.md` (9),
`plans/httpx-drop-in-phase-1-status.md` (7) and
`plans/release-candidate-final-evidence-and-enforcement-status.md` (6). Each
now carries a dated `## Open items (2026-10-22)` section, and every unticked box
in those files points at an open-item ID in it.

`plans/core-private-module-decomposition.md` was handled first and individually:
all ten criteria were re-verified against the current tree and are met, with
inline evidence (public-surface oracle green on all eight profiles, 223/223
semver checks, `ResolvedRouteKey`/`BoundedClientCache` `diff`-identical across
the pre-split and post-split source, single `build_hyper_client` definition, no
new public items in `client/`/`proxy/`, dependency-set diff against the
planning baseline, and 1,489 focused core tests passing with zero failures).
Its prerequisite citation in `plans/connect-wire-overlap-conformance.md`
(line 5) is therefore **accurate** and was not corrected.

### §6.2 — pending maintainer actions

Already satisfied before this change and verified, not edited:
`plans/README.md` carries `## Pending maintainer actions` at line 22, `AGENTS.md`
line 4 points at it correctly, and the six listed actions were checked against
the state of the tree after this change.

### §6.3 — fuzz lock

`fuzz/Cargo.lock` recorded `eggfetch-core 0.1.0` while
`crates/eggfetch-core/Cargo.toml` records `0.2.2`. The lock was refreshed with
`cargo update --manifest-path fuzz/Cargo.toml --offline -p eggfetch-core`, which
is the smallest change that makes the lock correct:

- `eggfetch-core` 0.1.0 → **0.2.2**;
- `eggfetch-http-connect` 0.2.2 added;
- `base64` 0.22.1 → 0.23.1 (required by the current core);
- `crossbeam-utils`, `dashmap`, `hashbrown`, `lock_api`, `parking_lot_core`,
  `redox_syscall`, `scopeguard` removed (they were only reachable through the
  old core's `dashmap` dependency);
- **134** of the **143** baseline lock packages unchanged in version.

The count is by distinct package name in `fuzz/Cargo.lock`: 143 names at the
baseline, 137 after the refresh, with 1 added, 7 removed and 2 version-changed
(`eggfetch-core` and `base64`). The raw `[[package]]` block count is 146 → 140,
the difference being three packages (`getrandom`, `webpki-roots`,
`windows-sys`) that carry two version entries each. The later dependency
patch that closed the live RustSec advisories moved three further registry
packages in the fuzz lock (`rustls`, `rustls-webpki`, `webpki-roots`) without
changing the package count, so the "unchanged in version" figure above
describes this milestone's own lock refresh.

A full `cargo generate-lockfile --offline` was rejected: it moved 85 packages,
including major-version bumps (`syn` 2→3, `http` 1.4→1.5, `hyper` 1.10→1.12,
`brotli` 8→9, `zstd` 0.13→0.14) that would have changed fuzz-target behavior —
a §14 stop condition.

**The fuzz tree now builds.** The lock refresh alone did not make it compile:
two pre-existing API drifts in the fuzz harness against `eggfetch-core` 0.2.2
blocked the build and had to be fixed before the refreshed lock could be
used. Those fixes are the **main agent's** edits, not this change's:

- `fuzz/fuzz_targets/fuzz_headers.rs:34` — `headers.extend(other)` →
  `headers.extend(&other)`, matching the current
  `Headers::extend(&mut self, other: &Self)` signature;
- `fuzz/fuzz_targets/fuzz_multipart.rs:37` and `:46-50` — `content_length()`
  returns `Option<u64>`, not `usize`; the invariant is now asserted only when
  the length is known.

With those in place, `cargo check --manifest-path fuzz/Cargo.toml --all-targets
--offline` exits **0** and checks all twelve declared fuzz-target binaries.
That green build is recorded evidence in the closure record (§4, row 15); it
was not re-run for this amendment.

### §6.4 — compat profiles

The historical SHA log was removed from the **comments** of both profiles. What
was removed per profile was **one full 40-character SHA string** and
**sixteen distinct abbreviated 8-character SHA prefixes**; the comment blocks
held no other SHA of either shape. The non-blank, non-comment content is
**41 lines per profile** and is byte-identical before and after (verified by
`grep -v '^[[:space:]]*#' | grep -v '^[[:space:]]*$'` before/after `diff`),
and neither profile retains any SHA in its comments. The machine-readable keys
(`rebaseline-sha`, `qualification-sha`, `previous-qualification-sha`,
`historical-current-qualification-sha`, `historical-qualification-sha`) are
unchanged. Each profile keeps a short comment pointing at
`plans/httpx-parity-correction-status.md` for the live binding and its history.

One history item is **not** held by the canonical ledger and needs a ledger
entry from the ledger's owner: `639bf186a71c054e11278d1b160ffe7a6f172c02`
(§14 stop condition, §6.4 note). The fact itself is not lost — it is recorded
in `plans/http3-post-freeze-diagnostics-requalification-corrective-closure.md`
and `plans/http3-production-qualification-evidence.md`.

### §6.5 — httpx replacement packages

Resolution: **two packages, one source**. The runtime code now lives once, in
`compat/httpx-replacement-source/httpx/__init__.py`, and both distributions
package it through setuptools' `package-dir` mapping. Both set
`__eggfetch_shim__ = True`, so the qualification path's identity check passes
regardless of which package is installed. The shim package was **not** deleted:
§14 requires explicit maintainer confirmation for that, and no such
confirmation exists.

Both wheels were rebuilt from the shared source and verified by content, not by
source inspection. `check_no_publish.py` and `verify_identity.py` both pass,
and the downstream identity check passes for the shim and for the controlled
replacement.
