# HTTPX 0.28.1 Parity Completion — Phase 5: SOCKS Proxy Support

Status: ready for implementation handoff

Date: 2026-08-07

Roadmap: `plans/httpx-parity-completion-roadmap.md`

Prerequisites:

- Phase 1 contract rebaseline complete;
- Phase 3 public proxy/transport signatures stable;
- Phase 4 advanced connector work complete or its relevant connector seam explicitly available.

Pinned reference: `httpx==0.28.1`

Compatibility designation: `Stage C candidate`

## Objective

Implement the optional SOCKS proxy capability exposed by HTTPX 0.28.1 while preserving EggFetch's existing HTTP proxy, CONNECT tunnel, TLS, timeout, cancellation, pooling, and environment-proxy behavior.

This phase is intentionally limited to the SOCKS behavior required by the pinned HTTPX public surface. It is not a general proxy-framework expansion.

## Corrected premise

HTTPX 0.28.1 publicly supports SOCKS proxies through its optional SOCKS dependency surface. Earlier EggFetch documentation incorrectly described SOCKS as outside HTTPX's public API. Phase 1 must correct that wording before this implementation is declared complete.

At the audited baseline EggFetch supports HTTP proxying and HTTPS CONNECT tunneling but does not support SOCKS.

## Core principles

### 1. One proxy selection pipeline

SOCKS must enter the existing proxy resolution/selection path rather than creating an independent request path.

The sequence should remain conceptually:

request → proxy selection / NO_PROXY → connector selection → proxy handshake if needed → origin TLS if needed → HTTP protocol → response.

### 2. SOCKS is opt-in

Ordinary direct HTTP/HTTPS and existing HTTP proxy connections must not pay a meaningful runtime or dependency cost when SOCKS is unused.

A Rust feature gate is preferred if it fits the current feature model. For the prebuilt Python wheel, the implementation may compile the capability in if optional wheel-time extras cannot meaningfully alter compiled Rust features; document that packaging choice explicitly instead of adding a Python proxy dependency.

### 3. Implement only the pinned reference surface

Do not add SOCKS4, UDP ASSOCIATE, proxy chaining, Tor-specific controls, PAC, generic tunnel plugins, or a broad proxy DSL unless direct HTTPX 0.28.1 public behavior requires them.

### 4. Credentials remain secret

Proxy URLs, configuration Debug/Display, errors, tracing, and pool keys must not expose SOCKS usernames/passwords.

## Likely implementation areas

Rust core:

- `crates/eggfetch-core/src/proxy.rs` or current proxy configuration/parser;
- direct/proxy connector implementation under `transport/`;
- pool/connection origin keying;
- proxy timeout/error mapping;
- feature definitions/dependencies in `crates/eggfetch-core/Cargo.toml` if required.

Python facade/binding:

- proxy URL parsing/conversion in `eggfetch.compat.httpx`;
- native binding plumbing only as required;
- environment proxy selection if `ALL_PROXY` or SOCKS env behavior is missing.

Tests:

- local in-process SOCKS server fixture;
- HTTP and HTTPS origin fixtures;
- direct HTTPX 0.28.1 differential cases.

## Scope firewall

### In scope

- SOCKS scheme(s) actually accepted by HTTPX 0.28.1;
- SOCKS CONNECT behavior required for ordinary HTTP/HTTPS requests;
- reference-supported username/password authentication;
- reference-supported destination address forms and DNS-resolution semantics;
- integration with `NO_PROXY`, `trust_env`, and standard proxy environment variables assigned by Phase 1;
- connect/proxy timeout behavior;
- cancellation and pool-lease release;
- HTTPS origin TLS over an established SOCKS tunnel;
- reference-compatible proxy error mapping;
- connection/pool isolation by proxy identity/configuration;
- redaction of credentials;
- minimal feature/dependency wiring.

### Out of scope

- SOCKS4/4a unless the pinned public reference accepts and documents them;
- UDP ASSOCIATE;
- BIND;
- proxy chains;
- SSH proxies;
- PAC/WPAD;
- Tor control protocol;
- DNS-over-HTTPS as a SOCKS feature;
- generic proxy plugins;
- Trio/AnyIO;
- release/CI expansion.

## Track 0 — Pin exact HTTPX SOCKS behavior

Do not begin implementation from generic SOCKS knowledge alone. First produce a small differential reference matrix using `httpx==0.28.1`.

### 0.1 Accepted proxy URL schemes

Determine exactly which public schemes HTTPX 0.28.1 accepts in normal client/transport use.

