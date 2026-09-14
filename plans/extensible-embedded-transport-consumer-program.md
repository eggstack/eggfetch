# Extensible Embedded Transport Consumer Program

Planning baseline: `475bd50f6f9b9f66b95eea814f06b4adb22ede93` (`main`, 2026-09-13; eggfetch 0.1.4)
Status: complete; qualification and documentation closure recorded 2026-09-14

## Objective

Make `eggfetch-core` usable as the single HTTP/TLS engine inside downstream Rust applications that already own their network route or raw byte-stream transport, while preserving eggfetch's existing public behavior and keeping all downstream-specific routing/proxy policy outside eggfetch.

The motivating integration needs an HTTP client that can run over a caller-supplied connection stream, can guarantee that one application-visible attempt is not silently multiplied by Hyper, can bound physical live connections independently from logical request concurrency, and can enforce inactivity guardrails on actual transport I/O. Those requirements are general HTTP-engine capabilities and must be implemented without adding any Eggpool-, Eggress-, provider-, account-, pproxy-, SSH-, Trojan-, Shadowsocks-, or other application-specific concepts to eggfetch.

This is an ownership/control program. It is **not** a binary-size reduction claim. Eggfetch's existing `docs/architecture/embedded-footprint.md` already classifies the current minimal Rust engine as not a footprint win against equivalently scoped reqwest builds. This program must measure downstream/minimal impact after implementation but must not make size reduction an acceptance criterion.

## Research findings

### 1. The core has no public raw-stream dialer seam

The current HTTP/1/2 client is built around crate-private Hyper connector types in `crates/eggfetch-core/src/transport/`. Direct, SNI, resolved-target, UDS, proxy/SOCKS and standard clients are constructed internally. Native callers can influence resolution and socket settings, but they cannot supply an already-connected `AsyncRead + AsyncWrite` stream for a logical destination.

A downstream that already owns a transport therefore has only bad options today: duplicate eggfetch's HTTP/TLS layer, expose crate-private Hyper connector details, or force its transport into the built-in HTTP/SOCKS proxy model. None is an acceptable reusable-engine boundary.

### 2. Logical retry policy and Hyper's implicit retry are distinct

`RetryPolicy` is opt-in and defaults to one logical attempt, but Hyper-util's legacy client has an independent canceled-request/stale-idle-connection retry mechanism. Eggfetch does not currently expose that builder policy. A caller that requires one submitted attempt to equal one actual HTTP transport attempt therefore cannot enforce that invariant through the public API.

The fix must preserve today's default behavior for existing eggfetch callers and add an explicit native control. It must be applied to every Hyper client construction path, not just the standard client.

### 3. `PoolConfig` intentionally does not bound physical sockets

`crates/eggfetch-core/src/pool.rs` explicitly defines `max_connections` as a compatibility alias for logical in-flight request concurrency. Hyper owns the physical connection pool; HTTP/2 may multiplex many logical requests over one TCP connection. Reinterpreting that field as a physical connection cap would be an API regression.

A separate connector-lifecycle admission policy is required for embedded/resource-constrained callers that need a hard bound on live physical connections, including idle pooled connections.

### 4. Existing write/read timeouts do not fully express raw transport inactivity

`Timeout.write` protects streamed request-body production and `Timeout.read` protects response/header/body progress. A buffered request whose peer stops consuming bytes can still require a lower-level socket/transport write guardrail. The current public timeout semantics should not be silently broadened or rewritten.

A separate optional transport-I/O inactivity policy should wrap the fully established Hyper connection so it applies to actual `poll_read`/`poll_write` progress while leaving connect/TLS establishment under the existing connect timeout.

### 5. TLS trust augmentation is not a prerequisite

Eggfetch already supports deterministic custom CA material through `TlsConfig`. Some downstream tests may prefer "default roots plus an extra test root," but that is not required for the production custom-transport boundary and does not justify coupling this program to trust-store redesign. If a concrete general-purpose augmentation use case remains after downstream migration, it can be planned separately.

### 6. Dependency alignment is not a prerequisite

Possible version alignment such as `getrandom` or `webpki-roots` must not be mixed into this work merely to make a dependency tree look smaller. Existing footprint evidence demonstrates that package count is not a reliable binary-size proxy. Dependency changes belong in a separate bounded audit when independently justified.

## Architectural target

The intended layering is:

```text
caller/application policy
    |
    | optional ClientBuilder custom dialer
    v
eggfetch-core
    request/redirect/retry pipeline
    logical pool/concurrency
    Hyper HTTP/1 or HTTP/2
    destination TLS owned by eggfetch
    physical connection admission / transport I/O guardrails
    |
    +--> built-in direct connector
    |
    +--> caller-supplied raw-stream dialer
```

A custom dialer controls only how the byte stream reaches the logical host/port. It does **not** control HTTP Host, URL identity, redirects, cookies/auth origin policy, destination TLS SNI/certificate validation, HTTP retry policy or response decoding. HTTPS remains terminated and verified by eggfetch after the custom stream is established.

