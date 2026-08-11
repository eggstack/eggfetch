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
  manifest.toml    # Machine-readable package inventory
  README.md        # This file
```

## How to Add a New Fixture

1. Pick a real PyPI package that uses httpx publicly.
2. Choose the most specific category it represents.
3. Add a `[[package]]` entry with all required fields.
4. Run the validation script:
   ```bash
   python scripts/run_downstream_compat.py --artifact-manifest /path/to/artifact-manifest.json
   ```
5. Run the meta-test to confirm structural validity:
   ```bash
   cd crates/eggfetch-python && maturin develop
   EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/test_downstream_portfolio.py -v
   ```

## Running Downstream Tests

### Validate manifest structure
```bash
python scripts/run_downstream_compat.py --artifact-manifest /path/to/artifact-manifest.json
```

### Run package-specific integration tests
```bash
cd crates/eggfetch-python && maturin develop
EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/test_downstream_portfolio.py -v --strict-markers
```

### Install all downstream packages for manual testing
```bash
pip install httpx==0.28.1 respx==0.21.1 pytest-httpx==0.30.0 starlette==0.37.2
pip install anthropic==0.39.0 groq==0.13.0 httpx-sse==0.4.0 httpcore==1.0.5
pip install anyio==4.8.0 httpx-auth==0.22.0 httpx-ws==0.7.0 pydantic==2.10.0
```

## Isolation Requirements

- **expected-network-isolation = true**: Package tests must run without
  any outbound network calls. Use mock transports or in-process ASGI.
- **expected-network-isolation = false**: Package tests may require
  network access (e.g., SDKs that need a real API endpoint). Provide
  mock endpoints or skip in CI.

## Update Process

1. Check for new versions of pinned packages monthly.
2. Build the eggfetch wheel and controlled HTTPX replacement, write their
   paths and SHA-256 values to an artifact manifest, and run
   `python scripts/run_downstream_compat.py --artifact-manifest ...`.

The release-blocking portfolio uses `--required-only`. Packages that import
private HTTPX modules or target an incompatible HTTPX generation remain
informational and are reported with their exact known incompatibility.
3. Run the full compat test suite to catch regressions.
4. Update `manifest.toml` with new versions and review notes.
5. Update `review-cadence` if a package changes its httpx usage.

## Categories

| Category | What It Tests |
|----------|--------------|
| contract-tests | Reference httpx API surface |
| mock-transport-user | MockTransport, Router, request interception |
| framework-test-client | Pytest fixtures, request matching |
| framework-asgi-transport | ASGITransport, in-process ASGI |
| sdk-async-client | AsyncClient, custom auth, streaming, timeouts |
| sdk-sync-client | Client, custom auth, timeout config |
| streaming-upload-download | Streaming responses, SSE, chunked transfer |
| custom-transport-subclass | BaseTransport/AsyncBaseTransport subclassing |
| async-testing-support | Async context managers, task groups |
| custom-auth-flow | Auth subclassing, OAuth, token refresh |
| event-hook-instrumentation | Transport extensions, connection lifecycle |
| heavy-config-user | base_url, params, headers, cookies, timeouts |