Test likely candidates only to establish reference behavior, not to assume support:

- `socks5://`;
- `socks5h://` if accepted;
- any additional SOCKS schemes shown by the pinned reference.

Record unsupported-scheme errors as well.

### 0.2 Authentication behavior

Determine:

- no-auth handshake;
- username/password URL syntax;
- empty username/password behavior;
- percent-encoded credentials;
- invalid/overlong credential behavior;
- proxy rejection behavior.

### 0.3 DNS-resolution semantics

This is a critical parity point. Determine whether each accepted scheme sends:

- a domain name to the SOCKS server for remote resolution; or
- a locally resolved IP address.

Do not infer this from the scheme spelling. Capture the actual address type observed by the local reference SOCKS fixture.

### 0.4 Error families

Pin the public exception class for:

- connection refused to proxy;
- handshake/auth failure;
- SOCKS reply failure;
- malformed proxy response;
- origin unreachable through proxy;
- timeout during proxy connection/handshake.

Exact message strings need not be cloned unless existing compatibility policy treats them as stable, but error class and request attachment must match.

### Track 0 acceptance criteria

- Accepted schemes are reference-derived.
- DNS behavior is observed, not assumed.
- Authentication and failure classes are pinned.
- The implementation target is finite before code changes.

## Track 1 — Decide implementation/dependency strategy

### 1.1 Compare a focused crate vs contained native handshake

Before adding a dependency, compare:

A. implementing the required SOCKS5 CONNECT/auth handshake directly using existing Tokio I/O primitives;

B. adding a small, maintained, safe Rust SOCKS connector crate with minimal features.

Evaluate:

- transitive dependency count;
- maintenance/security posture;
- ability to enforce existing timeout/cancellation behavior;
- ability to expose DNS mode required by the reference;
- credential redaction/control;
- binary/wheel size impact;
- feature-gating quality.

### 1.2 Prefer the smaller controlled solution

Because the required surface should be narrow, a small direct handshake may be preferable if it can be implemented clearly and safely without protocol creep. Conversely, do not hand-roll protocol parsing if a tiny established safe crate materially reduces correctness/security risk at negligible dependency cost.

Document the choice in implementation notes/PR.

### 1.3 Feature model

If a new Rust feature is added, prefer:

```text
socks = [focused dependencies only]
```

Do not force SOCKS dependencies into `eggfetch-core` default features unless the packaging model requires it and the size/dependency impact is reviewed.

For Python wheels, determine whether the project's existing build enables the feature unconditionally for compatibility. Do not create a Python runtime dependency on `python-socks`/httpcore merely to avoid Rust implementation.

### Track 1 acceptance criteria

- Implementation strategy is explicitly justified.
- No unsafe code is required.
- Dependency/feature impact is bounded and documented.
- No second Python networking stack is introduced.

## Track 2 — Proxy parsing and configuration

### 2.1 Extend the existing proxy model

The proxy configuration must distinguish at least:

- HTTP proxy;
- SOCKS proxy variant(s) required by the reference.

Preserve the current HTTP proxy code path unchanged when an HTTP/HTTPS proxy URL is provided.

### 2.2 Parse credentials safely

Normalize username/password according to HTTPX-observable behavior and URI percent-decoding rules.

Store secrets in a type/path that does not reveal raw values through Debug/Display.

### 2.3 Preserve `NO_PROXY`

SOCKS proxies must be bypassed under the same public environment/client selection rules as the reference.

Do not force SOCKS traffic through the proxy after `NO_PROXY` matches.

### 2.4 Audit `ALL_PROXY` and lowercase environment variables

Phase 1 must identify whether current EggFetch env behavior fully matches HTTPX. This phase owns missing proxy-environment behavior needed for practical proxy parity.

Differentially test:

- `HTTP_PROXY`;
- `HTTPS_PROXY`;
- `ALL_PROXY`;
- `NO_PROXY`;
- lowercase equivalents if HTTPX honors them;
- precedence when more than one variable applies;
- `trust_env=False`.

Do not change env precedence based on assumptions.

### Track 2 acceptance criteria

- HTTP vs SOCKS variants are explicit.
- Credentials are redacted.
- `NO_PROXY` works with SOCKS.
- standard env proxy semantics assigned by Phase 1 match the pinned reference.
- HTTP proxy regressions remain absent.

## Track 3 — SOCKS connection handshake

Implement only the command/auth/address modes required by Track 0.

### 3.1 Connect to the proxy under existing timeout policy

TCP connection to the SOCKS proxy must be governed by the same connect/proxy-connect timeout ownership as the existing proxy path.

