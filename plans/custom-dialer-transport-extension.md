# Custom Dialer Transport Extension

Planning baseline: `475bd50f6f9b9f66b95eea814f06b4adb22ede93` (`main`, 2026-09-13; eggfetch 0.1.4)
Parent program: `plans/extensible-embedded-transport-consumer-program.md`
Status: implemented; closure evidence recorded in the parent plan

## Objective

Add a small Rust-native extension point that lets an embedding application supply the byte stream used to reach a logical HTTP(S) host/port, while eggfetch continues to own HTTP, destination TLS, pooling, timeouts, redirects, retries and response streaming.

The API must be general enough for custom networks, service meshes, overlay transports, deterministic test harnesses, SSH/VPN-like connectors or other caller-owned routing systems. It must not model any particular downstream application or proxy protocol.

## Core invariant

A custom dialer changes only the physical path to the logical destination:

```text
logical URL / Host / auth origin / redirect origin / response URL
                         unchanged
                              |
                              v
                  eggfetch HTTP + TLS
                              |
                              v
                    custom byte stream
                              |
                              v
                    caller-owned route
```

For `https://service.example:443/path`, the custom dialer receives the logical destination `service.example:443`; eggfetch must still use `service.example` for SNI and certificate validation. Rewriting the request URL to the dialer's physical endpoint is forbidden.

## 1. Define an object-safe native dialer API

Introduce a dedicated transport module, for example `transport/dialer.rs`, and export only the types needed by native callers from `lib.rs`.

The exact syntax may differ, but the semantic shape should be close to:

```rust
pub trait Dialer: Send + Sync + 'static {
    fn dial(&self, target: DialTarget) -> DialFuture<'_>;
}

pub struct DialTarget {
    host: String,
    port: u16,
}

pub type DialFuture<'a> =
    Pin<Box<dyn Future<Output = Result<DialStream, DialError>> + Send + 'a>>;
```

`DialStream` should erase the concrete caller stream behind an eggfetch-owned trait object implementing Tokio `AsyncRead + AsyncWrite + Send + Unpin`. The caller must not implement Hyper's `Connection` trait or `tower_service::Service<Uri>`.

Requirements:

- `DialTarget` contains no credentials, headers, query strings or request body data.
- Hostnames and IP literals are passed as logical destinations, with an explicit effective port.
- The API is object-safe so one `Arc<dyn Dialer>` can be stored in `ClientInner`.
- No `async-trait` dependency is required solely for this trait.
- `Debug` for dialer-related public types must be safe and bounded.
- A custom dialer is client-scoped in v1. Do not introduce request-level dialer mutation unless implementation proves a concrete need.

## 2. Preserve caller errors without coupling eggfetch to transport protocols

A dialer needs a small error type whose public categories describe connection establishment rather than proxy products. Candidate categories are:

- connection/unreachable;
- timeout/cancelled establishment;
- authentication/authorization at the custom transport layer;
- target rejected/unavailable;
- other transport failure.

Do not add protocol-specific variants.

Preserve an error source chain so an embedding application can recover its own source error when needed. Because `eggfetch_core::Error` is currently a public exhaustive enum, do not casually change existing variant payloads or add a variant without checking downstream source compatibility. Prefer a design that preserves custom dial failure under an existing source-carrying error path and adds read-only inspection helpers if necessary.

Required behavior:

- a custom dial failure never becomes a direct fallback;
- no automatic error string includes a URL with credentials;
- caller-provided source errors are not duplicated into multiple log fields by eggfetch;
- timeout/cancellation remains distinguishable enough for eggfetch retry classification where appropriate;
- the custom dialer API documents that caller error types themselves must not expose secrets through `Display`/`Debug`.

## 3. Add a Hyper connector adapter owned by eggfetch

Implement a crate-internal connector that adapts the public `Dialer` into Hyper's expected connector service.

Conceptually:

```text
Service<Uri>
    -> extract logical host/effective port
    -> Dialer::dial(DialTarget)
    -> eggfetch wrapper implements Hyper Connection
    -> raw stream
```

