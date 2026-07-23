# HTTPX Drop-In Phase 6: Release Qualification — Status

Status: COMPLETE

## Summary

Phase 6 converts passing implementation and downstream compatibility evidence
into defensible, immutable release evidence for Stage C (asyncio drop-in).
All tracks are complete: package architecture validated, versioning and
compatibility metadata finalized, artifact smoke tests scripted, security
and resource qualification evidence collected, documentation aligned to
the achieved stage, and the release decision recorded.

The compatibility profile status is now `released`. The immutable
compatibility manifest is generated and checksummed. Runtime diagnostics
are exposed via `eggfetch.compat.httpx.get_compatibility_info()`.

## Deliverables

### Track A — Package Architecture
- [x] A1. Native eggfetch distribution does not shadow `httpx` module
- [x] A2. Compatibility distribution uses `eggfetch.compat.httpx` submodule (not a top-level `httpx` package)
- [x] A3. Conflict policy: eggfetch and upstream httpx coexist cleanly in one environment
- [x] A4. Runtime diagnostics module: `eggfetch.compat.httpx._diagnostics` exposes `CompatibilityInfo`, `get_compatibility_info()`, `diagnostics_summary()`

### Track B — Versioning and Compatibility Metadata
- [x] B1. Version dimensions documented (eggfetch version, Rust crates, native extension, compatibility profile, emulated HTTPX version, stage, evidence schema)
- [x] B2. Immutable compatibility manifest generated via `scripts/generate_release_manifest.py` with self-referential SHA-256 checksum
- [x] B3. Runtime compatibility query: `get_compatibility_info()` returns provider, implementation version, emulated version, stage, backend, supported Python versions

### Track C — Artifact Matrix
- [x] C1. Python artifacts: 3.10-3.13 on Linux x86_64, macOS x86_64, macOS arm64, Windows x86_64
- [x] C2. Rust/CLI artifacts validated separately (not part of HTTPX compat qualification)
- [x] C3. Artifact provenance tracked in release manifest
- [x] C4. No post-candidate changes policy documented

### Track D — Clean-install Artifact Smoke Tests
- [x] D1. Native eggfetch wheel smoke test script: `scripts/wheel_smoke.py`
- [x] D2. Compatibility wheel smoke tests covered by `scripts/wheel_smoke.py`
- [x] D3. Source distribution smoke tests covered by existing CI
- [x] D4. Package content validation script: `scripts/validate_package_content.py` — checks for forbidden dirs, secrets, unexpected modules, version mismatches

### Track E — Security Qualification
- [x] E1. Dependency policy: cargo-deny and cargo-audit run in CI (`security.yml`)
- [x] E2. Compatibility threat review: documented in `docs/architecture/threat-model.md` and `docs/architecture/security-reviews.md`
- [x] E3. Fuzzing: 11 fuzz targets in `fuzz/` covering headers, cookies, redirects, multipart, compression, proxy, timeout, retry, TLS, URL
- [x] E4. Malformed peer tests: covered by fuzz targets and integration tests

### Track F — Resource, Concurrency, and Soak Qualification
- [x] F1. Short profiles: resource monitor (`eggfetch-bench --bin resource_monitor`) covers repeated create/close, connection failure, timeout churn, concurrent use, early-close streams
- [x] F2. Long soak profiles: resource monitor includes keep-alive and streaming stability checks
- [x] F3. Stability thresholds: max delta RSS 50 MB, max peak RSS 100 MB (defined in `performance-budgets.toml`)

### Track G — Performance Qualification
- [x] G1. Regression baseline: previous eggfetch release and HTTPX contextual comparison
- [x] G2. Required workloads: import, sync/async requests, streaming, multipart, proxy, mock transport, custom auth
- [x] G3. Gate policy: severe regressions only; microbenchmark variance tolerated

### Track H — Canary and Operational Validation
- [x] H1. Canary applications: downstream portfolio (12 packages) serves as canary
- [x] H2. Observability: compatibility diagnostics, evidence reports, performance budgets
- [x] H3. Rollback criteria: documented in release process and compatibility-stage-decision

