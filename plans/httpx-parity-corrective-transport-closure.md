# HTTPX 0.28.1 Parity — Corrective Transport Closure

Status: ready for implementation handoff

Date: 2026-08-11

Roadmap: `plans/httpx-parity-completion-roadmap.md`

Related phases:

- `plans/httpx-parity-completion-phase-4-advanced-direct-transport.md`
- `plans/httpx-parity-completion-phase-5-socks-proxy.md`
- `plans/httpx-parity-completion-phase-6-differential-closure.md`

Current `main` at planning time: `a3f565dd9942e3c1e6474bc9200edc8a76f2819a`

Last executable qualification candidate recorded by the repository: `40beeec09f3e88db8901f39388da665c47ab84f6`

Pinned reference: `httpx==0.28.1`

Compatibility designation during this corrective pass: **Stage C candidate — qualification pending corrective transport closure**

## Objective

Close the remaining transport-level incompatibilities discovered after Phases 4–6 and produce one defensible exact-SHA qualification for the documented HTTPX 0.28.1 Python >=3.10 asyncio-supported surface.

This is a corrective closure pass, not a new parity roadmap. It must preserve the architecture and scope already established by the HTTPX parity program while correcting specific places where the implementation or qualification evidence overclaimed parity.

The pass is complete only when:

1. HTTPX-compatible `local_address` values reach the native connector without exposing Rust `SocketAddr` syntax to Python callers;
2. supported `socket_options` use platform-correct semantics rather than hard-coded Linux-like numeric constants;
3. UDS connections re-enter the normal HTTP/TLS machinery instead of using a bespoke buffered HTTP parser;
4. SOCKS tunnels re-enter the normal HTTP/TLS machinery instead of using HTTP-proxy framing after the SOCKS handshake;
5. SOCKS protocol edge cases, cancellation, reuse, HTTPS, IPv6 where available, and environment selection are proven locally;
6. environment proxy selection is request-scheme-aware and integrates `NO_PROXY` correctly;
7. stale compatibility/qualification claims are corrected;
8. routine validation, the pinned compatibility suite, API oracle, and downstream qualification all close on one exact executable SHA.

## Why this corrective pass exists

The Phase 2 and Phase 3 work landed correctly and should remain closed. The remaining defects are concentrated in the Phase 4/5 transport additions and Phase 6 qualification record.

The current implementation has several specific problems:

- the HTTPX reference accepts host-only `local_address` values such as `"127.0.0.1"`, while the EggFetch compatibility layer currently requires `"host:port"`;
- Python/Rust socket-option handling assumes fixed numeric values for options that differ across operating systems;
- UDS bypasses Hyper/the normal HTTP engine, buffers the request, reads the response to EOF, and rejects HTTPS;
- SOCKS performs a valid-looking SOCKS5 handshake but then uses bespoke proxy HTTP framing rather than treating the established tunnel like a direct origin connection;
- SOCKS local DNS failure currently has a loopback fallback path that must never be used for unresolved remote names;
- SOCKS authentication state is not driven by the authentication method actually selected by the proxy;
- environment proxy handling collapses `HTTP_PROXY`, `HTTPS_PROXY`, and `ALL_PROXY` into one client-level proxy instead of selecting by request scheme;
- environment `NO_PROXY` behavior is not integrated through the same selection path;
- documentation contains contradictory statements about environment proxy support and overstates UDS/SOCKS qualification;
- the recorded Phase 6 qualification includes two flaky pinned-suite tests and unresolved downstream SSE failures rather than one clean closure run.

None of these findings justify reopening the already-closed Python object/signature work.

## Non-negotiable closure rules

### 1. Do not obtain closure by reclassification

A transport behavior identified by this plan as required for the documented Stage C surface cannot be moved to `intentional` or `deferred` solely because it is inconvenient to implement.

A genuine blocker must use the stop-condition process at the end of this plan.

### 2. One HTTP protocol implementation

UDS and SOCKS must not own independent HTTP request/response parsers.

Their responsibility is connection establishment:

```text
UDS path -> connected Unix stream -> optional TLS -> normal HTTP machinery
SOCKS proxy -> SOCKS handshake -> connected tunnel stream -> optional TLS -> normal HTTP machinery
```

Do not extend the current raw HTTP/1.1 parsing paths to add chunked encoding, keep-alive, trailers, streaming, or TLS one feature at a time. Re-enter the existing HTTP engine instead.

### 3. Default direct path must remain unchanged

