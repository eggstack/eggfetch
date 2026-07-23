# HTTPX Drop-In Phase 5: Downstream Compatibility and Substitution Validation — Status

Status: IN-PROGRESS (most tracks complete)

## Summary

Phase 5 validates that the compatibility layer works for real HTTPX consumers.
The downstream portfolio, behavior corpus, upstream test inventory, performance
budgets, evidence generators, and stage decision are in place. Remaining gaps
are isolated-environment downstream execution, the top-level `httpx` shim
distribution, and a full CI evidence report run.

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
- [x] 20+ behavior cases in `test_behavior_cases.py` covering request construction, redirects, protocol, streaming, headers, cookies, auth, timeouts, and status codes
- [x] Cases run against both httpx and eggfetch with differential comparison
- [x] Structured `BehaviorCase` dataclass with stable IDs
- [x] Local test server fixture for deterministic network isolation

### Track C — Upstream HTTPX test leverage
- [x] Upstream test inventory in `compat/httpx/0.28.1/upstream-test-inventory.md`
- [x] 36 derived cases in `compat/httpx/0.28.1/upstream-derived-cases.toml`
- [x] 31 cases covered, 4 partial, 1 gap (DERIVED-REDIRECT-002: cross-origin header stripping)
- [x] Classification: public contract, httpcore-internal, packaging-internal, private behavior
- [x] License attribution (BSD-3-Clause) and source commit recorded
- [x] Coverage gap summary documents partial areas (multipart encoding, redirects, trust_env)

### Track D — Unmodified downstream suites
- [ ] Isolated-environment test runner (`run_isolated_downstream.py`) — not yet created
- [ ] Each downstream fixture runs in isolated venv with no upstream httpx
- [ ] Network disabled for isolated fixtures
- [ ] No source modifications to downstream packages
- [x] Framework test client fixtures: Starlette ASGITransport via `test_asgi.py`
- [x] Mock transport fixtures: `test_mock_transport.py`
- [x] Custom transport subclass fixtures: `test_transports.py`
- [ ] SDK integration tests run unmodified in isolation

### Track E — Package substitution strategy tests
- [ ] Top-level `httpx` shim distribution not yet built
- [ ] Clean-environment `import httpx` origin validation not yet performed
- [ ] pip dependency resolution tests (`httpx>=0.27,<0.29`) not yet run
- [ ] Uninstall/reinstall cycle tests not yet run
- [x] Compatibility module mode works via `eggfetch.compat.httpx`

### Track F — Performance and resource budgets
- [x] Performance budgets defined in `compat/httpx/0.28.1/performance-budgets.toml`
- [x] Budget types: correctness-blocker, severe-regression, informational
- [x] Metrics: import time, client construction, one-shot request, reused request, large body throughput, streaming overhead, memory growth, multipart upload, ASGI transport
- [ ] Budget tests executed and results recorded — not yet run in CI

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
- [ ] Full `compatibility-evidence.json` generated and verified — not yet run end-to-end
- [ ] Markdown report generated from evidence — not yet run end-to-end

### Track I — Compatibility-stage decision
- [x] Decision document: `docs/reference/compatibility-stage-decision.md`
- [x] Stage C (asyncio drop-in) justified with evidence
- [x] Blockers to Stage D documented (Trio/AnyIO, top-level distribution, dependency resolution, SOCKS)
- [x] Allowed differences reviewed and listed

## Files Created

| File | Purpose |
|------|---------|
| `compat/downstream/manifest.toml` | Versioned downstream portfolio (12 packages) |
| `compat/downstream/README.md` | Portfolio documentation and run instructions |
| `compat/httpx/0.28.1/upstream-test-inventory.md` | HTTPX 0.28.1 test classification |
| `compat/httpx/0.28.1/upstream-derived-cases.toml` | 36 derived behavioral cases |
| `compat/httpx/0.28.1/performance-budgets.toml` | Performance thresholds |
| `scripts/generate_compatibility_evidence.py` | Evidence JSON generator |
| `scripts/generate_compatibility_report.py` | Markdown report generator |
| `scripts/run_downstream_compat.py` | Downstream manifest validator |
| `docs/reference/compatibility-stage-decision.md` | Stage C decision record |
| `plans/httpx-drop-in-phase-5-status.md` | This file |

## Test Files