For HTTPS, compose this raw connector under `hyper_rustls::HttpsConnector` so destination TLS remains eggfetch-owned. The existing connect timeout must encompass the custom dial plus the destination TLS handshake, matching the ordinary direct connector's connect-phase semantics.

Do not implement TLS inside the public dialer contract. A caller may internally use encryption/tunneling to create the byte stream, but that is distinct from the destination HTTPS session.

The custom route should reuse existing Hyper request-body and response-body machinery rather than creating a parallel HTTP stack.

## 4. Integrate at `ClientBuilder` / `ClientInner`

Add an additive native builder method such as:

```rust
Client::builder().dialer(Arc<dyn Dialer>)
```

or an equivalent generic convenience that stores the dialer behind an `Arc` internally.

The builder must remain infallible if that is the current `ClientBuilder::build` contract. Configuration that cannot be validated until dispatch should surface as an ordinary request error rather than silently replacing the configured dialer.

Store the dialer immutably in the built client. Client clones share the same dialer and pool topology.

Do not expose the custom dialer through Python/CLI/HTTPX compatibility unless a separate use case is approved. This is a native engine extension point.

## 5. Route-selection semantics

Update the core route-selection contract so custom dialing has explicit, fail-closed precedence.

Recommended v1 rule:

```text
UDS request?                         -> UDS route
explicit incompatible request hint? -> reject before I/O
client has custom dialer?            -> custom-dialer route
built-in effective proxy?            -> proxy/SOCKS route
SNI/direct-special route?            -> existing direct route
standard                             -> standard route
```

However, do not silently allow combinations where two independent components both claim the physical route. The following combinations must be audited and documented explicitly:

### Built-in proxy + custom dialer

Initial contract should reject this at construction/preparation or define custom dialer as the connector *under* the built-in proxy only if that composition is deliberately implemented. Do not silently ignore either configuration. The preferred v1 is mutual exclusion because arbitrary proxy-under-custom-dialer composition introduces ambiguous DNS/TLS/auth ownership.

### UDS + custom dialer

Reject or let the explicit UDS request route win only if the behavior is documented and tested. Prefer rejection for contradictory per-request UDS plus client-wide custom dialer, because silently bypassing the dialer weakens fail-closed route expectations.

### `ResolvedTarget` + custom dialer

These are two physical-destination authorities. Reject the combination in v1. Do not reinterpret resolved addresses as hints to an opaque dialer.

### local-address/socket options + custom dialer

These options operate on sockets eggfetch creates itself. Reject their combination with a custom dialer unless the public dialer contract later gains a generic capability model. Do not pretend the options were applied.

### SNI override + custom dialer

This can remain compatible because SNI is destination TLS policy owned by eggfetch after the custom byte stream is established. Test it directly.

### H3 + custom dialer

Reject before QUIC network I/O. The custom-dialer v1 is for Hyper-based HTTP/1/2 byte streams; it does not represent UDP/QUIC transport.

### Alt-Svc

Do not allow an H3/Alt-Svc path to escape a configured custom dialer. If custom dialing is active, either suppress H3 discovery/use for that request or reject incompatible HTTP-version policy before network I/O.

## 6. Redirect and retry semantics

A client-wide dialer remains active across redirects and logical eggfetch retry attempts. Each new physical connection asks the same dialer to reach the current logical origin.

This differs from request-scoped `ResolvedTarget`, which pins a specific destination snapshot. Do not copy resolved-target redirect restrictions onto a client-wide dialer unless required by routing security.

Requirements:

- same-origin redirects reuse pooled connections normally;
- cross-origin redirects use the same dialer with the new logical host/port;
- auth/cookie/header stripping remains governed by existing redirect policy;
- retries use the same dialer and preserve body replayability rules;
- custom dialer configuration is immutable across the logical request;
- no redirect/retry path falls back to the default direct connector after a custom dialer failure.

## 7. Pool identity and reuse

A `Client` has at most one immutable custom dialer, so Hyper connection reuse within that client can remain keyed by logical origin as today. Two separately built clients with different dialers naturally own separate Hyper pools.

Do not introduce dialer pointer identity into logical `Pool` origin keys unless implementation demonstrates a cross-client pool-sharing path, which does not exist today.

