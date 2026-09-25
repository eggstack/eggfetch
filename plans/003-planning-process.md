# eggfetch Planning and Agent-Handoff Process

Status: normative planning governance

This document defines how eggfetch's long-term architecture is translated into
actionable work without allowing short-lived implementation details to
destabilize the canonical specification, terminology, or roadmap.

The keywords MUST, MUST NOT, REQUIRED, SHOULD, SHOULD NOT, and MAY are normative.

Normative complements: `docs/verification-policy.md` (CI/verification/release
policy), `.skills/verification-qualification.md` (tier workflow + exact-SHA
rule). This document governs planning shape; those govern validation truth.

## 1. Purpose

eggfetch requires two distinct planning horizons:

1. **Long-term planning** defines engine identity, adapter boundaries, feature
   profiles, compatibility contracts, invariants, non-goals, and end-state
   acceptance criteria.
2. **Interim planning** defines bounded implementation work against a
   particular repository baseline and is intended for handoff to coding agents.

These horizons MUST remain separate. Interim plans may discover evidence that
warrants a long-term change, but they MUST NOT silently edit long-term
direction to match the easiest implementation.

## 2. Document classes

### 2.1 Canonical long-term documents

Canonical long-term documents are:

- `plans/000-long-term-specification.md`;
- `plans/001-terminology-and-domain-model.md`;
- `plans/002-long-term-roadmap.md`;
- this planning-governance document.

The first three MUST remain stable during ordinary implementation. They MAY be
amended only when:

- product direction has intentionally changed;
- a contradiction or material omission has been identified;
- an accepted ADR requires the canonical end state to change;
- the maintainer explicitly directs a long-term architecture revision.

A corrective implementation pass is not, by itself, justification for changing
a long-term requirement.

### 2.2 Architecture decision records

ADRs capture one architectural decision affecting several milestones,
subsystems, or durable public contracts (engine ownership, pipeline shape,
qualification binding, trust semantics, public-surface containment).

An ADR MUST state context, drivers, considered options, the selected
decision, consequences, affected long-term sections and subsystems,
compatibility/migration implications, verification, and status (proposed,
accepted, rejected, deprecated, superseded).

Accepted ADRs MUST NOT be rewritten to conceal history. A later decision
supersedes the prior ADR and links to it.

### 2.3 Subsystem roadmaps

A subsystem roadmap translates relevant long-term requirements into one
coherent workstream. It is longer-lived than an implementation plan but more
adaptable than the canonical roadmap.

A subsystem roadmap MUST define: subsystem purpose and ownership boundary;
relevant specification and terminology references; invariants and non-goals;
current-state summary; dependency graph; ordered milestones; user-visible
exit conditions; cross-cutting security, migration, protocol, and
observability concerns; known risks and deferred work.

A subsystem roadmap SHOULD avoid commit-specific file lists, exact current
line numbers, and mechanical implementation sequences.

### 2.4 Milestone implementation plans

A milestone implementation plan is the primary handoff artifact for a coding
agent. It MUST be independently executable, bounded, and tied to a repository
baseline (`Repository baseline: <SHA>` + planning baseline where
qualification applies). It MUST include: source roadmap and milestone;
relevant ADRs and long-term requirements; objective and explicit non-goals;
current implementation evidence; invariants that cannot regress; expected
production-code changes; feature-profile and compatibility effects; ordered
work packages; focused and broad verification commands (Tier 1/2/3 as
applicable); static guards and documentation updates; acceptance and stop
conditions; closure evidence required.

An implementation plan MAY change as repository reality changes. Material
deviations MUST be recorded rather than hidden.

### 2.5 Closure records

A closure record determines whether a milestone is actually complete. It MUST
include: implementation commits; requirement-to-evidence matrix; tests and
gates run with outcomes (including exact-SHA requalification where the plan
changed executable inputs); compatibility evidence; security review where
applicable; documentation and operational evidence; known limitations;
unresolved findings by severity; recommendation: closed, conditionally
closed, corrective pass required, or blocked.

A commit message saying a plan is closed is not sufficient closure evidence.

### 2.6 Archive records

Completed, superseded, or abandoned interim plans SHOULD move under
`plans/archive/` once they are no longer active, preserving traceability with
original filenames and subsystem grouping. Legacy flat `plans/*.md` files are
grandfathered archive-in-place (see `plans/README.md`): they MUST NOT be
moved en masse, because qualification evidence cites them by path.

Canonical long-term documents and accepted ADRs MUST NOT be archived merely
because their initial implementation completed.

## 3. Work classification

Every planned item MUST be assigned one primary class.

### Invariant

A property that must remain true across releases and implementation
strategies (single-engine ownership, async-first adapters, frozen
`ResponseBody` shapes, total-deadline ownership, exact-SHA binding).
Invariant work normally requires static guards, property tests, or
architecture-level evidence.

### Capability

User-, developer-, operator-, or integration-visible behavior (compat
facades, CLI workflows, H2 negotiation, cookie jar). Capability completion
requires end-to-end acceptance evidence, not merely internal types.

### Infrastructure

Internal machinery used by one or more capabilities (pipeline decomposition,
route cache, Hyper client centralization, CONNECT wire primitive).
Infrastructure SHOULD expose clear contracts and tests but MUST NOT be
represented as completed capability until a consumer path exists.

### Polish