Cancellation during proxy connect/handshake must unwind and release the pool/connection permit.

### 3.2 Method negotiation

Support the authentication methods required by reference behavior, expected to include:

- no authentication;
- username/password if supplied.

Reject unsupported method selection deterministically.

### 3.3 Username/password authentication

If reference-supported, implement the required username/password subnegotiation exactly enough to:

- enforce protocol field-length limits;
- send no credentials when no auth is configured;
- map auth rejection to the correct facade error;
- never log credential bytes.

### 3.4 CONNECT request

Send SOCKS CONNECT for the intended origin endpoint using the correct address type based on reference DNS semantics:

- IPv4;
- IPv6 where applicable;
- domain name where applicable.

### 3.5 Parse reply defensively

Validate:

- version;
- reserved byte;
- reply code;
- address type;
- variable-length bound-address fields;
- EOF/truncation/malformed lengths.

Map reply failures into the existing proxy/connection error taxonomy. Do not panic on malformed upstream data.

### 3.6 Hand the established stream back to the existing HTTP/TLS path

After a successful SOCKS CONNECT, the resulting stream is simply a tunnel to the origin.

- `http://` origin: speak HTTP over the tunnel;
- `https://` origin: perform the existing origin TLS handshake over the tunnel, then HTTP.

Do not terminate origin TLS at the SOCKS proxy layer.

### Track 3 acceptance criteria

- Local fixture observes a valid SOCKS handshake and CONNECT.
- All required address forms are handled.
- Auth works/rejects correctly where required.
- Malformed responses cannot panic or corrupt proxy state.
- HTTPS performs origin TLS through the tunnel.

## Track 4 — Pool identity and connection ownership

### 4.1 Keep proxy routes distinct

A connection established through one SOCKS proxy must not be reused for:

- direct origin traffic;
- an HTTP proxy route;
- a different SOCKS proxy;
- a different credential set when authentication identity affects route safety;
- an incompatible DNS mode/scheme if the accepted schemes differ semantically.

### 4.2 Keep secrets out of keys/logs

If auth identity must participate in a pool key, use a non-displayable/internal identity representation. Do not put cleartext passwords into Debug/Display/error messages.

### 4.3 Reuse safely within one route

When HTTP semantics permit, multiple requests to the same compatible origin/proxy route should reuse the established tunneled connection according to the existing pool policy.

Do not disable pooling globally just to simplify SOCKS.

### Track 4 acceptance criteria

- Route isolation is proven by tests.
- Safe same-route reuse works.
- Credentials do not appear in observable key/debug output.
- Existing direct/HTTP-proxy pooling remains unchanged.

## Track 5 — End-to-end local SOCKS test fixture

Create a bounded deterministic local fixture; do not depend on an external Tor/SOCKS service.

The fixture should be capable of:

- parsing the greeting;
- selecting no-auth or username/password;
- recording the CONNECT destination/address type;
- optionally rejecting auth;
- returning selected SOCKS reply failures;
- proxying bytes to a local HTTP origin;
- proxying bytes to a local TLS origin for HTTPS tests.

This fixture is test infrastructure only. It does not need to implement a production SOCKS server.

Required differential scenarios:

1. HTTP through SOCKS;
2. HTTPS through SOCKS;
3. no-auth;
4. reference-supported auth;
5. DNS/address-type semantics;
6. IPv4 literal;
7. IPv6 literal where environment supports it;
8. proxy connect refusal;
9. auth rejection;
10. SOCKS reply failure;
11. malformed/truncated handshake;
12. connect/handshake timeout;
13. cancellation followed by a constrained follow-up request;
14. `NO_PROXY` bypass;
15. `trust_env=False`;
16. `ALL_PROXY`/env precedence if assigned by Phase 1.

### Track 5 acceptance criteria

- Tests prove actual traffic traverses the SOCKS fixture.
- The fixture records destination address type for DNS parity.
- HTTPS proves TLS remains origin-facing.
- failure/cancellation paths are deterministic.

## Track 6 — Python facade and error mapping

### 6.1 Accept reference-compatible proxy values

The compatibility `Proxy`/Client/transport path must accept the pinned reference's SOCKS URL forms.

Do not broaden accepted schemes beyond Track 0 merely because the native parser could.

### 6.2 Map errors to HTTPX-compatible classes

Use direct reference tests to select the facade exception family for each failure.

Ensure `.request` is attached where HTTPX attaches it.

### 6.3 Preserve HTTP proxy behavior