The normal TCP path with no UDS, `local_address`, socket options, or proxy must continue using the existing standard connector/pool path.

Do not replace all direct networking with the corrective connector merely to simplify implementation.

### 4. Safe Rust only

Do not add unsafe blocks or direct libc/FFI socket manipulation.

If platform-correct socket options require a small safe dependency or a semantic adapter at the Python/native boundary, document the choice and dependency impact before adding it.

### 5. No CI/release expansion

Do not add new GitHub Actions jobs, release automation, external proxy services, Docker services, or scheduled integration infrastructure.

Use local deterministic fixtures and the repository's existing validation commands.

## Scope firewall

### In scope

- `local_address` public semantics and binding conversion;
- supported `socket_options` representation and platform-safe application;
- UDS connector architecture, HTTP framing, streaming, keep-alive, TLS, timeout/cancellation, and pool identity;
- SOCKS5/5h handshake correctness, HTTP/TLS integration, DNS behavior, auth negotiation, timeout/cancellation, framing, and reuse;
- proxy environment selection for `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, lowercase forms, `NO_PROXY`, and `trust_env`;
- targeted compatibility and local integration tests;
- compatibility ledger/profile/docs corrections;
- final exact-SHA qualification.

### Out of scope

- Trio/AnyIO;
- Python 3.8/3.9;
- HTTPX versions other than 0.28.1;
- SOCKS4/4a unless the pinned reference unexpectedly requires it for the claimed public surface;
- SOCKS UDP ASSOCIATE/BIND;
- proxy chaining, PAC/WPAD, SSH proxies, Tor control APIs;
- generalized arbitrary connector/plugin frameworks;
- private HTTPX module parity;
- CI/release redesign;
- unrelated performance refactors.

---

# Track 0 — Reopen qualification and freeze the corrective baseline

## 0.1 Record the starting SHA

At implementation start, record current `main`.

The planning-time main SHA is:

`a3f565dd9942e3c1e6474bc9200edc8a76f2819a`

If `main` has advanced, inspect the delta before implementation and state whether any executable transport behavior changed.

## 0.2 Treat previous Phase 6 evidence as historical only

The repository currently records `40beeec09f3e88db8901f39388da665c47ab84f6` as qualified.

For this corrective pass, treat that SHA as the **previous qualification candidate**, not as authoritative closure evidence.

Do not delete its historical test counts. Instead update the current compatibility status to indicate that transport corrective qualification is pending.

## 0.3 Preserve the pinned reference

All differential work in this pass must use exactly:

```text
httpx==0.28.1
```

Do not silently move the target to a newer HTTPX release.

## Track 0 acceptance criteria

- starting implementation SHA recorded;
- executable changes since `a3f565d...` understood;
- `40beeec...` retained as historical evidence only;
- active status no longer implies final qualification before this plan closes;
- pinned reference remains HTTPX 0.28.1.

---

# Track 1 — Correct `local_address` semantics

## 1.1 Pin reference behavior first

Extend the existing HTTPX reference-pinning tests to cover at minimum:

- `local_address="127.0.0.1"`;
- `local_address="0.0.0.0"` if accepted;
- IPv6 host-only form where the platform/reference supports it;
- `local_address=None`;
- malformed hostname/address values;
- behavior when the requested local address is unavailable.

The test must run against `httpx==0.28.1` and record constructor/use-time behavior.

## 1.2 Stop exposing Rust `SocketAddr` syntax to Python

The HTTPX compatibility facade must accept the reference-compatible host/address form.

For a host-only local address, the native connector should bind that address with port `0` so the operating system chooses an ephemeral source port.

Do not require callers to write `"127.0.0.1:0"` merely because the current Rust binding parses `SocketAddr`.

An EggFetch-native API may retain an explicit `SocketAddr` type if useful, but the HTTPX compatibility facade must translate from the HTTPX contract before reaching it.

## 1.3 Preserve family correctness

Cover:

- IPv4 local bind -> IPv4 destination;
- IPv6 local bind -> IPv6 destination where supported;
- family mismatch;
- unavailable local address;
- DNS destination resolving to multiple families.

If a local bind constrains the usable address family, destination iteration must respect that constraint instead of blindly selecting the first resolved address.

## 1.4 Remove blocking resolver behavior from the advanced connector

The advanced connector currently uses synchronous `ToSocketAddrs` in an async future and takes the first result.

Replace this with a non-blocking/asynchronous resolution path consistent with the rest of the Tokio runtime, and attempt compatible resolved addresses deterministically rather than using only `.next()`.

Do not introduce a new general DNS subsystem.

## 1.5 End-to-end proof

Use a local listener that records the peer address.

Required proof:

- a compatibility-layer call using host-only `local_address` succeeds;
- the server observes the expected source IP;
- the source port is OS-selected;
- invalid/unavailable binds map to the expected public transport error family;
- the default connector path is unchanged when `local_address=None`.

## Track 1 acceptance criteria

- `HTTPTransport(local_address="127.0.0.1")` and async equivalent behave like HTTPX 0.28.1;
- Python callers are not required to provide a port;
- binding occurs before connect;
- async connector no longer performs blocking `ToSocketAddrs` resolution;
- multiple compatible destination addresses can be attempted;
- end-to-end peer observation proves the local bind;
- existing ordinary direct tests remain passing.

---

# Track 2 — Make `socket_options` platform-correct and bounded

## 2.1 Pin the supported HTTPX representation

Retain and extend the current reference tests for:

- one `(level, option, value)` triple;
- multiple triples;
- integer values;
- bytes values;
- invalid tuple lengths;
- invalid level/option/value types;
- OS-level option failure behavior where deterministic.

Do not assume Linux numeric values are portable.

## 2.2 Remove hard-coded cross-platform socket constants

The current Python compatibility code and Rust connector contain numeric assumptions such as fixed `SOL_SOCKET`, `SO_KEEPALIVE`, `SO_RCVBUF`, and `SO_SNDBUF` values.

Those assumptions must not remain in cross-platform compatibility code.

Choose the smallest safe strategy that preserves HTTPX-observable behavior for the supported option set. Preferred approaches, in order:

1. classify known options at the Python boundary using the current runtime's `socket` module constants, then pass a semantic native option representation;
2. use an existing safe crate/API that exposes the required typed socket setters without unsafe code;
3. use platform-gated safe mappings only if the first two options are materially worse, with tests on each supported platform.

Do not write a fake universal table of POSIX integers.

## 2.3 Keep a bounded supported option set truthful

The plan does not require a generic unsafe `setsockopt` escape hatch.

For the Stage C facade, support the common options that can be applied safely and deterministically, including at minimum the options already claimed by the repository:

- `TCP_NODELAY`;
- `SO_KEEPALIVE`;
- `SO_RCVBUF`;
- `SO_SNDBUF`.

If HTTPX accepts arbitrary OS options that EggFetch cannot express safely without unsafe code, retain that as an explicitly bounded platform/option limitation rather than pretending the tuple was applied.

Unsupported tuples must fail deterministically and must never be silently ignored.

## 2.4 Apply before connect

Socket options that affect a newly-created connection must be set on the socket before `connect()`.

Do not mutate an already-pooled stream to simulate constructor parity.

## 2.5 End-to-end and platform proof

Required tests:

- supplied Python runtime constants classify correctly;
- supported option reaches the native socket;
- unrecognized option fails deterministically;
- malformed/short values fail deterministically;
- Linux test uses actual Python/Linux constants rather than copied literals;
- macOS test/CI-compatible unit path uses actual Python/macOS constants rather than Linux values;
- default path with no options is unchanged.

## Track 2 acceptance criteria

- no cross-platform code assumes Linux socket constant numbers;
- known supported options work using the host platform's real constants;
- values are applied before connect;
- unsupported options are explicit errors, not no-ops;
- no unsafe code added;
- default connector remains unchanged when options are absent.

---

# Track 3 — Re-integrate UDS with the normal HTTP/TLS engine

## 3.1 Remove UDS-owned HTTP parsing

The current UDS transport manually:

- buffers the request body;
- formats HTTP/1.1 text;
- writes it to `UnixStream`;
- reads the socket to EOF;
- splits headers/body manually.

This path must no longer be the production UDS implementation.

Replace it with a UDS connector/stream adapter that hands the connected Unix stream into the same HTTP protocol machinery used for ordinary connections, or the narrowest equivalent Hyper connection primitive already present in the dependency graph.

Do not build a second HTTP parser.

## 3.2 Preserve HTTP authority semantics

The Unix socket path selects the transport endpoint only.

The request URL continues to determine:

- HTTP authority;
- `Host` header;
- request target;
- TLS server name when HTTPS is used.

The filesystem path must never replace HTTP authority.

## 3.3 Implement reference HTTPS-over-UDS behavior

Pin `httpx==0.28.1` behavior with a local UDS fixture and then match it.

For an HTTPS URL using UDS, the required architecture is:

```text
UnixStream -> TLS to origin authority -> HTTP over TLS
```

Do not retain the current unconditional HTTPS rejection if the pinned reference successfully performs TLS over UDS.

If HTTP/2 over TLS/UDS is exposed by the existing HTTPX public configuration and naturally works through the shared HTTP stack, preserve the existing protocol policy. Do not create a new UDS-specific HTTP/2 implementation.

## 3.4 Preserve streaming and HTTP framing

Required end-to-end cases:

- fixed `Content-Length` response with server keeping the connection open;
- chunked response;
- incremental/streaming body consumption;
- request body streaming consistent with the supported facade;
- response close before full consumption;
- keep-alive/reuse where the normal pool permits it.

A test server that closes every connection after every response is insufficient proof.

## 3.5 Timeout and cancellation

Prove:

- UDS connect timeout/total timeout obeys the existing timeout model;
- cancelling an in-flight UDS request releases connection/pool ownership;
- a constrained follow-up request succeeds;
- closing a partially consumed response does not leak the Unix stream.

## 3.6 Pool identity

Different UDS paths must never share one connection.

Within one compatible UDS transport instance/path, safe keep-alive reuse should follow the normal HTTP pool behavior rather than forcing a new connection for every request.

## 3.7 Platform behavior

On non-Unix systems:

- package import remains successful;
- constructor/use-time behavior matches the pinned reference as closely as practical;
- failure is deterministic and documented.

## Track 3 acceptance criteria

- production UDS no longer uses a bespoke manual HTTP response parser;
- real HTTP requests traverse `UnixStream` through the normal HTTP machinery;
- `Content-Length` keep-alive responses complete without waiting for EOF;
- chunked and streaming responses behave through the normal framing layer;
- HTTPS-over-UDS matches HTTPX 0.28.1 reference behavior;
- Host/authority/TLS name semantics are correct;
- cancellation/timeout/close release resources;
- same-path safe reuse works where the normal pool allows it;
- different UDS paths remain isolated.

---

# Track 4 — Correct SOCKS handshake state and re-enter the normal HTTP engine

## 4.1 Keep the bounded native SOCKS handshake

The RFC 1928/1929 native implementation is the correct architectural direction.

Do not replace it with `httpcore`, `python-socks`, requests, curl, or a second Python networking stack.

The corrective work should make the handshake produce a connected tunnel stream that can be consumed by the normal HTTP/TLS path.

## 4.2 Fix authentication method selection

The method-negotiation function must return the proxy-selected authentication method.

Behavior must be:

- if proxy selects `NO AUTH`, proceed without RFC 1929 even when credentials were offered;
- if proxy selects username/password, authenticate and require configured credentials;
- if proxy selects an unsupported method, fail;
- if proxy returns no acceptable methods, fail.

Do not decide whether to authenticate solely from whether client credentials exist.

Add a fixture case where credentials are configured but the proxy selects `NO AUTH`.

## 4.3 Remove loopback fallback on DNS failure

For `socks5://` local DNS mode, inability to resolve the destination must produce a DNS/connect error.

