# HTTPX Drop-In Compatibility and Production Readiness Roadmap

Status: ready for implementation handoff

## Purpose

This roadmap defines the work required to move eggfetch from a capable Rust-native HTTP client with an HTTPX-like Python surface to a production-grade, explicitly validated HTTPX drop-in replacement.

The current repository has broad HTTP functionality, a single Rust networking engine, sync and asyncio bindings, streaming responses, TLS, proxies, redirects, cookies, retries, HTTP/2, experimental HTTP/3, packaging automation, and substantial tests. Those capabilities are the foundation. They are not, by themselves, evidence that an existing HTTPX consumer can substitute eggfetch without semantic changes.

This roadmap therefore treats two outcomes as separate gates:

1. **Production-grade eggfetch client**: deterministic timeouts, bounded resources, correct shutdown, cancellation safety, fault tolerance, and release evidence.
2. **HTTPX drop-in compatibility**: matching public symbols, signatures, object semantics, configuration merging, exceptions, transports, streaming contracts, async backends, and integration behavior against a pinned HTTPX reference.

Neither gate may be inferred from feature-count parity. Both require executable evidence.

## Reference target

The initial compatibility target is **HTTPX 0.28.1**.

Implementation must pin the reference package and its direct compatibility fixtures in CI. Upstream HTTPX changes after the pinned target do not silently change acceptance criteria. Re-baselining requires:

- an explicit compatibility-profile update;
- a generated API and behavior delta report;
- review of changed defaults and exception contracts;
- updated differential fixtures;
- a compatibility-policy decision for every new or removed surface;
- a new immutable compatibility manifest committed to the repository.

The project must never describe itself as compatible with an unpinned moving HTTPX branch.

## Product-position stages

Compatibility claims must progress through the following stages. Documentation and package metadata must use the highest stage whose acceptance gate is currently satisfied.

### Stage A — Production-grade eggfetch

Eggfetch is safe for long-lived production clients written specifically against the eggfetch API.

Required characteristics:

- independently enforced pool, connect, read, and write deadlines;
- safe nonzero defaults or an explicitly versioned alternative policy;
- bounded connection and request concurrency defaults;
- deterministic close and interpreter-shutdown behavior;
- resource-leak and cancellation evidence;
- supported-platform release artifacts and immutable dry-run proof;
- no claim of HTTPX substitution compatibility.

### Stage B — HTTPX-compatible network-client subset

Ordinary network requests written against documented HTTPX client APIs work after changing the import, subject to a short, machine-readable list of unsupported extension surfaces.

Required characteristics:

- core public data model and exception compatibility;
- client/request/response semantics and signatures;
- client configuration merging;
- sync and asyncio behavior;
- mandatory differential tests with zero comparison skips;
- explicit unsupported-surface errors rather than silent behavior changes.

### Stage C — HTTPX-compatible asyncio drop-in

Existing asyncio applications and libraries using supported public HTTPX APIs run without source changes other than selecting the compatibility distribution or module.

Required characteristics:

- transport interfaces, mounts, event hooks, extensions, mock transport, and ASGI/WSGI test transports;
- complete request and response streaming contracts;
- custom auth flows;
- representative third-party compatibility suites;
- import and package-shim strategy validated in clean environments.

### Stage D — Full supported HTTPX drop-in

The supported public HTTPX 0.28.1 contract, including asyncio and Trio/AnyIO behavior, is implemented or every remaining deviation has been removed from the drop-in claim.

Required characteristics:

- supported async backend parity;
- no unexplained public API manifest deltas;
- no unexplained differential behavior deltas;
- unmodified representative consumers pass;
- compatibility release qualification and long-running canary evidence.

## Architectural invariants

The following remain non-negotiable:

