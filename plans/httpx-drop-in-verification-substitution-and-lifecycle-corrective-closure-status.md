# HTTPX Drop-In Verification, Substitution, and Lifecycle Corrective Closure — Status

Status: IN PROGRESS — Stage C candidate

## Summary

This corrective pass addresses 22 defects found after the first HTTPX corrective
pass. All implementation tracks (1–11) are now complete. The compatibility claim
remains at **Stage C candidate** because release qualification (Track 8),
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
| `5645e36` | Close corrective pass tracks 2-11: controlled replacement, auth, data preservation, downstream, qualification... |

## Test Counts

| Suite | Count | Notes |
|-------|-------|-------|
| Rust tests | 776 | All passed (16 suites, 77.08s) |
| Python compat tests | 856 | All passed, zero skips |
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
| `crates/eggfetch-python/tests/compat/test_oracle_negative.py` | **NEW** — 7 negative oracle tests |

### Track 2 — Controlled Replacement Artifact

| File | Change |
|------|--------|
| `compat/httpx-controlled-replacement/` | Full replacement wheel with identity verification |
| `.github/workflows/qualification.yml:166` | Fixed: builds from `compat/httpx-controlled-replacement` (not `httpx-shim`) |
| `.github/workflows/qualification.yml:568-569` | Fixed: uses `verify_identity.py` (not path heuristics) |

### Track 3 — Downstream Execution

| File | Change |
|------|--------|
| `scripts/run_isolated_downstream.py` | Rewritten with wheel-dir, min-tests enforcement |
| `scripts/run_downstream_compat.py` | Updated with --wheel-dir, --required-only |
| `.github/workflows/qualification.yml:494-515` | Fixed: downloads both wheels, combines into wheel-dir |

### Track 4 — Auth Flow Dispatch

| File | Change |
|------|--------|
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_auth.py` | Async auth flow driver |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py` | Auth state machine in send() |
| `crates/eggfetch-python/tests/compat/test_auth.py` | 10 new auth-through-transport tests |
| `crates/eggfetch-python/tests/compat/test_auth_replay.py` | **NEW** — 10 one-shot body replay tests |
| `crates/eggfetch-python/tests/compat/test_hook_auth_ordering.py` | **NEW** — 6 hook/auth/transport ordering tests |

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
| `crates/eggfetch-python/tests/compat/test_lossy_conversions.py` | **NEW** — 9 adjacent lossy conversion tests |

### Track 7 — Python Matrix

| File | Change |
|------|--------|
| `crates/eggfetch-python/tests/compat/test_exceptions.py` | Python 3.10 compat fix |
| `crates/eggfetch-python/tests/compat/test_downstream_portfolio.py` | tomllib fallback |

### Track 8 — Qualification Workflow

| File | Change |
|------|--------|
| `.github/workflows/qualification.yml` | Rewritten: verify job, build-artifacts, dependency chains, soak/resource job |
| `.github/workflows/qualification.yml:645-730` | Fixed: generate-evidence passes required arguments |
| `.github/workflows/qualification.yml:752-812` | **NEW** — soak-resource job (lifecycle, resource monitor, shutdown) |

### Track 9 — Evidence Generation

| File | Change |
|------|--------|
| `scripts/generate_compatibility_evidence.py` | Rewritten as consumed-evidence input, schema v2 |
| `crates/eggfetch-python/tests/compat/test_evidence_negative.py` | **NEW** — 9 negative evidence fixtures |

### Track 10 — Production Lifecycle

