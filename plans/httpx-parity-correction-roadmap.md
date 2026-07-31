# HTTPX 0.28.1 Behavioral Parity Correction Roadmap

Status: ready for implementation handoff

Date: 2026-07-31

## Purpose

This roadmap closes the remaining material behavioral differences between `eggfetch.compat.httpx` and the pinned HTTPX 0.28.1 public contract.

Eggfetch already has a broad HTTP feature set, a Rust-native async engine, sync and asyncio Python clients, streaming, transports, mounts, event hooks, authentication objects, cookies, proxies, TLS, HTTP/2, and an extensive compatibility suite. The remaining work is not a new feature expansion. It is a corrective pass over places where the compatibility facade accepts an HTTPX-shaped API but does not preserve HTTPX behavior.

The primary objective is:

> Existing code using the supported public HTTPX 0.28.1 API should either behave equivalently through `eggfetch.compat.httpx` or fail immediately with a precise, documented unsupported-feature error.

Silent argument loss, already-closed streaming responses, flattened cookie scope, incorrect response metadata, and hook/state-machine ordering differences are release-blocking for a drop-in claim even when ordinary buffered requests work.

## Audited baseline

Planning is anchored to:

- repository: `eggstack/eggfetch`
- branch: `main`
- audited baseline SHA: `b5114df51c52e65282a9610e07add7dfb912cbe2`
- eggfetch Python package version: `0.1.1`
- compatibility reference: `httpx==0.28.1`
- compatibility import: `eggfetch.compat.httpx`
- current compatibility status: `Stage C candidate`

HTTPX 0.28.1 remains the released compatibility target as of 2026-07-31. This roadmap does not authorize rebasing onto HTTPX 1.0 development releases or a moving upstream branch.

Before implementation begins, record the actual starting SHA. If `main` has advanced, review intervening commits and mark each finding below as still applicable, already resolved, or changed in shape. Do not mechanically implement a stale finding.

## Findings to close

The implementation plans treat the following as material parity defects or unsupported behavior that is currently too silent:

1. The top-level `stream()` helper returns only after its client and response contexts have exited, so the returned response is closed rather than being yielded by a context manager.
2. Top-level helpers do not route client-construction arguments such as `proxy`, `verify`, and `trust_env` according to HTTPX semantics.
3. `Client.stream()` and `AsyncClient.stream()` do not reliably forward per-call `auth`, `follow_redirects`, and `timeout` overrides.
4. Tuple auth, callable auth, URL credentials, and explicit auth disable behavior are not normalized consistently with HTTPX.
5. Closed clients can be recreated by internal lazy initialization paths instead of remaining permanently closed.
6. `http1=False` and unsupported transport options may be accepted without enforcing the requested behavior.
7. `Request` body construction rejects valid `data` plus `files` multipart combinations.
8. Direct `Request(params=...)` construction does not consistently embed parameters into the URL.
9. Low-level explicit `stream=` handling auto-populates headers where HTTPX intentionally does not.
10. Empty POST, PUT, and PATCH request header behavior differs from HTTPX.
11. Unread request and response content paths raise generic errors or return incomplete state instead of HTTPX stream exceptions.
12. Response HTTP version and reason phrase are stored in extensions but not surfaced correctly by compatibility properties.
13. `raise_for_status()` does not match HTTPX behavior for informational and redirect responses and does not require an attached request.
14. `Response.next_request`, elapsed timing, encoding state, and public stream-state semantics remain incomplete.
15. Event hooks run at the wrong point and frequency relative to auth and redirect hops.
16. Custom and mounted transport streaming can lose a buffered response body when it is rewrapped as a streaming response.
17. Mount routing lacks parts of HTTPX’s public matching behavior and cannot cleanly represent an explicit `None` no-proxy mount.
18. Transport constructor arguments such as UDS, local address, and socket options may be silently ignored.
19. The compatibility `Cookies` object flattens cookies by name and discards domain, path, secure, expiry, and duplicate-name semantics.
20. Redirect, auth, cookie, hook, and history behavior is split between the Python facade and the native engine in a way that prevents exact per-hop HTTPX behavior.
21. Public compatibility claims and allowed-difference records overstate parity in some of the above areas.

## Scope

This line of work includes:

