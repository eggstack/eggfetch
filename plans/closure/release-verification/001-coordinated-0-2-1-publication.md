# Release and Verification Milestone 001 — Closure Status

Status: closed

Source implementation plan:

- `plans/archive/implementation/release-verification/001-release-publication.md`

Source subsystem roadmap:

- `plans/subsystems/release-verification-roadmap.md#milestone-1--02x-coordinated-publication`

Repository baseline reviewed: `57887520029344c228afe6b537da7e33c6321ef7`

Implementation commits:

- `fe4d596ca8d91694fc887566acaa7875b918d25a` — final docs-only release candidate.
- Closure commit: this umbrella record and the M004 control-surface reconciliation; SHA recorded by Git history after commit.

## 1. Executive finding

The coordinated EggFetch 0.2.1 publication is complete across crates.io,
GitHub, and PyPI. The frozen release candidate is
`fe4d596ca8d91694fc887566acaa7875b918d25a`. M001A candidate preparation and
M002's original wheel rehearsal remain valid historical evidence; M004
performed the required refreshed-SHA rehearsal and completed publication.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Candidate preparation | M001A closure; original candidate `41757569123c0b8038550b956d8b244ab55094a6` | Pass | Historical preparation evidence retained |
| Python 3.15 rehearsal | M002 closure, run `36223505399`; refreshed M004 run `36253723990` | Pass | Both build-only; refreshed run is bound to final candidate |
| Six crates.io crates | Six coordinated crates at 0.2.1 | Pass | Published in required dependency order |
| Signed release tag | `v0.2.1` → `fe4d596ca8d91694fc887566acaa7875b918d25a` | Pass | SSH signature verified |
| PyPI release | Publish run `36255517733` | Pass | Trusted Publishing/OIDC; exactly 18 wheels + 1 sdist |
| GitHub Release | https://github.com/eggstack/eggfetch/releases/tag/v0.2.1 | Pass | Published, non-draft and non-prerelease |
| External registry smoke | Disposable Cargo consumer and clean Python venv install/import | Pass | Both resolved exact 0.2.1 artifacts |
| Release documentation | Candidate README/process correction and M004 reconciliation | Pass | No release channel remains pending |

## 3. Production implementation evidence

This operational release introduced no runtime code changes. The release
candidate delta was documentation/planning-only, and all release artifacts
refer to that candidate.

## 4. Verification executed

### Commands run

See the detailed M004 closure at
`plans/closure/release-verification/004-0-2-1-tagged-release-finalization.md`.
It records Tier 1/2/3, validators, security preflight, both PyPI workflows,
signature/tag identity, registry smoke, and exact outcomes.

### Results (pass/fail/skip with reason; counts without concealment)

All required release gates passed. The only workflow skips were the optional
Node native-artifact test and downstream manifest job because their required
artifacts were absent. All 3.15 wheel rows passed in rehearsal and publish
runs. No partial publication or recovery was required.

## 5. Invariant review

Historical `v0.2.0` remains immutable. All six Rust packages, the Python
package, signed tag, and GitHub Release are coordinated at 0.2.1. The
candidate/rehearsal/tag/PyPI exact-SHA chain is
`fe4d596ca8d91694fc887566acaa7875b918d25a`.

## 6. Failure, timeout, pool, and recovery review

No release failure, partial registry state, or recovery action occurred.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

No executable or qualification input changed. Stage C remains bound to
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`.

## 8. Security review

The immediate local live security preflight and the tagged workflow security
preflight passed. PyPI publication completed through the protected OIDC
Trusted Publishing job after assembled-set approval.

## 9. Documentation and operations

The completed rehearsal is no longer described as pending. The coordinated
release procedure requires a manual GitHub Release after PyPI and registry
smokes. Historical M001A/M002 closure evidence was not rewritten.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| None | No unresolved release-train findings | None | None |

## 11. Roadmap disposition

M001 umbrella and M004 are closed. No future implementation plan was
unblocked by publication. HTTPX 1.0 and H3 remain gated by their existing
external evidence. M003 remains standing proposed intake. No active release
task remains.

## 12. Registry updates

Registry and roadmap entries now point to the M001 and M004 closure records;
the completed implementation plans are archived and the release-verification
subsystem is closed at this milestone.
