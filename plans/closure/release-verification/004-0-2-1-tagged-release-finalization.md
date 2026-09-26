# Release and Verification Milestone 004 — Closure Status

Status: closed

Source implementation plan:

- `plans/archive/implementation/release-verification/004-0-2-1-tagged-release-finalization.md`

Source subsystem roadmap:

- `plans/subsystems/release-verification-roadmap.md#milestone-4--021-tagged-release-finalization`

Repository baseline reviewed: `57887520029344c228afe6b537da7e33c6321ef7`

Implementation commits:

- `fe4d596ca8d91694fc887566acaa7875b918d25a` — release documentation truth pass and frozen 0.2.1 candidate.
- Closure commit: this record and the planning-control reconciliation; SHA recorded by Git history after commit.

## 1. Executive finding

The coordinated 0.2.1 release is complete on candidate
`fe4d596ca8d91694fc887566acaa7875b918d25a`. All six crates.io crates, the
signed `v0.2.1` tag, all 19 PyPI files, and the published GitHub Release match
that candidate. Local Tier 1/2/3 checks, publication validators, security
preflight, exact-SHA wheel rehearsal, and external Rust/Python consumer smokes
passed. No runtime or qualification inputs changed; Stage C remains bound to
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Documentation-only final candidate | `41757569123c0b8038550b956d8b244ab55094a6..fe4d596ca8d91694fc887566acaa7875b918d25a`; 16 changed paths are README, release/verification docs, and planning records | Pass | No source, tests, workflow, dependency, profile, or validation-script changes |
| Tier 1, Tier 2, Tier 3 | `./scripts/check.sh`; `./scripts/check.sh extended`; `./scripts/check.sh package` on candidate | Pass | Explicit Node native-artifact and downstream-manifest skips are recorded below |
| Release metadata validators | `validate_release_versions.py`; `validate_publishable_internal_dependencies.py`; `git diff --check` | Pass | Run in the pinned project environment |
| Live security preflight | `./scripts/check_security.sh`, immediately before crates.io publication, 2026-09-26 16:24 UTC | Pass | cargo-deny and cargo-audit completed successfully |
| Renewed exact-SHA build-only PyPI rehearsal | Run [36253723990](https://github.com/eggstack/eggfetch/actions/runs/36253723990), `head_sha=fe4d596ca8d91694fc887566acaa7875b918d25a` | Pass | 18 wheels + 1 sdist; all CPython 3.15 rows used 3.15.0-rc.2; assembly and coverage checks passed; `publish=false` |
| Six crates.io packages | `eggfetch-http-connect`, `eggfetch-core`, `eggfetch-cli`, `eggfetch-ffi`, `eggfetch-python`, `eggfetch-node` all at 0.2.1 | Pass | Published in dependency order; dry-run passed before each publication |
| Signed tag identity | Remote `v0.2.1` dereferences to `fe4d596ca8d91694fc887566acaa7875b918d25a`; local SSH signature verified | Pass | ED25519 fingerprint `SHA256:gwin+vF6C6TSm44DJkRShuNILg8fPah7e832ujvYeKw` |
| PyPI Trusted Publishing | Run [36255517733](https://github.com/eggstack/eggfetch/actions/runs/36255517733), `head_sha=fe4d596ca8d91694fc887566acaa7875b918d25a` | Pass | Tag validation, live security scan, all matrix jobs, assembly, protected-environment approval, and OIDC publish job succeeded |
| PyPI file set | PyPI JSON for `eggfetch/0.2.1` | Pass | Exactly 19 files: 18 wheels across CPython 3.10–3.15 and 3 platforms, plus one sdist |
| GitHub Release | [v0.2.1](https://github.com/eggstack/eggfetch/releases/tag/v0.2.1), release ID `397322120` | Pass | Published 2026-09-26; non-draft, non-prerelease; uses existing signed tag and changelog-derived notes |
| External Rust consumer | Disposable consumer with `eggfetch-core = "=0.2.1"`; `cargo check` without path/git overrides | Pass | Resolved from crates.io |
| External Python consumer | Clean venv; `pip install eggfetch==0.2.1`; import/version smoke | Pass | Installed CPython 3.12 manylinux wheel; import reported `0.2.1` |
| README/release-doc truth | Focused scan for pending rehearsal and optional GitHub Release claims; updated package README and process docs | Pass | Historical plan/closure statements remain historical; active control surfaces reconciled |

## 3. Production implementation evidence

No production runtime implementation was needed or changed. The candidate
delta is documentation and planning only. Release guidance now puts the
manual GitHub Release after successful PyPI publication and initial registry
smokes. The root README no longer describes the completed 3.15 rehearsal as
pending.

## 4. Verification executed

### Commands run

- `./scripts/check.sh`
- `./scripts/check.sh extended`
- `./scripts/check.sh package`
- `python scripts/validate_release_versions.py`
- `python scripts/validate_publishable_internal_dependencies.py`
- `./scripts/check_security.sh`
- `git diff --check`
- `cargo publish --dry-run -p <crate>` for each of the six publishable crates
- `cargo check` in a disposable external consumer of `eggfetch-core = "=0.2.1"`
- Clean-venv `pip install eggfetch==0.2.1` and import/version smoke
- Build-only and publish PyPI workflows: runs `36253723990` and `36255517733`

### Results (pass/fail/skip with reason; counts without concealment)

- Tier 1, Tier 2, Tier 3: pass.
- Extended compatibility suite: 1,928 HTTPX compatibility tests passed; 134
  HTTPX smoke tests passed; 578 Python behavior tests passed.
- Build-only rehearsal and publish workflow: all 18 wheel jobs and one sdist
  passed, with exact 19-file assembly.
- `node test.js`: explicit skip because the native Node artifact was absent.
- Downstream manifest job: explicit skip because its manifest was absent.
- Python 3.15 clean external install was not separately run locally; all three
  3.15 platform wheel build/install/smoke rows passed in both workflow runs.
- Release-version and publishable-dependency validators, security preflight,
  Cargo registry consumer, Python install/import, and whitespace check: pass.

## 5. Invariant review

- `v0.2.0` remains untouched.
- Candidate SHA, renewed rehearsal SHA, signed tag target, and PyPI workflow
  SHA are all `fe4d596ca8d91694fc887566acaa7875b918d25a`.
- All coordinated package versions are 0.2.1; no immutable version was
  overwritten or yanked.
- No executable, API, feature, dependency, compatibility-profile, workflow, or
  qualification-input change occurred.
- Publication remained manual; the GitHub Release reuses the already-pushed
  signed tag.

## 6. Failure, timeout, pool, and recovery review

No publication failure, partial release, retry, or recovery action occurred.
All public channels now expose the same frozen release identity.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

There is no runtime compatibility delta. No Stage C renewal was triggered;
the live binding remains `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`. The
final candidate was nevertheless bound exactly across release rehearsal,
signed tag, and PyPI publication.

## 8. Security review

The live cargo-deny/cargo-audit preflight passed immediately before crate
publication. The tagged PyPI workflow independently passed its live security
preflight and published through the protected `pypi` environment using the
workflow's Trusted Publishing/OIDC path. Approval was granted only after the
exact-tag validation and successful 18-wheel + 1-sdist assembly.

## 9. Documentation and operations

The README, release process, verification policy, RC checklist, release
security checklist, and release-verification roadmap now reflect the
completed rehearsal and required manual GitHub Release. M001A/M002 closure
records remain immutable. The legacy 3.15 roadmap pointer was corrected to
the maintained milestone plan and its closed status.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| None | No unresolved release findings | None | None |

## 11. Roadmap disposition

M004 is closed. The M001 coordinated-publication umbrella is closed by
`plans/closure/release-verification/001-coordinated-0-2-1-publication.md`.
No new future plan is unblocked. HTTPX 1.0 remains gated on its upstream
release/API trigger; H3 graduation remains gated on independent interop,
drain, public-origin, impairment, and upstream-risk evidence. M003 remains
standing proposed intake rather than an implementation task.

## 12. Registry updates

The active registry and release roadmap record M001, M001A, M002, and M004 as
closed, M001B as superseded, and M003 as standing/proposed. The M004 plan is
archived. No active release task remains.