It must never substitute `127.0.0.1`, `::1`, or any other fallback address for an unresolved remote hostname.

Add a regression test using a guaranteed-nonexistent hostname and a local listener to prove no connection is redirected to loopback.

## 4.4 Pin and preserve DNS/address semantics

Differentially prove HTTPX 0.28.1 behavior for:

- `socks5://` hostname;
- `socks5h://` hostname;
- IPv4 literal;
- IPv6 literal where available;
- unresolved hostname;
- percent-encoded credentials;
- empty credential edge cases if accepted by the reference.

The local SOCKS fixture must record the actual ATYP and destination bytes.

## 4.5 Treat a completed SOCKS CONNECT as a direct origin stream

After SOCKS CONNECT succeeds:

- plain HTTP must use normal origin-form request targets (`/path?query`), not HTTP-forward-proxy absolute-form URLs;
- HTTPS must wrap the tunnel in origin TLS and then use normal HTTP framing;
- response body delimiting must be performed by the normal HTTP engine;
- keep-alive and reuse must use the normal connection/pool semantics.

Do not call the HTTP forward-proxy request writer for traffic inside a SOCKS tunnel.

## 4.6 Use one total deadline/budget

The total request/proxy timeout must not restart at each handshake read/write step.

Use the existing request deadline/remaining-budget model so TCP connect, method negotiation, auth, CONNECT, origin TLS, and HTTP setup consume one coherent budget.

