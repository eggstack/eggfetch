# HTTPX Parity Corrective 06 — Final Semantic Truthfulness

Baseline reviewed: `25c2c6f01138e2d6a59d1256076ec84972a92d83`
Current qualified executable SHA: `c44d4f25ffebc1a792335163ae4bc106076b3963`
Reference: `httpx==0.28.1` / `httpcore==1.0.9`
Follow-up closure: `plans/httpx-parity-corrective-07-final-exact-sha-requalification.md`

## Objective

Close the remaining semantic gaps found after Correctives 01–05 without reopening the broad HTTPX parity program or introducing a second transport architecture.

The current repository is structurally healthy: the exact-SHA bookkeeping is now correct, direct H2-only behavior is substantially implemented, proxy/origin TLS trust domains are separated, response wire metadata is exposed, and 101 upgrades have an owned network-stream abstraction. The remaining work is narrow but material because several current Stage C statements overclaim behavior that the implementation cannot yet guarantee.

This corrective pass is limited to four areas:

1. make `ssl.SSLContext` translation genuinely fail-closed rather than silently dropping unobservable or unmapped state;
2. make request-extension and trace behavior consistent across sync/async × buffered/streaming paths, with truthful async-callback support;
3. ensure 101 `network_stream` wrappers match the calling API mode rather than the response conversion path;
4. propagate H2-only policy through the remaining SNI-override and SOCKS routes, or classify those routes explicitly if they cannot be closed cleanly.

Do not use this pass to broaden the public API, replace Hyper, add a compatibility-only network stack, redesign CI, or chase metadata-only parity that is already correctly bounded.

---

## 0. Re-open the qualification claim before executable changes

The current profile is correctly exact-SHA-qualified for `c44d4f25...`, but this plan intentionally requires executable changes. The existing qualification must therefore become historical evidence rather than a current claim as soon as implementation begins.

Before or in the first executable corrective commit:

- update `compat/httpx/0.28.1/profile.toml` using the repository's existing valid profile schema/conventions so it no longer presents the modified tree as Stage C qualified;
- update `plans/httpx-parity-correction-status.md` to state that Corrective 06 is open and supersedes the `c44d4f25...` qualification for the new executable tree;
- retain the old qualification SHA and evidence as historical evidence; do not delete or rewrite it as though it never existed;
- do not assign a new `qualification-sha` until Corrective 07 runs against a frozen executable commit.

Acceptance:

- [ ] No executable descendant of `c44d4f25...` is described as already qualified.
- [ ] Historical qualification evidence remains auditable.
- [ ] The compatibility profile does not contain a placeholder/future SHA.

---

# Track A — Make SSLContext translation truly fail-closed

## A1. Current defect

`crates/eggfetch-python/python/eggfetch/compat/httpx/_ssl_context.py` still has semantic states that can be silently discarded.

Known examples from the reviewed tree:

- `_detect_client_cert()` returns `False` unconditionally, so a caller-created context may contain an mTLS identity that EggFetch cannot observe or export;
- configured ALPN state is not publicly inspectable through Python's `ssl.SSLContext`, so a caller may have changed ALPN while the translator treats the context as otherwise representable;
- TLS 1.2/1.3 minimum/maximum values are classified as representable, but the current kwargs translation does not carry them into native `TlsConfig`;
- `CERT_REQUIRED + check_hostname=False` is classified as representable, but the current compatibility conversion collapses the result into `verify`/`cert` kwargs and does not preserve the hostname-verification toggle;
- an empty verified trust store can be translated to `verify=True`, silently replacing “trust nothing” with EggFetch default roots;
- helper-created contexts can still be mutated through state that the existing fingerprint cannot observe, especially `load_cert_chain()` and `set_alpn_protocols()`.

The rule for this pass is strict:

> If EggFetch cannot prove that every connection-relevant semantic of the supplied SSLContext is represented by the Rust TLS configuration, it must reject the context before network dispatch.

No heuristic equivalence is allowed.

## A2. Stop translating SSLContext into loose `verify/cert` kwargs

The current `context_to_eggfetch_kwargs()` abstraction is too weak to carry the semantics already supported by `eggfetch_core::tls::TlsConfig`.

