# Release and Verification M001A — 0.2.1 Release-Candidate Preparation Closure

Status: closed

Source implementation plan:

- `plans/implementation/release-verification/001a-0-2-1-release-candidate-preparation.md`

Source subsystem roadmap:

- `plans/subsystems/release-verification-roadmap.md`

Candidate SHA (immutable M002/M001B input):

- `41757569123c0b8038550b956d8b244ab55094a6`
  (`release: bump coordinated versions to 0.2.1`)

Parent planning baseline: `42937e2b6bb8a2ef96cccf26920aeed915882b70`
(remote `main` HEAD at M001A start; remote CI run `36216550054` green on it).

## 1. Version audit

All coordinated manifests are exactly `0.2.1`:

- `crates/eggfetch-http-connect/Cargo.toml`, `crates/eggfetch-core/Cargo.toml`,
  `crates/eggfetch-cli/Cargo.toml`, `crates/eggfetch-ffi/Cargo.toml`,
  `crates/eggfetch-python/Cargo.toml`, `crates/eggfetch-node/Cargo.toml`
- `crates/eggfetch-python/pyproject.toml` (`eggfetch` distribution)
- `crates/eggfetch-node/package.json` (experimental prototype, kept coherent
  per the `0.2.0` bump precedent)
- Internal requirements: `cli → core`, `ffi → core`, `python → core` on
  `^0.2.1`; `core → http-connect` on `^0.2.1`; `node → ffi` on `^0.2.1`
- `Cargo.lock` self-package versions regenerated via
  `cargo update -p <six crates>` (only the six version lines changed)

`python scripts/validate_release_versions.py`: all six crates + pyproject
consistent at `0.2.1` (installed `eggfetch` distribution also `0.2.1` after
`maturin develop` rebuild).
`python scripts/validate_publishable_internal_dependencies.py`: all five
internal edges resolve at `^0.2.1`.

`CHANGELOG.md` carries a `0.2.1` section dated 2026-09-26 plus
`[Unreleased] → v0.2.1...HEAD` and new `[0.2.1]` tag links.
`docs/reference/versioning.md` and
`docs/architecture/rust-surface-containment.md` version references moved to
`0.2.1`. `README.md` needs no change (uses `0.2`, unchanged by a patch bump).

`v0.2.0` tag untouched (still targets
`8959ca890ee34f4cf456aed648315322f1e83ef7`). `v0.2.1` absent locally and on
`origin` at freeze time. Candidate worktree clean; `git diff --check` clean.

## 2. Candidate-delta audits

### `v0.2.0` tag → candidate (`8959ca89..41757569`)

- Release identity: coordinated `0.2.0 → 0.2.1` across the six Cargo
  manifests, `pyproject.toml`, `package.json`, `Cargo.lock`, CHANGELOG links.
- Qualification/test hardening (already Stage C-bound): hermetic
  `crates/eggfetch-core/tests/tls_response_completeness.rs` matrix plus the
  "Origin TLS shutdown contract" docs subsection. No production transport,
  API, or dependency change.
- Harness only: `eggfetch-bench` deterministic fixtures / `BenchProxy` /
  `native_streaming_tail` (never published; bench-dev-only `Cargo.lock`
  entries).
- Docs/planning only: architecture touch-ups, planning-system migration,
  M006/M006C1/M006C2 closures, M001A/M002/M001B decomposition. No CI
  workflow, packaging, or compatibility-profile change.

### Stage C freeze → candidate (`5247ff0e..41757569`)

Name-only delta is release identity (`Cargo.toml` × 6, `pyproject.toml`,
`package.json`, `Cargo.lock` versions, CHANGELOG `0.2.1` section), the
`test_sync.py` version assertion `0.2.0 → 0.2.1`, two docs version-reference
lines, one docs-only TLS-shutdown-contract subsection, and `plans/` records.
No `crates/*/src` change, no `scripts/` change, no `.github/` change, no
`compat/` change, no dependency source/version change beyond the six
self-versions. The `test_sync.py` edit is release identity (pins the new
coordinated version; Tier 1 fails without it), not a behavior-semantics
change.

## 3. Qualification gates (all on the candidate tree)

- Tier 1 `./scripts/check.sh`: green (`All routine checks passed`).
  One in-pass fix is recorded honestly: the first Tier 1 run failed at the
  version guard because the installed `eggfetch` wheel was still `0.2.0`;
  `maturin develop -m crates/eggfetch-python/Cargo.toml` rebuilt it to
  `0.2.1`, and the `test_sync.py` version assertion was bumped to `0.2.1`
  (both folded into the candidate commit before the green rerun).
- Tier 2 `./scripts/check.sh extended`: green
  (`Extended validation passed`, 2 allowed skips: Node JS surface native
  artifact absent, downstream artifact manifest absent).
- Tier 3 `./scripts/check.sh package` on the clean committed candidate:
  green (`All package checks passed`, incl. `cargo publish --dry-run` leaf,
  wheel build + smoke + typing smoke). Tier 3 correctly refused the
  uncommitted tree first (`--allow-dirty` not used); the candidate was
  committed, then Tier 3 rerun green.
- No live `check_security.sh` publication preflight claimed here: per M001A
  §5 it runs live immediately before crates.io publication in M001B.

## 4. Stage C disposition

Stage C remains bound to `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`.
The candidate proves no executable or qualification-input change relative to
that freeze beyond release identity + docs/plans (see §2). No ADR-0003
requalification was triggered.

## 5. Handoff

M001A is closed. The sole ready release task is now M002 (Python 3.10–3.15
wheel/sdist rehearsal with `publish=false` from exactly `41757569`).
M001B (crates.io × 6 in leaf order, signed `v0.2.1` tag on `41757569`,
`pypi.yml` `publish=true` from that tag, straight to prod PyPI, no
test.pypi.org) stays blocked until M002 closes on that exact SHA. This
closure record itself is a docs-only descendant and does not replace the
candidate SHA.
