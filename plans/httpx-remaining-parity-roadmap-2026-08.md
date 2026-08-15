# HTTPX Remaining-Parity Closure Roadmap

Status: handoff plan only; no feature implementation is included in this commit series.

Baseline reviewed: `main` at `a3a5dd1e8d3c0675317658c37f565d074b07c1dc` (2026-08-15).
Pinned compatibility reference: `httpx==0.28.1` with `httpcore==1.0.9`.

## Purpose

EggFetch is already Stage C qualified for its documented Python 3.10+ asyncio-compatible HTTPX 0.28.1 surface. This roadmap is a deliberately narrow follow-up for the remaining public HTTPX behavior that is either absent, only represented at the Python object layer, or explicitly retained as a bounded difference.

This is **not** authorization to reopen the completed redirect, cookie, auth, timeout, raw-stream, UDS, SOCKS, environment-proxy, or ordinary transport-parity work. Those areas remain closed unless a new differential test demonstrates a regression caused by this work.

The objective is to reduce the remaining gap between “Stage C qualified for the documented surface” and “practical drop-in HTTPX 0.28.1 replacement” without violating EggFetch’s architectural invariants.

## Upstream facts that define this work

The reference behavior is pinned to HTTPX 0.28.1 and httpcore 1.0.9, not HTTPX `master`.

HTTPX 0.28.1:

- accepts `verify=True`, `verify=False`, deprecated string paths, and arbitrary `ssl.SSLContext` instances;
- exports `create_ssl_context()` and passes the resulting context into the transport;
- exposes `Proxy(url, ssl_context=..., auth=..., headers=...)` and forwards proxy headers and proxy-specific TLS context to httpcore;
- accepts `http1=False, http2=True`, which httpcore treats as HTTP/2 prior knowledge, including cleartext HTTP/2;
- forwards request `extensions` to httpcore, including `timeout`, `trace`, `sni_hostname`, and `target`;
- returns response extensions including `http_version`, `reason_phrase`, HTTP/2 `stream_id`, and `network_stream`.

httpcore 1.0.9 additionally establishes the semantics that matter here:

- `target` replaces the request target while origin/URL routing remains otherwise unchanged;
- `sni_hostname` overrides the hostname used during TLS start without changing the TCP destination;
- `trace` callbacks observe connection, TLS, HTTP/1.1, HTTP/2, retry, and close events;
- HTTP/1.1 responses expose the underlying network stream, wrapping it with leading-data preservation for successful CONNECT and `101 Switching Protocols`;
- HTTP/2 responses expose both the shared underlying network stream and the per-request stream ID.

HTTPX `master` currently has an unreleased public `FunctionAuth` export. That is forward drift, not part of the 0.28.1 closure contract.

## EggFetch constraints that remain non-negotiable

1. All network I/O remains in `eggfetch-core`.
2. There is no parallel Python networking implementation.
3. The sync Python API continues to block on the same async Rust engine while releasing the GIL.
4. The default TLS backend remains rustls.
5. Workspace `unsafe_code = "forbid"` remains intact. Do not reach into CPython `_ssl` internals or extract OpenSSL `SSL_CTX*` pointers.
6. Do not add OpenSSL, native-tls, or a second TLS/network stack merely to claim parity without an explicit architecture decision from maintainers.
7. Existing Tier 1 CI remains simple. Do not add matrices, workflows, or release gates. New parity differentials belong in the existing test hierarchy.
8. Executable compatibility changes invalidate the current exact-SHA Stage C qualification until requalified.

## Research conclusion: not every remaining difference has the same implementation risk

### Straightforward and actionable

- HTTP/2-only / prior-knowledge mode. The core already has `HttpVersionPolicy::Http2Only` and connector construction already has an HTTP/2-only branch. The compatibility facade currently rejects the combination before reaching the core.
- `target` and `sni_hostname` request extensions. These can be represented as typed, narrowly-scoped request transport metadata in the core.
- HTTP/2 `stream_id`, provided the chosen Hyper seam exposes or can safely preserve the ID.
- Proxy-leg headers. The proxy subsystem already has distinct forwarding and CONNECT construction points; the missing piece is a bounded proxy-only header channel with strict leakage prevention.

### Moderate architecture work