- All real network I/O remains owned by `eggfetch-core`.
- The Rust engine remains async-first.
- Python adapters must not grow a second independent network implementation.
- Python-only in-process transports such as ASGI, WSGI, and mock transports may bypass the network engine because they do not perform network I/O.
- Rust APIs remain idiomatic and are not forced to mirror HTTPX's Python object model.
- HTTPX compatibility objects belong in the Python adapter or a dedicated compatibility layer unless the concept is independently useful to the Rust core.
- Compatibility behavior must be derived from tests and reference observation, not recollection or feature tables.
- Secure behavior may intentionally exceed HTTPX only when the difference is explicit, opt-in compatibility behavior is impossible or unsafe, and the claim is adjusted accordingly.
- Experimental HTTP/3 must not complicate or weaken the HTTPX 0.28.1 compatibility path, because HTTPX 0.28.1 does not expose HTTP/3.

## Scope

The line of work includes:

- compatibility contract and executable oracle;
- production timeout, resource-limit, lifecycle, and concurrency corrections;
- HTTPX public object model and exception hierarchy;
- `Client` and `AsyncClient` signature and merge semantics;
- request construction, `build_request()`, `send()`, and response metadata;
- true Python upload and download streaming;
- transport, mount, hook, auth, extension, mock, WSGI, and ASGI boundaries;
- environment proxy and certificate behavior;
- supported async backend strategy;
- real-world downstream package compatibility;
- compatibility packaging and import strategy;
- production and compatibility release qualification.

## Non-goals

This roadmap does not require:

- making Rust APIs look like Python HTTPX;
- replacing the Rust engine with httpcore or Python networking;
- matching undocumented HTTPX internals solely for libraries that import private modules;
- enabling HTTP/3 by default;
- adding browser behavior or browser-grade cookie policy;
- implementing application serving in eggfetch;
- preserving an eggfetch behavior that conflicts with a tested HTTPX contract merely because it shipped in an alpha release;
- claiming parity with future HTTPX versions without an explicit re-baseline.

Private-module compatibility may be considered only for a specifically approved downstream package and must not distort the public architecture.

## Workstream sequence

### Phase 0 — Compatibility contract and mandatory oracle

Create the versioned compatibility profile and make compatibility evidence fail closed.

Primary outcomes:

- pin HTTPX 0.28.1 and Requests where comparison is useful;
- correct known compatibility-document errors;
- make reference dependencies mandatory in compatibility CI;
- fail on skipped differential tests;
- generate public API, signature, inheritance, default, and attribute manifests;
- define allowed-difference records with owners and expiry/review policy;
- establish a local deterministic behavior server and malformed-peer fixtures;
- split native eggfetch tests from compatibility tests while requiring both.

Detailed plan: `plans/httpx-drop-in-phase-0-compatibility-contract.md`

Exit gate:

> The repository has a reproducible, pinned, fail-closed definition of what HTTPX compatibility means, and current gaps are measured rather than described informally.

### Phase 1 — Production semantics and lifecycle

Correct production blockers before expanding the compatibility facade.

Primary outcomes:

- independently enforced DNS/TCP/TLS/proxy connect deadlines;
- HTTPX-compatible default timeout profile for the compatibility layer;
- Python `Limits` surface and bounded defaults;
- deterministic client, pool, runtime, and socket shutdown;
- shared-client thread-safety contract;
- cancellation and timeout cleanup across every request phase;
- descriptor, thread, task, memory, and pool stabilization tests;
- explicit `trust_env` behavior and environment isolation tests.

Detailed plan: `plans/httpx-drop-in-phase-1-production-semantics.md`

Exit gate:

> Long-lived sync and async clients remain bounded, cancellable, and deterministic under normal use, failures, shutdown, and concurrency.

### Phase 2 — HTTPX object model and core API parity

Build the public compatibility model without duplicating network behavior.

Primary outcomes:

- `URL`, `QueryParams`, `Headers`, `Cookies`, `Request`, `Response`, `Timeout`, `Limits`, `Proxy`, status-code helpers, and stream base types;
- HTTPX-compatible exception hierarchy with attached request/response context;
- `Client` and `AsyncClient` constructor signatures and defaults;
- client-level `base_url`, params, headers, cookies, auth, timeout, limits, proxy, mounts, hooks, transport, environment, and encoding state;
- `build_request()`, `send()`, request methods, top-level helpers, and top-level stream behavior;
- deterministic client/request configuration merging;
- response request attachment, elapsed time, next request, extensions, encoding control, and chainable `raise_for_status()`.