Replace or supplement it with one binding-layer structured translation object, conceptually similar to:

- trust policy / exact CA DER anchors;
- certificate verification enabled/disabled;
- hostname verification enabled/disabled;
- minimum TLS version;
- maximum TLS version;
- client certificate/key provenance when known;
- SNI policy only when the source context semantics are known;
- a representability disposition.

The exact Python/Rust type name is implementation-defined. Requirements:

- it remains a binding-layer object/value; do not place Python objects in `eggfetch-core`;
- sync Client, AsyncClient, `Proxy(ssl_context=...)`, and `NetworkStream.start_tls(...)` use the same translation rules;
- native construction ultimately builds one `TlsConfig` rather than re-expanding to lossy generic kwargs;
- the direct non-SSLContext `verify=` and deprecated `cert=` paths remain supported as they are today unless a concrete bug requires adjustment.

## A3. Caller-created / passthrough SSLContext policy

A generic caller-created `ssl.SSLContext` cannot safely expose whether a client certificate/private key has been loaded, nor can Python reliably expose all ALPN state. Therefore generic external-context acceptance cannot be called exact translation merely because its visible CA/cipher/version state looks ordinary.

Preferred disposition:

- treat arbitrary caller-created and passthrough `ssl.SSLContext` objects as **unrepresentable by default** unless the implementation can prove every relevant state through documented public APIs;
- fail with a stable `TypeError` before DNS/connect/TLS dispatch;
- retain this as an explicit bounded difference from HTTPX/OpenSSL rather than silently dropping opaque state.

A narrower external-context subset may remain supported only if a proof is added that client-auth state, ALPN state, verification state, trust roots, and version bounds are all known. “No public evidence that a feature is configured” is not proof that it is absent.

Do not infer safety from:

- CA count;
- CA names/order;
- default cipher equality alone;
- class name `SSLContext`;
- absence of a discoverable client certificate;
- the fact that HTTPX would accept the object.

## A4. Helper-created context provenance

`create_ssl_context()` must continue to behave like the pinned HTTPX helper at the Python surface: it returns an actual `ssl.SSLContext` and applies the requested initial trust/cert behavior.

For later EggFetch translation, choose one of these safe strategies after a focused feasibility check:

### Preferred strategy: provenance-aware helper context

Use a genuine `ssl.SSLContext` subclass or equivalent safe wrapper strategy only if it preserves normal Python SSLContext behavior and `isinstance(ctx, ssl.SSLContext)` semantics. Track connection-relevant mutators that cannot otherwise be observed, including at least:

- `load_cert_chain()`;
- `set_alpn_protocols()`;
- `set_npn_protocols()` where present;
- custom cipher mutation;
- trust-store mutation.

Observable properties such as `minimum_version`, `maximum_version`, `verify_mode`, `check_hostname`, and options may continue to be checked by a live fingerprint/snapshot.

Any unrepresentable or opaque post-construction mutation marks the context dirty/untranslatable and causes a pre-dispatch `TypeError`.

### Fallback strategy: narrow helper translation contract

If a genuine provenance-aware context cannot be implemented without fragile monkey-patching/private CPython/OpenSSL state, preserve the real SSLContext helper for Python compatibility but reject it for EggFetch transport translation whenever exact provenance cannot be guaranteed.

It is preferable to document a narrower safe boundary than to maintain nominal SSLContext acceptance with silent semantic loss.

Forbidden approaches:

- OpenSSL pointer extraction;
- private CPython structure inspection;
- unsafe FFI into Python's SSL object internals;
- copying private key material out of an arbitrary caller context;
- heuristic “probably default” classification.

## A5. Preserve representable TLS policy exactly

For helper-created/provenance-safe contexts, map the following when representable:

- `verify_mode == CERT_NONE` -> certificate verification disabled and hostname verification disabled;
- `CERT_REQUIRED + check_hostname=True` -> certificate + hostname verification;
- `CERT_REQUIRED + check_hostname=False` -> certificate verification retained, hostname verification disabled;
- explicit TLS 1.2 minimum/maximum;
- explicit TLS 1.3 minimum/maximum;
- a valid 1.2–1.3 range;
- exact custom CA DER set;
- helper-provenance client cert/key paths;
- default helper trust policy based on the helper's actual construction inputs.