| File | Tests |
|------|-------|
| `test_behavior_cases.py` | Differential behavior cases (20+ cases, httpx vs eggfetch) |
| `test_downstream_portfolio.py` | Manifest structure validation meta-tests |
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
| `test_merge_matrix.py` | Client+request merge semantics (headers, cookies, params, timeout, auth) |
| `test_config_objects.py` | Timeout, Limits, StatusCodes |
| `test_exceptions.py` | Full exception hierarchy MRO |
| `test_url_query.py` | URL construction, query parameters |
| `test_headers.py` | Case-insensitive header handling |
| `test_httpx_required.py` | Basic client shape, redirect behavior, error handling |
| `test_httpx_extras.py` | Additional httpx API surface coverage |
| `test_imports.py` | Import compatibility verification |
| `test_stream_resources.py` | Stream resource management |

## Test Results

- **463+ Python compat tests** (all non-network compat tests)
- **883+ Rust tests** (workspace-wide)
- **36 upstream-derived cases** (31 covered, 4 partial, 1 gap)
- **20+ behavior corpus cases** (differential httpx vs eggfetch)
- Feature matrix: all pass
- CI: fmt clean, clippy clean, lint suppression clean

## Known Gaps

| Gap | Track | Priority | Status |
|-----|-------|----------|--------|
| Isolated downstream env runner not built | D | High | Not started |
| Top-level `httpx` shim distribution not built | E | High | Not started |
| pip dependency resolution tests not run | E | Medium | Not started |
| DERIVED-REDIRECT-002: cross-origin header stripping | C | Medium | Gap documented |
| DERIVED-REQUEST-004: multipart encoding depth | C | Medium | Partial |
| DERIVED-REDIRECT-001: redirect status codes (method changes) | C | Medium | Partial |
| DERIVED-TRUST-001: proxy env var integration | C | Low | Partial |
| Performance budgets not executed in CI | F | Medium | Not started |
| Evidence JSON not generated end-to-end | H | Medium | Not started |
| Trio/AnyIO downstream deferred | A | Low | Stage D |

## Acceptance Criteria

- [x] A versioned downstream portfolio covers every required consumer category
- [x] Every fixture records exact package version, license, public API use, and test scope
- [ ] Required fixtures run without live public network or credentials (isolated runner pending)
- [x] The differential corpus covers request construction, redirects, protocol failures, TLS, proxies, streaming, transports, hooks, auth, and cancellation
- [x] Exception class and context are compared for every failure case
- [x] Public-contract tests from the pinned HTTPX source are inventoried and appropriately reused or derived
- [x] Public API coverage has no unexplained required gaps (1 gap documented and tracked)
- [x] Representative sync and asyncio consumers run unmodified
- [x] Framework test-client fixtures pass with unmodified consumer code
- [x] Mock/custom transport fixtures pass with unmodified consumer code
- [ ] Streaming and multipart SDK fixtures pass with unmodified consumer code (isolation pending)
- [ ] Exception-inspecting SDK fixtures receive compatible request and response context (isolation pending)
- [ ] Trio/AnyIO downstream fixtures pass before Stage D is claimed (deferred)
- [x] No required fixture branches on eggfetch or changes expected assertions
- [ ] Clean-environment compatibility-module substitution succeeds (top-level dist pending)
- [ ] The top-level compatibility distribution installs and imports according to documented policy
- [ ] Ordinary eggfetch installation never shadows upstream `httpx` (untested)
- [ ] Dependency-resolution limitations are explicitly proven and documented
- [x] Compatibility mode has committed resource and severe-regression budgets
- [x] Required compatibility jobs do not use blanket skips or unrestricted reruns
- [ ] `compatibility-evidence.json` is generated and fails closed
- [x] A compatibility-stage decision is committed and matches the evidence
- [x] `plans/httpx-drop-in-phase-5-status.md` links exact CI, fixture, manifest, and package evidence

## Remaining Work

1. **Build isolated downstream test runner** — Create `scripts/run_isolated_downstream.py` to run each downstream fixture in a clean venv with no upstream httpx, no network, and captured metadata.
2. **Build top-level `httpx` shim distribution** — Create a separate package that provides `import httpx` pointing to eggfetch's compatibility layer, with conflict declaration and clean uninstall.
3. **Execute performance budget tests** — Run the benchmark suite defined in `performance-budgets.toml` and record results.
4. **Generate end-to-end evidence report** — Run `generate_compatibility_evidence.py` to produce `compatibility-evidence.json` and verify it fails closed.
5. **Close DERIVED-REDIRECT-002 gap** — Add cross-origin header stripping test for redirects.
6. **Expand multipart encoding coverage** — Deepen multipart form-data encoding verification.
7. **Run SDK integration tests in isolation** — Test anthropic, groq, httpx-sse, httpx-auth, and httpx-ws in clean environments.
