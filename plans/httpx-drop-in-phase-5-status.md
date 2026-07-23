# HTTPX Drop-In Phase 5: Downstream Compatibility and Substitution Validation — Status

Status: COMPLETE

## Summary

Phase 5 validates that the compatibility layer works for real HTTPX consumers.
All 9 tracks are complete. The downstream portfolio, behavior corpus, upstream
test inventory, performance budgets, evidence generators, stage decision,
isolated downstream execution, shim distribution, and pip resolution tests are
in place. The exit criterion — evidence that representative downstream users
can run without eggfetch-specific branches — has been met.

## Deliverables

### Track A — Downstream compatibility portfolio
- [x] Versioned downstream portfolio in `compat/downstream/manifest.toml` (12 packages)
- [x] Schema-version 1 with all required fields per entry
- [x] Consumer categories covered: contract-tests, mock-transport-user, framework-test-client, framework-asgi-transport, sdk-async-client, sdk-sync-client, streaming-upload-download, custom-transport-subclass, async-testing-support, custom-auth-flow, event-hook-instrumentation, heavy-config-user
- [x] Public vs private HTTP usage classified per package
- [x] Known incompatibilities recorded (respx, pytest-httpx, anthropic, httpx-sse, httpcore, httpx-ws)
- [x] Update owner and review cadence assigned
- [x] Meta-test validates manifest structure (`test_downstream_portfolio.py`)
- [x] README documents isolation requirements and update process

### Track B — Expanded behavior corpus
- [x] 29+ behavior cases in `test_behavior_cases.py` covering request construction, redirects, protocol, streaming, headers, cookies, auth, timeouts, and status codes
- [x] Cases run against both httpx and eggfetch with differential comparison
- [x] Structured `BehaviorCase` dataclass with stable IDs
- [x] Local test server fixture for deterministic network isolation

### Track C — Upstream HTTPX test leverage
- [x] Upstream test inventory in `compat/httpx/0.28.1/upstream-test-inventory.md`
- [x] 36 derived cases in `compat/httpx/0.28.1/upstream-derived-cases.toml`
- [x] 32 cases covered, 4 partial, 0 gaps (DERIVED-REDIRECT-002 closed)
- [x] Classification: public contract, httpcore-internal, packaging-internal, private behavior
- [x] License attribution (BSD-3-Clause) and source commit recorded
- [x] Coverage gap summary documents partial areas (multipart encoding, trust_env)

### Track D — Unmodified downstream suites
- [x] Isolated-environment test runner (`run_isolated_downstream.py`) validated
- [x] All 12 downstream fixtures run in isolated venvs with no upstream httpx
- [x] Network disabled for isolated fixtures (proxy env vars cleared)
- [x] No source modifications to downstream packages
- [x] Framework test client fixtures: Starlette ASGITransport via `test_asgi.py`
- [x] Mock transport fixtures: `test_mock_transport.py`
- [x] Custom transport subclass fixtures: `test_transports.py`
- [x] SDK integration: anthropic, groq, httpx-sse, httpx-auth, httpx-ws import validated in clean venvs

### Track E — Package substitution strategy tests
- [x] Top-level `httpx` shim distribution built (`compat/httpx-shim/`)
- [x] Clean-environment `import httpx` origin validation passed
- [x] pip dependency resolution tests (`httpx>=0.27,<0.29`) passed
- [x] Uninstall/reinstall cycle tests passed (force-reinstall workflow documented)
- [x] Compatibility module mode works via `eggfetch.compat.httpx`
- [x] eggfetch does not shadow upstream httpx when installed alongside

### Track F — Performance and resource budgets
- [x] Performance budgets defined in `compat/httpx/0.28.1/performance-budgets.toml`
- [x] Budget types: correctness-blocker, severe-regression, informational
- [x] All 9 budget metrics executed and passing
- [x] Results recorded in `performance-budget-results.json`

### Track G — Failure triage and allowed differences
- [x] 9 allowed differences in `compat/httpx/0.28.1/allowed-differences.toml`
- [x] 3 resolved (EVENT-HOOKS-001, TRANSPORTS-001, MOUNTS-001)
- [x] 4 intentional (REDIRECT-DEFAULT-001, TIMEOUT-TUPLE-001, EXCEPTION-NAMES-001, RAISE-FOR-STATUS-001)
- [x] 1 resolved (PROXY-ENV-001)
- [x] 1 not-applicable (TRIO-ANYIO-001)
- [x] No blanket skips — individual cases marked with machine-readable policy
- [x] Flake policy: investigations required, no unrestricted retries

### Track H — Evidence report
- [x] Evidence generator: `scripts/generate_compatibility_evidence.py`
- [x] Report generator: `scripts/generate_compatibility_report.py`
- [x] Downstream compat runner: `scripts/run_downstream_compat.py`
- [x] Compat profile validator: `scripts/validate_httpx_compat_profile.py`
- [x] Lint suppression checker: `scripts/check_compatibility_claims.py`
- [x] Full `compatibility-evidence.json` generated end-to-end (overall_pass=True)
- [x] Markdown report generated from evidence

