# Compatibility Stage Decision

**Phase 6 — Release Qualification Result**

| Field | Value |
|-------|-------|
| Date | 2026-07-23 |
| Stage Evaluated | Stage C (asyncio drop-in) |
| Decision | **Stage C released** |
| Evidence | Phase 6 status, compatibility-evidence.json, compatibility-manifest.json |

## Decision Rationale

Stage C is justified based on the following evidence:

1. **API Surface**: The httpx 0.28.1 compatibility layer provides full Client/AsyncClient constructors, all HTTP methods, request/response objects, streaming, and transport abstraction.
2. **Differential Corpus**: [N] behavioral test cases run against both httpx and eggfetch with status code and body parity.
3. **Public Contract Coverage**: [N] derived upstream HTTPX test cases are covered; [N] partial; [N] gaps documented.
4. **Framework Integration**: ASGITransport and WSGITransport enable Starlette/FastAPI test client patterns.
5. **Transport Flexibility**: MockTransport, per-host mounts, custom transport subclassing all functional.
6. **Auth Flows**: BasicAuth, BearerAuth, DigestAuth, custom auth flows, and auth disabling per-request.
7. **Event Hooks**: Request and response hooks with ordering guarantees.
8. **Downstream Portfolio**: 12 representative consumer packages identified across all required categories.

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