A pooled connection created through the custom dialer must never be inserted into or reused by the standard client's pool. The custom route should own its own Hyper client instance inside `ClientInner`.

## 8. Connection metadata

Ordinary pooled Hyper responses already have bounded metadata visibility. A custom dialer's stream may not expose meaningful local/peer socket addresses.

Do not fabricate metadata. If the first version cannot provide socket metadata, report unknown/None exactly as existing metadata policy requires. A future optional metadata object on `DialStream` can be considered independently if a real consumer needs it.

## 9. Focused deterministic tests

Add tests under the existing core integration-test structure using only local fixtures and a synthetic dialer. At minimum prove:

1. a hostname with no usable ordinary route succeeds through a custom dialer to a local server;
2. the HTTP Host/authority remains the logical hostname;
3. HTTPS connects through the custom stream while SNI/certificate validation uses the logical hostname;
4. invalid destination or dial failure returns an error and performs no ordinary direct fallback;
5. custom dialer receives the effective explicit/default port correctly;
6. cross-origin redirect calls the dialer with the redirected logical target and still applies normal credential stripping;
7. explicit retry invokes the same dialer and does not mutate route policy;
8. proxy + custom dialer rejects before network I/O under the chosen v1 contract;
9. UDS + custom dialer rejects before network I/O under the chosen v1 contract;
10. `ResolvedTarget` + custom dialer rejects before network I/O;
11. local-address/socket-option + custom dialer rejects rather than silently ignoring socket policy;
12. SNI override remains effective over the custom route;
13. H3-only + custom dialer rejects before QUIC I/O;
14. separate clients with separate dialers do not share physical connections;
15. dropping/cancelling a request drops the dialer-owned stream normally.

Use counters/recorded targets in the synthetic dialer so no-fallback assertions are direct rather than inferred from request success.

## 10. Documentation changes during implementation

Update at least:

- `docs/architecture/core-engine.md` — route selection and ownership;
- `docs/architecture/core-tls-proxy-protocols.md` — custom dialer vs proxy/TLS/H3 boundaries;
- `docs/rust/guide.md` — native API example and incompatibility table;
- `docs/architecture/feature-flags.md` only if implementation changes feature ownership;
- public rustdoc for `Dialer`, `DialTarget`, `DialStream` and builder methods.

Documentation must describe this as caller-supplied transport/dialing, not a new proxy implementation.

## Dependency policy

This plan should add no new runtime dependency. Tokio, Hyper, Hyper-Rustls, `tower-service` and futures utilities already provide the required primitives.

If object-safe ergonomics appear to require `async-trait`, first implement the boxed-future form and compare complexity. A new dependency requires explicit justification under `docs/architecture/dependency-policy.md`.

## Required validation

Run focused custom-dialer tests and all affected route/TLS/proxy/redirect/retry tests, then:

```sh
cargo check -p eggfetch-core --no-default-features --features http1
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo test -p eggfetch-core
./scripts/check.sh
```

Do not renew exact-SHA HTTPX/HTTPX2 compatibility evidence in this child plan; executable behavior remains unfrozen until the parent closure plan.

## Non-goals

- no protocol-specific dialer implementation;
- no downstream repository dependency;
- no public Hyper connector/service types;
- no request-scoped dialer switching in v1;
- no custom DNS resolver redesign;
- no transport-specific credential store;
- no automatic composition with built-in proxy routes;
- no QUIC/H3 dialer;
- no claim that the new route is smaller or faster.

## Exit criteria

- [x] native callers can provide a byte-stream dialer without Hyper knowledge;
- [x] destination HTTP/TLS remains eggfetch-owned;
- [x] logical Host/SNI/certificate identity is preserved;
- [x] custom-dial failure cannot fall back to direct networking;
- [x] conflicting route authorities fail before I/O;
- [x] retries/redirects use the immutable custom dialer predictably;
- [x] no unsafe cross-dialer pool reuse exists;
- [x] error source information remains recoverable without protocol-specific core variants;
- [x] focused tests cover direct, TLS, wire-target preservation, route conflicts, redirect credential stripping, retry reuse, separate-client pool isolation, and cancellation cleanup;
- [x] ordinary callers not configuring a dialer behave exactly as before;
- [x] routine validation passes.
