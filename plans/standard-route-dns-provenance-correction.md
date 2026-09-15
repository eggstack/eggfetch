# Standard Route DNS Provenance Correction

Planning baseline: `76d3fb78ad346aaacb617d25897fa5346a043a47` (`main`, 2026-09-15)
Status: ready for implementation
Related completed work: `native-request-failure-introspection.md`
Motivating downstream review: Gregg system-monitor client integration. Gregg is requirements evidence only; no Gregg type, endpoint model, status mapping, feature flag, or adapter belongs in eggfetch.

## Objective

Complete the existing native detailed-request failure surface so the ordinary standard Hyper HTTP/HTTPS route can report `NetworkFailureKind::Dns` when name resolution itself fails, without parsing error strings, changing the public `Error` enum, changing `Error::kind()`, changing ordinary `send()` behavior, replacing Hyper's standard TCP connector behavior, or adding downstream-specific policy.

The completed native request-failure introspection work deliberately reports standard-route DNS failures only as generic `Connect` because the resolver provenance has already become opaque by the time `map_send_error_with_context()` sees the final Hyper client error. That behavior is truthful, but the current transport construction has an earlier typed seam available: Hyper-util's `HttpConnector` can be constructed with a caller-supplied resolver service. Eggfetch should use that seam to preserve resolver provenance privately before Hyper's connector layer collapses it.

This is a narrow corrective pass on the existing detailed-failure implementation, not a new diagnostics subsystem and not another transport program.

## Why eggfetch owns this

`eggfetch-core` now owns the generic native request-failure vocabulary (`RequestFailure` and `NetworkFailureKind`) and the transport-boundary plumbing that populates it. Standard DNS resolution is part of eggfetch's HTTP transport implementation, so retaining typed resolver provenance is also an eggfetch responsibility.

Keeping standard-route DNS classification in each downstream would force consumers to repeat platform-specific resolver-code checks and/or message matching after the transport has already discarded useful type information. That defeats the purpose of the detailed native error surface and creates duplicated maintenance outside the HTTP client.

The correction must remain generic. It must be equally useful to any native Rust caller that needs to distinguish name-resolution failure from refusal or another connect failure.

## Current repository truth to preserve

1. `NetworkFailureKind::{Dns, ConnectionRefused, Connect}` and `RequestFailure` already exist as additive native Rust APIs.
2. `Client::send_detailed()` and `RequestBuilder::send_detailed()` allocate a small private request-local failure context only when the caller opts into detailed failures.
3. `map_send_error_with_context()` performs a bounded typed source-chain walk and currently records typed direct-connector provenance, standard-route connection refusal, or generic connect failure.
4. The private direct/resolved/SNI connector already preserves DNS provenance through `DirectConnectError`; this plan must not duplicate or replace that path.
5. The standard HTTP/HTTPS client currently uses Hyper-util's normal `HttpConnector` (directly for cleartext and under `hyper-rustls` for TLS), wrapped by eggfetch's existing connect-timeout and lifecycle connectors.
6. Hyper-util supports constructing `HttpConnector` with a resolver service. This lets eggfetch retain resolver failure provenance while leaving Hyper-util responsible for address selection, TCP establishment, Happy Eyeballs/address racing, and normal connector behavior.
7. Existing public standard-route connection failures remain represented by the existing `Error::HyperClient` path. Detailed metadata must not change that compatibility surface.
8. Proxy, UDS, HTTP/3, custom-dialer, direct/resolved/SNI, retry, redirect, pool, timeout, TLS, body, Python, CLI, HTTPX/HTTPX2, FFI, Node, native-body, and Tower behavior are outside the semantic scope except for regression verification.

## Scope guardrails

This plan may add crate-private resolver/error types and the minimum private connector construction changes needed to preserve standard resolver provenance.

It must not:

