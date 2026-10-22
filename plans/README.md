# eggfetch Planning System

> **Current execution gate:** eight milestones were implemented and are
> **closed** on 2026-10-22 — TLS M005,
> python-httpx-compat M004/M005, core M008/M009/M010, and
> release-verification M006/M007. They were executed as one coordinated
> program and committed as the executable freeze
> `d6c41adf4cf8f62ef990cccedf2f86512858fa0b`. Each changes executable inputs,
> so each invalidated the live Stage C binding; the rebinding and the
> frozen-SHA Tier 1/2/3 run have since been completed on
> `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, where Tier 1, Tier 2, Tier 3
> and the live security preflight are green, so the binding now names that
> SHA. **No registered milestone remains open** — the four previously queued
> milestones have all been executed, and the next unit of work needs its own
> plan. The publication decisions remain with the maintainer: the rebinding is
> a compatibility binding and authorizes no release.
> ADR-0008 and ADR-0009 were accepted on 2026-10-22.
> Core-transport M007, release-verification M005, and TLS/protocol M004 remain
> closed and published; coordinated **0.2.2** remains the current public
> version. TLS/protocol M004 is closed on
> `42a9c96db5b25354843bd83bd458c71757e95651` with an **unconditional** exact-SHA
> Stage C binding. It was a Rust-only feature handoff, not an H3-graduation or
> release task, and it authorized no publication. H3 stays experimental and H3
> graduation, HTTPX 1.0, and Node maturation remain separate gated futures.
> The execution order and the pending actions are recorded in
> `plans/registry.md`.
> **Live status for agents:** the exact-SHA Stage C binding is `plans/httpx-parity-correction-status.md` (+ `compat/*/profile.toml`). Completed sections and legacy plan files are historical records, not current gates (verification-policy principle 9). Validation tiers: `.skills/verification-qualification.md`.

## Pending maintainer actions

These stay visible until they are done. The same list, with owners, is in
`plans/registry.md`.

1. **Decide whether the FFI publishes a strict HTTP/3 policy value** — the
   published values are `0`=`Http1Only`, `1`=`Http2Only`, `2`/`3`=`Auto`, so
   no strict HTTP/3 value exists to reject. The guard path is written and
   unit-tested; only the ABI value and `docs/ffi/README.md` are missing.
2. **Decide CLI adapter ownership** — whether the CLI gets its own subsystem
   roadmap or is admitted under an existing one. The CLI's 581-line `run()`
   decomposition is deferred until this is decided.
3. **Decide the `tracing` feature's fate** — enabled in an adapter, given a
   test, or explicitly retired.
4. **Decide whether to authorise an HTTP/2 server fixture dev-dependency in
   `eggfetch-cli`** — so the CLI's `--http2` negotiation-success path can be
   asserted end to end. The rejection path is fully covered today.
5. **Supply the downstream behavioral-fixture artifact for each future
   qualification** — Tier 2's downstream gate fails closed by design.
   `target/downstream-qualification/artifact-manifest.json` has no
   in-repository producer, so `./scripts/check.sh extended` cannot complete
   without it. This is an operator obligation, not a code change. It was
   supplied for the 2026-10-22 qualification and the portfolio passed 4/4
   required packages; the next qualification needs it again.
6. **Decide the fate of four stale lint suppressions and two dead items** —
   `crates/eggfetch-cli/src/main.rs:24` (`clippy::cast_precision_loss`, the
   cast is gone), `crates/eggfetch-core/src/compression.rs:1332` (same),
   `crates/eggfetch-core/src/transport/http3.rs:1035`
   (`clippy::too_many_lines` on an 8-line function),
   `crates/eggfetch-python/src/response.rs:697` (a function with no caller),
   plus `crates/eggfetch-core/src/http_version.rs:50` and
   `crates/eggfetch-core/src/response.rs:285` (both dead). The lint set was
   frozen for M007, so they are recorded rather than removed.
9. **Decide whether the IPv6 pool-key divergence gets its own milestone** —
   the high-level path keys `http://[::1]:8080` as `[::1]` and the native path
   as `::1`, so the same IPv6 endpoint gets two different pool keys depending
   on the entry path. It is recorded in `docs/residual-differences.md`; closing
   it changes pool keying and needs its own plan.
10. **Decide whether the `639bf186…` HTTP/3 SHA needs an HTTPX-ledger entry** —
   it was named by the compat profiles' comment log, which M007 removed. It is
   an HTTP/3 qualification binding and is held in the H3 records; its
   provenance is now stated in `docs/architecture/overview.md`.

ADR-0008 and ADR-0009 were accepted on 2026-10-22 and are no longer pending.

This directory separates durable architectural direction from temporary
execution planning. Since the 2026-09-25 migration it follows the
convention below (adapted from the codegg planning style): canonical
long-term documents, ADRs, subsystem roadmaps, bounded milestone handoffs,
closure records, a compact registry, and a frozen legacy archive.

## Canonical long-term documents

The following files define the intended product and architecture and MUST
NOT be edited as part of ordinary implementation work:

- `000-long-term-specification.md` — normative end-state specification and
  invariants.
- `001-terminology-and-domain-model.md` — normative language and identity
  model (engine, profiles, routes, deadlines, facades, Stage C).
- `002-long-term-roadmap.md` — dependency-ordered forward roadmap (Phases
  0–4; milestones A–Z remain historical in `ROADMAP.md`).
- `003-planning-process.md` — rules for deriving and managing interim plans.

The first three documents are stable architectural references. Changes to
them require an explicit long-term architecture decision, not an
implementation convenience. Interim plans MUST reference them rather than
copying or silently revising their requirements.

## Planning hierarchy

```text
Long-term specification and terminology
        |
        v
Architecture decision records
        |
        v
Forward roadmap (002) + subsystem roadmaps
        |
        v
Milestone implementation plans
        |
        v
Implementation and verification
        |
        v
Closure records and archive
```

## Directory roles

- `adrs/` — durable architecture decisions. Accepted decisions are
  superseded, not rewritten.
- `subsystems/` — subsystem specifications and dependency-ordered roadmaps
  translating the long-term documents into coherent workstreams.
- `implementation/` — focused milestone plans handed to implementation
  agents. Operational; may evolve as code changes.
- `closure/` — verification, evidence, residual-risk, and completion records
  for implemented milestones.
- `archive/` — completed or superseded interim planning retained for
  traceability (plus the grandfathered flat-file archive below).
- `registry.md` — compact index of active roadmaps, implementation plans,
  closure work, gates, and blockers.

## Core rule

Long-term documents state **what eggfetch is becoming and what must remain
true**. Interim documents state **what an agent should implement next
against a specific repository baseline**.

Implementation agents MUST NOT add commit-specific steps, transient file
lists, current test counts, or short-lived corrective work to the canonical
long-term documents.

## Planning lifecycle

1. Identify the relevant long-term specification sections and invariants.
2. Record any unresolved architectural decision in `adrs/`.
3. Create or update a subsystem roadmap in `subsystems/`.
4. Select one dependency-ready milestone.
5. Write a bounded handoff plan under `implementation/`.
6. Implement and verify the milestone (Tier 1 always; Tier 2/3 + renewed
   exact-SHA qualification when executable inputs changed).
7. Write a closure record under `closure/`.
8. Update `registry.md` and the subsystem roadmap status.
9. Move completed or superseded interim documents to `archive/` when they no
   longer represent active work.

No milestone is complete merely because code landed. Completion requires the
closure evidence defined by its implementation plan and subsystem roadmap.

## Required classification

Every subsystem roadmap and implementation plan MUST distinguish:

- **Invariant** — a property that must always remain true.
- **Capability** — user- or operator-visible behavior.
- **Infrastructure** — internal machinery required by capabilities.
- **Polish** — ergonomics, diagnostics, performance tuning, cleanup, or
  documentation.

Infrastructure and polish MUST NOT be presented as completed user capability
unless the user-visible acceptance criteria are actually satisfied.

## Naming conventions

- ADR: `adrs/ADR-NNNN-short-title.md`
- Subsystem roadmap: `subsystems/<subsystem>-roadmap.md`
- Milestone implementation plan: `implementation/<subsystem>/NNN-short-title.md`
- Closure record: `closure/<subsystem>/NNN-status.md`
- Archived document: retain its original relative structure beneath `archive/`

Use stable subsystem names (`core-transport-policy`, `python-httpx-compat`,
`tls-proxy-protocols`, `release-verification`, `performance-footprint`). Do
not encode dates in filenames unless the document is inherently time-bound.

## Starting a new workstream

Begin with `subsystems/README.md`, then use the templates and rules in:

- `adrs/README.md`
- `implementation/README.md`
- `closure/README.md`

Register active work in `registry.md` before handing implementation plans to
agents.

## Normative live compatibility records

`httpx-parity-correction-status.md` remains the live exact-SHA status for
both facades. Profiles: `compat/httpx/0.28.1/profile.toml`,
`compat/httpx2/2.12.0/profile.toml`. Preview: `compat/httpx/1.0-preview/`
(unqualified by design; status in `preview-status.toml`).

### HTTPX 1.0 migration trigger

A future HTTPX 1.0 implementation/qualification program opens only when all
three hold (recorded here and in `plans/ROADMAP.md`; never by renaming the
preview plan into a Stage C plan):

1. upstream publishes an RC with an explicitly frozen public API, or a
   stable 1.0 release;
2. release notes indicate no further major compatibility reset before stable;
3. a fresh delta inventory shows the target is stable enough to justify
   implementation, pinned to the exact RC/stable release.

## Legacy archive (pre-migration flat plans)

The ~250 flat `plans/*.md` files (milestones A–Z, parity phases, programs,
correctives, closures) plus `ROADMAP.md` are frozen historical records kept
at their original paths, because qualification evidence, compatibility docs,
and profiles cite them by path. They remain valid evidence citations but
MUST NOT be extended with new active scope — new work uses the taxonomy
above. Each subsystem roadmap's milestone-status section links its
controlling legacy plans. This index and `registry.md` are the authoritative
navigation layers; legacy files stay in place.

Plan files stay in place; this index is the authoritative navigation layer.