Ergonomics, diagnostics, performance tuning, cleanup, or documentation that
does not establish the principal capability boundary. Polish SHOULD follow
correctness closure unless it removes an immediate safety or usability
blocker. Benchmark/fixture repairs are polish or corrective, never silent
capability claims.

## 4. Dependency model

The roadmap provides macro-level ordering. Subsystem roadmaps refine
dependencies into milestones. Each milestone MUST declare dependencies as one
of: **hard** (cannot correctly begin before the dependency closes),
**interface** (may proceed against an agreed contract or test double),
**soft** (parallel work possible, integration depends on the other),
**operational** (implementation can land, but deployment/release depends on
external evidence — e.g. maintainer publication, wheel rehearsal dispatch).

A milestone is dependency-ready only when every hard dependency is closed and
every interface dependency has a stable written contract. `plans/registry.md`
MUST identify blocked milestones and their blockers.

## 5. Milestone sizing

A handoff milestone SHOULD be small enough that one implementation agent can
understand the ownership boundary, implement the changes, add focused tests,
run the required gates, update documentation, and report residual risks in one
coherent pass without redesigning unrelated subsystems.

A milestone is too large when it combines several independently releasable
capability boundaries, spans multiple feature profiles without need, or
contains several unresolved architecture decisions. A milestone is too small
when it only renames one symbol or adds isolated coverage without meaningful
closure evidence, unless it is a corrective required to unblock another
milestone. Prefer vertical slices that establish one complete contract over
broad horizontal refactors with no consumer.

## 6. Agent handoff contract

An implementation agent receives one primary milestone plan. The plan MUST
tell the agent which documents are authoritative and which may be edited.

The default authority order is:

1. `docs/verification-policy.md` for validation and release truth;
2. canonical long-term specification and terminology;
3. accepted ADRs;
4. subsystem roadmap;
5. milestone implementation plan;
6. current repository evidence.

When repository evidence conflicts with the plan, the agent SHOULD preserve
long-term invariants, record the discrepancy, and make the smallest coherent
adjustment necessary. The agent MUST NOT invent a new architecture merely to
finish the checklist.

The agent MUST: inspect current code before editing; preserve unrelated
changes; keep all I/O in the engine; respect feature-profile boundaries;
never synthesize native `total` from facade timeouts; never paper over
`docs/residual-differences.md`; update tests and architecture docs with code;
run Tier 1 before finishing; produce a closure-oriented status report
identifying anything not completed.

## 7. Corrective passes

A corrective pass is a new implementation plan, not an amendment pretending
the original milestone succeeded. Corrective plans MUST: reference the
original milestone and closure record; list each unclosed requirement or
discovered defect; explain why original verification did not catch it;
include regression tests or guards preventing recurrence; renew exact-SHA
qualification when executable inputs changed; avoid reopening already closed
scope without evidence. Repeated corrective passes REQUIRE revising the
subsystem roadmap or milestone sizing.

## 8. Updating subsystem roadmaps

Subsystem roadmaps MAY evolve as implementation reveals new dependencies or
better decomposition. Updates MUST preserve links to canonical requirements,
completed milestone history, reasons for reordering, and explicit status of
removed or deferred items. A roadmap MUST NOT mark a capability complete
solely because its infrastructure milestone landed.

## 9. Registry requirements

`plans/registry.md` is the active planning control surface. It MUST remain
compact and SHOULD contain only: active subsystem roadmaps;
dependency-ready implementation plans; active or recently completed plans;
required closure passes; blocked work and blockers; latest status or closure
record. It MUST link to source documents rather than duplicate detailed
content. Pending maintainer actions stay visible at the top of
`plans/README.md` and in the registry until done.

## 10. Required planning review

Before handoff, review the plan for: correct long-term references; unresolved
decisions; dependency readiness; bounded scope and non-goals; explicit
ownership and invariants; feature-profile and compatibility effects;
cancellation, timeout, pooling, and failure semantics; security and secret
handling; required tests and static-guard evidence; unambiguous closure
criteria. If these are not answerable, the work is not ready for handoff.

## 11. Planning anti-patterns

Prohibited or strongly discouraged: adding transient TODO checklists to
canonical documents; one roadmap mixing all subsystems at file granularity;
handing an agent a broad goal without a bounded milestone contract; equating
compilation (or a landed commit) with closure; changing terminology per
subsystem; letting plans silently override accepted architecture; retaining
stale active plans after the repository materially changed; repeating
requirements across files without one authoritative source; recording only
successful evidence while omitting blocked or unrun verification; converting
missing evidence (H3 interop, wheel rehearsal, downstream manifests) into a
pass; adding CI jobs, matrices, evidence schemas, or publish automation
without explicit maintainer approval.

## 12. Initial subsystem decomposition

The roadmap produces subsystem roadmaps approximately along these boundaries:

- core transport and request policy (pipeline, routes, retry, redirect, body);
- Python bindings and HTTPX compatibility (native surface, facades, SSE/WS);
- TLS, proxy, and protocols (trust, CONNECT/SOCKS, H1/H2/H3, UDS, dialer);
- release and verification (tiers, oracles, packaging, publication, wheels);
- performance and footprint (benchmarks, ownership optimization, embedded
  evidence, bench fixtures).

This is an initial decomposition, not a fixed release list. A boundary MAY be
split when ownership or dependency analysis warrants it. Create only roadmaps
ready to be reasoned about.