Add a fixture that delays multiple handshake phases so repeated full-duration timeouts cannot exceed the configured total by multiplication.

## 4.7 HTTPS-over-SOCKS proof

Add a deterministic local TLS origin behind the SOCKS fixture.

Prove:

- TLS terminates at the origin side of the SOCKS tunnel;
- certificate verification behaves according to the existing TLS configuration;
- SNI/server name is the destination origin, not the proxy;
- HTTP response is parsed by the normal HTTP path.

## 4.8 Cancellation and constrained follow-up

With `max_connections=1` or the equivalent constrained ownership case:

1. start a SOCKS request that stalls during connect/handshake/TLS;
2. cancel it;
3. issue a follow-up request through the same compatible route;
4. prove the follow-up succeeds without a leaked lease/connection slot.

## 4.9 Route identity and safe reuse

Connections must remain isolated across:

- direct vs SOCKS;
- HTTP proxy vs SOCKS;
- different SOCKS proxy endpoints;
- `socks5` vs `socks5h` when DNS mode affects route identity;
- materially different authentication identities.

For the same compatible SOCKS route/origin, prove safe keep-alive reuse rather than always opening a fresh proxy connection.

Credential material must not appear in displayable pool keys.

## Track 4 acceptance criteria

- proxy-selected auth method controls whether RFC 1929 runs;
- unresolved local-DNS destinations never fall back to loopback;
- SOCKS DNS/ATYP behavior matches the pinned reference;
- plain HTTP inside SOCKS uses origin-form semantics;
- HTTPS inside SOCKS uses origin TLS and normal HTTP framing;
- fixed-length/chunked/streaming responses do not depend on socket EOF;
- one coherent timeout budget covers all SOCKS setup phases;
- cancellation releases constrained ownership;
- route isolation is proven;
- same-route safe reuse is proven;
- credentials remain redacted;
- malformed proxy replies cannot panic or allocate without bound.