## Cross-program invariants

Every child plan must preserve these invariants:

1. Existing callers that do not opt into the new APIs keep current behavior.
2. No existing `PoolConfig` field changes meaning.
3. Existing `Timeout` fields keep their documented request/body semantics.
4. Destination TLS remains owned and verified by eggfetch for HTTPS over a custom dialer.
5. A custom dialer never causes silent fallback to the ordinary direct connector after the dialer has been selected.
6. Logical URL/Host/SNI/certificate identity is never replaced by a dialer's physical route.
7. Built-in HTTP/SOCKS proxy behavior remains unchanged unless a caller explicitly selects the new custom-dialer path.
8. Ambiguous combinations fail before network I/O rather than silently ignoring one routing authority.
9. Explicit eggfetch `RetryPolicy` remains separate from the lower Hyper canceled-request retry control.
10. Physical connection admission is separate from logical request concurrency and remains correct under HTTP/2 multiplexing.
11. Transport I/O inactivity timers reset on real progress and do not reclassify DNS/TCP/TLS establishment away from the connect phase.
12. No new external service, CI matrix, benchmark framework or downstream repository dependency is added to routine eggfetch validation.
13. Public Debug/Display/error behavior must not expose secrets supplied by a custom transport.
14. HTTP/3 remains independent and experimental; this program must not claim custom-dialer or physical-admission coverage for QUIC unless it is explicitly implemented and tested.

## Execution order

### Plan 1 — `custom-dialer-transport-extension.md`

Add a native, object-safe custom dialer abstraction that returns a Tokio-compatible byte stream for a logical host/port. Integrate it below destination HTTP/TLS without exposing Hyper connector internals. Define fail-closed interactions with proxy, UDS, static resolved routing, local binding/socket options, redirects and H3. Add deterministic local tests proving logical Host/TLS identity and no direct fallback.

### Plan 2 — `strict-underlying-transport-attempt-control.md`

Expose the Hyper legacy client's implicit canceled-request retry policy as an explicit native client setting while preserving current default behavior. Centralize application of this builder setting across standard, direct, SNI, resolved, UDS, SOCKS and custom-dialer Hyper clients. Add a deterministic stale-idle-connection regression demonstrating the difference between default and strict one-transport-attempt mode.

### Plan 3 — `physical-connection-admission-and-io-inactivity-guardrails.md`

Add a connector-lifecycle wrapper shared by Hyper-based routes that can independently limit live physical connections and enforce optional transport read/write inactivity timers. The wrapper must acquire admission only for new physical connections, retain the permit for the connection lifetime including idle pooling, preserve connect-phase timing, and report truthful metrics/error phases.

### Plan 4 — `post-extensible-transport-qualification-and-closure.md`

Qualify the new native surface with a tiny external-style Rust fixture using a synthetic custom dialer, run Tier 1/extended/package validation, inspect feature/dependency and minimal-consumer footprint impact, renew exact-SHA compatibility evidence once on a frozen executable tree, and update documentation/plan status without turning downstream migration into an eggfetch responsibility.

Plans 1 and 2 can be implemented independently if they touch distinct builder code, but Plan 3 must account for every Hyper connector route that exists after Plan 1. Plan 4 starts only after executable behavior in Plans 1–3 is complete.

## Public API direction

Exact names may change during implementation, but the native surface should remain small and typed. The expected shape is conceptually:

```rust
pub trait Dialer: Send + Sync + 'static {
    fn dial(&self, target: DialTarget) -> DialFuture<'_>;
}

pub struct DialTarget {
    // logical destination only; no credentials
    host: String,
    port: u16,
}

Client::builder()
    .dialer(...)
    .retry_canceled_requests(false)
    .physical_connection_policy(...)
    .transport_io_timeout(...)
    .build();
```

Do not make callers implement `tower_service::Service<Uri>`, Hyper's `Connection` trait, Rustls handshakes, pool integration or eggfetch internal response-body plumbing. Eggfetch owns the adapter from the small public abstraction into its transport engine.

A custom dialer's runtime error must retain enough source information for an embedding application to distinguish its own transport failures without requiring eggfetch to know application-specific protocols. Prefer source-preserving error plumbing and additive inspection helpers over changing existing public `Error` variant payloads or repurposing proxy-specific error categories.

## Compatibility and versioning constraints

`eggfetch-core` is pre-1.0 but this program should still avoid needless source breakage:

- do not rename/remove public types or methods;
- do not change existing enum variant payloads;
- do not change `PoolConfig.max_connections` semantics;
- do not change existing `Timeout` field semantics;
- do not require a new default Cargo feature for custom dialing;
- do not force Python/CLI users to understand the native custom-dialer surface;
- do not alter HTTPX compatibility defaults merely because the Rust-native API gains stricter controls.

If implementation discovers that a new public error enum variant would break exhaustive downstream matches, prefer source-preserving wrapping plus additive query methods, or document the versioning decision explicitly before proceeding.

