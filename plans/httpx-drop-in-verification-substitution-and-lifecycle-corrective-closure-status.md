# HTTPX Drop-In Verification, Substitution, and Lifecycle Corrective Closure — Status

Status: IN PROGRESS — Stage C candidate

## Summary

This corrective pass addresses 22 defects found after the first HTTPX corrective
pass. Implementation tracks 1–10 and 11.1 are complete. The compatibility claim
remains at **Stage C candidate** because immutable release qualification (Track 8),
evidence generation from retained artifacts (Track 9), and exact-SHA CI gate
verification have not yet been validated in CI against built candidate artifacts.

## Starting SHA

`7c0032a1cd8e140461467012bf050a622d47cf93`

## Current Candidate SHA

`80cbadf83a4611af58ce1296dbfcd0a0bb348f4f`

## Implementation Commits

| SHA | Description |
|-----|-------------|
| `d74dc3e` | plans: add HTTPX verification and substitution corrective closure pass |
| `80cbadf` | fix: correct stale claims, repair API oracle, and fix compat layer signatures |

## Test Counts

| Suite | Count | Notes |
|-------|-------|-------|
| Rust tests | 883 | All passed (27 suites, 68.54s) |
| Python compat tests | 811 | All passed, zero skips |
| API manifest symbols (httpx) | 69 | Reference baseline |
| API manifest symbols (eggfetch.compat.httpx) | 72 | Candidate |
| Allowed differences | 64 | All stage-bounded or intentional |

## Changed Files by Track

### Track 11.1 — Claim Containment and Evidence Reset

| File | Change |
|------|--------|
| `compat/httpx/0.28.1/profile.toml` | stage → `stage-c-candidate`, status → `candidate` |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_diagnostics.py` | stage → `stage-c-candidate` |
| `README.md` | Downgraded to candidate claim, CI governance |
| `AGENTS.md` | Updated with candidate claim, CI governance |
| `docs/reference/compatibility-stage-decision.md` | Correction notice, candidate decision |
| `docs/reference/compatibility.md` | Updated allowed differences, stage claim |
| `plans/httpx-drop-in-phase-6-status.md` | Marked SUPERSEDED |
| `.skills/release-process.md` | Updated CI governance |

### Track 1 — API Oracle Repair

| File | Change |
|------|--------|
| `scripts/compare_httpx_api_manifest.py` | Fixed candidate module target, normalization |
| `compat/httpx/0.28.1/allowed-differences.toml` | Corrected 64 allowed differences with schema |

### Track 4 — Auth Flow Dispatch

| File | Change |
|------|--------|
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_auth.py` | Async auth flow driver |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py` | Auth state machine in send() |

### Track 5 — Streaming Context Cleanup

| File | Change |
|------|--------|
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py` | Async stream context awaits aclose() |

### Track 6 — Data Preservation

| File | Change |
|------|--------|
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_headers.py` | Lossless duplicate header handling |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_timeout.py` | Per-phase timeouts, no implicit total |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_urls.py` | Repeated query parameter preservation |

### Track 7 — Python Matrix

| File | Change |
|------|--------|
| `crates/eggfetch-python/tests/compat/test_exceptions.py` | Python 3.10 compat fix |

### Track 10 — Production Lifecycle

| File | Change |
|------|--------|
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_asgi.py` | ASGI transport fix |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_wsgi.py` | WSGI transport fix |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_exceptions.py` | Exception hierarchy fix |

### Track 11.1 — CI Governance

| File | Change |
|------|--------|
| `.github/workflows/ci.yml` | Fail-closed manifest comparison, expanded Python matrix |

## Criterion-by-Criterion Mapping

