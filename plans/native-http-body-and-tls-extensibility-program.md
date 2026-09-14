# Native HTTP Body and TLS Extensibility Program

Planning baseline: `45c08e0e7587eb1e8f713d49e6f7902478c27a35` (`main`, 2026-09-14; eggfetch 0.1.4)
Program opened: 2026-09-14
Motivating downstream: Synvoid integration evaluation, but every change in this program must remain generally useful to native Rust consumers and must not introduce a Synvoid-specific API.
Status: complete; closed on executable/test/fixture freeze `fdfe060`

## Closure record

Implemented in `fdfe060` (`Add native HTTP body and TLS extensibility`), with
the native frame-body tests and external AWS-LC/private-PKI fixture included
in that freeze. The exact-SHA closure gates passed on the clean tree:

- Tier 1, extended validation, and package validation passed;
- three sequential full compatibility runs each passed 1,870 tests with 26
  non-failing warnings and no skips/xfails/failures;
- explicit-provider, additional-root, frame/trailer, cancellation, and
  read/write-timeout focused tests passed;
- known limitations remain unchanged: HTTP/3 is experimental, Rust 1.80 is
  unavailable for the current dependency resolution, the Node native artifact
  is not built, and the optional downstream artifact manifest was absent.

Post-freeze changes are limited to this plan/qualification documentation and
compatibility-profile metadata.

## Objective

Extend `eggfetch-core` so transport-oriented native Rust consumers can reuse eggfetch as their HTTP/TLS engine without rebuilding body transport, TLS-provider policy, or private-PKI trust composition outside the crate.

The program has three concrete product goals:

1. provide an additive `http_body::Body` interoperability path that can carry arbitrary streaming request bodies into eggfetch and return a frame-preserving response body without changing the existing `RequestBody` / `ResponseBody` high-level API;
2. make Rustls `CryptoProvider` selection explicit per `TlsConfig` and provider-neutral, including client-certificate key loading, instead of hard-coding ring when no process default exists and inside the mTLS resolver;
3. allow additional private trust anchors to augment the selected base trust store while preserving the existing replacement semantics of `ca_certificate_*` and `TrustStore::Custom`.

This is an engine-extensibility program, not a downstream adapter. The resulting APIs should be useful to reverse proxies, gateways, service meshes, middleware, custom-network clients, test harnesses, enterprise/private-PKI clients, compliance-sensitive applications, and other native Rust embedders even if Synvoid never migrates.

## Why this program exists

The completed embedded-client and extensible-transport programs made eggfetch suitable for consumers that need feature-minimal builds, pinned destinations, caller-owned dialing, explicit physical-connection admission, established-I/O inactivity guards, and strict control over Hyper's stale-idle retry. Those improvements intentionally left several higher-level boundaries unchanged.

Current source inspection on the planning baseline shows three remaining generic limitations:

### 1. Public request/response streaming is byte-stream oriented rather than HTTP-frame oriented

`RequestBody::Stream` is a `Stream<Item = Result<Bytes>>`. `RequestBody::into_http_body()` converts each chunk to `Frame::data`, so callers cannot provide an arbitrary `http_body::Body` without first flattening it to bytes. On the response side, `wrap_incoming()` converts `hyper::body::Incoming` into `BoxBytesStream`; data frames survive, trailers are copied into `SharedTrailers`, and any future non-data/non-trailer frame currently ends the byte stream.

That is appropriate for an application-oriented HTTP client but insufficient as the only native boundary for a gateway or middleware consumer that already has an HTTP body and wants frame/trailer/backpressure fidelity. `http-body` 1.x deliberately uses `poll_frame` / `Frame` as its forward-compatible abstraction, including data and trailers and room for future frame types. eggfetch already depends directly on `http-body = "1"`, so a native interop surface does not require a new runtime dependency.

Adding another public variant to `RequestBody` or `ResponseBody` is specifically undesirable: both enums are already public and externally matchable. A new variant would create avoidable source compatibility churn for existing Rust callers.

### 2. TLS configuration is not fully provider-neutral

`TlsConfig::build_rustls_config()` obtains a provider through `process_crypto_provider()`. Today that helper honors an already-installed process provider, but otherwise explicitly installs ring because `tls-rustls` wires `hyper-rustls/ring`. `SingleCertResolver` then directly calls `rustls::crypto::ring::sign::any_supported_type`, so mTLS remains ring-specific even when another process provider was installed.

Rustls 0.23 already exposes the right generic contract: `CryptoProvider` contains the cipher suites, key-exchange groups, signature verification algorithms, secure random source, and `key_provider`; `KeyProvider::load_private_key()` is the provider-neutral mechanism used by Rustls's own certificate builder APIs. A caller can therefore supply a provider without eggfetch adding a dependency on every crypto backend.

The desired result is per-client/per-`TlsConfig` provider selection, not a new global initialization convention and not a Synvoid/AWS-LC mode. Existing ring behavior must remain the default for current eggfetch users.

### 3. Custom CA configuration is replacement-only