Detailed plan: `plans/httpx-drop-in-phase-2-object-model-and-api.md`

Exit gate:

> The supported public HTTPX data model imports correctly, exposes compatible signatures, and passes construction and merge semantics before network behavior is considered.

### Phase 3 — Streaming and body architecture

Replace Python-side buffering and per-stream runtime/thread patterns with scalable, backpressured streaming.

Primary outcomes:

- request content from sync iterables, async iterables, file-like objects, and stream base classes;
- lazy multipart file and field encoding without whole-body buffering;
- raw and decoded response iteration;
- exact single-consumption, read, close, and context-manager state behavior;
- bounded channel bridges driven by shared runtimes;
- cancellation-safe producer and consumer shutdown;
- replayability classification integrated with redirects and retries;
- chunk-boundary, text-decoder, decompression, and backpressure differential tests.

Detailed plan: `plans/httpx-drop-in-phase-3-streaming-and-bodies.md`

Exit gate:

> Python uploads and downloads are genuinely streaming, bounded, cancellation-safe, and behaviorally compatible with the reference API.

### Phase 4 — Transports, extensions, auth, and async backends

Implement the extension surfaces required by real HTTPX consumers.

Primary outcomes:

- sync and async base transport protocols;
- Rust-backed HTTP transports;
- custom transport dispatch and mount routing;
- mock, WSGI, and ASGI transports;
- event hooks and request/response extension propagation;
- custom auth flow interfaces plus HTTPX built-ins required by the target profile;
- environment proxy, certificate, and netrc behavior;
- UDS, local-address, and transport-level options where present in the target profile;
- SOCKS support behind an optional dependency when required for parity;
- an explicit asyncio/Trio/AnyIO backend architecture and implementation path.

Detailed plan: `plans/httpx-drop-in-phase-4-transports-extensions-and-backends.md`

Exit gate:

> Public transport, hook, auth, environment, and supported async-backend contracts can be used by downstream libraries without special eggfetch branches.

### Phase 5 — Downstream compatibility and substitution validation

Prove compatibility against real consumers and a substantially complete behavior corpus.

Primary outcomes:

- generated API-manifest comparison with no unexplained differences;
- broad local differential corpus covering success, malformed peers, timeouts, redirects, proxies, TLS, HTTP/2, streaming, and cancellation;
- selected upstream HTTPX tests run against the compatibility layer where licensing and architecture permit;
- pinned representative downstream applications and libraries run unmodified;
- test-client, mocking, SDK, proxy, streaming, and custom-auth consumer categories;
- clean-environment package substitution tests;
- compatibility performance and resource budgets.

Detailed plan: `plans/httpx-drop-in-phase-5-downstream-validation.md`

Exit gate:

> Representative unmodified HTTPX consumers pass, and all remaining differences are explicit, narrow, reviewed, and reflected in the product claim.

### Phase 6 — Compatibility release qualification

Turn passing implementation work into defensible release evidence.

Primary outcomes:

- compatibility package/import strategy;
- immutable versioned compatibility manifest;
- full supported wheel matrix;
- package-content and symbol smoke tests;
- security, fuzzing, soak, fault-injection, and concurrency evidence;
- signed or attested artifacts where supported;
- canary deployments and rollback criteria;
- documentation and metadata aligned to the achieved compatibility stage;
- a fully green immutable release dry run.

Detailed plan: `plans/httpx-drop-in-phase-6-release-qualification.md`

Exit gate:

> A release candidate has immutable evidence for production safety and the exact compatibility stage claimed in package metadata and documentation.

## Dependency graph

The default execution order is strict:

