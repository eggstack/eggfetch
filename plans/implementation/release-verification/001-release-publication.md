# Release and Verification Milestone 001 — 0.2.x Coordinated Publication

Status: ready

Repository baseline: live Stage C freeze per
`plans/httpx-parity-correction-status.md` (see ledger for the SHA; never
copied here)

Source roadmap:

- `plans/subsystems/release-verification-roadmap.md` §7 Milestone 1

Long-term requirements:

- `plans/000-long-term-specification.md` §6, §7
- `plans/001-terminology-and-domain-model.md` §10, §11
- `plans/002-long-term-roadmap.md` Phase 1

Applicable ADRs:

- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`

Primary class: infrastructure (operational)

## 1. Objective

Publish the qualified 0.2.x line — coordinated crates.io publication in
dependency-leaf order, version tag, PyPI dispatch — with zero behavior
change and zero automation change.

## 2. Why this milestone is ready

Implementation + qualification complete on the frozen executable SHA
(`plans/issue-24-release-qualification-and-closure.md`; release commit
bumps coordinated versions with CHANGELOG, CI-green). Hard dependencies
closed. Remaining work is maintainer operation, not engineering.

## 3. Current implementation evidence

Legacy plan records implementation freeze, Tier 1/extended/package/security/
MSRV/API-oracle/compat evidence, and green remote CI on both freeze and
release commits. Downstream eggsearch retains its temporary
`.decompress(false)` workaround until it adopts a published fix separately.

## 4. Invariants that must not regress

- Live Stage C binding untouched unless executable inputs changed.
- No CI/workflow change; no publish automation added.
- `eggfetch-bench` and `fuzz/` never published.

## 5. Scope

### In scope

- Trusted-local Tier 2 + Tier 3 + `check_security.sh` confirmation.
- `cargo publish` per crate in order: http-connect → core → cli → ffi →
  python → node, then tag.
- Manual `pypi.yml` dispatch on the tag (Trusted Publishing).
- Post-publication verification (`cargo search`, installed-wheel smoke).

### Explicitly out of scope

- Any executable, test, fixture, or validation change (would void the
  freeze and require requalification first — stop and open that milestone).
- HTTPX 1.0 work, H3 graduation, Node maturation, wheel-matrix changes.

## 6. Required production changes

None by design. Version/tag/release records only.

## 7. Ordered work packages

### Work package A — Pre-publication confirmation

Intent: prove the tree is the qualified freeze plus docs-only descendants.

Required changes: none; verify `git status` clean-equivalent and descendant
audit (docs/profile/ledger-only after the freeze).

Acceptance evidence: Tier 2 + Tier 3 + live preflight green in the trusted
environment.

### Work package B — Publish, tag, dispatch

Intent: release through the manual path only.

Required changes: ordered `cargo publish`; `v0.x` tag; `pypi.yml`
`workflow_dispatch` with `publish=true`.

Acceptance evidence: registry listings, tag, PyPI project page, workflow run
IDs recorded in the closure record.

## 8. Failure, cancellation, timeout, and pool semantics

Not applicable (no runtime change). Partial publication (some crates
published, later one fails) MUST be recorded exactly; yank/retag decisions
are maintainer calls documented in closure, never improvised retries of
versioned publishes.

## 9. Compatibility and migration

No consumer-facing change. If preparation reveals drift requiring an
executable fix, this plan stops: file a corrective milestone and requalify.

## 10. Required tests

None beyond the gate reruns in §7. No new tests.

## 11. Required verification commands

```bash
./scripts/check.sh extended   # Tier 2, trusted local env
./scripts/check.sh package    # Tier 3, trusted local env
./scripts/check_security.sh   # live preflight before publication
```

## 12. Documentation updates

- CHANGELOG/release notes already landed; verify version consistency via
  `test_validate_release_versions.py` (Tier 1).
- Record run IDs and published versions in the closure record.

## 13. Acceptance criteria

- All six crates published in order; tag pushed; PyPI release live via
  dispatched workflow.
- Live Stage C binding still valid (no executable-input change), or a
  requalification milestone was opened instead.

## 14. Stop conditions

- Preparation touches executable inputs → stop, open requalification.
- Any gate red → stop, resolve as corrective work, do not publish around it.
- Credential/OIDC/environment failure → stop and report; never weaken
  publication security to proceed.

## 15. Closure evidence required

Published versions, tag, workflow run IDs, gate outputs, descendant audit,
and (if applicable) the requalification handoff reference.

## 16. Handoff notes

Maintainer-only execution from a trusted local environment. No agent should
attempt publication without explicit maintainer direction per commit.