`TrustStore::Custom` and the existing `ca_certificate_path` / `ca_certificate_pem` / `ca_certificate_der` helpers intentionally replace the default roots. `add_ca_certificate_path` adds to the custom replacement set; it does not mean "system/WebPKI roots plus this private CA".

Both policies are legitimate and must remain distinguishable. Public internet trust plus one or more private enterprise roots is a common native-client requirement and should not require downstream callers to reconstruct eggfetch's root-loading/fallback policy themselves.

## External API facts validated for this plan

The implementation should be based on upstream contracts rather than private behavior:

- `http-body` 1.x models bodies with `Body::poll_frame`; `Frame` carries DATA and trailers and is explicitly designed for forward-compatible frame handling.
- Rustls 0.23 supports explicit `Arc<CryptoProvider>` selection when building `ClientConfig`; `CryptoProvider::key_provider` / `KeyProvider::load_private_key` is the provider-neutral private-key loading path.
- Current Rustls has built-in ring and AWS-LC providers and permits third-party providers through the same `CryptoProvider` type.
- Current Quinn 0.11 supports wrapping a custom Rustls `ClientConfig` for QUIC and also has explicit `rustls-aws-lc-rs` support. HTTP/3 provider behavior must still be tested on eggfetch's pinned dependency graph rather than assumed.

These facts justify generic extensibility. They do not require eggfetch to add an AWS-LC dependency or to promote HTTP/3 from experimental status.

## Architectural invariants

The program must preserve all current repository invariants and the following additional rules:

- Existing `RequestBody`, `ResponseBody`, `Response`, `RequestBuilder`, Python, CLI, FFI, Node prototype, HTTPX 0.28.1, HTTPX2 2.12.0, redirect, retry, decompression, cookie and auth behavior remain source- and behavior-compatible unless a concrete correctness defect requires a separately documented correction.
- Do not add a downstream-specific body, WAF, route, metrics, site/backend, proxy-server, or certificate-policy type.
- Do not expose `hyper::body::Incoming` as the long-term public interoperability contract. Hyper remains the engine; `http_body::Body` is the stable native interoperability boundary.
- Do not add variants to the existing public `RequestBody`, `ResponseBody`, or `TrustStore` enums merely to implement this program.
- The frame-preserving path is additive and transport-oriented. It must reuse eggfetch's existing connectors, connection pools, TLS, route selection, timeout/lifecycle controls and physical-connection policy rather than introducing a second HTTP stack.
- A one-shot arbitrary HTTP body is never silently made replayable. The native frame path must not enable logical redirects/retries that require replay unless a future replay contract is designed explicitly.
- The existing high-level application-client path keeps its current redirect/retry/decompression/cookie/auth semantics.
- Existing custom-CA methods continue to replace default roots. Additional trust anchors are a separate explicit API.
- Existing ring-based default builds remain valid and retain their current ordinary behavior and feature profile.
- Explicit TLS provider selection is local to the `TlsConfig`; it must not require installing that provider as the process-wide Rustls default.
- Provider selection must apply coherently to certificate verification and private-key signing. A connection may not advertise one provider policy while client-auth keys are loaded through another hard-coded backend.
- Security-sensitive incompatible states fail before network I/O. No fallback may silently weaken an explicitly selected provider, trust source, route or verification policy.
- HTTP/3 remains experimental. This program may make provider behavior more coherent but does not constitute H3 graduation evidence.
- Compatibility evidence remains exact-SHA evidence. Executable/test changes invalidate the current compatibility binding until the closure plan freezes and requalifies one final executable tree.

## Ordered implementation plans

Execute the following plans in order unless a child plan explicitly permits overlap:

1. `plans/native-http-body-interoperability.md`
   - add a Rust-native, additive `http_body::Body` execution surface;
   - preserve request and response frames, trailers, backpressure, errors and cancellation;
   - reuse the existing Hyper/route/pool/TLS/lifecycle machinery;
   - keep the existing high-level body enums unchanged;
   - establish a small external-style gateway fixture proving the public API without importing downstream code.

2. `plans/tls-crypto-provider-extensibility.md`
   - allow an explicit `Arc<rustls::crypto::CryptoProvider>` on `TlsConfig`;
   - retain current ring defaults when no explicit provider is supplied;
   - remove hard-coded ring key loading from mTLS and use the selected provider's key loader;
   - test an external provider configuration, including an AWS-LC-backed fixture, without making AWS-LC an eggfetch product-specific mode;
   - audit H1/H2/direct/custom-dialer/UDS/proxy/H3 provider propagation.

3. `plans/tls-additional-trust-anchors.md`
   - add a distinct additive trust-anchor API layered on top of the selected base `TrustStore`;
   - preserve every existing replacement-style CA method and default root policy;
   - use one composed root-store result across ordinary origin TLS routes and explicitly configured proxy TLS where the same `TlsConfig` is supplied;
   - test private-root augmentation, fallback behavior, deduplication and fail-closed parsing.

