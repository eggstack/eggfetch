# Closure and Verification Records

Evidence-based completion records for implementation milestones. A closure
record is the gate deciding whether a subsystem milestone is complete — not a
retrospective summary alone.

## Layout and naming

```text
closure/<subsystem>/NNN-status.md
```

Use the same milestone number as the source implementation plan.

## Required closure-record template

```markdown
# <Subsystem> Milestone NNN — Closure Status

Status: closed | conditionally closed | corrective pass required | blocked

Source implementation plan:

- `plans/implementation/<subsystem>/NNN-...md`

Source subsystem roadmap:

- `plans/subsystems/<subsystem>-roadmap.md#...`

Repository baseline reviewed: `<SHA>`

Implementation commits:

- `<SHA>` — summary

## 1. Executive finding

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |

## 3. Production implementation evidence

## 4. Verification executed

### Commands run

### Results (pass/fail/skip with reason; counts without concealment)

## 5. Invariant review

## 6. Failure, timeout, pool, and recovery review

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

## 8. Security review

## 9. Documentation and operations

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |

## 11. Roadmap disposition

## 12. Registry updates
```

## Closure rules

A milestone MUST NOT be marked closed when: only compilation or formatting
was verified; required gates were not run without justified substitute
evidence; a capability has only internal infrastructure; a security or
migration requirement is unimplemented; a known high-severity defect remains;
an executable change skipped exact-SHA requalification; closure depends on
unrecorded assumptions.

A milestone MAY be conditionally closed when implementation is complete but
named external or operational evidence (wheel rehearsal dispatch, publication
confirmation, independent H3 evidence) cannot be obtained in the current
environment. The condition, risk, and exact future evidence must be explicit.

## Corrective follow-up

Keep the closure record immutable except for factual corrections; create a
new implementation plan under the same subsystem; reference every unclosed
finding; add regression guards; do not reopen unrelated closed scope.