Reject rather than alter:

- TLS versions below 1.2;
- impossible version ranges;
- empty verified trust stores when the native configuration cannot represent “trust no roots” exactly;
- custom OpenSSL cipher policy not representable by rustls;
- custom ALPN policy not intentionally owned by EggFetch's HTTP protocol selection;
- unknown client-certificate provenance;
- other unobservable connection-affecting state.

Do not turn an empty trust store into default roots.

## A6. TLS network-proof tests

Add focused tests at the compatibility boundary, not just classifier tests.

Required sync and async cases where applicable:

1. helper default context succeeds against trusted fixture;
2. helper custom CA trusts only the intended CA;
3. two CA stores with equal cardinality but different contents produce different handshake behavior;
4. `CERT_REQUIRED + check_hostname=False` accepts a chain-valid certificate whose hostname would otherwise fail, while still rejecting an untrusted chain;
5. TLS 1.3-only client fails a TLS 1.2-only server and succeeds a TLS 1.3 server;
6. TLS 1.2-only client fails a TLS 1.3-only server and succeeds a TLS 1.2 server;
7. empty verified trust store is rejected before dispatch or behaves as an actually empty trust store — never default trust;
8. external context with `load_cert_chain()` is rejected before dispatch unless exact provenance support has genuinely been implemented;
9. helper context mutated with `load_cert_chain()` is rejected unless provenance tracking reconstructs it exactly;
10. external/helper context with custom ALPN mutation is rejected unless the exact semantics are intentionally supported;
11. passthrough `create_ssl_context(verify=external_ctx)` returns the same Python object but does not gain fake EggFetch provenance;
12. proxy SSLContext and `NetworkStream.start_tls()` exercise the same rejection/translation rules.

For every fail-closed test, include a fixture counter proving no TCP connection or TLS ClientHello was emitted after a translation error when that can be observed deterministically.

## Track A acceptance

- [ ] `_detect_client_cert()` is not used as a false proof that no client identity exists.
- [ ] Generic external SSLContext objects are rejected unless exact representability is provable.
- [ ] Helper-context opaque mutations cannot be silently ignored.
- [ ] `check_hostname=False` is preserved where support is claimed.
- [ ] TLS min/max versions are preserved where support is claimed.
- [ ] Empty verified trust is never converted to default roots.
- [ ] Custom CA contents, not count, determine trust.
- [ ] The same translator is used by Client, AsyncClient, Proxy TLS, and `start_tls`.
- [ ] Documentation describes the actual safe subset, not a broader hypothetical subset.

---

# Track B — Unify request extensions and make trace claims truthful

## B1. One parser for all native request paths

The binding already has `extract_native_extensions()` for the sync paths, but the reviewed async streaming code still manually parses only `target` and `sni_hostname`.

Eliminate duplicate parsing.

All native paths must use one binding helper:

| API | Buffered | Streaming |
| --- | --- | --- |
| sync `Client` | shared parser | shared parser |
| async `AsyncClient` | shared parser | shared parser |

Known native extension keys remain:

- `target`;
- `sni_hostname`;
- `trace`.

Unknown compatibility Request extension entries remain stored on the Python Request object and are not pushed into core as arbitrary Python values.

## B2. Correct coroutine-function detection

Audit `PyTraceObserver::new` and replace any indirect/incorrect module invocation with an actual `inspect.iscoroutinefunction(callback)` equivalent or another reliable public Python check.

Add direct tests proving the detector distinguishes:

- ordinary `def` callback;
- `async def` callback;
- callable object with sync `__call__`;
- callable object with async `__call__` if the pinned reference recognizes it;
- `functools.partial` wrapping relevant callbacks if the reference behavior matters.

Do not use an unreachable network endpoint as evidence that callback validation worked.

## B3. Decide the supported async trace contract explicitly

The pinned httpcore async trace callback is awaited at the lifecycle event. EggFetch's core `TraceObserver` is synchronous.