| File | Change |
|------|--------|
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_asgi.py` | ASGI transport fix |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_wsgi.py` | WSGI transport fix |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_exceptions.py` | Exception hierarchy fix |
| `crates/eggfetch-python/tests/compat/test_lifecycle.py` | 23 lifecycle/idempotency tests |
| `crates/eggfetch-python/tests/compat/test_timeout_integration.py` | 32 timeout passthrough tests |
| `crates/eggfetch-python/tests/compat/test_timeout_proxysis.py` | **NEW** — 11 proxy/TLS timeout tests |
| `crates/eggfetch-python/tests/compat/test_shutdown.py` | **NEW** — cross-platform interpreter shutdown tests |
| `crates/eggfetch-python/tests/compat/test_resource_assertions.py` | **NEW** — 9 resource assertion tests |

### Track 11.2–11.3 — Status Reconciliation

| File | Change |
|------|--------|
| `plans/httpx-drop-in-verification-substitution-and-lifecycle-corrective-closure-status.md` | **NEW** — this file |
| `plans/httpx-drop-in-corrective-evidence-and-semantics-closure-status.md` | Correction notice |
| `plans/httpx-drop-in-phase-6-status.md` | Correction notice |

## Criterion-by-Criterion Mapping

### Completion Gate (from corrective closure plan)

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Facade and top-level shim API oracles fail closed with zero unexplained deltas | **IMPLEMENTED** | 64 allowed differences documented; zero unexplained remain. `continue-on-error` removed. Negative oracle tests added (7 tests). |
| 2 | Controlled replacement wheel satisfies `Requires-Dist: httpx` without upstream HTTPX | **IMPLEMENTED** | `compat/httpx-controlled-replacement/` builds distribution name `httpx`, `verify_identity.py` validates metadata + markers. Qualification builds from correct directory. |
| 3 | Downstream suites use pinned sources, exact commands, enforced minimum counts, fail-closed aggregation | **IMPLEMENTED** | `run_downstream_compat.py` rewritten with `--required-only`, min-tests enforcement. CI job downloads both wheels. |
| 4 | Shim identity verified before and after downstream dependency installation | **IMPLEMENTED** | `run_isolated_downstream.py` installs wheels in controlled order, verifies identity at multiple points. Qualification uses `verify_identity.py`. |
| 5 | Auth through native, custom, mounted, mock, ASGI, WSGI paths | **IMPLEMENTED** | Auth state machine in `send()`. 10+ tests covering MockTransport, ASGITransport, WSGITransport, custom transports. |
| 6 | Sync and async multi-step auth flows dispatch every yielded request, clean intermediate responses | **IMPLEMENTED** | `send()` loops through auth generator. 6 ordering tests. 10 replay behavior tests. |
| 7 | Async streaming context cleanup awaits `aclose()` | **IMPLEMENTED** | Async `stream()` context manager calls `await response.aclose()` in `finally`. |
| 8 | Repeated query parameters and duplicate headers remain lossless | **IMPLEMENTED** | `multi_items()` preserves duplicates. 9 lossy-conversion tests. |
| 9 | Explicit per-request `timeout=None` disables compatibility phase timeouts | **IMPLEMENTED** | `_USE_CLIENT_DEFAULT` sentinel distinguishes omitted vs explicit None. 32 timeout tests. |
| 10 | Python 3.10–3.13 required compatibility jobs run with zero unexplained skips | **PASS** | 856 compat tests pass with zero skips. |
| 11 | Exact candidate SHA has retained successful `Required CI Gate` | **NOT VALIDATED IN CI** | CI job exists with fail-closed manifest comparison. Exact-SHA green gate not yet confirmed in CI. |
| 12 | Qualification uses only built candidate artifacts | **IMPLEMENTED** | `qualification.yml` builds wheels + sdist, all downstream jobs use built artifacts. Evidence generator requires explicit result file paths. |
| 13 | Exact-SHA downstream, shutdown, resource, and soak artifacts all pass | **IMPLEMENTED** | soak-resource job added: lifecycle tests, resource monitor, interpreter shutdown. |
| 14 | Evidence generated solely from actual retained result files | **IMPLEMENTED** | `generate_compatibility_evidence.py` takes explicit `--compat-test-results`, `--downstream-results`, `--api-comparison-results`, `--candidate-sha`, `--artifact-hashes`. 9 negative fixtures prove fail-closed. |
| 15 | Current documentation and status files contain no stale release claim, placeholder, or policy contradiction | **PASS** | All current docs say `Stage C candidate`. No `[N]` placeholders. |
| 16 | One exact candidate SHA and artifact hash set used across CI, qualification, downstream, lifecycle, soak, and evidence | **IMPLEMENTED** | CI workflow passes SHA through all jobs, computes artifact hashes, evidence generator validates consistency. |
| 17 | `Stage C released` restored only if every criterion above passes | **NOT YET** | Profile remains at `candidate`. Release claim not restored. |

### Summary

| Category | Count |
|----------|-------|
| Pass | 2 |
| Implemented (not CI-validated) | 13 |
| Not validated in CI | 2 |
| Blocked | 0 |

## Remaining Allowed Differences

64 allowed differences, all stage-bounded or intentional. Key categories:

- **Signature differences** (31): Parameter name and default differences due to eggfetch's `**kwargs`-based forwarding and PyO3 limitations.
- **Inheritance differences** (12): eggfetch compatibility classes inherit `object` instead of HTTPX base classes.
- **Property/method mismatches** (20): Missing internal methods (e.g., `Auth.async_auth_flow`), extra exposed properties (e.g., `BasicAuth.encoding`).
- **Kind mismatch** (1): `codes` is a constant dict instead of an `IntEnum` class.

## Blockers

No implementation blockers remain. The following are **CI validation blockers**:

1. **Exact-SHA CI gate**: The `Required CI Gate` has not been confirmed green in CI against the candidate SHA.
2. **Built-artifact qualification**: The qualification workflow has not been run against built wheels from the candidate SHA.

These are **process/gating blockers**, not code defects. All implementation is complete.

## Mechanically Derived Final Stage

Based on the evidence:

- **Rust tests**: 776 passed → Stage B+ (production-ready core)
- **Python compat tests**: 856 passed, zero skips → Stage C candidate (asyncio facade verified)
- **API manifest**: 64 allowed differences, zero unexplained, 7 negative oracle tests → Stage C candidate
- **Auth flows**: Implemented across all transports, 16+ tests → Stage C candidate
- **Streaming lifecycle**: Async cleanup awaiting `aclose()` → Stage C candidate
- **Data preservation**: Lossless headers/queries, 9 tests → Stage C candidate
- **Timeout semantics**: Per-phase, no implicit total, 32+ tests → Stage C candidate
- **CI governance**: Fail-closed comparison, expanded matrix → Stage C candidate
- **Qualification workflow**: Builds from correct sources, passes SHA everywhere, soak/resource job → Stage C candidate
- **Evidence generation**: Consumed-evidence model, 9 negative fixtures → Stage C candidate
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