- `trace`. Matching HTTPX means pinning to the httpcore 1.0.9 event vocabulary and translating Rust transport events into Python callbacks without holding the GIL across network waits.
- `network_stream` metadata (`client_addr`, `server_addr`, TLS details) requires the connector to retain safe shared connection metadata rather than only an opaque Hyper client.
- CONNECT/Upgrade `network_stream` read/write requires explicit connection handoff semantics. Hyper’s normal pooled client path currently consumes the response into a body stream and discards the upgrade handle.

### Hard boundary: arbitrary Python `ssl.SSLContext`

An arbitrary CPython `ssl.SSLContext` is an OpenSSL object. EggFetch’s transport consumes a rustls `ClientConfig`. Python’s public SSL API exposes enough information to recover some settings (CA certificates, verification mode, hostname checking, minimum/maximum TLS versions, cipher descriptions), but it does **not** expose the private key loaded by `SSLContext.load_cert_chain()` and does not provide a lossless export of all OpenSSL state. Therefore arbitrary-context fidelity cannot be honestly or safely implemented by “converting” every `SSLContext` into rustls.

The TLS phase must first implement a safe capability analysis and support the subset that can be represented exactly. If exact arbitrary-context behavior is still required after that, it requires a separate maintainer-level architecture decision. The plan explicitly forbids unsafe CPython/OpenSSL object introspection as a shortcut.

## Execution order

### Phase 01 — TLS context truth, `create_ssl_context`, and safe translation

Plan: `plans/httpx-parity-phase-01-tls-context-feasibility-and-safe-translation.md`

Goals:

- replace the current raising `create_ssl_context()` stub with the real HTTPX 0.28.1 construction contract at the Python API layer;
- introduce a fail-closed translation boundary for representable `ssl.SSLContext` state;
- support custom trust roots, verification/hostname policy, and TLS version bounds where lossless;
- explicitly identify and reject unrepresentable arbitrary-context state instead of silently weakening TLS or dropping client identity;
- reconcile the compatibility ledger so it describes actual behavior.

This phase is a prerequisite for proxy-specific TLS-context work.

### Phase 02 — HTTP/2-only and prior knowledge

Plan: `plans/httpx-parity-phase-02-h2-only-prior-knowledge.md`

Goals:

- remove the facade-level rejection of `http1=False, http2=True`;
- wire the existing core `Http2Only` policy from both compatibility clients and transports;
- prove both HTTPS H2-only and plaintext h2 prior-knowledge behavior against HTTPX.

This phase is intentionally small and should land independently.

### Phase 03 — Bounded request/response extensions

Plan: `plans/httpx-parity-phase-03-request-response-extensions.md`

Goals:

- implement `target`;
- implement `sni_hostname`;
- implement the pinned `trace` callback surface without Python-owned I/O;
- preserve response `http_version`/`reason_phrase` behavior and add `stream_id` where the core can determine it exactly;
- define typed native metadata rather than putting arbitrary Python dictionaries into `eggfetch-core`.

`network_stream` is intentionally excluded from this phase because its ownership/lifecycle problem is materially different.

### Phase 04 — `network_stream`, CONNECT, and Upgrade handoff

Plan: `plans/httpx-parity-phase-04-network-stream-upgrade-connect.md`

Goals:

- expose safe network-stream metadata for live responses;
- implement correct HTTP/1.1 CONNECT and `101` upgrade ownership transfer, including preservation of bytes already read after the response head;
- expose a sync and async compatibility object with HTTPX’s `read`, `write`, `close`, `start_tls`, and `get_extra_info` shape where the underlying connection can support it;
- explicitly define HTTP/2 shared-network-stream semantics and prevent unsafe concurrent raw I/O from corrupting a multiplexed connection.

This phase must pass a feasibility gate before implementation because the current Hyper legacy-client abstraction hides part of the connection ownership needed by HTTPX.

### Phase 05 — Proxy metadata and proxy TLS fidelity

Plan: `plans/httpx-parity-phase-05-proxy-metadata-and-proxy-tls.md`

Goals:

- forward `Proxy(headers=...)` only on the proxy leg for both forward-proxy requests and CONNECT setup;
- guarantee proxy metadata never reaches the origin;
- use Phase 01’s TLS translation boundary for representable proxy `ssl_context` values;
- retain explicit rejection for unrepresentable arbitrary proxy contexts rather than silently substituting origin TLS settings.