Perform a bounded implementation feasibility check before broadening the core trait.

Preferred outcomes, in order:

### Outcome 1 — Binding-local async bridge is cleanly feasible

If an async callback can be awaited from the Python binding without moving Python objects into core and without blocking/reentering the event loop incorrectly:

- AsyncClient accepts async trace callbacks;
- each event callback is awaited before transport proceeds past the reference-equivalent lifecycle point;
- callback exceptions abort the request;
- event order matches the pinned reference for the supported event subset.

### Outcome 2 — Async trace callbacks remain a bounded difference

If true awaiting would require an async trait conversion across the core transport solely for Python compatibility, a second event engine, or risky GIL/event-loop reentrancy:

- support sync trace callables on AsyncClient if they are safe and reference-compatible enough;
- reject coroutine trace callbacks deterministically before dispatch with a clear `TypeError` or compatibility-specific error;
- add an active bounded-difference ledger row naming the exact behavior;
- remove documentation claiming coroutine callback support.

Do not call a coroutine without awaiting it. Do not report async callback support based on constructor acceptance.

## B4. Callback failure must stop useful work as early as the architecture allows

Current core `TraceObserver::on_event()` returns `()`, so callback exceptions can be recorded while transport continues.

Add an abort signal that remains Python-free in core. A suitable design is conceptually:

- observer callback stores the original `PyErr` in the binding-owned error slot;
- observer returns a simple core-owned control result (`Continue` / `Abort`, boolean, or core error code);
- transport/pipeline stops the request when `Abort` is returned;
- binding checks the slot and raises the original callback exception.

Do not put `PyErr`, `PyObject`, or Python callback references into `eggfetch-core`.

If some already-completed lifecycle action cannot be undone, document the exact boundary. At minimum, a failure in a pre-send event must not allow subsequent response processing to continue as though the callback succeeded.

## B5. Fix error-slot handling on all paths

Audit every path that creates a trace error slot.

Requirements:

- sync buffered: drain after dispatch and surface original error;
- sync streaming: surface errors at the relevant entry/iteration boundary;
- async buffered: do not bind the slot to `_trace_slot` and discard it; check it and surface the original callback error;
- async streaming: install the trace bridge through the shared parser and surface errors deterministically;
- redirects/retries: trace observer propagation follows the pinned reference semantics or is explicitly classified; no accidental loss during request reconstruction.

## B6. Differential trace evidence

Use local fixtures and run the same conceptual scenario against pinned HTTPX/httpcore and EggFetch.

Required cases:

- successful HTTP/1.1 request;
- TLS request where supported trace events include connect/start-TLS;
- request with a body;
- response body consumption where body events are claimed;
- connection failure;
- sync callback on sync Client;
- sync callback on AsyncClient;
- async callback on AsyncClient if support is claimed;
- async callback rejected before dispatch if retained as a bounded difference;
- callback raises during an early event -> request aborts and original exception propagates;
- buffered and streaming variants;
- redirect/retry observer propagation for the subset the core truthfully emits.

Event names and info dictionaries must use the pinned `httpcore==1.0.9` vocabulary already recorded in the repo. Do not fabricate unavailable request/return objects merely to fill fields.

## Track B acceptance

- [ ] One extension parser is used by sync/async buffered/streaming native requests.
- [ ] Async streaming no longer has its own target/SNI-only parser.
- [ ] Coroutine-function detection is proven by direct unit/behavior tests.
- [ ] Async trace callback support is either genuinely awaited or explicitly rejected/classified.
- [ ] Every trace error slot is consumed and cannot silently disappear.
- [ ] Callback failure halts transport work at the declared boundary.
- [ ] Original Python callback exceptions propagate.
- [ ] Trace docs and ledger match the actual supported subset.

---

# Track C — Correct network_stream wrapper mode across all response paths

## C1. Current defect

The current conversion associates wrapper type with response implementation rather than caller API mode:

- `PyStreamingResponse` extracts a `PyAsyncNetworkStream`, including when returned from synchronous `Client.stream()`;
- buffered `PyResponse` extracts a sync `PyNetworkStream`, including when returned by async `AsyncClient.request()`.