---

# Track 5 — Correct environment proxy routing and `NO_PROXY`

## 5.1 Pin HTTPX environment semantics

Using `httpx==0.28.1`, build a small differential matrix for:

- uppercase `HTTP_PROXY`;
- lowercase `http_proxy`;
- uppercase `HTTPS_PROXY`;
- lowercase `https_proxy`;
- uppercase `ALL_PROXY`;
- lowercase `all_proxy`;
- `NO_PROXY` / lowercase form if honored;
- precedence when multiple values are present;
- HTTP destination vs HTTPS destination;
- explicit `proxy=` combined with environment variables;
- `trust_env=False`.

Do not infer precedence from variable names.

## 5.2 Replace one-global-proxy environment selection

The current binding resolves one environment URL in priority order and attaches it as `Proxy::all(...)` to the whole client.

That cannot represent scheme-specific HTTPX behavior when both HTTP and HTTPS proxy variables are set.

Refactor environment selection so proxy choice occurs by request destination scheme or by an equivalent per-scheme mount/config representation.

Do not create a general mount framework if a small internal selector is sufficient.

## 5.3 Integrate `ALL_PROXY` as fallback

`ALL_PROXY`/`all_proxy` must behave as the pinned reference fallback rather than being treated as either wholly unsupported or as a higher-priority universal replacement for scheme-specific variables.

Correct the current documentation after behavior is pinned.

## 5.4 Integrate environment `NO_PROXY`

Environment bypass rules must enter the existing `NoProxy` matching path or an equivalent shared selector.

Required cases:

- exact host;
- domain suffix;
- host+port where applicable;
- localhost/loopback;
- wildcard;
- HTTP and HTTPS destinations;
- SOCKS selected through environment;
- `trust_env=False` bypassing all environment proxy state.

## 5.5 Preserve explicit configuration precedence

An explicit client/transport `proxy=` value must follow HTTPX precedence relative to environment variables.

Per-request proxy disable/override behavior must remain intact.

## Track 5 acceptance criteria

- HTTP and HTTPS destinations can select different environment proxies;
- `ALL_PROXY` is a reference-compatible fallback;
- uppercase/lowercase precedence matches the pinned reference;
- environment `NO_PROXY` bypass works for direct, HTTP proxy, and SOCKS routes;
- `trust_env=False` ignores all proxy environment state;
- explicit proxy configuration precedence matches HTTPX;
- no client-wide single-proxy shortcut remains where it changes observable behavior.

---