- top-level HTTPX helper behavior;
- sync and async client lifecycle and per-request override routing;
- auth input normalization;
- HTTPX request body construction and stream-state semantics;
- HTTPX response metadata, errors, timing, redirect state, and encoding behavior;
- custom transport and mounted transport body preservation;
- mount matching and explicit unsupported transport options;
- per-hop redirect, auth, event-hook, cookie, history, and cleanup sequencing;
- scoped cookie representation and synchronization;
- focused differential tests against pinned HTTPX 0.28.1;
- compatibility documentation and allowed-difference correction.

## Non-goals

This roadmap must not expand into a second HTTPX program. It does not authorize:

- Trio support;
- general AnyIO backend selection;
- SOCKS proxy support;
- Unix-domain socket support;
- local-address binding support;
- socket-option support;
- Python 3.8 or 3.9 support;
- compatibility with HTTPX private modules;
- compatibility with HTTPX 1.0 development versions;
- new HTTP protocol features;
- changing Rust APIs merely to resemble HTTPX;
- moving network I/O into Python;
- adding a second Python networking stack;
- adding new CI jobs, matrices, evidence schemas, release workflows, or publication automation;
- expanding the existing downstream portfolio unless a currently retained consumer directly proves one of the corrected behaviors;
- adding broad soak, fuzz, or security programs unrelated to these parity defects.

Unsupported advanced transport options must fail clearly rather than being implemented as part of this pass.

## Architectural invariants

1. All real network I/O remains in `eggfetch-core`.
2. The compatibility facade may own HTTPX-specific orchestration, object semantics, and in-process transports.
3. A Python redirect/auth/cookie state machine may dispatch one network hop at a time through the Rust engine; that is orchestration, not a second network implementation.
4. There must be one authoritative compatibility-client cookie state, not a lossy Python dictionary layered over an unrelated native jar.
5. Sync and async code paths must implement equivalent semantics without calling async methods synchronously or vice versa.
6. A compatibility option that cannot be honored must raise at construction or call time. Silent no-ops are prohibited.
7. Existing test and CI infrastructure must be reused. Add focused tests, not another qualification framework.
8. The repository remains `Stage C candidate` until all roadmap exit criteria are met and the existing qualification path passes without waiving the corrected behavior.

## Execution sequence

### Phase 1 — Entrypoints, client configuration, auth normalization, and lifecycle

Correct the user-facing call paths before deeper object and state-machine work.

Primary outcomes:

- true top-level streaming context manager;
- correct separation of temporary-client arguments from request arguments;
- exact per-call override propagation through sync and async streaming;
- tuple, callable, object, URL-credential, and disabled-auth normalization;
- permanent closed-client state and idempotent cleanup;
- mutable client properties where HTTPX exposes them;
- correct default request headers visible to hooks and custom transports;
- `http1` and `http2` policy validation;
- immediate errors for unsupported constructor options.

Detailed plan:

`plans/httpx-parity-correction-phase-1-entrypoints-client-lifecycle.md`

Exit gate:

> Top-level functions, reusable clients, and streaming calls route all supported configuration correctly, never silently discard an HTTPX option, and preserve HTTPX lifecycle and auth-input behavior.

### Phase 2 — Request and response object semantics

Correct HTTPX’s public data-model and body behavior independent of multi-hop orchestration.

Primary outcomes:

- correct direct request URL/query construction;
- valid multipart `data` plus `files` behavior;
- correct body-source, empty-body, and low-level stream header semantics;
- `RequestNotRead` and `ResponseNotRead` behavior;
- response protocol metadata from extensions;
- HTTPX-compatible `raise_for_status()` behavior;
- `next_request`, elapsed time, encoding, repr, links, and stream-state semantics;
- lossless buffered and streaming body wrapping.

Detailed plan:

`plans/httpx-parity-correction-phase-2-request-response-semantics.md`

Exit gate:

> Constructed and received Request/Response objects behave like HTTPX 0.28.1 for supported public operations before redirects, auth challenges, or cookies add multi-hop state.

### Phase 3 — Transport, mount, hook, and one-hop dispatch boundaries

Create the minimal dispatch boundary needed for exact HTTPX orchestration without duplicating networking.

Primary outcomes:

- a single-hop native dispatch path with redirects disabled;
- body-preserving sync and async custom transport wrapping;
- correct mount priority, wildcard, port, path, and explicit `None` handling;
- request and response hooks at the HTTPX-defined per-hop locations;
- consistent extension propagation;
- deterministic transport ownership and close semantics;
- fail-fast handling for unsupported UDS, local-address, and socket-option settings.

