# Post-Maturation Documentation and Plan Hygiene

Planning baseline: `48f4457069d77528bd21fb342ff6ee4f27e3ae4f` (`main`, 2026-09-04)
Parent program: `plans/post-audit-architecture-and-surface-maturation-program.md`
Depends on: successful completion of `plans/post-maturation-httpx-requalification-and-closure.md`

## Objective

Perform one documentation-only truth and maintenance pass after final executable requalification. Reconcile architecture, compatibility, Node/H3 maturity, observability, native concurrency terminology, roadmap state and planning records with the implementation that actually qualified.

This plan is deliberately non-executable. If documentation review discovers a required code/test/build/validation/package correction, stop, implement that correction under the appropriate executable plan, and repeat exact-SHA requalification before resuming.

## 1. Bind the documentation pass to the final qualification state

Before editing broad docs, record:

- final qualified executable SHA;
- qualification-record commit SHA;
- descendant audit result proving no qualification-sensitive executable drift.

Acceptance:

- [ ] this plan starts from a docs-only descendant of the qualified executable tree;
- [ ] no source/test/build/validation/package file is modified by this pass.

## 2. Reconcile architecture documentation

Update the architecture index and relevant deep dives so they describe the post-maturation structure accurately.

At minimum review:

- `docs/architecture/overview.md`;
- `docs/architecture/core-engine.md`;
- `docs/architecture/core-body-streaming.md`;
- `docs/architecture/core-timeout-pool.md`;
- `docs/architecture/core-tls-proxy-protocols.md`;
- `docs/architecture/ffi-and-node.md`;
- `docs/architecture/testing-fuzzing.md` where validation coverage changed;
- `AGENTS.md` where implementation invariants or current maturity statements changed.

Required updates:

- request preparation/retry/redirect transformation architecture;
- shared transport-response lifecycle boundaries;
- final H3 cache/DNS/timeout/idle/concurrency semantics;
- trailer lifecycle;
- connection metadata/transport metrics boundaries;
- supported vs unavailable physical-connection observations;
- Node supported or experimental contract.

Acceptance:

- [ ] architecture docs match code ownership and lifecycle boundaries;
- [ ] no old duplicate-path description remains normative.

## 3. Reconcile native API and user guides

Review Rust/Python/CLI/FFI/Node user-facing material for terminology and feature truth.

Especially:

- native logical in-flight request limits versus physical connection/stream limits;
- alias/deprecation guidance if native limit names changed;
- H3 experimental/stable label based on the actual final decision;
- trailer availability and lifecycle;
- connection metadata and metrics availability;
- Node byte/stream/cancellation/error support, or explicit prototype limitations;
- compatibility facade terminology remaining HTTPX-shaped even where native names differ.

Acceptance:

- [ ] examples compile/read consistently with final APIs;
- [ ] no guide claims physical connection control where only logical request concurrency exists;
- [ ] Node/H3 maturity is consistent across README, guides and architecture docs.

## 4. Reconcile compatibility documentation

Update documentation that names the current HTTPX qualification state or differences:

- README compatibility section;
- `docs/reference/compatibility.md`;
- `docs/residual-differences.md`;
- migration guides where native API aliases changed;
- compatibility profile/README pointers if the qualification closure plan did not already update them.

Do not broaden the compatibility claim beyond the evidence collected by the closure plan.

Acceptance:

- [ ] current qualification SHA/date and stage are consistent everywhere;
- [ ] native-only additions do not appear as implied HTTPX parity features;
- [ ] intentional differences remain explicit.

## 5. Clean up roadmap status

The existing roadmap mixes historical milestones, completed features, current closure work and future production tracks. Rewrite the status sections so a reader can identify current work without reconstructing historical chronology.

Preferred structure:

- current product position;
- current supported surfaces;
- experimental/limited surfaces;
- active work: none or explicitly named next work;
- future candidates;
- historical milestone index/pointer rather than repeating every implementation narrative.

Preserve useful historical records, but stop treating completed milestone descriptions as the primary source of current truth.

Acceptance:

- [ ] `plans/ROADMAP.md` identifies one unambiguous current state;
- [ ] Node and H3 status match implementation;
- [ ] exact-SHA compatibility policy remains linked to the live ledger rather than duplicated inconsistently.

## 6. Introduce plan indexing/archive hygiene

The `plans/` directory contains many large corrective/closure documents. Improve discoverability without deleting useful history.

Preferred approach:

- create or update a small `plans/README.md` index separating:
  - active plans;
  - recently completed/current-generation plans;
  - historical plans;
  - normative live ledgers/status files;
- optionally move completed historical implementation plans under `plans/archive/` only if repository link churn is acceptable and all references are updated;
- if moving files would create excessive churn, keep them in place and use the index as the authoritative navigation layer.

Do not make historical plans normative again. `docs/verification-policy.md`, release docs and the live HTTPX status ledger remain authoritative where already specified.

Acceptance:

- [ ] a new contributor can identify active versus historical plans immediately;
- [ ] live ledgers are clearly marked as normative where appropriate;
- [ ] no broken links result from any archival move.

## 7. Reduce duplicated historical status prose

Where README, ROADMAP, compatibility docs and plan files repeat long qualification narratives, prefer linking to the live status ledger rather than copying volatile SHA/evidence prose everywhere.

Keep user-facing summaries concise and keep detailed evidence in the canonical ledger.

Acceptance:

- [ ] fewer independent places need edits when qualification SHA changes;
- [ ] user-facing docs still state the compatibility scope clearly.

## 8. Documentation validation

Run existing documentation validation only. Because this pass must remain docs-only, do not edit validation scripts to make the docs pass.

Use the existing docs checks, for example through:

```sh
./scripts/check.sh extended
```

or the canonical documentation subcommands already defined by the repository when a full extended environment is unavailable.

Also verify links after any plan indexing/archive changes.

Acceptance:

- [ ] documentation examples and links pass existing checks;
- [ ] no executable/test/build/validation/package file changed;
- [ ] descendant diff from the qualified executable SHA contains only permitted documentation/ledger/plan files.

## Non-goals

- no feature implementation;
- no new compatibility qualification;
- no CI workflow changes;
- no package/release automation changes;
- no deletion of historical evidence solely to reduce file count;
- no rewrite of old plans to pretend they were written for the final architecture.

## Exit criteria

- [ ] README, architecture docs, user guides and compatibility docs describe the final qualified implementation consistently;
- [ ] roadmap current state is clear;
- [ ] plan navigation clearly separates active/current/historical/normative records;
- [ ] volatile qualification evidence is centralized rather than duplicated broadly;
- [ ] docs validation passes;
- [ ] final descendant audit confirms this pass remained documentation-only.
