# Native URI Dependency Separation

Planning baseline: `b0a09eed95b88199db2d0188ea68bf100c43b50a` (`main`, 2026-09-17; `eggfetch-core` 0.1.6)
Status: planned

Depends on: `native-pool-map-dependency-reduction.md`

## Objective

Allow low-level/native eggfetch consumers to use the HTTP/TLS/pooling/custom-dialer engine without pulling the `url` -> `idna` -> ICU dependency closure, while preserving the existing high-level string/URL request API and its semantics for normal/default builds.

This is a feature-boundary and URI-ownership correction, not a replacement URL parser. The native API already receives a validated `http::Request<B>` with an `http::Uri`; it should not have to serialize that URI and parse it again as `url::Url` merely to recover scheme, host and port.

The work must remain general-purpose. Do not add Eggpool-, provider-, proxy-product-, or downstream-specific types or features.

## Current state on the planning baseline

`crates/eggfetch-core/Cargo.toml` declares `url = "2"` unconditionally.

The high-level eggfetch API legitimately depends on `url::Url` for user-facing string URL parsing and policy that needs URL semantics: request building, redirects, cookies, proxy/no-proxy matching, auth/origin checks and other high-level behavior. That use should remain.

The low-level native API does not inherently need those semantics. `Client::execute_http_body()` accepts `http::Request<B>`, but `pipeline::send_native_http_body()` currently does:

```rust
let logical_url = url::Url::parse(&request.uri().to_string())?;
```

and then uses that `Url` for:

- HTTP/HTTPS + authority validation;
- effective-port calculation;
- resolved-target port validation;
- TLS scheme selection;
- logical origin key construction;
- wire URI reconstruction / target override;
- resolved-route cache/client lookup.

`OriginKey` also accepts `url::Url` directly even though its stored state is only scheme/host/effective port plus optional proxy identity.

For a native caller that already constructed an absolute `http::Uri`, this reparse is unnecessary and forces the IDNA/ICU closure into minimal embedding builds.

## Design principles

1. Preserve the existing high-level `Client::get/post/request`, `RequestBuilder`, redirect, cookie, proxy and URL behavior.
2. Do not hand-roll a general URL parser or IDNA implementation.
3. Native execution should derive only the transport facts it requires from `http::Uri`.
4. Existing Cargo feature combinations must not silently lose APIs if they currently work.
5. Cargo features should remain additive. Do not introduce a subtractive `native-only` feature whose presence removes APIs activated by another dependency.
6. Do not split or duplicate the HTTP transport engine solely for dependency accounting.
7. Do not create a second client type with independent pooling/TLS behavior.
8. Keep `Client::execute_http_body()` and `NativeHttpService` on the same underlying `Client`/transport implementation.

## Feature-boundary strategy

The preferred compatibility-preserving approach is to introduce lower-level transport protocol features and keep the existing public `http1`/`http2` features as compatibility/high-level aliases.

Illustrative shape:

```toml
[features]
default = ["http1", "tls-rustls", "tls-native-roots"]

# Existing feature names retain their current user-visible behavior.
http1 = ["native-http1", "high-level-url"]
http2 = ["native-http2", "high-level-url"]

# New low-level slices for http::Request/http::Uri consumers.
native-http1 = ["hyper/http1", "hyper-util/http1", "hyper-rustls?/http1"]
native-http2 = ["dep:h2", "hyper/http2", "hyper-util/http2", "hyper-rustls?/http2"]

high-level-url = ["dep:url"]
```

Exact feature names may be adjusted if a clearer repository-consistent naming scheme is found, but the compatibility property is normative:

> A consumer enabling the existing `http1` or `http2` feature must continue to receive the existing high-level API and behavior. A consumer explicitly selecting the new native transport feature may omit the high-level URL layer.

Do not make `url` optional by simply removing it from existing `http1`/`http2` feature combinations; that would break current `default-features = false` users who use the high-level API.

`http3`, built-in proxy routing, cookies and other high-level facilities may continue to imply the high-level URL feature where required. The native frame API already rejects built-in proxy and HTTP/3 routes where unsupported; do not expand those semantics as part of this plan.

If implementation proves that compatibility-preserving feature decomposition requires duplicating the transport engine or an unreasonably broad public API fork, stop and document the blocker before creating a new crate. A new transport subcrate is a fallback design, not the starting point.

## 1. Introduce a native origin representation

Add a small crate-private representation derived from `http::Uri`, for example:

```rust
struct HttpOrigin {
    scheme: HttpScheme,
    host: String,
    port: u16,
}

enum HttpScheme {
    Http,
    Https,
}
```

Naming is implementation-selected. Required semantics:

- only `http` and `https` schemes are accepted;
- an authority/host is required;
- effective default port is 80 for HTTP and 443 for HTTPS;
- explicit ports are preserved;
- IPv4, bracketed IPv6 and DNS hostnames are handled correctly;
- empty host is rejected;
- URL userinfo remains rejected;
- path/query do not participate in origin identity;
- no IDNA transformation is invented in the native path.

