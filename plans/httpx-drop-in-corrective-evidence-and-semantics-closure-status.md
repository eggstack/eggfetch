# HTTPX Drop-In Corrective Evidence and Semantics Closure — Status

Status: IN PROGRESS

## Summary

This corrective pass addresses defects found after the HTTPX drop-in roadmap
was implemented. The compatibility claim was downgraded from `Stage C released`
to `Stage C candidate` to reflect the actual evidence state.

## Starting SHA

`dcf773b09a4e511facbf470d22b038c4f6712a77` (main branch)

## Corrective Changes by Track

### Track A — Claim Containment and Evidence Reset
- Profile status: `released` → `candidate`
- Diagnostics stage: `stage-c` → `stage-c-candidate`
- Evidence files marked `invalidated`
- Phase 6 status marked `SUPERSEDED`
- README, AGENTS.md, skills updated with candidate claim and CI governance

### Track B — Compatibility Oracle Repair
- CI manifest comparison now targets `eggfetch.compat.httpx` (not root `eggfetch`)
- Removed `continue-on-error: true` from manifest comparison (fail-closed)
- Resolved allowed-differences entries updated to reflect actual behavior
- Timeout conversion no longer sets implicit total deadline

### Track C — Downstream Substitution Validation
- `run_isolated_downstream.py` rewritten to install eggfetch wheel (not upstream httpx)
- Added upstream httpx detection (fails when upstream is present)
- Added shim identity verification (asserts `httpx.__file__` resolves to eggfetch)
- `run_downstream_compat.py` rewritten to actually run downstream tests (not just imports)
- Added `--wheel-dir`, `--required-only`, `--packages` options
- Downstream manifest updated with `test-command` and `min-tests` fields
- Downstream portfolio tests expanded to validate new manifest fields

### Track D — Timeout Semantics
- `_convert_timeout` no longer passes `seconds=timeout.total` to native layer
- Numeric timeouts map to per-phase fields (connect, read, write, pool) only
- HTTPX `Timeout(5.0)` no longer creates a 5-second total deadline

### Track E — Auth Flow Dispatch
- Sync and async `send()` now loop through the auth flow generator
- Multi-step auth (DigestAuth 401 → re-auth) dispatches follow-up requests
- Auth works through native, mounted, mock, and custom transports

### Track F — Streaming Context Cleanup
- Sync `stream()` context manager closes response in `finally` block
- Async `stream()` context manager closes response in `finally` block
- Response is closed on normal exit, exception, and early cancellation

### Track G — Extension Merging
- Client-level extensions preserved when request extensions are omitted
- Request extensions override matching client keys
- Unrelated client keys remain in merged result

### Track I — CI and Governance
- `wheel-smoke` CI job expanded to all Python versions (3.10-3.13)
- `compat-httpx` CI job expanded to all Python versions (3.10-3.13)
- Manifest comparison is now fail-closed (no `continue-on-error`)

### Track K — Immutable Release Requalification
- Created `qualification.yml` workflow for release qualification
- Builds wheels across 5 platforms × 4 Python versions
- Validates package content via `validate_package_content.py`
- Runs wheel smoke tests across all Python/OS combinations
- Runs HTTPX compat tests across all Python versions
- Runs downstream substitution tests (respx, httpx-sse, httpx-auth, starlette, anyio, pydantic)
- Tests shim substitution with identity verification
- Generates qualification evidence
- Has dry-run mode for non-publishing validation

## Files Modified

| File | Track |
|------|-------|
| `compat/httpx/0.28.1/profile.toml` | A |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_diagnostics.py` | A |
| `compatibility-evidence.json` | A |
| `compatibility-report.md` | A |
| `performance-budget-results.json` | A |
| `plans/httpx-drop-in-phase-6-status.md` | A |
| `README.md` | A, I |
| `AGENTS.md` | A, I |
| `.skills/release-process.md` | I |
| `.github/workflows/ci.yml` | B, I |
| `.github/workflows/qualification.yml` | K |
| `compat/httpx/0.28.1/allowed-differences.toml` | B |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py` | D, E, F, G |
| `scripts/run_isolated_downstream.py` | C |
| `scripts/run_downstream_compat.py` | C |
| `compat/downstream/manifest.toml` | C |
| `crates/eggfetch-python/tests/compat/test_downstream_portfolio.py` | C |

## Remaining Work

- Track H: Production-semantics gaps (soak workflow, Phase 1 status cleanup)

## Final Stage Decision

**Stage C candidate** — The implementation has corrected known defects and
built qualification infrastructure. Immutable release qualification is not
yet complete (Track H remains). The claim may be restored to `Stage C released`
only after every acceptance criterion in the corrective plan is satisfied
against one exact candidate SHA.
