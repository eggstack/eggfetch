# Release and Verification Milestone 010 — 0.4.1 Publication Closure

Status: closed

Qualification record:

- `plans/httpx-parity-correction-status.md`
- `docs/reference/compatibility-stage-decision.md`
- `docs/residual-differences.md`

## Release identity

| Field | Value |
|---|---|
| Version | `0.4.1` |
| Release commit and tag target | `dc0f580f82b3a1f694451cd1ac73e945f2ef1e40` |
| Tag | `v0.4.1`, signed (object `c4121576efa36ee78885e0ac0cea6239883340ba`) |
| Tag signing key | `B135C6B22EDB6FFE855FE37C6C5DD2B2BBBAA128`, "eggfetch release <releases@eggfetch.dev>", generated for this release on the release host and not published to a keyserver; the public key is exported to `target/release-signing-key.asc` on that host |
| Routine CI | [Run 45320317560](https://github.com/eggstack/eggfetch/actions/runs/45320317560), success on `dc0f580f` |
| Build-only PyPI rehearsal | [Run 45320371329](https://github.com/eggstack/eggfetch/actions/runs/45320371329), success on `dc0f580f` |
| PyPI publication | [Run 45326299885](https://github.com/eggstack/eggfetch/actions/runs/45326299885), success on tag `v0.4.1` |
| GitHub Release | [v0.4.1](https://github.com/eggstack/eggfetch/releases/tag/v0.4.1) |

## Evidence

- **Tier 1** at `dc0f580f`: exit 0 — 6915 Rust workspace test results with 0
  failures, 584 Python behaviour tests (1 skipped), 134 compatibility smoke
  tests. One policy-defined skip: the Node JS surface, whose native artifact
  is not built in that environment.
- **Tier 2** at `dc0f580f`: `./scripts/check.sh extended` exit 0 — 6915 Rust
  test results with 0 failures; both HTTPX compatibility profiles
  `2079 passed, 4 skipped`; eight-profile Rust public-surface oracle
  `223 checks: 223 pass, 30 skip` with `no semver update required`; MSRV
  rows on the exact Rust 1.89.0 toolchain; docs, FFI, resource, lifecycle,
  soak, downstream portfolio, lossless merge and benchmarks. One
  policy-defined skip: the Node JS surface.
- **Tier 3** at `dc0f580f`: `./scripts/check.sh package` exit 0 — the
  `eggfetch-http-connect` publish dry-run, package-structure validation for
  the five dependent crates, a wheel build, wheel smoke, package content
  validation and the installed-wheel typing smoke.
- **Live security preflight** at `dc0f580f`: exit 0 immediately before
  publication — `advisories ok, bans ok, licenses ok, sources ok`, and
  `cargo audit` clean across 273 dependencies. The CI-side preflight in the
  publication run passed as well, with the workflow's pinned
  `cargo-deny 0.19.0` and `cargo-audit 0.22.2`.
- `python scripts/validate_release_versions.py --tag v0.4.1 --publish` ran in
  the publication workflow and passed: all six publishable crates and the
  Python distribution are at `0.4.1`.
- The six crates were published to crates.io in dependency order
  (`eggfetch-http-connect`, `eggfetch-core`, `eggfetch-cli`, `eggfetch-ffi`,
  `eggfetch-python`, `eggfetch-node`), and each resolves at `0.4.1` from
  the registry API and from the registry index afterwards.
- PyPI carries 33 distributions for `0.4.1` — 32 wheels plus one sdist — and
  an isolated `pip install eggfetch==0.4.1` smoke reports runtime version
  `0.4.1` with the `eggfetch.compat.httpx` facade importing cleanly.
- The published free-threaded wheel was installed into a fresh CPython 3.15.2t
  environment with the GIL disabled (`Py_GIL_DISABLED=1`); the runtime version
  and the installed distribution version both report `0.4.1`.

## The release wheel set

32 wheels across the four release platforms: Linux x86_64 and aarch64, macOS
arm64 and Windows x86_64, for Python 3.10 through 3.15 with the GIL enabled
(24 wheels), plus free-threaded CPython 3.14t and 3.15t (8 wheels). The
free-threaded Linux wheels target `manylinux_2_34`. The assembled set was
validated by `scripts/validate_wheel_coverage.py` in the rehearsal and again
locally against the downloaded artifact.

## Defect found and fixed during the release

The first build-only rehearsal,
[run 45316378399](https://github.com/eggstack/eggfetch/actions/runs/45316378399),
failed: all six GIL-enabled Windows rows (3.10 through 3.15) stopped at the
wheel smoke step with `python3.12: command not found`. The free-threaded
wheel work had changed that step from `python` to the version-suffixed
interpreter name, and the Windows runner images expose that name only for the
free-threaded builds.

`f02a3751` fixed it: the step now prefers the version-suffixed name and
falls back to `python` and then `python3`, all of which resolve to the
interpreter setup-python selected for the row. The second rehearsal,
[run 45320371329](https://github.com/eggstack/eggfetch/actions/runs/45320371329),
passed on the corrected candidate with all 32 wheels and the sdist.

The defect never reached a user: it was introduced by the unreleased
free-threaded wheel work and caught before any wheel was published.

## Version decision

0.4.1 is a **patch** bump. The only behavioural change in the release is the
addition of the free-threaded wheel rows, which is additive and breaks
nothing, so under `docs/reference/versioning.md` patch versions are always
backward-compatible.

The Rust public-surface oracle reports `no semver update required`, which
independently confirms the change is not a breaking one.

## Recorded deviation

`cargo publish -p eggfetch-python` fails its standalone verification on
PyO3's `extension-module` symbols, as `docs/releases/process.md` documents.
The crate archive was therefore published with `--no-verify`, and only after
the build-only PyPI rehearsal had validated the assembled set at the same
SHA.

## What this release does not change

The HTTPX Stage C binding stays invalidated. The executable-input changes
after the `48ff1a39…` freeze — the free-threaded wheel work, the benchmark
harness changes and the version bump — mean the tree is still unqualified
against the compatibility profiles, and no publication may cite the previous
Stage C binding until a fresh exact-SHA qualification runs. This release
therefore does not advance that binding.

## Follow-up

- A fresh exact-SHA Stage C requalification is outstanding. It needs Tier 2
  plus the operator-supplied `target/downstream-qualification/artifact-manifest.json`.
- The `rustix` dependency is still 1.1.6. The live advisory database used for
  this release reports it as clean.
- The `abi3t` / PEP 803 question remains open and is recorded as a
  maintainer action in `plans/registry.md`.
