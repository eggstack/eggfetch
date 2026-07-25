# Downstream Compatibility Portfolio

Purpose-built inventory of representative consumer packages that exercise
distinct httpx compatibility surfaces. Each entry is a pinned, real PyPI
release selected for what it tests—not for popularity.

## Why This Exists

Phase 5 validates that eggfetch works as a drop-in httpx replacement for
real-world consumers. This manifest defines the test matrix:

- Which packages to install alongside eggfetch
- Which API surfaces each package exercises
- Expected isolation (no network calls) or integration behavior
- Known incompatibilities to track

## Directory Layout

```
compat/downstream/
  manifest.toml           # Machine-readable package inventory
  README.md               # This file
  status.toml             # Current qualification status
  behavioral_fixtures/    # Package-specific behavioral test fixtures
    test_httpx_contract_behavior.py
    test_respx_behavior.py
    test_pytest_httpx_behavior.py
    test_starlette_behavior.py
    test_httpx_sse_behavior.py
    test_httpx_auth_behavior.py
    test_opentelemetry_behavior.py
    test_anthropic_behavior.py
```

## How to Add a New Fixture

1. Pick a real PyPI package that uses httpx publicly.
2. Choose the most specific category it represents.
3. Add a `[[package]]` entry with all required fields.
4. Run the validation script:
   ```bash
   python scripts/run_downstream_compat.py
   ```
5. Run the meta-test to confirm structural validity:
   ```bash
   cd crates/eggfetch-python && maturin develop
   EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/test_downstream_portfolio.py -v
   ```

## Running Downstream Tests

### Validate manifest structure
```bash
python scripts/run_downstream_compat.py
```

### Generate downstream matrix
```bash
python scripts/generate_downstream_matrix.py \
  --manifest compat/downstream/manifest.toml \
  --output /tmp/downstream-matrix.json
```

### Run package-specific integration tests
```bash
cd crates/eggfetch-python && maturin develop
EGGFETCH_COMPAT_REQUIRED=1 pytest compat/downstream/behavioral_fixtures/ -v --strict-markers
```

### Install all downstream packages for manual testing
```bash
pip install httpx==0.28.1 respx==0.23.1 pytest-httpx==0.36.2 starlette==0.37.2
pip install anthropic==0.39.0 httpx-sse==0.4.3 httpx-auth==0.23.1
pip install opentelemetry-instrumentation-httpx==0.65b0 opentelemetry-sdk==1.44.0
pip install httpcore==1.0.5 anyio==4.8.0 pydantic==2.10.0
```

## Isolation Requirements

- **expected-network-isolation = true**: Package tests must run without
  any outbound network calls. Use mock transports or in-process ASGI.
- **expected-network-isolation = false**: Package tests may require
  network access (e.g., SDKs that need a real API endpoint). Provide
  mock endpoints or skip in CI.

## Update Process

1. Check for new versions of pinned packages monthly.
2. Run `python scripts/run_downstream_compat.py` after updates.
3. Run the full compat test suite to catch regressions.
4. Update `manifest.toml` with new versions and review notes.
5. Update `review-cadence` if a package changes its httpx usage.

## Categories

| Category | What It Tests |
|----------|--------------|
| contract-tests | Reference httpx API surface |
| mock-transport-request-matching | MockTransport, Router, request interception |
| framework-test-client | Pytest fixtures, request matching |
| asgi-test-client | ASGITransport, in-process ASGI |
| sdk-async-client | AsyncClient, custom auth, streaming, timeouts |
| sdk-sync-client | Client, custom auth, timeout config |
| streaming-sse-consumption | Streaming responses, SSE, chunked transfer |
| custom-transport-subclass | BaseTransport/AsyncBaseTransport subclassing |
| async-testing-support | Async context managers, task groups |
| custom-auth-flow | Auth subclassing, OAuth, token refresh |
| event-hooks-instrumentation | Transport extensions, connection lifecycle |
| heavy-config-user | base_url, params, headers, cookies, timeouts |

## Release-Blocking Coverage

All 8 Stage C categories are covered by release-blocking packages:

| Category | Package | Version |
|----------|---------|---------|
| contract-tests | httpx | 0.28.1 |
| mock-transport-request-matching | respx | 0.23.1 |
| framework-test-client | pytest-httpx | 0.36.2 |
| asgi-test-client | starlette | 0.37.2 |
| sdk-async-client | anthropic | 0.39.0 |
| streaming-sse-consumption | httpx-sse | 0.4.3 |
| custom-auth-flow | httpx-auth | 0.23.1 |
| event-hooks-instrumentation | opentelemetry-instrumentation-httpx | 0.65b0 |