# Track 6 — Complete the missing transport differential test matrix

The final test corpus must cover the behavior above at both native-core and HTTPX-compatibility levels where applicable.

## 6.1 Required UDS cases

- HTTP over UDS;
- HTTPS over UDS if accepted by HTTPX 0.28.1;
- correct Host/request target;
- fixed-length response with persistent server connection;
- chunked response;
- streaming response;
- cancellation and follow-up;
- timeout;
- same-path reuse;
- different-path isolation;
- non-Unix behavior.

## 6.2 Required direct connector cases

- host-only `local_address` sync and async;
- observed source address;
- IPv6 where environment supports it;
- family mismatch/unavailable bind;
- actual runtime socket constants;
- supported option application;
- unsupported option error;
- async DNS/address iteration behavior.

## 6.3 Required SOCKS cases

At minimum:

1. HTTP through `socks5://`;
2. HTTPS through SOCKS;
3. no-auth;
4. username/password auth;
5. credentials offered + server selects no-auth;
6. auth rejection;
7. local DNS hostname -> IP ATYP;
8. remote DNS hostname -> domain ATYP;
9. IPv4 literal;
10. IPv6 literal where available;
11. unresolved local-DNS host fails without loopback fallback;
12. connect refusal;
13. SOCKS reply failure;
14. malformed/truncated handshake;
15. total timeout across multiple delayed phases;
16. cancellation + constrained follow-up;
17. `NO_PROXY` bypass;
18. `trust_env=False`;
19. `ALL_PROXY` fallback;
20. HTTP/HTTPS env precedence;
21. route isolation;
22. same-route keep-alive reuse;
23. credential redaction;
24. origin-form request target observed by destination server.

## 6.4 Reference-driven assertions

Where a case is about HTTPX public behavior rather than a native invariant, run the same scenario against the pinned reference and EggFetch facade.

Do not create tests that merely assert the EggFetch implementation's current preferred behavior.

## Track 6 acceptance criteria

- every required case above has a deterministic local test or an explicit platform skip with rationale;
- reference-facing tests compare against HTTPX 0.28.1 semantics;
- no external proxy/origin service is required;
- fixture servers are bounded and terminate cleanly;
- failures prove the intended error family rather than only `is_err()` where the public exception class matters.

---

# Track 7 — Reconcile compatibility ledger and documentation

## 7.1 Correct contradictory environment documentation

The repository currently contains both:

- core documentation saying the Rust core does not read environment proxy variables directly; and
- Python compatibility behavior that does read proxy environment variables when `trust_env=True`.

Document this distinction clearly:

- core/native proxy configuration is explicit;
- the HTTPX compatibility binding may interpret environment state and translate it into native proxy configuration.

Do not state that EggFetch globally does not support `ALL_PROXY` if the facade supports it.

## 7.2 Correct UDS scope claims

Do not describe UDS as fully qualified until the shared HTTP/TLS path and HTTPS/reference semantics are proven.

Once Track 3 closes, document the exact supported behavior.

## 7.3 Correct SOCKS scope claims

Do not claim HTTP/HTTPS SOCKS parity merely because the handshake succeeds.

Qualification wording must be backed by the Track 4/6 tests for framing, TLS, timeout, cancellation, environment routing, and reuse.

## 7.4 Reconcile `allowed-differences.toml`

After implementation:

- regenerate the candidate API manifest;
- run the existing comparator;
- keep resolved records out of the active allowlist;
- preserve historical resolved records in `resolved-differences.toml`;
- do not relabel a still-observable public mismatch as intentional merely to achieve zero unexplained differences.

## 7.5 Reclassify the previous Phase 6 qualification record

Preserve `40beeec...` as historical evidence but make clear that final transport qualification was superseded by this corrective pass.

The final profile must point to the new exact executable SHA, not a documentation-only SHA.

## Track 7 acceptance criteria

- docs accurately distinguish core proxy policy from compatibility-layer environment behavior;
- UDS/SOCKS claims match proved behavior exactly;
- no stale statement says `ALL_PROXY` is unsupported if it is implemented;
- active allowlist has no disguised must-close transport gaps;
- previous qualification remains auditable but is not presented as current final closure;
- final qualification SHA identifies executable code.

---

# Track 8 — Downstream qualification and final exact-SHA closure

## 8.1 Resolve the downstream SSE result instead of dismissing it

The previous qualification recorded:

- 54 downstream passes;
- 5 shim-detection failures described as expected;
- 3 `httpx-sse` failures attributed to EventSource API incompatibility.

