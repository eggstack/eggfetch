# Release and Verification Milestone 009 — 0.4.0 Publication Closure

Status: closed

Qualification record:

- `plans/httpx-parity-correction-status.md`
- `docs/reference/compatibility-stage-decision.md`
- `docs/residual-differences.md`

## Release identity

| Field | Value |
|---|---|
| Version | `0.4.0` |
| Qualified executable SHA | `873cf2a3a645649766fe7422528901eb4b2fc670` |
| Merge of the qualification PR | `48ff1a39733c295906fd873cae757cc9eb149d6c` (PR [#34](https://github.com/eggstack/eggfetch/pull/34)) |
| Release commit and tag target | `c9ddf4c2831693da854034f624055087fb312a33` |
| Tag | `v0.4.0`, signed (object `03a74fef7548bfc131d3186c5c3467513f4ca297`) |
| Build-only PyPI rehearsal | [Run 44862515526](https://github.com/eggstack/eggfetch/actions/runs/44862515526), success on `c9ddf4c2` |
| PyPI publication | [Run 44865916864](https://github.com/eggstack/eggfetch/actions/runs/44865916864), success on tag `v0.4.0` |
| GitHub Release | [v0.4.0](https://github.com/eggstack/eggfetch/releases/tag/v0.4.0) |

## Evidence

- **Tier 1** at the release commit: exit 0 — 1828 Rust workspace tests with 0
  failures, 581 Python behaviour tests, 134 compatibility smoke tests.
- **Tier 2** at `873cf2a3…`: `./scripts/check.sh extended` exit 0 — 6909 Rust
  tests with 0 failures; both HTTPX compatibility profiles `2082 passed,
  1 skipped` (identical to the 0.3.0 figures); eight-profile Rust public-surface
  oracle `223 checks: 223 pass, 30 skip` with `no semver update required`; MSRV
  rows on the exact Rust 1.89.0 toolchain; docs, FFI, resource, lifecycle, soak,
  downstream portfolio, lossless merge and benchmarks. One policy-defined skip:
  the Node JS surface, whose native artifact is not built in that environment.
- **Tier 3** at `873cf2a3…`: `./scripts/check.sh package` exit 0.
- **Live security preflight**: exit 0 immediately before publication —
  `advisories ok, bans ok, licenses ok, sources ok`, with one allowed `rustix`
  unsound warning (RUSTSEC-2026-0486), as at 0.3.0.
- The release commit's tree is byte-identical to `873cf2a3…`; the only paths
  changed between them are planning documents, which the exact-SHA rule treats
  as not invalidating executable inputs.
- `python scripts/validate_release_versions.py --tag v0.4.0` reports
  `All versions consistent: 0.4.0` for all six publishable crates.
- The six crates were published to crates.io in dependency order
  (`eggfetch-http-connect`, `eggfetch-core`, `eggfetch-cli`, `eggfetch-ffi`,
  `eggfetch-python`, `eggfetch-node`) and each resolved at `0.4.0` from the
  registry API afterwards.
- PyPI carries 25 distributions for `0.4.0` — 24 wheels plus one sdist — and an
  isolated `pip install eggfetch==0.4.0` smoke reports runtime version `0.4.0`
  with the `eggfetch.compat.httpx` facade importing cleanly.

## Version decision

0.4.0 is a **minor** bump, not a patch. The `proxy` feature no longer implies the
`logical-retry`/`redirects`/`basic-auth` bundle, so a downstream crate that
relied on that closure is observably affected; under
`docs/reference/versioning.md` patch versions are always backward-compatible.

The Rust public-surface oracle baseline advanced to `05f14f44…` for the same
change: `cargo-semver-checks` classifies `feature_no_longer_enables_feature` as
a major lint under both patch and minor release types, and no per-lint downgrade
suppresses it.

## Recorded deviation

`cargo publish -p eggfetch-python` fails its standalone verification on PyO3's
`extension-module` symbols, as `docs/releases/process.md` documents. The crate
archive was therefore published with `--no-verify`, and only after the build-only
PyPI rehearsal had validated the assembled set at the same SHA.

## Follow-up

- Two pre-existing fixture races were fixed during qualification and are recorded
  in `plans/closure/core-transport-policy/011-profile-independent-request-policy-and-native-uri-builder.md`
  §10: the stale-idle retry fixture (`5d8108ef`) and the FFI protocol-capability
  fixture (`873cf2a3`).
- The `rustix` RUSTSEC-2026-0486 advisory remains an allowed warning; upgrading
  past 1.1.6 clears it.