Run the existing HTTP proxy and HTTPS CONNECT compatibility tests. SOCKS parsing must not alter current HTTP proxy semantics or credentials.

### Track 6 acceptance criteria

- Valid HTTPX SOCKS source works unchanged under the facade.
- public failure classes match the reference corpus.
- existing HTTP proxy tests remain passing.

## Track 7 — Ledger/documentation closure

After implementation:

- regenerate the candidate API manifest;
- run the API comparator;
- remove Phase 5 resolved records from the active allowlist;
- preserve them in `resolved-differences.toml` as appropriate;
- update README/reference matrix from "SOCKS unsupported" to the exact proved capability;
- document any feature/build requirement truthfully;
- update env proxy documentation if `ALL_PROXY`/lowercase behavior was closed;
- do not remove Trio/AnyIO or Python-version limitations.

## Required validation

Mandatory:

- focused Rust SOCKS/proxy tests;
- local SOCKS end-to-end Python tests;
- direct HTTPX 0.28.1 differential tests;
- existing HTTP proxy/CONNECT/NO_PROXY/TLS tests;
- cancellation/pool tests;
- `./scripts/check.sh`;
- API oracle.

Run the full pinned compatibility suite at the Phase 5 boundary if practical. Phase 6 will run final qualification authoritatively.

No new CI job, external SOCKS service, Docker service dependency, or scheduled network integration test is required.

## Phase acceptance criteria

Phase 5 is complete only when:

- every SOCKS scheme claimed as supported is accepted by the pinned HTTPX reference;
- local fixture proves traffic actually traverses SOCKS;
- reference DNS/address-type semantics match;
- reference-supported username/password authentication works;
- HTTPS over SOCKS completes origin TLS correctly;
- `NO_PROXY`, `trust_env`, and assigned environment proxy semantics match;
- connect/handshake timeouts and cancellation release resources/leases;
- pool identity prevents unsafe route reuse while preserving safe reuse;
- malformed/rejecting proxies cannot panic the client;
- credentials are redacted;
- existing direct and HTTP proxy behavior remains passing;
- `./scripts/check.sh` passes;
- API oracle is clean after ledger updates;
- no SOCKS4/UDP/chaining/general proxy framework, Trio/AnyIO, CI, or release scope is added.

## Rejection criteria

Reject the implementation if:

- SOCKS is implemented with a Python/httpcore side path;
- accepted schemes/DNS semantics are guessed rather than pinned;
- HTTP proxy code is duplicated wholesale for SOCKS;
- username/password values can appear in logs/errors/Debug;
- HTTPS skips origin TLS verification;
- SOCKS routes can collide with direct/other-proxy pool keys;
- timeout/cancellation does not cover the handshake;
- malformed SOCKS replies can panic or cause unbounded allocation;
- an external Tor/SOCKS server is required for routine tests;
- a broad proxy framework is introduced;
- docs claim unsupported SOCKS variants.

## Stop conditions

Stop and report a bounded blocker if:

- the reference requires a SOCKS mode that cannot be integrated into the existing stream connector without replacing the proxy subsystem;
- the only viable dependency has unacceptable unsafe/transitive/security impact;
- exact DNS-mode parity conflicts with an existing security invariant and cannot be configured safely;
- origin TLS cannot be layered over the established SOCKS stream using the current native transport seam.

The blocker report must include the exact reference case, missing primitive, affected acceptance criteria, and smallest follow-up proposal. Do not silently downgrade it into an intentional difference.

## Suggested commit decomposition

1. `test: pin HTTPX SOCKS proxy behavior`
2. `feat: add bounded SOCKS proxy connector`
3. `feat: integrate SOCKS auth DNS and TLS routing`
4. `test: prove SOCKS timeout cancellation and pool isolation`
5. `docs: resolve HTTPX SOCKS compatibility gap`

## Handoff checklist

Report:

- starting SHA;
- final Phase 5 executable SHA;
- supported reference SOCKS schemes;
- DNS semantics per scheme;
- auth modes supported;
- dependency/feature decision and size/dependency impact;
- local HTTP-over-SOCKS result;
- local HTTPS-over-SOCKS result;
- failure/malformed/timeout/cancellation results;
- `NO_PROXY`/`ALL_PROXY`/`trust_env` results;
- pool-isolation proof;
- credential-redaction proof;
- `./scripts/check.sh` result;
- full pinned suite result if run;
- API oracle before/after count;
- exact retained SOCKS limitations;
- confirmation that direct/HTTP proxy behavior and CI/release architecture remain unchanged.