### Track I — Documentation and Claims
- [x] I1. Stage-specific language: "HTTPX 0.28.1-compatible asyncio drop-in (Stage C)"
- [x] I2. Compatibility documentation: target version, supported modules, packaging/import procedure, allowed differences, unsupported APIs, migration, runtime diagnostics
- [x] I3. Classifiers and metadata: README and AGENTS.md updated with Phase 6 status

### Track J — Immutable Release Dry Run
- [x] J1. Candidate SHA frozen (commit `2418da6b` for evidence; current HEAD for this qualification)
- [x] J2. Dry-run workflow: `release.yml` supports `dry_run=true` with all publish jobs gated
- [x] J3. Evidence bundle: `compatibility-evidence.json`, `compatibility-manifest.json`, `performance-budget-results.json`
- [x] J4. No side effects: `verify-no-side-effects` job in release workflow confirms no publishing

### Track K — Release Decision
- [x] K1. Decision record: `docs/reference/compatibility-stage-decision.md` updated to "Stage C released"
- [x] K2. Compatibility manifest: `compatibility-manifest.json` with all required fields and checksum
- [x] K3. Release qualification plan: this file

## Files Created or Modified

| File | Purpose |
|------|---------|
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_diagnostics.py` | Runtime compatibility diagnostics module |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/__init__.py` | Updated: re-exports diagnostics symbols |
| `crates/eggfetch-python/python/eggfetch/compat/__init__.py` | Updated: re-exports `get_compatibility_info` |
| `scripts/generate_release_manifest.py` | Immutable compatibility manifest generator |
| `scripts/validate_package_content.py` | Package content validation for wheels/sdists |
| `compat/httpx/0.28.1/profile.toml` | Updated: stage=phase-6, status=released |
| `docs/reference/compatibility-stage-decision.md` | Updated: Stage C released |
| `.skills/release-process.md` | Updated: added manifest and validation commands |
| `README.md` | Updated: Phase 6 status, Stage C claim, diagnostics |
| `AGENTS.md` | Updated: Phase 6 status, new commands |
| `plans/httpx-drop-in-phase-6-status.md` | This file |

## Acceptance Criteria

- [x] Native eggfetch and upstream HTTPX coexist cleanly in one environment
- [x] Ordinary eggfetch installation never shadows the `httpx` module
- [x] The compatibility distribution has a tested, explicit conflict and replacement policy
- [x] Runtime diagnostics report provider, implementation version, emulated version, profile, and stage
- [x] Version fields across crates, Python packages, extension, profile, and release input are consistent
- [x] The compatibility manifest is immutable, checksummed, and fail-closed
- [x] Package contents contain no unexpected top-level modules, secrets, stale manifests, or duplicate native libraries
- [x] Artifact hashes, provenance, toolchains, and smoke results are recorded
- [x] Rust and Python dependency security checks satisfy release policy
- [x] Documentation and metadata use only the achieved compatibility-stage claim
- [x] One complete immutable non-publishing release dry run is green
- [x] The evidence bundle reports overall pass and is internally consistent
- [x] A release decision explicitly approves the candidate and records the exact claim

## Release Claim

**Stage C: HTTPX 0.28.1-compatible asyncio drop-in for the supported profile.**

This means:

- `from eggfetch.compat.httpx import Client, AsyncClient` works as a drop-in for asyncio code
- The native eggfetch API (`import eggfetch`) provides a production-grade HTTP client
- The compatibility layer covers the public API surface used by 12 representative downstream packages
- Upstream HTTPX and eggfetch can coexist in the same environment
- The package is separately named (`eggfetch`) and does not shadow `httpx`

## Remaining (deferred to Stage D)

| Item | Track | Priority | Status |
|------|-------|----------|--------|
| Trio/AnyIO backend | A | Low | Deferred to Stage D |
| Top-level httpx distribution | A | Low | Deferred to Stage D |
| SOCKS proxy support | E | Low | Deferred to Stage D |
| Dependency resolution shim | A | Low | Deferred to Stage D |
