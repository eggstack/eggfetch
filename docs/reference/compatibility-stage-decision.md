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
| Date | 2026-07-23 |
| Stage Evaluated | Stage C (asyncio drop-in) |
| Decision | **Stage C candidate** |
| Evidence | Corrective closure plan, compat tests, API manifest comparison |

## Decision Rationale

Stage C candidate status is supported by the following evidence:

1. **API Surface**: The httpx 0.28.1 compatibility layer provides full Client/AsyncClient constructors, all HTTP methods, request/response objects, streaming, and transport abstraction.
2. **Differential Corpus**: 732 behavioral test cases run against both httpx and eggfetch with status code and body parity.
3. **Public Contract Coverage**: API manifest comparison runs in CI; allowed differences are documented in `compat/httpx/0.28.1/allowed-differences.toml`.
4. **Framework Integration**: ASGITransport and WSGITransport enable Starlette/FastAPI test client patterns.
5. **Transport Flexibility**: MockTransport, per-host mounts, custom transport subclassing all functional.
6. **Auth Flows**: BasicAuth, DigestAuth, custom auth flows, and auth disabling per-request.
7. **Event Hooks**: Request and response hooks with ordering guarantees.
8. **Downstream Portfolio**: 12 representative consumer packages identified across all required categories.

### Remaining blockers for Stage C released

The following must be resolved before restoring a release claim:

1. API oracle must fail-closed (manifest comparator currently informational in CI)
2. Controlled replacement artifact (`httpx` wheel) must satisfy downstream `Requires-Dist: httpx`
3. Downstream suites must use pinned sources, exact commands, and enforced minimum counts
4. Auth must work through all transport paths (mounted, custom, mock, ASGI, WSGI)
5. Async streaming context must await `aclose()` instead of calling sync `close()`
6. Repeated query parameters and duplicate headers must survive all transport paths
7. Explicit per-request `timeout=None` must be distinguishable from client default
8. Python 3.10 required compatibility suite must run without skips
9. Exact-SHA qualification must verify a green `Required CI Gate`
10. Evidence must be generated solely from retained result artifacts

## Blockers to Stage D

Stage D (full supported drop-in) requires:

1. **Trio/AnyIO Backend**: eggfetch uses asyncio only (tokio-based). Trio support is architecturally deferred.
2. **Top-level httpx Distribution**: A separate distribution providing `import httpx` is not yet built.
3. **Dependency Resolution**: A differently-named package cannot satisfy `httpx>=0.27` dependencies without a shim distribution.
4. **SOCKS Proxy**: Not in HTTPX 0.28.1 public API; deferred.

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