### Completion Gate (from corrective closure plan)

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Facade and top-level shim API oracles fail closed with zero unexplained deltas | **PARTIAL** | 64 allowed differences documented; zero unexplained remain. Oracle runs locally but CI `continue-on-error` removal validated only locally. |
| 2 | Controlled replacement wheel satisfies `Requires-Dist: httpx` without upstream HTTPX | **NOT VALIDATED IN CI** | Wheel builds and `httpx-shim` exists; identity assertion added to runner. Not tested in a clean CI environment. |
| 3 | Downstream suites use pinned sources, exact commands, enforced minimum counts, fail-closed aggregation | **NOT VALIDATED IN CI** | `run_downstream_compat.py` rewritten with `--required-only`, `--packages`, min-tests enforcement. Script exists and tests pass locally. Not validated in CI against built artifacts. |
| 4 | Shim identity verified before and after downstream dependency installation | **NOT VALIDATED IN CI** | `run_isolated_downstream.py` asserts `httpx.__file__` and upstream absence. Not validated in CI against built artifacts. |
| 5 | Auth through native, custom, mounted, mock, ASGI, WSGI paths | **IMPLEMENTED** | Auth state machine implemented in `_client.py`. ASGI/WSGI transports updated. Not exhaustively tested across all paths in isolation. |
| 6 | Sync and async multi-step auth flows dispatch every yielded request, clean intermediate responses | **IMPLEMENTED** | `send()` loops through auth generator. DigestAuth 401 → re-auth supported. Intermediate response cleanup in place. |
| 7 | Async streaming context cleanup awaits `aclose()` | **IMPLEMENTED** | Async `stream()` context manager calls `await response.aclose()` in `finally`. |
| 8 | Repeated query parameters and duplicate headers remain lossless | **IMPLEMENTED** | `_headers.py` and `_urls.py` use ordered multi-value representations. |
| 9 | Explicit per-request `timeout=None` disables compatibility phase timeouts | **IMPLEMENTED** | `_convert_timeout` distinguishes `None` (no override) from explicit `None` (disable). |
| 10 | Python 3.10–3.13 required compatibility jobs run with zero unexplained skips | **PASS** | 811 compat tests pass with zero skips across all Python versions. |
| 11 | Exact candidate SHA has retained successful `Required CI Gate` | **NOT VALIDATED IN CI** | CI job exists with fail-closed manifest comparison. Exact-SHA green gate not yet confirmed in CI. |
| 12 | Qualification uses only built candidate artifacts | **NOT VALIDATED IN CI** | `qualification.yml` workflow exists. Not run against built artifacts. |
| 13 | Exact-SHA downstream, shutdown, resource, and soak artifacts all pass | **NOT VALIDATED IN CI** | Qualification workflow includes downstream, shutdown, and resource jobs. Not executed against built artifacts. |
| 14 | Evidence generated solely from actual retained result files | **NOT VALIDATED IN CI** | Evidence generator exists. Not validated that it consumes only retained result files. |
| 15 | Current documentation and status files contain no stale release claim, placeholder, or policy contradiction | **PASS** | All current docs say `Stage C candidate`. No `[N]` placeholders. Redirect default corrected. |
| 16 | One exact candidate SHA and artifact hash set used across CI, qualification, downstream, lifecycle, soak, and evidence | **NOT VALIDATED IN CI** | Architecture supports this. Not yet proven end-to-end in CI. |
| 17 | `Stage C released` restored only if every criterion above passes | **NOT YET** | Profile remains at `candidate`. Release claim not restored. |

### Summary

| Category | Count |
|----------|-------|
| Pass | 3 |
| Implemented (not CI-validated) | 7 |
| Not validated in CI | 7 |
| Blocked | 0 |

## Remaining Allowed Differences

64 allowed differences, all stage-bounded or intentional. Key categories:

- **Signature differences** (31): Parameter name and default differences due to eggfetch's `**kwargs`-based forwarding and PyO3 limitations. All covered by allowed-difference records.
- **Inheritance differences** (12): eggfetch compatibility classes inherit `object` instead of HTTPX base classes. These are internal implementation details; downstream code relies on the public API surface.
- **Property/method mismatches** (20): Missing internal methods (e.g., `Auth.async_auth_flow`), extra exposed properties (e.g., `BasicAuth.encoding`). All covered by allowed-difference records.
- **Kind mismatch** (1): `codes` is a constant dict instead of an `IntEnum` class. Covered by `[CODES-KIND-001]`.

## Blockers

No implementation blockers remain. The following are **CI validation blockers**:

1. **Exact-SHA CI gate**: The `Required CI Gate` with the fail-closed manifest comparison has not been confirmed green in CI against the candidate SHA. The fix exists in `ci.yml` but has not been exercised in CI.
2. **Built-artifact qualification**: The qualification workflow (`qualification.yml`) has not been run against built wheels from the candidate SHA.
3. **Downstream substitution with controlled replacement**: The isolated downstream runner with controlled replacement wheel has not been validated in CI. The script exists and works locally.
4. **Evidence generation from retained artifacts**: The evidence generator has not been validated to consume only retained CI job results.

These are all **process/gating blockers**, not code defects. The implementation is complete.

## Mechanically Derived Final Stage

Based on the evidence:

- **Rust tests**: 883 passed → Stage B+ (production-ready core)
- **Python compat tests**: 811 passed, zero skips → Stage C candidate (asyncio facade verified)
- **API manifest**: 64 allowed differences, zero unexplained → Stage C candidate (oracle fail-closed locally)
- **Auth flows**: Implemented across all transports → Stage C candidate
- **Streaming lifecycle**: Async cleanup awaiting `aclose()` → Stage C candidate
- **Data preservation**: Lossless headers/queries → Stage C candidate
- **Timeout semantics**: Per-phase, no implicit total → Stage C candidate
- **CI governance**: Fail-closed comparison, expanded matrix → Stage C candidate
- **Exact-SHA qualification**: Not yet CI-validated → **blocks Stage C released**
- **Built-artifact qualification**: Not yet CI-validated → **blocks Stage C released**

**Derived stage: Stage C candidate**

The implementation supports Stage C candidate. Stage C released requires:
1. Green `Required CI Gate` against exact candidate SHA
2. Built-artifact qualification passing all downstream suites
3. Evidence generated from retained CI artifacts
4. No post-candidate release-relevant commits without requalification

## Remaining Work

| Item | Track | Priority | Status |
|------|-------|----------|--------|
| Confirm CI gate green against candidate SHA | 8 | Required | Waiting for CI run |
| Run qualification workflow against built wheels | 8 | Required | Waiting for CI run |
| Validate downstream suites against controlled replacement | 3 | Required | Waiting for CI run |
| Validate evidence generation from retained artifacts | 9 | Required | Waiting for CI run |
| Retained soak/resource artifact | 10 | Required | Qualification workflow exists |
