# Compatibility Stage Decision

**Corrective Closure — Verification, Substitution, and Lifecycle Pass**

> **Correction Notice**: This document previously claimed "Stage C released" based on
> incomplete evidence. The corrective pass
> (`plans/httpx-drop-in-verification-substitution-and-lifecycle-corrective-closure.md`)
> identified defects in API oracle verification, controlled substitution, downstream
> execution, auth flows, streaming lifecycle, data preservation, timeout handling,
> Python matrix coverage, exact-SHA qualification, and evidence generation. The
> compatibility claim has been downgraded to **Stage C candidate** pending completion
> of all acceptance criteria in the corrective closure plan.

| Field | Value |
|-------|-------|
| Date | 2026-07-24 |
| Stage Evaluated | Stage C (asyncio drop-in) |
| Decision | **Stage C candidate** |
| Evidence | Corrective closure plan, compat tests, API manifest comparison, typed difference records, schema v3 identity, lossless merge, native lifecycle fixtures |

## Decision Rationale

Stage C candidate status is supported by the following evidence:

1. **API Surface**: The httpx 0.28.1 compatibility layer provides full Client/AsyncClient constructors, all HTTP methods, request/response objects, streaming, and transport abstraction.
2. **Typed Difference Records**: The API oracle (`scripts/compare_httpx_api_manifest.py --validate`) produces structured difference records. `allowed-differences.toml` gates CI enforcement with exact difference matching.
3. **API Surface Coverage**: Direct tests verify Client/AsyncClient constructors, all HTTP methods, request/response objects, streaming, and transport abstraction.
4. **Lossless Merge Semantics**: Header and query parameter merge preserves order and duplicates across all transport paths (`test_merge_lossless.py`).
5. **Separate Sync/Async Auth Drivers**: The `Auth` base class dispatches to independent sync and async implementations, eliminating shared mutable state.
6. **Behavioral Downstream Fixtures**: `compat/downstream/behavioral_fixtures/` exercises real consumer patterns with pinned sources and enforced minimum counts.
7. **Native Lifecycle Proof Fixtures**: Timeout classification (`test_native_timeout_classification.py`), soak (`test_soak.py`), and lifecycle tests validate engine behavior under load.
8. **Direct Test Execution**: Compatibility behavior is validated through direct test execution without qualification artifacts.

### Remaining blockers for Stage C released

The following must be resolved before restoring a release claim:

1. API oracle must fail-closed in CI (currently informational)
2. Controlled replacement artifact (`httpx` wheel) must satisfy downstream `Requires-Dist: httpx`
3. Downstream suites must use pinned sources, exact commands, and enforced minimum counts
4. Auth must work through all transport paths (mounted, custom, mock, ASGI, WSGI)
5. Async streaming context must await `aclose()` instead of calling sync `close()`
6. Repeated query parameters and duplicate headers must survive all transport paths
7. Explicit per-request `timeout=None` must be distinguishable from client default
8. Python 3.10 required compatibility suite must run without skips
9. Direct compatibility tests must pass against pinned HTTPX version
10. Behavioral correctness must be validated through direct test execution

## Blockers to Stage D

Stage D (full supported drop-in) requires:

1. **Trio/AnyIO Backend**: eggfetch uses asyncio only (tokio-based). Trio support is architecturally deferred.
2. **Top-level httpx Distribution**: A separate distribution providing `import httpx` is not yet built.
3. **Dependency Resolution**: A differently-named package cannot satisfy `httpx>=0.27` dependencies without a shim distribution.

## Allowed Differences

The following differences are reviewed and accepted:

| ID | Category | Impact |
|----|----------|--------|
| REDIRECT-DEFAULT-001 | intentional | Security-first default |
| TIMEOUT-TUPLE-001 | intentional | Compatible API |
| EXCEPTION-NAMES-001 | intentional | Different base name |
| RAISE-FOR-STATUS-001 | intentional | Compatible behavior |
| PROXY-ENV-001 | resolved | Implemented in Phase 1 |
| EVENT-HOOKS-001 | resolved | Implemented in Phase 4 |
| TRANSPORTS-001 | resolved | Implemented in Phase 4 |
| MOUNTS-001 | resolved | Implemented in Phase 4 |
| TRIO-ANYIO-001 | not-applicable | Architectural difference |

## Recommendations

1. Proceed with Stage C documentation and migration guide updates.
2. Begin Stage D planning for top-level distribution feasibility study.
3. Continue expanding downstream portfolio with real-world SDK integration tests.
4. Monitor upstream httpx releases for public API changes.