Investigate the three SSE failures directly.

For each failure, classify it as one of:

1. valid HTTPX 0.28.1 public behavior within the Stage C contract -> fix it;
2. behavior requiring a deliberately excluded private/Trio/unsupported surface -> document exact dependency and keep it outside the claim;
3. a fixture/shim-detection artifact -> correct the fixture so it does not count as a behavioral failure.

Do not write "all actual behavioral tests pass" while unexplained downstream behavioral failures remain.

## 8.2 Run routine validation on the final executable SHA

Required repository checks include the existing routine validation, at minimum:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
./scripts/check.sh
```

If `scripts/check.sh` already includes a command, do not duplicate it in CI; the handoff report may still list the constituent result.

## 8.3 Run the full pinned compatibility suite cleanly

Required command:

```sh
EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Closure requires one clean complete run on the final executable SHA.

Do not record known failures as "flaky" and still mark the SHA qualified. If a lightweight fixture is nondeterministic, fix the fixture or timeout ownership so the authoritative qualification run is clean.

## 8.4 Run the API oracle

Use the established generator/comparator.

Required result:

- zero unexplained differences;
- zero stale allowed entries;
- zero resolved-in-active entries;
- zero requires-resolution entries.

Allowed active differences must all have reviewed Stage C rationale.

## 8.5 Run downstream behavioral fixtures

Required outcome:

- zero unexplained public-contract failures within the claimed Stage C surface;
- shim-detection cases separated from behavioral qualification rather than counted as ordinary failures;
- any retained exclusion tied to an explicitly out-of-scope surface and documented in the compatibility profile.

## 8.6 Bind all evidence to one executable SHA

The final report must name exactly one executable SHA for:

- routine validation;
- targeted transport tests;
- full pinned compatibility suite;
- API oracle;
- downstream qualification.

Documentation-only commits after that SHA may update prose, but the profile's `qualification-sha` must identify the executable tree that was actually tested.

## Track 8 acceptance criteria

- routine validation clean;
- targeted Track 1–6 tests clean;
- one complete pinned compatibility run has zero failures;
- API oracle is clean with only reviewed Stage C differences;
- downstream public-contract tests have zero unexplained failures;
- all evidence is tied to one exact executable SHA;
- profile/status may then return to `qualified`.

---

# Global acceptance criteria

This corrective pass is complete only when all of the following are true:

- [ ] `local_address` accepts HTTPX-compatible host-only values and binds with an OS-selected port;
- [ ] advanced connector DNS resolution is non-blocking and does not use only the first address unconditionally;
- [ ] supported `socket_options` use real platform/runtime semantics, not copied Linux numbers;
- [ ] unsupported socket options are deterministic errors and are not silently ignored;
- [ ] no unsafe socket code is introduced;
- [ ] UDS production traffic uses the normal HTTP protocol machinery;
- [ ] UDS fixed-length/chunked/streaming responses do not depend on EOF;
- [ ] HTTPS-over-UDS matches HTTPX 0.28.1 behavior;
- [ ] UDS timeout/cancellation/reuse/isolation are proven;
- [ ] SOCKS auth follows the proxy-selected method;
- [ ] SOCKS local DNS failure never falls back to loopback;
- [ ] SOCKS HTTP uses origin-form semantics after CONNECT;
- [ ] SOCKS HTTPS performs origin TLS and then normal HTTP framing;
- [ ] SOCKS total timeout uses one coherent deadline/budget;
- [ ] SOCKS cancellation releases constrained ownership;
- [ ] SOCKS route isolation and same-route reuse are proven;
- [ ] `HTTP_PROXY`, `HTTPS_PROXY`, and `ALL_PROXY` selection is request-scheme-aware;
- [ ] environment `NO_PROXY` and `trust_env=False` match the pinned reference;
- [ ] explicit proxy precedence remains correct;
- [ ] compatibility docs no longer contradict implemented environment behavior;
- [ ] previous `40beeec...` qualification is preserved as historical, not current final evidence;
- [ ] downstream SSE failures are resolved or precisely shown to depend on excluded surface;
- [ ] routine validation passes;
- [ ] one full pinned compatibility run passes with zero failures;
- [ ] API oracle has zero unexplained/stale/resolved-active/requires-resolution entries;
- [ ] downstream qualification has zero unexplained failures in the claimed surface;
- [ ] all final evidence points to one executable SHA.

# Rejection criteria

Reject the corrective implementation if any of the following occurs:

