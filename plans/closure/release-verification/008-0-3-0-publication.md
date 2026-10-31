# Release and Verification Milestone 008 — 0.3.0 Publication Closure

Status: closed with one recorded signing deviation

Qualification record:

- `plans/httpx-parity-correction-status.md`
- `docs/reference/compatibility-stage-decision.md`
- `docs/residual-differences.md`

## Release identity

| Field | Value |
|---|---|
| Version | `0.3.0` |
| Qualified executable SHA | `6093cc7aa0961194f096cbf2f92c3fc057671a34` |
| Release commit and tag target | `9e0a7a909b146c19fe77d94ce2ce6e57b9e80cde` |
| Qualification PR | [#26](https://github.com/eggstack/eggfetch/pull/26) |
| Release-count correction PR | [#27](https://github.com/eggstack/eggfetch/pull/27), merged at the release commit |
| Build-only PyPI rehearsal | [Run 41181702488](https://github.com/eggstack/eggfetch/actions/runs/41181702488), success on the release commit |
| PyPI publication | [Run 41183284339](https://github.com/eggstack/eggfetch/actions/runs/41183284339), success on tag `v0.3.0` |
| GitHub Release | [v0.3.0](https://github.com/eggstack/eggfetch/releases/tag/v0.3.0) |

## Evidence

- The six coordinated crates were published in dependency order and each
  became resolvable before its dependents were uploaded:
  `eggfetch-http-connect`, `eggfetch-core`, `eggfetch-cli`, `eggfetch-ffi`,
  `eggfetch-python`, and `eggfetch-node`. Each registry version is `0.3.0`.
- `cargo publish --dry-run -p eggfetch-python` could not link PyO3's
  `extension-module` symbols in Cargo's standalone verifier on the release
  host. The package archive was published using
  `cargo publish --no-verify -p eggfetch-python` after package-content
  validation. The release workflow independently built and smoke-tested all
  supported wheels and built the source distribution.
- `publish=false` and `publish=true` workflows each passed release validation,
  package validation, all 24 wheel builds, sdist validation, and assembled
  release-set validation. The publish run verified exactly 25 files before
  using Trusted Publishing; PyPI reports 25 files for `eggfetch==0.3.0`.
- The `pypi` deployment approval was issued after assembled-set validation
  passed. The publish job completed successfully.
- A clean local Python 3.14.2 environment installed `eggfetch==0.3.0` from
  the PyPI source distribution and imported `eggfetch.Client`. This host runs
  an x86_64/Rosetta shell, for which the project intentionally publishes no
  Intel macOS wheel. The release workflow's supported platform wheel smoke
  tests, including macOS arm64, passed on their native runners.
- The GitHub Release was created from the existing `v0.3.0` tag with
  changelog-derived notes and registry links.

## Recorded deviations and follow-up

1. The `v0.3.0` tag is annotated but unsigned. No signing key associated with
   the authenticated GitHub account was available in the release environment;
   the locally present GPG keys belonged to unrelated identities. The tag was
   not replaced or retargeted. Signed version tags remain the release-process
   requirement for future releases; provision the repository's intended
   signing identity before the next release.
2. The first `publish=false` run on the pre-correction branch passed but did
   not execute the `publish=true` file-count guard. PR #27 corrected that guard
   from 19 to 25. The corrected build-only run and the final publishing run
   both passed the 25-file assembled-set check.

No release blockers remain for EggFetch 0.3.0. The two process follow-ups above
are recorded for the next release cycle and do not change the published
artifacts or their qualification evidence.
