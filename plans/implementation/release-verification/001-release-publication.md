# Release and Verification Milestone 001 — Coordinated 0.2.x Publication

Status: ready (decomposed; execute M001A next)

Repository planning baseline: `36ab862dabf8f30fdcf05900a68fdf9f93060879`

Source roadmap:

- `plans/subsystems/release-verification-roadmap.md` §7 Milestone 1

Long-term requirements:

- `plans/000-long-term-specification.md` §6, §7
- `plans/001-terminology-and-domain-model.md` §10, §11
- `plans/002-long-term-roadmap.md` Phases 1–3

Applicable ADR:

- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`

Primary class: infrastructure / operational

## 1. Objective

Publish the next coordinated EggFetch release without reusing or moving an
already-consumed release identity, and without weakening the repository's
manual release/security policy.

This milestone is now decomposed:

1. **M001A** — prepare and qualify a fresh coordinated `0.2.1` release
   candidate from the current qualified tree;
2. **M002** — run the build-only Python 3.15 wheel rehearsal from that exact
   candidate;
3. **M001B** — publish all coordinated crates, create the signed `v0.2.1`
   tag, publish the already-rehearsed Python distribution through Trusted
   Publishing, and close the release.

Only M001A is executable initially.

## 2. Why decomposition is required

The repository manifests and Python metadata currently say `0.2.0`, but the
GitHub `v0.2.0` tag already exists and points to
`8959ca890ee34f4cf456aed648315322f1e83ef7`. A GitHub release for that tag
also already exists.

The current live Stage C executable freeze is
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`, and current `main` is a
descendant containing later qualification/test/planning work. Re-pointing or
reusing `v0.2.0` would destroy release identity and is prohibited.

The next clean coordinated version is therefore `0.2.1`. No `v0.2.1` tag
exists at this planning baseline.

Python 3.15 is currently at 3.15.0rc2, the final planned release candidate,
with final scheduled for 2026-10-01. The repository already includes the
Python 3.15 classifier and a 3.15 wheel row, so the build-only rehearsal must
close before publishing `0.2.1`.

## 3. Dependency graph

```text
M001A 0.2.1 release-candidate preparation
    |
    v
M002 Python 3.15 build-only wheel rehearsal
    |
    v
M001B coordinated crates.io/tag/PyPI publication
```

M001 closes only when M001A, M002, and M001B are closed.

## 4. Invariants

- Never move, delete, or reuse `v0.2.0`.
- All six publishable Rust crates and the Python distribution share one
  coordinated version.
- The candidate rehearsed in M002 is the exact commit tagged/published by
  M001B.
- No executable behavior change is hidden inside release preparation.
- Any executable/test/qualification-input change invokes ADR-0003 and stops
  the release until requalification.
- crates.io publication remains maintainer-local and manual.
- PyPI publication remains manual `workflow_dispatch` using Trusted
  Publishing/OIDC.
- `eggfetch-bench` and `fuzz/` are never published.
- Python 3.15 support is not claimed as release-backed until M002 closes.

## 5. Child plans

- `plans/implementation/release-verification/001a-0-2-1-release-candidate-preparation.md`
- `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md`
- `plans/implementation/release-verification/001b-0-2-1-coordinated-publication.md`

## 6. Umbrella closure

Create:

`plans/closure/release-verification/001-coordinated-0-2-1-publication.md`

after M001B closes.

The umbrella closure must link the M001A candidate SHA, M002 rehearsal run,
all crates.io package versions, signed tag target, PyPI publication run, and
post-publication smoke evidence.

## 7. Stop conditions

Stop this release train if:

- any existing `v0.2.1` tag/version appears unexpectedly;
- release preparation requires runtime/API/dependency changes;
- M002 fails on any required wheel row;
- any release/security/package gate is red;
- partial publication occurs and a new patch version is required.

Do not improvise tag movement, registry overwrites, matrix reduction, or
security-policy weakening to keep the nominal `0.2.1` version.