The two already-covered quadrants happen to be the correct ones (sync buffered and async streaming), leaving sync streaming and async buffered under-tested.

## C2. Make binding mode explicit

Introduce a small binding-layer mode enum/helper, conceptually `Sync` / `Async`, passed into response conversion when a core response owns an upgraded stream.

Requirements:

- sync Client buffered 101 -> sync `NetworkStream`;
- sync Client streaming 101 -> sync `NetworkStream`;
- async Client buffered 101 -> async `NetworkStream`;
- async Client streaming 101 -> async `NetworkStream`;
- HTTPX compatibility facade preserves the same wrapper through `Response.extensions["network_stream"]`;
- ordinary pooled responses remain `None` as currently documented;
- internal proxy CONNECT tunnels remain non-exposed;
- wrapper selection does not alter core ownership or pool behavior.

Implementation details may use separate response conversion helpers or a Python-object field capable of storing either wrapper. Prefer a simple explicit constructor parameter over duplicating the entire response model.

## C3. Sync runtime ownership

Retain the improvement from Corrective 03:

- sync upgraded stream has an explicit Tokio runtime handle;
- transferred stream retains the runtime lease it needs after Client close;
- no normal operation depends solely on ambient `Handle::current()`;
- no nested-runtime panic.

Audit fallback code paths such as `PyResponse::from_core_response()` that call `Handle::current()` when no handle is supplied. For any public 101 route, either supply a valid persistent handle/lease or reject creation of an unusable upgraded wrapper rather than panic later.

## C4. Async semantics

For async wrappers:

- `read`/`write`/`aclose` are genuinely awaitable;
- network waits do not hold the GIL;
- cancellation does not alias/return the owned upgraded socket to the HTTP pool;
- close is idempotent;
- use-after-close yields a stable error;
- wrapper remains usable after parent AsyncClient close to the same extent the owned stream contract permits.

## C5. Four-quadrant tests

Add local 101 echo/leading-byte coverage for all four native and compat combinations:

1. sync buffered;
2. sync streaming;
3. async buffered;
4. async streaming.

Each must assert:

- status 101;
- `extensions["network_stream"]` is non-None;
- object exposes the correct sync vs async operation shape;
- first read returns leading bytes;
- write/read echo works;
- parent client closure does not invalidate transferred ownership unexpectedly;
- close is idempotent;
- the upgraded connection is not reused as a normal pooled HTTP connection.

Add negative tests for ordinary HTTP/1.1 and H2 across the same API modes: no writable stream object.

## C6. start_tls remains bounded and safe

Do not broaden `start_tls` beyond what the core stream variant can safely support.

Requirements remain:

- only owned plain-TCP stream variants may transition to TLS;
- Hyper opaque Adapter variants fail before consuming the stream;
- already-TLS variants fail clearly;
- supplied SSLContext uses Track A's exact translator;
- an unrepresentable context fails before destructive I/O;
- SNI and timeout behavior remain tested on supported variants.

## Track C acceptance

- [ ] Wrapper type is selected by sync/async API mode, not buffered/streaming implementation class.
- [ ] All four 101 response quadrants have end-to-end tests.
- [ ] Sync streaming does not expose an async-only wrapper.
- [ ] Async buffered does not expose a blocking sync wrapper.
- [ ] Runtime ownership remains explicit for sync upgraded streams.
- [ ] Ordinary pooled responses/internal CONNECT behavior remains unchanged.

---

# Track D — Propagate H2-only policy through SNI and SOCKS routes

## D1. Keep already-closed H2 behavior intact

Do not regress the Corrective 04 closures for:

- direct standard TLS H2-only;
- H1-only TLS rejection under H2-only policy;
- cleartext H2 prior knowledge;
- local-address/socket-option direct connector H2;
- UDS H2;
- response `http_version == "HTTP/2"` where H2 succeeds.

Retain `stream_id` as a metadata residual unless the pinned Hyper stack gains a real supported path; do not synthesize it.