4. `plans/post-native-http-body-and-tls-extensibility-qualification-and-closure.md`
   - audit the three child plans for API boundedness and cross-feature coherence;
   - run the external-style native fixture(s), feature/provider/trust matrices and existing repository gates;
   - freeze one exact executable/test/fixture SHA;
   - renew HTTPX 0.28.1 and HTTPX2 2.12.0 compatibility evidence only after the freeze;
   - update architecture/Rust guide/feature/dependency documentation and plan/roadmap status truthfully;
   - record footprint impact without claiming a size win unless measured.

Plans 2 and 3 may overlap after their shared `TlsConfig` field/layout direction is agreed, but both must close before plan 4. Plan 1 may proceed independently except where a final qualification fixture combines frame-preserving HTTP with explicit TLS provider/trust policy.

## Program acceptance criteria

The program is complete only when all of the following are true:

- [x] Existing high-level Rust/Python/CLI/compatibility APIs remain behaviorally intact.
- [x] No downstream-specific type, feature flag, adapter crate or protocol policy enters eggfetch.
- [x] A native Rust caller can submit an arbitrary `http_body::Body<Data = Bytes>` through an eggfetch client without flattening the request to `Stream<Bytes>` first.
- [x] The native response path exposes an eggfetch-owned body implementing `http_body::Body<Data = Bytes>` and preserves trailer frames and backpressure.
- [x] Unknown/future HTTP body frames are not accidentally interpreted as EOF by the new native path; the high-level byte adapter is also audited for safe forward-compatible behavior.
- [x] Dropping/cancelling a native response releases eggfetch logical pool/connection admission state exactly as the existing streaming response path does.
- [x] Read/I/O timeout and body-error semantics remain deterministic at the frame boundary.
- [x] Native frame execution does not silently replay one-shot request bodies through eggfetch redirect/retry policy.
- [x] Existing `RequestBody` and `ResponseBody` public enum shapes remain unchanged.
- [x] A `TlsConfig` can carry an explicit `Arc<CryptoProvider>` without mutating the process-global Rustls provider.
- [x] When no explicit provider is supplied, current ring/default behavior remains unchanged for ordinary builds.
- [x] Client-certificate signing uses the selected provider's `key_provider`, not a hard-coded ring signing function.
- [x] Explicit provider selection is tested across all relevant TLS route constructors; unsupported combinations fail clearly rather than falling back to another provider.
- [x] Existing custom CA APIs remain replacement-style exactly as documented.
- [x] A separate additional-root API can compose native/WebPKI/custom base trust with one or more additional anchors.
- [x] `NativeOnly` still fails if native roots are unavailable; additional anchors do not silently redefine that policy.
- [x] `NativeWithWebPkiFallback` still performs its existing fallback before additional roots are overlaid.
- [x] Root/provider state is redacted/bounded in `Debug` output and private key/certificate contents are never logged.
- [x] No unnecessary runtime dependency is added to the ordinary default profile.
- [x] The external-style fixture proves the intended public API without depending on Synvoid or another downstream repository.
- [x] One final frozen executable/test/fixture SHA passes the repository's required validation and compatibility requalification before current exact-SHA claims are renewed.
- [x] Documentation and `plans/README.md` / `plans/ROADMAP.md` describe the final implementation accurately.

## Explicit non-goals

Do not expand this program into:

- a Synvoid facade, Synvoid feature flag, reverse-proxy server framework, WAF hook, site/backend registry, bandwidth callback or application policy layer;
- changing the default high-level `Response` into `http::Response` or replacing the existing application-client ergonomics;
- making `hyper::body::Incoming`, Hyper connector traits or Hyper client types part of the public stable API;
- adding a replay factory for arbitrary bodies without a separate concrete use case;
- automatic redirect/retry of one-shot native bodies;
- a Tower middleware framework;
- a general server implementation;
- adding AWS-LC, FIPS or post-quantum policy as a mandatory eggfetch default;
- removing ring support or changing current default provider behavior;
- changing existing replacement custom-CA semantics;
- trusting both public and private roots implicitly without an explicit additive-root call;
- H3 graduation, 0-RTT expansion, WebTransport, MASQUE or a new QUIC transport family;
- downstream migration work;
- a claim that the new API reduces binary size or improves throughput without measurement;
- a new routine CI matrix or verification framework.

## Validation policy

Use the repository's existing verification policy. Focused tests belong in each child plan; routine validation remains `./scripts/check.sh`. Extended/package/compatibility qualification belongs at meaningful closure points rather than every intermediate commit.

The final child plan owns the exact-SHA compatibility freeze. If any executable/test/fixture correction lands after that freeze, invalidate the freeze and repeat the required qualification rather than updating compatibility records onto a different executable tree.

## Handoff note

The motivating Synvoid evaluation should be used only as a requirements check: can a transport-oriented consumer delete its own generic HTTP/TLS/pool/body plumbing and keep its application-specific routing/security policy above eggfetch? No Synvoid source dependency or naming should appear in implementation. The external fixture must express the same requirements with generic local test bodies, TLS identities and trust anchors.
