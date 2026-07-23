> **WARNING**: This report was generated from an older commit and is invalidated. See the corrective evidence for the current candidate.

# Compatibility Evidence Report

## Summary

| Field | Value |
|-------|-------|
| Status | **PASS** |
| Compatibility Stage | phase-5 |
| eggfetch Version | 0.1.0 |
| Reference HTTPX Version | 0.28.1 |
| eggfetch Commit | `2418da6bb440` |
| Generated At | 2026-07-23T14:20:05.594981+00:00 |

## API Coverage

| Metric | Count |
|--------|-------|
| Total Symbols | 69 |
| class | 26 |
| exception | 28 |
| constant | 4 |
| function | 11 |

## Differential Test Results

| Metric | Value |
|--------|-------|
| Declared Test Count | 659 |
| pytest Total | 728 |
| Passed | 728 |
| Failed | 0 |
| Errors | 0 |
| Pass Rate | 100.0% |

## Downstream Portfolio

| Package | Expected | Installed | Available |
|---------|----------|-----------|-----------|
| anthropic | {'name': 'anthropic', 'version': '0.39.0', 'license': 'MIT', 'category': 'sdk-async-client', 'rationale': 'Major AI SDK using httpx.AsyncClient with custom auth, base_url, timeouts, streaming, and exception inspection.', 'usage': 'public', 'test-subset': 'integration', 'expected-network-isolation': False, 'optional-dependencies': ['httpx', 'pydantic'], 'known-incompatibilities': ['anthropic accesses httpx.Response.stream internally for SSE parsing'], 'update-owner': 'eggfetch-team', 'review-cadence': 'quarterly'} | - | No |
| anyio | {'name': 'anyio', 'version': '4.8.0', 'license': 'MIT', 'category': 'async-testing-support', 'rationale': 'Async compatibility library used by httpx for async backend selection. Exercises async context manager and task group integration.', 'usage': 'private', 'test-subset': 'unit', 'expected-network-isolation': True, 'optional-dependencies': [], 'known-incompatibilities': [], 'update-owner': 'eggfetch-team', 'review-cadence': 'quarterly'} | unknown | Yes |
| groq | {'name': 'groq', 'version': '0.13.0', 'license': 'Apache-2.0', 'category': 'sdk-sync-client', 'rationale': 'Groq Python SDK using httpx.Client for synchronous API calls with custom auth and timeout configuration.', 'usage': 'public', 'test-subset': 'integration', 'expected-network-isolation': False, 'optional-dependencies': ['httpx', 'pydantic'], 'known-incompatibilities': [], 'update-owner': 'eggfetch-team', 'review-cadence': 'quarterly'} | - | No |
| httpcore | {'name': 'httpcore', 'version': '1.0.5', 'license': 'BSD-3-Clause', 'category': 'custom-transport-subclass', 'rationale': 'Underlying transport layer for httpx. Provides ConnectionPool, HTTP/1.1 and HTTP/2 transports. Exercises transport API contract.', 'usage': 'public', 'test-subset': 'unit', 'expected-network-isolation': True, 'optional-dependencies': [], 'known-incompatibilities': ["httpcore is httpx's direct dependency; eggfetch replaces both"], 'update-owner': 'eggfetch-team', 'review-cadence': 'every-phase'} | 1.0.9 | Yes |
| httpx | {'name': 'httpx', 'version': '0.28.1', 'license': 'BSD-3-Clause', 'category': 'contract-tests', 'rationale': 'Reference implementation of the httpx API surface. Used for contract tests and API manifest comparison.', 'usage': 'public', 'test-subset': 'spec', 'expected-network-isolation': True, 'optional-dependencies': [], 'known-incompatibilities': [], 'update-owner': 'eggfetch-team', 'review-cadence': 'every-phase'} | 0.28.1 | Yes |
| httpx-auth | {'name': 'httpx-auth', 'version': '0.22.0', 'license': 'BSD-3-Clause', 'category': 'custom-auth-flow', 'rationale': 'Authentication library implementing OAuth, Digest, and API key flows on top of httpx.Auth. Exercises custom auth subclassing.', 'usage': 'public', 'test-subset': 'unit', 'expected-network-isolation': True, 'optional-dependencies': ['httpx'], 'known-incompatibilities': ['httpx-auth subclasses httpx.Auth and accesses auth flow hooks'], 'update-owner': 'eggfetch-team', 'review-cadence': 'quarterly'} | - | No |
| httpx-sse | {'name': 'httpx-sse', 'version': '0.4.0', 'license': 'BSD-3-Clause', 'category': 'streaming-upload-download', 'rationale': 'Server-Sent Events extension for httpx. Exercises streaming response consumption and transport subclassing.', 'usage': 'public', 'test-subset': 'unit', 'expected-network-isolation': True, 'optional-dependencies': ['httpx'], 'known-incompatibilities': ['httpx-sse accesses httpx.Response.aiter_lines and iter_lines internals'], 'update-owner': 'eggfetch-team', 'review-cadence': 'every-phase'} | - | No |
| httpx-ws | {'name': 'httpx-ws', 'version': '0.7.0', 'license': 'BSD-3-Clause', 'category': 'event-hook-instrumentation', 'rationale': 'WebSocket extension for httpx. Exercises connection upgrade handling and transport extension patterns.', 'usage': 'public', 'test-subset': 'unit', 'expected-network-isolation': True, 'optional-dependencies': ['httpx', 'anyio'], 'known-incompatibilities': ['httpx-ws accesses httpx._transport and httpx._config internals'], 'update-owner': 'eggfetch-team', 'review-cadence': 'every-phase'} | - | No |
| pydantic | {'name': 'pydantic', 'version': '2.10.0', 'license': 'MIT', 'category': 'heavy-config-user', 'rationale': 'Data validation library whose HTTP integration examples use httpx with base_url, params, headers, cookies, and timeouts extensively.', 'usage': 'private', 'test-subset': 'integration', 'expected-network-isolation': True, 'optional-dependencies': [], 'known-incompatibilities': [], 'update-owner': 'eggfetch-team', 'review-cadence': 'quarterly'} | 2.13.4 | Yes |
| pytest-httpx | {'name': 'pytest-httpx', 'version': '0.30.0', 'license': 'BSD-3-Clause', 'category': 'framework-test-client', 'rationale': 'Pytest fixture plugin that intercepts httpx requests via MockTransport. Exercises fixture injection and request matching.', 'usage': 'public', 'test-subset': 'unit', 'expected-network-isolation': True, 'optional-dependencies': ['pytest', 'httpx'], 'known-incompatibilities': ['pytest-httpx relies on httpx._transports.default for interception'], 'update-owner': 'eggfetch-team', 'review-cadence': 'every-phase'} | - | No |
| respx | {'name': 'respx', 'version': '0.21.1', 'license': 'BSD-3-Clause', 'category': 'mock-transport-user', 'rationale': 'Widely-used mock transport library that patches httpx transports. Exercises MockTransport and Router APIs.', 'usage': 'public', 'test-subset': 'unit', 'expected-network-isolation': True, 'optional-dependencies': ['httpx'], 'known-incompatibilities': ['respx patches httpx._transports.default which is private API'], 'update-owner': 'eggfetch-team', 'review-cadence': 'every-phase'} | - | No |
| starlette | {'name': 'starlette', 'version': '0.37.2', 'license': 'BSD-3-Clause', 'category': 'framework-asgi-transport', 'rationale': 'ASGI framework whose TestClient uses httpx.ASGITransport. Exercises in-process transport and ASGI app integration.', 'usage': 'public', 'test-subset': 'integration', 'expected-network-isolation': True, 'optional-dependencies': ['httpx', 'anyio'], 'known-incompatibilities': [], 'update-owner': 'eggfetch-team', 'review-cadence': 'every-phase'} | - | No |