Retain HTTP CONNECT proxy H2 origin framing (`H2-009`) as a bounded difference unless a very small change can reuse the canonical H2 transport. Do not rewrite the hand-rolled CONNECT path solely to remove this residual.

## D2. SNI-override route

`ClientInner::sni_client()` builds a separate direct Hyper client. It must inherit the client's `HttpVersionPolicy`.

For H2-only:

- configure the connector TLS ALPN/policy so only legitimate H2 is accepted as required by the selected route;
- apply `hyper_util::client::legacy::Client::builder(...).http2_only(true)` or the equivalent canonical setting used by the standard/direct route;
- ensure `DirectStream::connected()` reports negotiated H2 only when ALPN actually selected `h2`;
- no HTTP/1 fallback is allowed;
- the SNI cache must not cross incompatible transport policy. Since policy is client-level today, document why the current cache key is sufficient or include policy in the key if future/request-level policy makes it necessary.

Tests:

- H2-only + SNI override + H2 TLS fixture succeeds and reports HTTP/2;
- H2-only + SNI override + H1-only TLS fixture fails without sending a valid H1 request;
- SNI changes TLS identity only; TCP destination/Host/logical URL remain unchanged;
- Auto and H1-only behavior remain unchanged.

## D3. SOCKS route

The SOCKS client construction also needs protocol-policy propagation.

For H2-only:

- set the legacy client H2-only policy;
- for HTTPS through SOCKS, inspect the origin TLS ALPN on `SocksStream::Tls` and return `Connected::negotiated_h2()` only when `h2` was actually selected;
- cleartext H2-only through SOCKS should use H2 prior knowledge if Hyper's `http2_only(true)` over the established TCP tunnel supports it cleanly;
- if cleartext or TLS H2 through SOCKS cannot be supported without a second client stack, classify the exact SOCKS route as a bounded difference rather than silently falling back to H1.

Tests should use a local SOCKS5 fixture plus local H2/H1 origin fixture. Cover:

- H2-only HTTPS through SOCKS -> H2 success if claimed;
- H2-only HTTPS through SOCKS to H1-only origin -> failure, never H1 fallback;
- H2-only cleartext through SOCKS -> H2 preface if claimed, otherwise active bounded difference;
- Auto through SOCKS still negotiates normally;
- SOCKS auth and DNS behavior do not regress.

## D4. Ledger truthfulness

After route testing:

- if SNI and SOCKS H2-only behavior is closed, add resolved parity evidence and do not create residuals;
- if a route remains unsupported, create a separate active difference with exact route + reference behavior + candidate behavior + named differential test;
- do not hide SNI/SOCKS under the HTTP CONNECT `H2-009` row because they are different transport paths.

## Track D acceptance

- [ ] Every client construction path that can carry H2-only either propagates the policy or has an explicit residual.
- [ ] SNI override H2-only cannot silently send H1.
- [ ] SOCKS H2-only cannot silently send H1.
- [ ] `Connected::negotiated_h2()` is based on actual TLS ALPN, not policy intent.
- [ ] Existing direct/UDS/H2C tests remain green.
- [ ] HTTP CONNECT `H2-009` remains narrowly described.

---

# Track E — Repair tests, ledgers, and documentation before requalification

Corrective 06 is not complete when code merely passes the existing suite. The current suite missed the defects above, so tests must be strengthened first.

## E1. Replace false-positive tests

Audit and rewrite tests whose names/claims are stronger than their assertions.

At minimum:

- the external-mTLS test must actually call `load_cert_chain()` and assert fail-closed behavior or exact mTLS behavior;
- hostname-verification tests must prove wire behavior, not merely presence of a `verify` key;
- TLS version tests must prove handshake acceptance/rejection;
- async callback tests must use an actual `async def` callback and distinguish callback-validation failure from network failure;
- trace callback failure tests must verify transport abort behavior/counters, not merely “some exception happened”;
- network-stream tests must cover all four sync/async × buffered/streaming combinations.

## E2. Update the compatibility ledgers

Review:

- `compat/httpx/0.28.1/allowed-differences.toml`;
- `compat/httpx/0.28.1/resolved-differences.toml`;
- `compat/httpx/0.28.1/parity-cases.toml`;
- `docs/residual-differences.md`;
- `docs/reference/compatibility.md`;
- `plans/httpx-parity-correction-status.md`.