- add a public resolver trait, resolver configuration API, DNS cache, DNS client, or DNS dependency;
- replace Hyper-util's standard TCP connector with eggfetch's `DirectConnector` merely to obtain diagnostics;
- implement a second address-selection or Happy Eyeballs algorithm;
- parse `Display`/`Debug` strings or inspect localized resolver text;
- add libc/WinSock resolver-code heuristics to the standard route when typed resolver provenance is already available at the resolver seam;
- change the shape, exhaustiveness, display, or `kind()` values of the existing public `Error` enum;
- add a new `NetworkFailureKind` variant;
- change ordinary `Client::send` / `RequestBuilder::send` signatures or semantics;
- change timeout, retry, redirect, pooling, TLS, proxy, HTTP version, or connection-reuse policy;
- make ordinary non-detailed requests allocate detailed-failure state;
- add Gregg-specific names, error categories, or compatibility adapters;
- add a new dependency unless the current dependency graph genuinely cannot express the small resolver wrapper;
- raise the current Rust 1.80 MSRV;
- add a new CI matrix, scheduled qualification, or permanent evidence subsystem.

## 1. Add a private standard-resolver provenance wrapper

Add a crate-private module/type near the standard connector implementation (for example under `transport`) that wraps Hyper-util's ordinary `GaiResolver` or an equivalent generic resolver service.

A representative internal model is:

```rust
pub(crate) struct ClassifyingResolver<R> {
    inner: R,
}

pub(crate) struct ResolverFailure {
    source: std::io::Error,
}
```

Exact names may follow repository conventions. The important contract is:

- successful resolution is delegated unchanged;
- resolver failures are wrapped in a private, downcastable marker before entering `HttpConnector`;
- the original resolver error remains available as `std::error::Error::source()`;
- `Debug`/`Display` do not introduce secrets or application data beyond the error text the existing connector path already exposes;
- the wrapper is `Clone`/`Service<Name>` compatible with the standard `HttpConnector` requirements;
- both readiness errors and call/future errors are mapped if the resolver service can surface failure at either boundary;
- successful lookups should not require a heap allocation merely because provenance is enabled.

Prefer a small concrete future wrapper using current dependencies (for example the already-present `pin-project-lite`) rather than boxing every resolution future. If the pinned Hyper-util resolver API already supplies a concrete future that can be mapped without allocation, use the simplest zero/low-overhead form.

Do not broaden the marker into a public DNS taxonomy. The only fact needed here is: resolution failed before a destination address was produced.

Acceptance:

- [ ] Resolver success delegates addresses and ordering unchanged.
- [ ] Resolver failure carries a private typed marker and preserves the original source.
- [ ] No error-string matching is used.
- [ ] No new public API is introduced.
- [ ] The success path does not add avoidable heap allocation.

## 2. Install the wrapper only under the standard Hyper connector

Update the private standard connector construction so its `HttpConnector` uses the classifying resolver.

### Plain HTTP / non-Rustls build

Construct `HttpConnector::new_with_resolver(...)` with the wrapped default resolver and preserve the current HTTP-only enforcement and all existing wrapper layers.

The final connector must still pass through the existing:

1. connect-phase timeout wrapper;
2. physical lifecycle/admission wrapper;
3. Hyper legacy client builder and its existing retry-canceled-request and idle-pool policy.

### HTTP/HTTPS with Rustls

Do not let `HttpsConnectorBuilder::build()` silently create a fresh default `HttpConnector`, because that would bypass the provenance wrapper.

Instead:

1. construct the wrapped `HttpConnector` explicitly;
2. configure the underlying connector for the same HTTPS-or-HTTP behavior required by the current Rustls route (including the correct `enforce_http` setting for an HTTPS-capable connector);
3. pass that connector through the pinned `hyper-rustls` builder's connector-wrapping API (currently exposed by the 0.27 line as `wrap_connector(...)`; confirm the exact API against the repository's locked version during implementation);
4. preserve the current HTTP/1 and HTTP/2 enablement/ALPN policy exactly;
5. preserve the existing `ConnectTimeout`, `LifecycleConnector`, Hyper client builder, idle pool limits, and canceled-request retry configuration.

The resulting standard client should differ only in its ability to preserve a resolver failure marker. Hyper-util must remain responsible for TCP connection establishment and its normal address fallback/racing semantics.

Do **not** make `DirectConnector` the default route. Its current purpose is resolved/SNI/socket-option/local-address routing; changing the normal route to it would turn a diagnostic correction into a transport-policy change.

Acceptance:

- [ ] Ordinary standard HTTP requests still use Hyper-util's standard `HttpConnector` connection behavior.
- [ ] Ordinary standard HTTPS requests still use the same Rustls policy, HTTP version policy, ALPN behavior, pooling and timeout wrappers.
- [ ] No route-selection precedence changes.
- [ ] Direct/resolved/SNI, proxy, UDS, custom-dialer and H3 construction is untouched except for compile/test adjustments that are mechanically required.