- `local_address` still requires `host:port` in the HTTPX compatibility API;
- socket option behavior depends on hard-coded Linux constants in cross-platform code;
- unsupported socket options are silently ignored;
- UDS retains a production-only raw HTTP parser/read-to-EOF framing path;
- HTTPS-over-UDS is rejected without reference evidence supporting rejection;
- SOCKS uses absolute-form HTTP-forward-proxy targets after the SOCKS tunnel is established;
- SOCKS DNS failure can target loopback or another fallback address;
- SOCKS auth runs without regard to the selected method;
- each SOCKS handshake step receives a fresh full "total" timeout budget;
- SOCKS/UDS disable pooling globally merely to simplify correctness;
- direct, HTTP proxy, UDS, or SOCKS pool identities can collide unsafely;
- credentials appear in logs/errors/debug/pool keys;
- environment proxy logic still chooses one global proxy before knowing request scheme;
- `NO_PROXY` environment values are parsed but not actually applied to request routing;
- a Python/httpcore/curl side stack is added;
- unsafe Rust/libc code is introduced for socket options;
- CI/release infrastructure is expanded for this pass;
- compatibility closure is achieved by moving a known public mismatch to `intentional` without a positive technical reason;
- the final qualification still contains known flaky failures or unexplained downstream failures.

# Stop conditions

Stop and produce a bounded blocker report instead of silently changing scope if:

1. HTTPX 0.28.1 requires a socket option that cannot be expressed with the project's safe-Rust constraint and no small safe dependency/semantic adapter is viable;
2. HTTPS-over-UDS cannot be layered into the existing HTTP/TLS machinery without replacing a major subsystem;
3. the existing pool abstraction cannot accept preconnected UDS/SOCKS streams without a broad architectural rewrite;
4. a required HTTPX environment-proxy precedence rule fundamentally conflicts with an established security invariant;
5. a downstream SSE failure depends on an HTTPX public behavior that would require a materially broader transport/concurrency scope than Stage C.

A blocker report must include:

- exact reference case;
- failing EggFetch behavior;
- missing primitive;
- affected acceptance criteria;
- smallest corrective follow-up;
- why the item cannot be completed inside this pass.

Do not downgrade the blocker to `intentional` solely to restore a qualified status.

# Suggested commit decomposition

Keep commits narrow enough that failures can be bisected:

1. `docs: reopen HTTPX transport qualification pending corrective closure`
2. `fix: align HTTPX local_address semantics and async address resolution`
3. `fix: make socket option handling platform-correct`
4. `refactor: route UDS streams through normal HTTP and TLS machinery`
5. `fix: correct SOCKS auth DNS and deadline semantics`
6. `refactor: route SOCKS tunnels through normal HTTP and TLS machinery`
7. `fix: implement scheme-aware environment proxy and NO_PROXY routing`
8. `test: complete HTTPX transport differential closure matrix`
9. `docs: record exact-SHA HTTPX transport qualification evidence`

Combining adjacent commits is acceptable if the resulting commit remains reviewable. Do not combine unrelated cleanup.

# Required handoff report

The implementing agent must report:

- starting SHA;
- final executable SHA;
- final documentation SHA if different;
- exact HTTPX version used;
- `local_address` reference and candidate results;
- socket option strategy and platform behavior;
- any dependency change and transitive/size rationale;
- UDS HTTP/HTTPS/framing/streaming/reuse results;
- SOCKS schemes supported;
- SOCKS DNS semantics per scheme;
- SOCKS auth-selected-method result;
- unresolved-DNS regression result;
- HTTP and HTTPS-over-SOCKS results;
- SOCKS timeout/cancellation result;
- SOCKS isolation/reuse result;
- environment proxy precedence matrix result;
- `NO_PROXY` and `trust_env=False` result;
- targeted transport test counts;
- `cargo fmt` result;
- `cargo clippy` result;
- `cargo test --workspace` result;
- `./scripts/check.sh` result;
- full pinned compatibility suite result;
- API oracle counts;
- downstream fixture result and SSE disposition;
- active allowed-difference count/classification;
- confirmation that Trio/AnyIO, Python 3.8/3.9, private HTTPX modules, CI, and release scope were not expanded;
- final compatibility designation.

The intended end state is a truthful **Stage C qualified** HTTPX 0.28.1 compatibility profile for the documented Python >=3.10 asyncio surface, with the transport claim supported by executable local evidence rather than constructor acceptance or allowlist reclassification.