Rules:

- no active row describes a behavior now resolved;
- no resolved row describes a behavior still unsupported;
- every retained SSLContext/trace/H2 route limitation has a named differential test;
- “supported” means the tested behavior, not merely parameter acceptance;
- ordinary pooled `network_stream` absence, `stream_id`, four-element unsafe socket options, and HTTP CONNECT H2 can remain bounded as already justified;
- do not inflate the residual list with implementation-internal differences that are not observable at the supported HTTPX surface.

## E3. No architectural expansion

The following are explicitly out of scope:

- replacing Hyper/hyper-util;
- exposing writable ordinary pooled sockets;
- implementing actual H2 stream IDs through inference;
- a second Python-only networking engine;
- Trio/AnyIO backend support;
- Python 3.8/3.9 support changes;
- private HTTPX module parity;
- HTTPX CLI emulation;
- unsafe OpenSSL/CPython introspection;
- CI/release redesign.

---

# Corrective 06 validation gate

Before the implementation is handed to Corrective 07, run at least:

1. targeted Track A SSLContext tests;
2. targeted Track B extension/trace tests;
3. targeted Track C 101/network-stream tests;
4. targeted Track D H2 SNI/SOCKS differential tests;
5. existing H2 differential suite;
6. existing proxy TLS isolation/redaction suite;
7. existing corrective kernel;
8. `./scripts/check.sh`.

Corrective 06 may use targeted runs during development, but **do not update the profile to Stage C qualified in this phase**. Full exact-SHA evidence belongs only to Corrective 07 after the executable tree is frozen.

---

# Final acceptance criteria

Corrective 06 is complete only when all of the following are true:

- [ ] Current Stage C qualification is explicitly reopened before executable work and retained only as historical evidence.
- [ ] SSLContext translation cannot silently discard client-auth state, ALPN policy, hostname-verification policy, TLS-version bounds, or empty trust semantics.
- [ ] Generic external contexts are rejected unless exact representability is actually provable.
- [ ] Helper-created contexts cannot silently ignore opaque post-construction mutations.
- [ ] Representable hostname-verification and TLS-version settings reach `eggfetch_core::TlsConfig` exactly.
- [ ] Client, AsyncClient, Proxy TLS, and `NetworkStream.start_tls()` share one safe TLS translation policy.
- [ ] One native extension parser serves sync/async buffered/streaming requests.
- [ ] Async streaming installs the same trace/target/SNI plumbing as the other request paths.
- [ ] Coroutine trace behavior is either truly awaited or explicitly rejected and classified; there is no fake async support.
- [ ] Trace callback errors cannot be silently stored and dropped.
- [ ] Trace callback exceptions stop request processing at the documented boundary and propagate as the original exception.
- [ ] 101 network streams use sync wrappers for sync APIs and async wrappers for async APIs in both buffered and streaming responses.
- [ ] All four 101 response quadrants have leading-byte, read/write, lifecycle, and close tests.
- [ ] SNI and SOCKS H2-only routes cannot silently downgrade to H1; unsupported routes are explicit bounded differences.
- [ ] Existing direct TLS H2-only, h2c prior knowledge, direct/local-address/socket-option H2, and UDS H2 remain green.
- [ ] `stream_id`, HTTP CONNECT H2, ordinary pooled raw-stream absence, and unsafe four-element socket-option limits remain narrowly truthful unless intentionally changed.
- [ ] Tests that previously produced false-positive evidence are replaced with wire-level or direct semantic assertions.
- [ ] `./scripts/check.sh` passes on the final Corrective 06 executable tree.
- [ ] No Stage C `qualification-sha` is assigned to the new executable tree until Corrective 07.

## Handoff boundary

When these criteria are met, stop executable work. Record the final executable/test commit SHA and hand that exact SHA to `plans/httpx-parity-corrective-07-final-exact-sha-requalification.md`.

Any executable change after that handoff invalidates the frozen SHA and requires restarting Corrective 07 from its first qualification step.