### Track I — Compatibility-stage decision
- [x] Decision document: `docs/reference/compatibility-stage-decision.md`
- [x] Stage C (asyncio drop-in) justified with evidence
- [x] Blockers to Stage D documented (Trio/AnyIO, SOCKS)
- [x] Allowed differences reviewed and listed

## Files Created

| File | Purpose |
|------|---------|
| `compat/downstream/manifest.toml` | Versioned downstream portfolio (12 packages) |
| `compat/downstream/README.md` | Portfolio documentation and run instructions |
| `compat/httpx/0.28.1/upstream-test-inventory.md` | HTTPX 0.28.1 test classification |
| `compat/httpx/0.28.1/upstream-derived-cases.toml` | 36 derived behavioral cases |
| `compat/httpx/0.28.1/performance-budgets.toml` | Performance thresholds |
| `compat/httpx-shim/` | Top-level httpx shim distribution |
| `scripts/generate_compatibility_evidence.py` | Evidence JSON generator |
| `scripts/generate_compatibility_report.py` | Markdown report generator |
| `scripts/run_downstream_compat.py` | Downstream manifest validator |
| `scripts/run_isolated_downstream.py` | Isolated venv downstream test runner |
| `scripts/run_performance_budgets.py` | Performance budget execution |
| `scripts/test_pip_resolution.py` | pip resolution and uninstall/reinstall tests |
| `docs/reference/compatibility-stage-decision.md` | Stage C decision record |
| `plans/httpx-drop-in-phase-5-status.md` | This file |

## Test Files

| File | Tests |
|------|-------|
| `test_behavior_cases.py` | Differential behavior cases (29+ cases, httpx vs eggfetch) |
| `test_downstream_portfolio.py` | Manifest structure validation meta-tests |
| `test_httpx_required.py` | Required compatibility: client, redirect, error, timeout, auth, cross-origin header stripping |
| `test_transports.py` | Transport protocol, HTTPTransport, custom transports |
| `test_mock_transport.py` | MockTransport sync/async, mismatch detection |
| `test_asgi.py` | ASGITransport scope, channels, streaming, disconnect |
| `test_wsgi.py` | WSGITransport environ and streaming |
| `test_auth.py` | BasicAuth, DigestAuth, NetRCAuth, custom auth |
| `test_hooks.py` | Hook ordering, error cleanup, response mutation |
| `test_mounts.py` | Component-based mount routing and priority |
| `test_extensions.py` | Extension passthrough across all paths |
| `test_backends.py` | Async context manager, closed client, type validation |
| `test_environment.py` | trust_env, proxy env vars, base URL resolution |
| `test_client.py` | Client/AsyncClient constructors, send, base_url |
| `test_request.py` | Request construction, body types, auto-headers |
| `test_request_streaming.py` | Stream construction, transfer-encoding |
| `test_response.py` | Response construction, status predicates, iterators |
| `test_response_streaming.py` | Sync/async streaming, iter_raw, aiter_raw |
| `test_cookies.py` | Cookie jar API, domain matching, mutation |
| `test_merge_matrix.py` | Client+request merge semantics |
| `test_config_objects.py` | Timeout, Limits, StatusCodes |
| `test_exceptions.py` | Full exception hierarchy MRO |
| `test_url_query.py` | URL construction, query parameters |
| `test_headers.py` | Case-insensitive header handling |
| `test_httpx_extras.py` | Additional httpx API surface coverage |
| `test_imports.py` | Import compatibility verification |
| `test_stream_resources.py` | Stream resource management |

## Test Results

- **729 Python compat tests** (all pass)
- **880+ Rust tests** (3 pre-existing compression failures unrelated to Phase 5)
- **36 upstream-derived cases** (32 covered, 4 partial, 0 gaps)
- **29+ behavior corpus cases** (differential httpx vs eggfetch)
- **12 downstream packages** validated in isolated venvs (11 passed, 1 skipped-no-tests)
- **5 pip resolution tests** (all pass)
- **9 performance budget metrics** (all pass)
- Feature matrix: all pass
- CI: fmt clean, clippy clean, lint suppression clean

## Bug Fix

Fixed a cookie re-addition bug in `crates/eggfetch-core/src/pipeline.rs:365-378`:
cross-origin redirects were stripping the Cookie header but then immediately
re-adding it from the cookie jar due to a condition inversion (`!cookie_header_allowed`
instead of `cookie_header_allowed`).

## Remaining (deferred to Stage D)

| Item | Track | Priority | Status |
|------|-------|----------|--------|
| Trio/AnyIO downstream fixtures | A | Low | Deferred to Stage D |
| SOCKS proxy support | E | Low | Deferred to Stage D |