The native caller owns construction of the `http::Uri`. If a Unicode hostname cannot be represented by the `http` crate's URI type, the caller must provide an appropriate ASCII/punycode URI. That is preferable to silently giving native execution a second URL-normalization policy.

Add focused tests for:

- `http://example.com` -> port 80;
- `https://example.com` -> port 443;
- explicit non-default ports;
- IPv4;
- IPv6 with and without explicit port;
- missing scheme;
- missing authority;
- unsupported schemes;
- userinfo if `http::Uri` accepts such an authority form;
- malformed authority / invalid port.

Do not weaken validation relative to the current `url::Url` reparse.

## 2. Make `OriginKey` component-based

Promote the existing component constructor concept to production use. `OriginKey` should not require `url::Url` merely to store scheme, host and effective port.

Preferred shape:

```rust
impl OriginKey {
    fn from_origin(origin: &HttpOrigin) -> Self;
}
```

or a similarly narrow component constructor.

The high-level path may still construct an `OriginKey` from `url::Url`, but that adapter should live behind the high-level URL feature and reduce immediately to the same canonical component representation.

Keep proxy identity/tunnel-mode keying unchanged for the built-in high-level proxy route.

## 3. Stop reparsing native request URIs through `url::Url`

Refactor `send_native_http_body()` so its first validation step works directly from `request.uri()`.

Do not use:

```rust
request.uri().to_string()
url::Url::parse(...)
```

in the native transport path after this change.

Use the parsed native origin for:

- scheme validation;
- host/SNI identity;
- effective-port checks;
- `ResolvedTarget` port validation;
- origin pool keying;
- TLS-required selection;
- custom-dialer logical target;
- resolved-route cache identity where applicable.

Preserve all current error categories and fail-closed behavior. Exact error strings may be adjusted only where required by the new parser boundary; avoid gratuitous diagnostic churn.

## 4. Add a native wire-URI helper

`prepare::resolve_request_uri()` currently accepts `url::Url` because the high-level path starts from that type. Do not force the native path through it.

Add a narrow `http::Uri`-based helper for native requests that:

- returns the original absolute URI when no `TransportHints::target` override is present;
- when `target` is present, validates the target using the existing request-smuggling guard;
- replaces only `path_and_query` while preserving the original scheme/authority used for connector routing and TLS;
- preserves `*` target support if currently supported;
- does not alter Host header policy;
- does not accept C0/DEL/leading/trailing-whitespace target bytes.

Share the target-validation primitive with the high-level helper rather than duplicating security checks.

## 5. Decouple native transport types from the high-level request module

`NativeRequestOptions`, `ResolvedTarget` and `TransportHints` currently live alongside high-level URL request types in `request.rs`. To compile the low-level slice without `url`, move only the protocol-neutral native/shared transport controls to a small module if necessary.

Possible shape:

```text
request.rs              # high-level URL request / RequestBuilder
transport_hints.rs      # ResolvedTarget, TransportHints, NativeRequestOptions
```

The root re-export names must remain unchanged for existing users:

```rust
pub use ...::{NativeRequestOptions, ResolvedTarget, TransportHints};
```

Do not force a public module-path migration just to accomplish the internal split.

`ProxyOverride`, high-level `Request`, `RequestBuilder` and URL-auth/request-policy state may remain behind the high-level URL feature.

## 6. Preserve one Client implementation

`Client` and `ClientBuilder` remain the transport owners.

Under a native transport feature without the high-level URL feature:

- `Client::execute_http_body()` must compile and work;
- `Client::native_service()` must compile and work;
- TLS builder/config, custom `Dialer`, `PhysicalConnectionPolicy`, `TransportIoTimeout`, pool config/metrics and transport diagnostics needed by native callers must remain available;
- high-level string-URL constructors/methods may be cfg-gated because that feature slice explicitly opted out of the high-level URL layer.

Under existing `http1`/`http2` features and under default features, all current high-level methods remain present.

Do not create a parallel `NativeClient` with a separate pool or connector stack.

## 7. Audit all `url` references before making the dependency optional

Use repository search plus compile-feature tests to classify every `url::Url` use into one of:

- genuinely high-level URL/request policy;
- shared transport code that should consume `HttpOrigin`/components instead;
- HTTP/3 or built-in proxy behavior that legitimately requires URL semantics;
- tests/examples that should follow their production owner.

Do not gate a module merely to make compilation pass if native transport still semantically depends on it.

Likely files requiring review include, at minimum:

- `src/client.rs`;
- `src/request.rs`;
- `src/pipeline/mod.rs`;
- `src/pipeline/prepare.rs`;
- route/cache helpers receiving `url::Url`;
- `src/pool.rs`;
- redirect/cookie/proxy/redaction modules;
- HTTP/3 authority/origin helpers;
- tests and examples.

The implementation should reduce shared transport interfaces to scheme/host/port/URI components rather than propagating conditional `Url` parameters through low-level code.

## 8. Cargo feature/dependency closure

Once production code supports the split:

- change `url = "2"` to an optional dependency;
- wire it only to the high-level URL feature(s);
- preserve the existing default feature behavior;
- ensure existing `http1`/`http2` public feature names still activate the high-level URL API;
- ensure the new native HTTP feature slice compiles without `url`;
- keep `percent-encoding` only if a remaining native/shared owner genuinely needs it; audit separately rather than assuming it belongs with `url`;
- do not disable security or TLS features to make the dependency graph look smaller.