## 3. Recognize the resolver marker in detailed failure mapping

Extend the existing bounded source-chain logic in `transport::direct::map_send_error_with_context()` (or the common private helper it delegates to) to recognize the new private `ResolverFailure` marker before falling through to generic connection classification.

When the marker is present and a detailed failure context exists:

```text
ResolverFailure -> NetworkFailureKind::Dns
```

Then continue returning the same legacy/public standard-route error that the request would have returned without detailed diagnostics. For the current standard Hyper route that means the public error category remains `Error::HyperClient`; `send_detailed()` adds only the `Dns` metadata.

Preserve the existing precedence rules:

- nested eggfetch request/body/timeout errors remain their original `Error`;
- `DirectConnectError` remains authoritative for direct/resolved/SNI routing;
- standard typed `io::ErrorKind::ConnectionRefused` remains refusal;
- another Hyper connect error remains generic `Connect`;
- routes with no structured evidence remain generic/unknown rather than guessed.

The bounded source walk must remain bounded. Do not add a second independent source-chain traversal if the existing one can recognize the resolver marker directly.

Acceptance:

- [ ] Standard resolver failure produces `Some(NetworkFailureKind::Dns)` from `send_detailed()`.
- [ ] The underlying public `Error` and `Error::kind()` are unchanged from ordinary `send()`.
- [ ] Standard connection refusal still produces `ConnectionRefused`.
- [ ] Other standard connect failures still degrade to `Connect`.
- [ ] Ordinary `send()` remains free of detailed-failure context allocation.

## 4. Add deterministic resolver-provenance tests

Do not test this feature by depending on public DNS, `.invalid` behavior, local resolver configuration, internet reachability, or localized OS error strings.

Add focused private/unit coverage for the resolver seam:

1. a synthetic resolver success returns the same address set/order through the classifying wrapper;
2. a synthetic resolver failure becomes the private typed resolver marker and retains the original error as a source;
3. readiness failure, if supported by the resolver trait boundary, is marked consistently;
4. the standard failure classifier recognizes the marker as `Dns` without inspecting text;
5. the marker does not convert an unrelated I/O/connect error into DNS.

Add a deterministic client-level or near-client-level regression that exercises the actual **standard connector path**, not the direct connector, with a synthetic failing resolver where practical. Keep any resolver injection seam crate-private or `#[cfg(test)]`; do not expose a public testing/configuration API merely to make the test convenient.

If the concrete standard-client construction makes a full client-level synthetic resolver fixture disproportionately invasive, extract the smallest private standard-connector builder/helper that can be instantiated with a test resolver while production continues to supply only the wrapped default `GaiResolver`. The test must still prove that the production connector composition cannot accidentally bypass the wrapper.

Retain/regress the existing detailed tests:

- standard loopback refusal remains `ConnectionRefused`;
- direct/resolved refusal remains `ConnectionRefused` with the same public `connect` error kind;
- timeout phase behavior remains unchanged;
- detailed success remains ordinary response semantics;
- retry/fallback success does not expose stale failure metadata;
- ordinary requests still have no detailed context.

Tests must run without external network dependencies.

Acceptance:

- [ ] Standard-route DNS provenance is covered without public DNS access.
- [ ] Tests would fail if the Rustls standard connector accidentally returned to an internally-created unwrapped `HttpConnector`.
- [ ] Existing direct connector and refusal classification tests remain green.
- [ ] No platform-specific expected error message is asserted.

## 5. Update route-coverage documentation and changelog

Update current normative documentation, not the historical closure record of the already-completed introspection plan.

At minimum review/update:

- `docs/reference/errors.md` — standard Hyper route now supports typed DNS and refusal provenance for detailed sends; remove the statement that its resolver is opaque to eggfetch;
- `docs/rust/guide.md` — ensure the detailed-failure example/description reflects standard-route DNS support and still treats absence of metadata as unknown;
- `docs/architecture/core-engine.md` and any current architecture/error-boundary text that describes detailed-failure route coverage;
- `.skills/rust-development.md`, `.skills/documentation.md`, `AGENTS.md`, or other contributor guidance **only where they currently encode the old standard-DNS limitation**;
- `CHANGELOG.md` `[Unreleased]` — record that native detailed standard-route failures can now preserve DNS provenance. Do not imply a new public type or broader DNS subsystem.