## Allowed Differences

| ID | Category | Symbol | Stage Impact | Security |
|----|----------|--------|--------------|----------|
| REDIRECT-DEFAULT-001 | intentional-difference | httpx.Client(follow_redirects=...) | phase-0 | Yes |
| PROXY-ENV-001 | required-now | httpx.Client(proxy=...) | phase-1 | No |
| TIMEOUT-TUPLE-001 | intentional-difference | httpx.Timeout(...) | phase-0 | No |
| EVENT-HOOKS-001 | resolved | httpx.EventHooks / httpx.Client(event_hooks=...) | phase-0 | No |
| TRANSPORTS-001 | resolved | httpx.ASGITransport / httpx.WSGITransport | phase-0 | No |
| MOUNTS-001 | resolved | httpx.Client(mounts=...) | phase-0 | No |
| TRIO-ANYIO-001 | not-applicable | httpx.AsyncClient(transport=...) | phase-0 | No |
| EXCEPTION-NAMES-001 | intentional-difference | httpx.HTTPError / eggfetch.EggfetchError | phase-0 | No |
| RAISE-FOR-STATUS-001 | intentional-difference | httpx.Response.raise_for_status() | phase-0 | No |

## Platform Details

| Field | Value |
|-------|-------|
| Platform | Linux-6.8.0-134-generic-x86_64-with-glibc2.39 |
| Python | 3.12.3 |
| Python Implementation | CPython |
| Architecture | x86_64 |
