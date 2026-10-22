# Release and Verification Roadmap

Status: active — M006 and M007 conditionally closed

Long-term references:

- `plans/000-long-term-specification.md` §6, §7
- `plans/001-terminology-and-domain-model.md` §10, §11
- `plans/002-long-term-roadmap.md` Phases 1–3

Related ADRs:

- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`

Normative policy: `docs/verification-policy.md`. Workflow:
`.skills/verification-qualification.md`.

## 1. Purpose and ownership boundary

Owns validation and release integrity: Tier 1/2/3 gates, API oracles,
packaging checks, security preflight, publication order, wheel matrix, and
the live Stage C binding renewal process.

Consumes: engine/adapter evidence from other subsystems. Must not own:
product behavior changes, CI redesign (one automatic job; additions need
explicit approval + regression history), publish automation.

## 2. Work classification

### Invariants

- `scripts/check.sh` is the single validation source; CI repeats it.
- Completed plans non-normative; historical qualification never gates.
- Publication manual; no workflow publishes, tags, or authorizes a SHA.
- Live binding exact-SHA; docs-only descendants do not invalidate.

### Capabilities

- Reproducible crates.io publication order; 18-wheel + 1-sdist PyPI matrix
  with build-only rehearsal mode.

### Infrastructure

- Oracle tooling (pinned `cargo-public-api`/`cargo-semver-checks`/nightly),
  manifest generators, compat profiles, coverage validators.

### Polish

- Release notes, classifier/coverage alignment, operator docs.

## 3. Non-goals

- New CI jobs/matrices/evidence schemas without explicit request.
- Behavior changes disguised as release work; H3/Node graduation claims.

## 4. Current state

Tier 1/2/3 + security preflight are defined on the live freeze; six-profile
oracle vs the oracle baseline and both API oracles have zero unexplained
delta. M001A closed on candidate `41757569123c0b8038550b956d8b244ab55094a6`.
M002 run `36223505399` then proved all 18 Python 3.10–3.15 wheels plus one
sdist with `publish=false` on that exact candidate.
The coordinated 0.2.1 release remains closed and immutable across crates.io,
signed tag, PyPI, and GitHub Release. M004 build-only run `36253723990` and
publish run `36255517733` are historical green evidence.

Core-transport M007 is closed and its additive TransportFailureKind /
Error::transport_failure_kind() API is now published: M005 released
coordinated 0.2.2 from candidate
`015a56d7ec3edf186eec8ebccff01cbf5584274e`, which is also the live Stage C
binding. All six crates.io packages, the signed `v0.2.2` tag, the 19 PyPI
files, and GitHub Release `401746856` are public and were verified from a
fresh consumer. The release gate also found and fixed one real defect on that
candidate (`eggfetch-ffi` did not compile where `c_char` is `u8`); details and
the full evidence set are in
`plans/closure/release-verification/005-0-2-2-m007-publication.md`.

## 5. Target architecture

M005 published the already-qualified post-0.2.1 capability without adding
runtime feature work. The workstream is now back at steady-state
release/corrective intake; no standing automation growth.

## 6. Dependency graph

```text
core-transport M006C2 post-closure reconciliation (closed)
    |
    v
M001A 0.2.1 candidate preparation (closed)
    |
    v
M002 Python 3.15 wheel rehearsal (closed)
    |
    v
M004 tagged 0.2.1 release finalization (closed; superseded unexecuted M001B)

M001 umbrella closed after M004.
M003 standing corrective intake remains soft/ongoing.

core-transport M007 closure
    |
    v
M005 0.2.2 M007 publication (closed)