### Phase 06 — Contract cleanup, upstream drift decision, and exact-SHA qualification

Plan: `plans/httpx-parity-phase-06-final-rebaseline-functionauth-qualification.md`

Goals:

- remove resolved entries from `allowed-differences.toml` and move them to the resolved ledger;
- correct stale `create_ssl_context`, H2-only, proxy, and extension descriptions;
- decide which irreducible differences remain intentional;
- keep Trio/AnyIO and Python 3.8/3.9 outside the pinned 0.28.1 Stage C contract unless maintainers explicitly reopen those product decisions;
- record HTTPX `master`’s public `FunctionAuth` change as future-version drift rather than contaminating the 0.28.1 manifest;
- run the existing exact-SHA qualification process after all executable phases have landed.

## Explicit exclusions from this closure program

The following are not implementation targets for this roadmap:

- Trio or AnyIO backend support. EggFetch remains asyncio/Tokio-first.
- Python 3.8/3.9 support.
- HTTPX private modules.
- HTTPX CLI emulation; EggFetch has its own CLI.
- adding a second Python or C/OpenSSL networking path.
- broad redesign of retries, redirect behavior, cookies, auth, decompression, UDS, SOCKS, or environment proxies.
- HTTP/3 parity with HTTPX; HTTPX 0.28.1 has no HTTP/3 public transport.

The existing four-element socket-option form `(level, option, None, optlen)` remains a separate safe-Rust boundary. Do not introduce raw pointer semantics or `unsafe` solely for this compatibility edge. If a safe OS abstraction can represent it later, treat it as an independent micro-pass rather than coupling it to the phases above.

## Cross-phase verification policy

Each executable phase must add focused tests first and run the existing routine gate. The full 0.28.1 compatibility suite, API oracle, and required downstream runner are reserved for phase closure/requalification rather than being added to routine CI.

For every feature that exists in both HTTPX and EggFetch, differential tests must compare at least:

- constructor acceptance/rejection;
- sync and async behavior;
- network-visible request bytes or TLS behavior where relevant;
- exception class and failure timing;
- response metadata and lifecycle state;
- resource release/cancellation on the async path.

No plan is considered complete because a new EggFetch-only unit test passes. Closure requires pinned-reference differential evidence.

## Program-level acceptance criteria

This roadmap is complete only when all of the following are true:

1. `http1=False, http2=True` no longer differs from HTTPX 0.28.1 for the qualified direct transport cases.
2. `target` and `sni_hostname` extensions have direct differential coverage.
3. `trace` behavior is either implemented for the pinned event contract or remains explicitly documented with precise unsupported-event differences; it may not be silently ignored.
4. `stream_id` is exposed for HTTP/2 responses if Hyper provides a reliable source; otherwise the inability is documented with a tested, narrow difference.
5. CONNECT/Upgrade `network_stream` behavior has either landed with lifecycle-safe ownership or been rejected by a documented feasibility decision explaining why the current Hyper abstraction cannot support it without replacing the transport architecture.
6. `Proxy(headers=...)` is forwarded proxy-only or the exact residual blocker is documented; silent dropping remains forbidden.
7. `create_ssl_context()` no longer has contradictory ledger/docs behavior.
8. Passing `ssl.SSLContext` never silently discards trust, verification, hostname, version, or client-auth semantics. Unsupported state fails closed.
9. Active allowed differences contain no stale/resolved entries and no claim of behavior that the implementation does not provide.
10. The final executable SHA passes `./scripts/check.sh`, the pinned full compatibility suite, the API oracle with zero unexplained/stale differences, and the required downstream compatibility runner using the repository’s existing qualification procedure.

## Handoff rule

Implement these phases in order. Do not combine Phase 04 with unrelated transport refactors. If Phase 01 or Phase 04 hits an architectural impossibility under the stated invariants, stop that phase at the documented decision gate, record the evidence, and keep the difference explicit. A truthful bounded difference is preferable to a compatibility shim that weakens TLS, leaks proxy headers, corrupts a pooled connection, or introduces an unaudited second networking stack.