1. Phase 0 must complete before compatibility implementation is judged.
2. Phase 1 must complete before a production-grade claim or broad downstream testing.
3. Phase 2 may begin after Phase 0 and may overlap late Phase 1 work that does not alter public semantics.
4. Phase 3 depends on the Phase 2 request, response, stream, and exception model.
5. Phase 4 depends on the Phase 2 object model and should use Phase 3 streaming primitives.
6. Phase 5 begins incrementally after Phase 2, but its exit gate depends on Phases 1 through 4.
7. Phase 6 depends on every acceptance gate required by the intended release stage.

No phase may declare success by weakening or deleting a reference test. Allowed differences require an explicit compatibility-policy record.

## Compatibility evidence model

The repository must maintain machine-readable evidence with at least these dimensions:

- reference package and version;
- eggfetch commit SHA;
- public symbol presence;
- callable signatures and defaults;
- class inheritance and exception relationships;
- object attributes and return types;
- behavior case identifier;
- sync, asyncio, and other supported backend result;
- platform and Python version;
- expected reference result;
- eggfetch result;
- allowed-difference identifier, if any;
- evidence timestamp and generator version.

A human-authored feature table is documentation, not the compatibility oracle.

## Allowed-difference policy

Every allowed difference must include:

- stable identifier;
- affected symbol or behavior case;
- reference behavior;
- eggfetch behavior;
- rationale;
- security and migration impact;
- affected compatibility stage;
- owner;
- review date or removal milestone;
- tests proving the difference is deliberate and bounded.

An allowed difference that prevents ordinary downstream substitution blocks the relevant drop-in stage.

## Package and import strategy

The normal `eggfetch` distribution remains the native product package.

During development, compatibility users should import an explicit module such as `eggfetch.compat.httpx` or use `import eggfetch as httpx` only where the manifest confirms the required surface.

A separate opt-in compatibility distribution that provides the top-level `httpx` module may be created only after Phase 5. Such a distribution must:

- conflict explicitly with upstream `httpx` rather than silently co-installing two providers;
- identify the emulated HTTPX version in metadata and at runtime;
- expose eggfetch's own implementation version separately;
- install and uninstall cleanly in isolated environments;
- not shadow upstream HTTPX accidentally in the normal `eggfetch` package;
- pass package-manager resolution and import-origin tests.

## Testing strata

The final validation stack must include:

1. Rust unit and integration tests for transport and state machinery.
2. Python native API tests for eggfetch-specific behavior.
3. Public-surface manifest tests against pinned HTTPX.
4. Differential behavior tests against pinned HTTPX.
5. Fault-injection tests with deterministic local peers.
6. Resource and cancellation stress tests.
7. Representative downstream package tests.
8. Built-wheel and source-distribution smoke tests.
9. Long-running canary and soak evidence.

A green native test suite cannot substitute for a skipped compatibility stratum.

## Security posture

Compatibility must not introduce silent security regression. Specific review is required for:

- environment proxy trust and credential leakage;
- netrc credential discovery;
- redirect auth stripping;
- custom CA and client-certificate loading;
- TLS verification disable paths;
- proxy CONNECT and proxy authentication;
- decompression and unbounded streaming;
- custom transports and in-process application transports;
- event-hook exception behavior;
- URL user-info and display redaction;
- request/response exception serialization and logging.

Where HTTPX behavior and project security policy conflict, the implementation must either provide an explicit compatibility mode or lower the compatibility claim.

## Completion definition

This line of work is complete only when:

- every detailed phase acceptance gate required for Stage D is satisfied;
- HTTPX 0.28.1 is installed and exercised in required compatibility CI;
- no required compatibility test is skipped;
- the public API manifest has no unexplained differences;
- the behavior corpus has no unexplained differences;
- production timeout, limit, lifecycle, and cancellation evidence is green;
- representative downstream consumers run unmodified;
- the compatibility distribution and ordinary eggfetch distribution coexist according to documented policy;
- the supported platform matrix passes from built artifacts;
- a full immutable release dry run is green;
- documentation, classifiers, versioning, and compatibility claims match the evidence exactly.

Until those conditions hold, documentation should use a narrower phrase such as "HTTPX-like API" or "HTTPX-compatible subset" rather than "drop-in replacement."