M005 consumed the closed M007 evidence and owned candidate versioning,
requalification, rehearsal, publication, and release closure. Future work
enters through M003 as a bounded corrective or feature plan.
```

## 7. Milestones

### Milestone 1 — 0.2.x coordinated publication

Class: infrastructure (operational). Status: closed.

Umbrella plan:

- `plans/archive/implementation/release-verification/001-release-publication.md`

Execution history / sequence:

1. `plans/implementation/release-verification/001a-0-2-1-release-candidate-preparation.md` — closed
2. `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` — closed
3. `plans/implementation/release-verification/001b-0-2-1-coordinated-publication.md` — superseded before execution
4. `plans/archive/implementation/release-verification/004-0-2-1-tagged-release-finalization.md` — closed

The existing `v0.2.0` tag points at an older release commit and is immutable
history. The next coordinated identity is `0.2.1`; do not move/reuse v0.2.0.

Exit conditions: satisfied. A truthful docs-clean final 0.2.1 candidate is frozen and
rehearsed on its exact SHA, six crates are published in dependency order,
signed `v0.2.1` targets that candidate, PyPI publishes 18 wheels + 1 sdist
from that tag through OIDC, a non-draft/non-prerelease GitHub Release exists
for the same tag, and external registry/install smoke passes.

### Milestone 2 — Python 3.15 wheel production rehearsal

Class: capability (packaging). Status: closed.

Implementation plan:

- `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md`

The earlier flat-file detail is absent from the migrated planning tree; the
maintained plan is `plans/archive/implementation/release-verification/002-python-315-wheel-rehearsal.md` (closed).

Objective: `publish=false` rehearsal from the exact M001A 0.2.1 candidate;
prove 18 wheels + 1 sdist, including Linux/macOS/Windows Python 3.15 rows.
Closed by run `36223505399` on
`41757569123c0b8038550b956d8b244ab55094a6`; M004 renewed the rehearsal on
the final candidate in run `36253723990`.

Exit conditions: 19 distributions assemble; Tier 1 + package green; bounded
3.15-only prerelease fallback documented.

### Milestone 4 — 0.2.1 tagged release finalization

Class: infrastructure / operational release. Status: closed.

Implementation plan:

- `plans/archive/implementation/release-verification/004-0-2-1-tagged-release-finalization.md`

Objective: reconcile stale release-facing documentation, freeze a docs-only
final candidate, renew the exact-SHA wheel rehearsal, and complete coordinated
crates.io, signed tag, PyPI, GitHub Release, external smoke, and planning
closure. This supersedes the unexecuted M001B plan.

Closed on candidate `fe4d596ca8d91694fc887566acaa7875b918d25a`; build-only
run `36253723990`; PyPI publish run `36255517733`. Closure records:
`plans/closure/release-verification/004-0-2-1-tagged-release-finalization.md`
and `plans/closure/release-verification/001-coordinated-0-2-1-publication.md`.

Exit conditions: satisfied; all acceptance criteria passed and the M001
umbrella closure is written.

### Milestone 3 — Standing corrective intake

Class: process. Status: proposed (standing).

Objective: route each future corrective through a bounded plan + closure +
renewed binding where applicable. Repeated correctives in one subsystem
force a roadmap revision.

### Milestone 5 — 0.2.2 M007 publication and release polish

Class: infrastructure / operational release with bounded polish. Status: closed.

Implementation plan:

- `plans/implementation/release-verification/005-0-2-2-m007-publication.md`

Objective: publish the closed M007 native transport failure classifier as the
coordinated 0.2.2 release, renew the exact-SHA candidate binding after
version/package edits, rehearse the full PyPI matrix, publish all public
channels, and reconcile release/planning documentation.

Hard dependency: core-transport M007 closed (satisfied).

Closed on candidate `015a56d7ec3edf186eec8ebccff01cbf5584274e`; build-only
rehearsal run `36991057953` (18 wheels + 1 sdist, `publish=false`); PyPI
publish run `36994291293`. Closure record:
`plans/closure/release-verification/005-0-2-2-m007-publication.md`.

Exit conditions: satisfied. Six crates + PyPI at 0.2.2, signed v0.2.2 on the
exact frozen candidate, 18-wheel + 1-sdist rehearsal and publication green,
GitHub Release `401746856` present, fresh Rust/Python install smoke green,
and closure evidence recorded. One low-severity compatibility-fixture finding
was routed to M003; no high-severity finding remains open.

### Milestone 6 — Verification guard hardening

Class: invariant. Status: ready.

Implementation:
`plans/implementation/release-verification/006-verification-guard-hardening.md`.

Objective: close four defects in the repository's own verification tooling
that currently let a regression pass unnoticed or let a release gate report
success while doing nothing.

Dependencies:
- hard: none;
- interface: `docs/verification-policy.md`, whose rule that Tier 2 "fails,
  never skips" is extended from the toolchain gate to the downstream
  portfolio;
- interface: the bare-`RUSTFLAGS=` treatment that the Tier 1 contract matrix
  already applies and documents.

Deliverable boundary: the lint-suppression guard matches blanket lints in any
attribute position including the repository's own `reason = "…"` idiom,
covers `expect`, scans `fuzz/`, and gains a `--self-test` that asserts it
*rejects* what it must reject; the downstream Tier 2 step fails rather than
skips, or gains a repository-owned artifact producer, and no longer refers to
a workflow that does not exist; `tier2_feature_matrix` and `tier2_msrv` apply
the bare-`RUSTFLAGS=` treatment so Tier 2's outcome no longer depends on the
caller's environment; the H3 qualification runner's self-test runs in Tier 1
without any network access.

User or operator value: four states that currently report success while
providing no signal stop doing so, and Tier 2 becomes environment
independent.

Exit conditions: the lint guard's self-test is green in Tier 1 and fails when
the guard is broken; the downstream step does not report success while
skipping; Tier 2's feature matrix and MSRV gate pass with and without
`RUSTFLAGS="-D warnings"` while the all-features check still fails on a
warning; the H3 self-test runs in Tier 1 with no H3 conformance claim; CI
remains a single job; Tier 1/2 green with renewed exact-SHA binding.

Deferred: H3 graduation and any network-dependent H3 qualification, which
remain separately gated; adding any CI job or matrix.

### Milestone 7 — Planning and documentation truthfulness hygiene

Class: polish. Status: ready.

Implementation:
`plans/implementation/release-verification/007-planning-and-documentation-hygiene.md`.

Objective: make the repository's planning and qualification records state
what is actually true.

Dependencies:
- hard: release-verification M006 for the lint-suppression justification
  pass, because M006 corrects the guard that should have caught it;
- otherwise none; this is a records milestone and must not delay the
  correctness milestones.

Deliverable boundary: every unticked acceptance criterion under `plans/` is
classified and dispositioned, with `plans/core-private-module-decomposition.md`
handled individually because another plan cites it as a prerequisite; the
pending-maintainer-actions section returns to the top of `plans/README.md`
and `AGENTS.md` becomes accurate again; `fuzz/Cargo.lock` is refreshed off the
stale `eggfetch-core 0.1.0` pin; the compat profiles' historical SHA comment
history moves out of the machine-read files into the canonical ledger with
their machine-readable content byte-identical; the two near-identical httpx
replacement packages are reconciled; every `allow`/`expect` attribute in
`crates/*/src` carries a justification.

User or operator value: a reader can rely on the planning records. At the
baseline 83 plan files carry 2,197 unchecked acceptance criteria beside
completion records, `AGENTS.md` points at a section that does not exist, and
the compat profiles retain eleven historical SHAs in comments.

Exit conditions: no plan file carries a complete execution record while
retaining unticked boxes describing unmet conditions; `AGENTS.md` is accurate;
`fuzz/Cargo.lock` records `eggfetch-core 0.2.2`; both profiles' machine-readable
content is byte-identical; the replacement packages are reconciled with the
publish guard and identity verification still passing; every suppression is
justified with no lint set changed; Tier 1/2 green.

Deferred: moving the 249 flat legacy plan files into `plans/archive/`; the
CLI's 581-line `run()` decomposition, which needs a maintainer ownership
decision because no subsystem roadmap owns the CLI adapter; the `tracing`
feature's missing test and consumer, which is a capability decision; the Node
adapter's missing tests and the FFI's missing capabilities, which are
registered gated futures.

## 8. Cross-cutting requirements

Security preflight live before publication; Trusted Publishing (OIDC) for
PyPI; clean worktree for packaging; no `--allow-dirty`.

## 9. Verification strategy

Tier 2 + Tier 3 + `check_security.sh` for publication milestones; Tier 1 +
package + rehearsal artifacts for wheel qualification; per-corrective gates
for M003. M005 additionally requires exact-SHA Stage C renewal after candidate
version/package edits and a build-only 19-distribution rehearsal on that same
candidate.

## 10. Risks and decision points

Risk: publication pressure skipping rehearsal. Control: M002 gate is
explicit — no 3.15 claim without rehearsal evidence.

Risk: publishing before the mandatory Windows qualification is complete.
Control: M006C1 now records both the direct native-Windows EggFetch matrix
and corrected EggReplay M013F hosted-Windows 300 KiB rerun. Continue to follow
the existing maintainer-run publication and wheel-rehearsal procedures.

## 11. Completion definition

M001/M004 and M005 are all closed release evidence; 0.2.2 is the current
public version. M003 remains standing process, never "complete," and now holds
one low-severity fixture finding. HTTPX 1.0 and H3 retain their independent
external gates.

The subsystem reopens for M006 and M007, registered from a repository audit
that found the lint-suppression guard unable to match the repository's own
suppression idiom, a release-blocking downstream gate that silently skipped,
a Tier 2 feature matrix that failed under the flag CI sets, an H3 tooling
self-test that no tier ran, and planning records whose acceptance criteria
were never evaluated.

**Standing Stage C intake (added 2026-10-22).** Four milestones were
implemented and conditionally closed outside this subsystem on 2026-10-22:
TLS/protocol M005, python-httpx-compat M004 and M005, and core transport
M008. Each changes executable inputs, so each invalidates the live Stage C
binding, and none is committed yet. The rebinding work — commit the tree,
freeze the SHA, run the full Tier 1 / Tier 2 / Tier 3 qualification on it,
and rebind both HTTPX profiles in `compat/httpx/0.28.1/profile.toml` and
`compat/httpx2/2.12.0/profile.toml` plus the ledger's Stage C SHA — is
release-verification intake work and is tracked as pending maintainer action 2
in `plans/registry.md`. It is deliberately **not** folded into M006 or M007:
M006 hardens the gates and M007 is polish, while the rebinding is a
qualification run against a specific frozen commit and must not be coupled to
either.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 umbrella | closed | `plans/archive/implementation/release-verification/001-release-publication.md` | `plans/closure/release-verification/001-coordinated-0-2-1-publication.md` | — |
| M001A 0.2.1 candidate prep | closed | `plans/implementation/release-verification/001a-0-2-1-release-candidate-preparation.md` | `plans/closure/release-verification/001a-0-2-1-release-candidate-preparation.md` | — |
| M002 Python 3.15 rehearsal | closed | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | `plans/closure/release-verification/002-python-315-wheel-rehearsal.md` | — |
| M001B 0.2.1 publication | superseded | `plans/implementation/release-verification/001b-0-2-1-coordinated-publication.md` | — | Replaced before execution by M004 |
| M004 0.2.1 tagged release finalization | closed | `plans/archive/implementation/release-verification/004-0-2-1-tagged-release-finalization.md` | `plans/closure/release-verification/004-0-2-1-tagged-release-finalization.md` | — |
| M003 intake | proposed | — | — | — |
| M005 0.2.2 M007 publication | closed | `plans/implementation/release-verification/005-0-2-2-m007-publication.md` | `plans/closure/release-verification/005-0-2-2-m007-publication.md` | — |
| M006 verification guard hardening | conditionally closed | `plans/implementation/release-verification/006-verification-guard-hardening.md` | `plans/closure/release-verification/006-verification-guard-hardening.md` | Stage C rebinding plus the frozen-SHA Tier 1/2/3 gates. M007 may proceed. |
| M007 planning/documentation truthfulness hygiene | conditionally closed | `plans/implementation/release-verification/007-planning-and-documentation-hygiene.md` | `plans/closure/release-verification/007-planning-and-documentation-hygiene.md` | Stage C rebinding plus the frozen-SHA Tier 1/2/3 gates. |