## Feature/dependency policy

The custom-dialer abstraction should use Tokio traits and dependencies already present in `eggfetch-core`; adding a proxy/connector framework or `async-trait` solely for ergonomic convenience is not justified. Prefer an explicit boxed future/type alias or another dependency-free object-safe pattern.

Do not add a `custom-transport` micro-feature unless implementation demonstrates a real unavoidable dependency/code-size cost that can be removed when disabled. The minimal HTTP/1 + Rustls profile must remain valid.

## Non-goals

This program does not:

- implement any specific proxy, VPN, SSH, overlay, provider or account router;
- add Eggress or another transport library as an eggfetch dependency;
- expose Hyper connector internals as public API;
- change built-in HTTP/SOCKS proxy semantics;
- migrate any downstream application;
- make custom transport request-scoped in v1 unless a concrete correctness need appears;
- redesign retry policy or status-code retry semantics;
- redefine logical pooling as physical pooling;
- add HTTP/3 custom dialing;
- add MASQUE, CONNECT-UDP or QUIC tunneling;
- redesign TLS trust stores;
- bump unrelated dependencies for aesthetic graph alignment;
- claim smaller binaries.

## Program verification

Each executable child runs focused tests plus the canonical routine gate:

```sh
./scripts/check.sh
```

Before final closure, run:

```sh
./scripts/check.sh extended
./scripts/check.sh package
```

Use existing feature-matrix/MSRV/downstream/compatibility machinery rather than creating a new CI workflow. The verification-policy complexity budget remains authoritative.

## Completion criteria

The program is complete when:

- [x] a native caller can supply a raw-stream dialer without depending on Hyper internals;
- [x] HTTPS over that dialer preserves eggfetch-owned SNI/certificate validation;
- [x] custom-dialer failure never falls back to direct I/O;
- [x] callers can disable Hyper's implicit canceled-request retry while the default remains unchanged;
- [x] all Hyper client construction paths honor that setting;
- [x] callers can independently bound live physical connections without changing logical pool semantics;
- [x] callers can independently bound actual transport read/write inactivity without changing existing body timeout semantics;
- [x] focused regressions cover route conflicts, stale pooled connections, connection admission, idle-pool permit retention and stalled transport I/O;
- [x] a tiny external-style fixture proves the public API requires no crate-private types;
- [x] minimal/default dependency and binary effects are measured rather than inferred;
- [x] Tier 1, extended and package validation pass;
- [x] exact-SHA compatibility evidence is renewed after executable work freezes;
- [x] plan index and architecture/native Rust documentation accurately describe the final surface.

## Closure evidence (2026-09-13)

Implementation commit: `af03f006f377979550ee6cb96a30b28193c8708d`.

- `git pull --rebase` completed before implementation.
- Tier 1 passed locally with the repository's documented single-job/resource-stabilized settings.
- The corrected feature matrix passed the no-default, HTTP/1, and HTTP/1 + Rustls core profiles.
- The pinned HTTPX 0.28.1 compatibility profile passed `1870` tests on the clean implementation commit; the isolated reference SOCKS/TLS retry after one transient full-suite failure also passed.
- Tier 3 package validation passed on the clean commit, including crate dry-runs, package listings, release wheel build, smoke tests, and content validation.
- `qualification/embedded-custom-dialer/` compiled and ran using only public `eggfetch-core` exports.
- Minimal dependency inspection showed no new runtime dependency and no proxy/H2/H3/compression/cookie/multipart capability pulled into the HTTP/1 profiles by these controls.

The previously retained stale-idle, lifecycle-matrix, and control-using
footprint follow-ups are closed by the final evidence below. The existing
footprint conclusion remains authoritative: eggfetch is not presently a
footprint win.

## Final closure evidence (2026-09-14)

The executable/test/fixture freeze is `43c68bd1bcff45301fc8b6b163b6b6e06d98a786`.
The final qualification record is in
`plans/post-extensible-transport-qualification-and-closure.md`.

- The deterministic stale-idle tests now cover default transparent retry,
  strict failure, explicit retry, and unreplayable-body behavior; lifecycle
  metrics tests cover successful reuse/idle permit retention and independent
  physical admission across origins.
- Minimal HTTP/1 + Rustls feature trees remain free of optional proxy, H2, H3,
  compression, cookie, and multipart dependencies. No dependency was added
  for the new native controls.
- The final x86_64 footprint run remains **not a footprint win** against
  aligned reqwest Rustls profiles. The control-using fixture is reported
  separately because reqwest has no equivalent profile.
- Tier 1, extended, and package validation passed. Extended validation records
  only the documented unavailable Node artifact, unsupported local Rust 1.80 /
  old Cargo MSRV environment, and absent downstream artifact skips.

The program is complete as a general-purpose eggfetch engine capability. No
downstream migration, protocol-specific routing model, or HTTP/3 graduation
is part of this closure.