Leave `plans/native-request-failure-introspection.md` as a truthful historical record of the implementation state at its closure SHA. This corrective plan becomes the record of the later coverage improvement.

Documentation must continue to state that `NetworkFailureKind` is evidence-backed and route-dependent. Do not claim DNS/refusal subtype coverage for proxy, UDS, H3 or custom dialer routes unless separate structured evidence actually exists.

## 6. Dependency, footprint and MSRV constraints

No new dependency is expected. The required service/future/error machinery should be expressible using the standard library plus dependencies already present in `eggfetch-core` (`hyper-util`, `tower-service`, and, if useful, `pin-project-lite`).

After implementation inspect:

```sh
cargo tree -p eggfetch-core
cargo tree -p eggfetch-core -e features
cargo tree -p eggfetch-core -d
```

Record any dependency-tree change. A feature enabled on an existing dependency still counts as a dependency/footprint change and must be justified.

Keep the workspace Rust floor at 1.80 and exercise the existing MSRV path through the repository's extended validation. Do not raise MSRV for this correction.

This plan makes no binary-size claim. If connector generic types change code size measurably, record the observation rather than optimizing around an unmeasured assumption.

Acceptance:

- [ ] No new dependency unless explicitly justified in the closure record.
- [ ] No unintended feature activation.
- [ ] Rust 1.80 policy remains unchanged.

## 7. Validation and exact-SHA compatibility requalification

This changes executable `eggfetch-core` connector/error plumbing, so it invalidates the current exact-SHA compatibility binding under repository policy.

During implementation run focused checks for each modified module and the native detailed-failure tests. Before closure run the repository's existing gates:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Also run the relevant all-feature Rust checks/clippy/fmt/doc checks used by current repository policy and the current deterministic standard HTTP/HTTPS tests.

Follow the live compatibility ledger/process exactly. If current policy still requires frozen executable requalification after a core change:

1. freeze the final executable/test tree;
2. run the existing HTTPX 0.28.1 and HTTPX2 2.12.0 exact-SHA qualification procedure, including the required repeat count/oracles already defined by the repository;
3. update both compatibility profiles and the live status ledger together;
4. keep subsequent closure-only edits documentation/profile/plan-only.

Do not create a new compatibility harness or CI matrix for this correction.

## 8. Closure record

When implementation is complete, amend this plan with:

- final executable SHA;
- exact private resolver-wrapper and resolver-marker names;
- the final standard HTTP and HTTPS connector composition;
- proof that the public `Error`/`Error::kind()` and ordinary send APIs did not change;
- focused resolver, refusal, timeout and detailed-failure test results;
- Tier 1, extended and package results;
- dependency/feature-tree delta;
- MSRV result or the repository's already-supported truthful optional-skip record;
- compatibility-profile/freeze SHA and repeated qualification results when required by the live process;
- confirmation that standard-route DNS classification uses typed resolver provenance and no text matching;
- confirmation that Hyper-util remains responsible for standard TCP/address-selection behavior;
- documentation/changelog files updated.

Once closed, this plan becomes a historical implementation record under the normal plan-index rules.

## Exit criteria

- [ ] Standard HTTP and HTTPS detailed sends can classify a typed resolver failure as `NetworkFailureKind::Dns`.
- [ ] The same failure still returns the same legacy/public `Error` category and `Error::kind()` as ordinary `send()`.
- [ ] Standard refusal and generic-connect behavior remain unchanged.
- [ ] Hyper-util's standard connector still owns TCP connection/address fallback behavior; eggfetch has not substituted `DirectConnector` as the default route.
- [ ] No DNS message parsing, platform resolver-code heuristic, public resolver API, or downstream-specific adapter is introduced.
- [ ] Non-detailed callers still allocate no request-failure diagnostics state.
- [ ] Proxy/UDS/H3/custom/direct route semantics are unchanged except for truthful existing route-specific classifications.
- [ ] No new dependency or MSRV increase is introduced without explicit, reviewed justification.
- [ ] Current documentation and `[Unreleased]` changelog describe the corrected standard-route coverage without overstating other routes.
- [ ] Tier 1, extended, package, dependency/MSRV and applicable exact-SHA compatibility checks pass under existing repository policy.
