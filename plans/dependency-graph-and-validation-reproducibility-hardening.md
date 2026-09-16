# Dependency Graph and Validation Reproducibility Hardening

Planning baseline: current post-corrective/refactor program tree
Parent program: `plans/post-maintenance-core-integrity-and-verification-program.md`
Normative verification policy: `docs/verification-policy.md`
Security entry point: `./scripts/check_security.sh`

## Objective

Tighten the existing validation and dependency-security path without expanding eggfetch's deliberately small CI architecture.

The current repository already has the right high-level separation: deterministic Tier 1 validation on one Ubuntu job, explicit extended/package validation, and a fail-closed live security preflight for security/release use. This plan fixes two narrower reproducibility/coverage gaps:

1. `deny.toml` currently sets `all-features = false` and omits Windows from its target list, so cargo-deny's license/source/ban graph is narrower than eggfetch's supported optional dependency and publication surface.
2. routine CI creates a venv and installs `maturin pytest pytest-asyncio mypy` without versions, so the same eggfetch SHA can change validation behavior when PyPI releases a new tool.

Do not add another automatic workflow, CI matrix, scheduled advisory scan, or publication authority.

## Required work

### 1. Define the dependency-policy graph eggfetch intends to audit

Audit workspace features and release/platform support before changing `deny.toml`.

The security/license/source graph should cover all dependency families users can enable in supported combinations, including at minimum:

- HTTP/1 + HTTP/2;
- Rustls/native roots;
- proxy/SOCKS;
- cookies;
- compression codecs;
- tracing;
- JSON;
- HTTP/3 experimental dependencies;
- Python/FFI/Node workspace crates where cargo-deny sees them.

If `all-features = true` represents a valid superset for cargo-deny even where runtime feature combinations are not product profiles, prefer it because this is dependency policy rather than behavioral qualification. If mutually exclusive feature semantics make all-features misleading, enumerate the supported graph another way and document why.

Acceptance:

- [ ] No optional dependency family is omitted from advisory/license/source/ban policy merely because it is non-default.
- [ ] H3 remains experimental; inclusion in dependency scanning is not a graduation claim.

### 2. Cover supported target families in cargo-deny

Audit the actual package/wheel/release targets and update `[graph].targets` so dependency policy includes platform-conditional crates for supported publication families.

At minimum include Windows x86_64 in addition to the current Linux/macOS targets if it remains a supported release platform.

Add Linux AArch64 only if current release/support documentation treats it as a supported target rather than an aspirational downstream build.

Do not add every Rust target indiscriminately.

Acceptance:

- [ ] Windows-only dependency branches used by supported releases are visible to cargo-deny.
- [ ] Target list matches documented support rather than contributor workstation convenience.

### 3. Keep live advisory semantics unchanged

Retain:

```sh
cargo deny check advisories licenses bans sources
cargo audit --file Cargo.lock
```

or an equivalent explicit fail-closed pair.

`cargo-audit` lockfile scanning remains a useful independent RustSec check. Do not treat cargo-deny all-features coverage as a replacement for cargo-audit.

Do not move live advisory fetches into `./scripts/check.sh` or routine CI.

### 4. Pin routine Python validation tooling

Create one repository-controlled requirements/constraints file for the Python tools installed by `.github/workflows/ci.yml`.

At minimum pin exact reviewed versions of:

- maturin;
- pytest;
- pytest-asyncio;
- mypy.

Where practical, reuse `scripts/release-requirements.txt` for the maturin version rather than independently drifting two pins. Acceptable structures include:

- `scripts/ci-requirements.txt` with exact pins and a comment that maturin must stay aligned with release requirements; or
- a shared constraints file consumed by both CI and release validation.

Do not pin ordinary runtime/test dependencies twice if they are already controlled by the existing compatibility requirement files.

### 5. Make CI install fail predictably

Change `.github/workflows/ci.yml` from unconstrained package names to the repository-controlled requirements file using `python -m pip install -r ...`.

Do not add pip cache choreography unless needed. The goal is deterministic tool selection, not a more complex workflow.

If hashes are practical across the one Linux/Python 3.12 CI environment, they are welcome but not required; exact versions are the minimum acceptance criterion.

### 6. Establish an update rule

Add a brief comment/documentation rule:

- tool bumps are reviewed repository changes;
- bump one tool set deliberately;
- run `./scripts/check.sh` and affected packaging/type gates;
- do not auto-update validation tools independently of source review.

Avoid introducing Dependabot/Renovate solely for these files in this pass.

### 7. Validate dependency-policy changes against current graph

Run the current explicit security gate after updating graph coverage.

If all-feature/Windows coverage surfaces additional duplicate/license/source/advisory findings:

- investigate exact dependency paths;
- upgrade/remove only when justified;
- document warnings that remain non-failing;
- never add blanket advisory ignores to make the gate green.

Any new RustSec exception must follow the governance already documented in the completed security plan.

### 8. Keep the CI complexity budget intact

This plan must leave the following unchanged:

- one automatic workflow;
- one required Ubuntu job;
- no routine matrix;
- no artifact exchange;
- no automatic publication;
- live security preflight separate from deterministic Tier 1.

If a proposed change violates that budget, it is outside this plan.

## Files expected to change

Likely:

- `deny.toml`;
- `.github/workflows/ci.yml`;
- `scripts/ci-requirements.txt` or shared constraints equivalent;
- possibly `scripts/release-requirements.txt` if consolidating the maturin pin;
- narrow verification/dependency-policy prose.

No production runtime dependency is expected solely from this plan.

## Validation

Required:

```sh
./scripts/check.sh
./scripts/check_security.sh
```

Also validate the CI requirements file in a fresh Python 3.12 venv and run package validation if maturin/release tooling ownership changes:

```sh
./scripts/check.sh package
```

## Non-goals

- no second automatic workflow;
- no scheduled RustSec scan;
- no CI OS/Python matrix;
- no automatic tool updater;
- no SBOM/provenance program;
- no wholesale dependency refresh;
- no new release automation;
- no change to manual crates.io or protected PyPI publication policy.

## Exit criteria

- [ ] cargo-deny policy covers supported optional dependency families rather than default features only.
- [ ] cargo-deny target coverage includes supported Windows release dependencies.
- [ ] explicit live security scanning remains fail closed and separate from routine CI.
- [ ] routine CI Python tool versions are repository-controlled at a given SHA.
- [ ] maturin validation/release pins cannot silently diverge without a reviewed diff or explicit rationale.
- [ ] one-workflow/one-job/no-matrix policy remains intact.
- [ ] Tier 1, security preflight, and package validation (when affected) pass.
