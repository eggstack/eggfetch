# Release and Verification M002 — Python 3.15 Wheel Rehearsal Closure

Status: closed

Source implementation plan:

- `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md`

Candidate SHA (from M001A): `41757569123c0b8038550b956d8b244ab55094a6`

Rehearsal dispatch ref: `origin/release/0.2.1-candidate` (branch pinned at
the exact candidate; `main` had moved one docs-only closure commit past it,
so per M002 §3 the ref — not `main` — preserves `head_sha` identity).

Workflow run: `36223505399` (`PyPI Wheels`, `publish=false`, first dispatch,
no retries), `head_sha` `41757569` == candidate. Conclusion: `success`.
Publish job: `skipped` (nothing published).

## 1. Local pre-dispatch gates (from the exact candidate)

- Six crate versions + `pyproject.toml` = `0.2.1`; internal requirements
  `^0.2.1` on all five edges (`validate_release_versions.py` and
  `validate_publishable_internal_dependencies.py` green from a clean
  `git worktree` at the candidate).
- `git diff --check` clean; candidate worktree clean.
- Tier 1 / Tier 2 extended / Tier 3 package already green on byte-identical
  content (M001A evidence; the candidate commit is exactly the tested tree).

## 2. Workflow results (22 jobs)

| Job | Result |
|---|---|
| `validate-release` (version identity, internal topology, routine + package validation) | success |
| 18 wheels: linux-x86_64, macos-arm64, windows-x86_64 × CPython 3.10–3.15 (each: exactly-one-wheel, install, `wheel_smoke.py`, METADATA `eggfetch 0.2.1` + `_native` + `__init__.py`) | 18/18 success |
| `sdist` (build, exactly-one-sdist, isolated outside-repo rebuild + install `eggfetch 0.2.1`, `validate_package_content.py`) | success |
| `assemble` (18 wheels + 1 sdist = 19, no duplicate filenames, `validate_wheel_coverage.py` → "All 18 wheels present with expected coverage", `twine check` PASSED on all distributions, `pypi-distributions` artifact uploaded) | success |
| `publish` | skipped (`publish=false`) |

## 3. Python 3.15 evidence

`actions/setup-python` selected **CPython 3.15.0-rc.2** on all three rows
(linux-x86_64, macos-arm64, windows-x86_64); each 3.15 wheel built,
installed into its matching interpreter, passed smoke, and reported
`METADATA version: 0.2.1`. Matches the plan expectation (3.15.0 final
scheduled 2026-10-01; prerelease resolution confined to the 3.15 rows).

## 4. Failure policy

No row failed; no rerun; no repository change. The M001A candidate stands.

## 5. Handoff

M002 is closed. M001B (coordinated crates.io × 6 in leaf order, signed
`v0.2.1` tag on `41757569`, `pypi.yml` `publish=true` from that tag straight
to prod PyPI) is now the sole ready release task.