After the preceding pool-map plan, the intended minimal native slice should contain neither `dashmap` nor `url`.

## Security and semantic parity tests

Native-path tests must prove parity for:

1. absolute HTTP/HTTPS URI requirement;
2. unsupported scheme rejection;
3. missing authority rejection;
4. userinfo rejection;
5. effective-port validation for `ResolvedTarget`;
6. logical hostname preservation into a custom `Dialer`;
7. HTTPS SNI/hostname verification using the logical host;
8. custom SNI override behavior;
9. Host header behavior;
10. target/path override request-smuggling validation;
11. IPv4 and IPv6 authorities;
12. connection-pool origin isolation across scheme/host/port;
13. resolved-route cache reuse/isolation;
14. custom-dialer fail-closed rules;
15. no redirects, cookies, logical retries, auth injection or decompression on the native frame API;
16. DATA/trailer streaming and pool-lease recovery on EOF/error/drop.

Existing high-level compatibility tests must remain green, including redirects, IDN/URL behavior, proxies, cookies and HTTPX compatibility. The point is to remove URL policy from the low-level feature slice, not to change URL policy.

## Feature-matrix tests

Add/adjust compile coverage for at least:

```text
existing default features
existing no-default + http1 + tls-rustls
existing no-default + http2 + tls-rustls
new no-default + native-http1 + tls-rustls
new no-default + native-http2 + tls-rustls (if supported)
new native-http1 + custom Dialer fixture
```

The exact command names should follow the repository's existing feature-matrix tooling.

The two existing feature combinations must retain their current high-level surface. The new native combinations are the only ones expected to omit it.

## Dependency/footprint proof

Use a minimal external-style fixture that performs a real native request construction and links the relevant APIs; do not rely only on `cargo metadata` for a library that the linker may prune differently.

For the new minimal native HTTP/1 + Rustls/WebPKI slice, record:

- resolved package count;
- enabled eggfetch features;
- `cargo tree -i url` result (expected: package absent for the fixture);
- `cargo tree` evidence that `idna` and ICU packages are absent;
- `cargo tree -i dashmap` result after the preceding plan;
- stripped or release artifact size under the same target/toolchain/profile as the before measurement.

Compare against an equivalent current-baseline native fixture. Do not compare a native-only final build to a high-level/default baseline.

Binary-size reduction is expected if the URL/ICU closure is truly absent, but no arbitrary percentage is an acceptance criterion. If the packages disappear and the binary does not materially shrink, record that result rather than distorting the design.

Do not add a permanent byte-size CI gate.

## Documentation

Update feature documentation to make the distinction explicit:

- `http1` / `http2`: existing high-level-compatible feature surface;
- native transport feature(s): low-level `http::Request`/`http_body::Body` embedding surface;
- high-level URL semantics remain backed by `url` and are intentionally absent from the minimal native slice.

Document that native callers provide a valid `http::Uri` and therefore own any IDNA/punycode conversion needed before construction.

Do not market the native slice as a generic URL client; it is the transport-level API.

## Validation

Use the repository's current verification policy. At minimum:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo doc --workspace --all-features --no-deps
```

Run the repository's Tier 1, extended, package and exact-SHA compatibility gates required for core public feature/API changes.

Because this changes Cargo features and public API availability in new slices, package/dry-run verification is required before closure.

## Stop conditions

Stop and document rather than forcing the design if any of these occur:

- existing `http1`/`http2` feature users would have to add a new feature to retain current high-level APIs;
- eliminating `url` from the native slice requires a second transport implementation;
- the native path would need a home-grown parser for general URL, IDNA, redirect, cookie or proxy semantics;
- security validation would become weaker than the current `url::Url`-backed path;
- HTTP/HTTPS Host/SNI/effective-port identity becomes ambiguous with `http::Uri` alone;
- feature unification would cause the proposed lower-level slice to behave subtractively or unpredictably.

If a blocker is real, evaluate a separate low-level transport crate as a follow-up design with explicit maintenance-cost analysis rather than silently widening this plan.

## Completion criteria

This plan is complete when:

- the native `http::Request<B>` path no longer reparses its URI through `url::Url`;
- a canonical native origin representation supplies scheme/host/effective-port facts;
- `OriginKey` and native resolved-route identity do not require `url::Url`;
- `url` is optional in `eggfetch-core`;
- current `http1`/`http2` and default feature behavior remains compatible;
- an explicitly selected low-level native HTTP feature slice builds without `url`, `idna` or ICU dependencies;
- after the pool-map plan, that slice also builds without `dashmap`;
- native URI/TLS/SNI/Host/resolved-target/security tests are green;
- high-level URL/redirect/cookie/proxy compatibility remains green;
- dependency and artifact measurements are recorded under comparable conditions;
- repository qualification and packaging gates are green.

Record the executable freeze and measurements in this plan at closure. Any downstream adoption or version bump belongs in the downstream repository after an upstream release is available.