Detailed plan:

`plans/httpx-parity-correction-phase-3-transport-mount-hook-dispatch.md`

Exit gate:

> Every network or in-process transport dispatch represents exactly one request/response hop, preserves the body and metadata, and exposes the point at which HTTPX hooks and state transitions must run.

### Phase 4 — Redirect, authentication, cookie, and history state machine

Use the one-hop boundary to align stateful behavior across redirects and auth challenges.

Primary outcomes:

- HTTPX-compatible redirect request construction and manual `next_request` behavior;
- correct method rewriting, body replay, header stripping, and response draining;
- complete sync and async auth flow drivers;
- request and response hooks for every hop;
- one authoritative scoped cookie jar;
- multiple `Set-Cookie` parsing and domain/path/secure/expiry selection;
- cookie extraction and insertion across auth and redirect hops;
- accurate history ownership and intermediate response cleanup.

Detailed plan:

`plans/httpx-parity-correction-phase-4-redirect-auth-cookie-state.md`

Exit gate:

> Redirects, auth challenges, cookies, event hooks, history, and cleanup produce the same externally visible state transitions as HTTPX 0.28.1 for the supported asyncio and sync profiles.

### Phase 5 — Focused differential closure and claim reconciliation

Prove the corrections with the smallest reliable validation footprint.

Primary outcomes:

- targeted sync and async differential cases for every roadmap finding;
- regression tests for previously silent argument loss and body loss;
- focused existing downstream checks where they exercise corrected public behavior;
- corrected allowed-difference records;
- accurate README, API guide, compatibility matrix, diagnostics, and stage status;
- no new CI architecture.

Detailed plan:

`plans/httpx-parity-correction-phase-5-differential-closure.md`

Exit gate:

> Every material parity finding has a passing reference/candidate test or a precise intentional-difference record, and documentation no longer claims unsupported behavior.

## Dependency graph

The default order is strict:

1. Phase 1 first, because all other tests depend on correct public call routing and lifecycle.
2. Phase 2 may overlap late Phase 1 work once constructor and client merge semantics are stable.
3. Phase 3 depends on the Phase 1 client surface and Phase 2 response/body representation.
4. Phase 4 depends on the one-hop dispatch boundary from Phase 3.
5. Phase 5 runs incrementally but cannot close until Phases 1 through 4 satisfy their exit gates.

The implementation may split commits within a phase, but it should not merge half of a state-machine correction that leaves sync and async behavior inconsistent.

## Verification policy

Use the repository’s existing paths:

```sh
./scripts/check.sh

EGGFETCH_COMPAT_REQUIRED=1 \
python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Add small focused test modules or cases under the existing compatibility suite. Prefer deterministic local transports and local HTTP fixtures. Do not add a new workflow, matrix, evidence contract, or general-purpose harness.

Each corrected behavior should normally have:

- a direct HTTPX 0.28.1 reference assertion;
- the same assertion against `eggfetch.compat.httpx`;
- sync coverage where HTTPX exposes sync behavior;
- asyncio coverage where HTTPX exposes async behavior;
- a negative case proving unsupported inputs fail rather than no-op;
- cleanup assertions when response, client, transport, or iterator ownership is involved.

## Completion definition

This line of work is complete only when all of the following hold:

- top-level `stream()` is a functioning context manager;
- temporary-client settings are routed correctly by top-level helpers;
- sync and async stream calls preserve all per-request overrides;
- all supported auth input forms are normalized correctly;
- closed clients cannot be reopened;
- unsupported options fail explicitly;
- direct Request and Response semantics match the pinned reference for the scoped findings;
- HTTP version, reason phrase, elapsed time, redirect state, and stream-state metadata are correct;
- custom and mounted transports do not lose response bodies;
- event hooks run once per actual request/response hop in correct order;
- redirects, auth flows, cookie state, history, and cleanup are aligned;
- scoped duplicate cookies are representable and selected correctly;
- focused differential tests pass with no skips or xfails for required cases;
- no new CI architecture was introduced;
- active allowed differences describe only genuine, bounded differences;
- user-facing compatibility claims match the proven surface;
- the existing compatibility and routine validation paths pass on the exact implementation SHA.

Until this completion definition is met, retain `Stage C candidate` and qualify “HTTPX drop-in” language with the supported-surface limitations.