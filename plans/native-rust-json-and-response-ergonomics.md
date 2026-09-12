# Native Rust JSON and Response Ergonomics

Planning baseline: `445149bd2af3c3ce5c0d34daacbde1bf8de57297` (`main`, 2026-09-11; eggfetch 0.1.3)
Parent program: `plans/embedded-rust-client-footprint-and-routing-program.md`

## Objective

Finish the already-reserved native Rust `json` capability and add a small set of high-value native request/response conveniences that reduce downstream boilerplate without turning eggfetch into a reqwest compatibility facade or expanding the default dependency graph.

The implementation must remain idiomatic to eggfetch's own body/response model and preserve single-consumption, replayability, body-limit and error semantics.

## Current problem

`eggfetch-core` declares a `json` feature, but the feature is currently empty. The architecture documentation explicitly reserves it for future Rust-native Serde integration while Python JSON support is implemented independently at the Python boundary.

As a result, native Rust consumers must manually:

- call `serde_json::to_vec`/`to_string`;
- set `Content-Type: application/json` correctly;
- create a byte-backed request body;
- buffer response bytes;
- call `serde_json::from_slice`;
- map serialization/deserialization errors themselves.

This is routine HTTP-client functionality and fits the existing feature contract better as an optional core capability.

The audit also identified two narrow ergonomics worth considering because the core already holds the relevant state: parsed numeric response content length and request-scoped decoded-body limits. These should be implemented only if they remain small and semantically clear.

# 1. Wire the `json` feature to optional dependencies

Add optional direct dependencies on `serde` and `serde_json` and make the existing `json` feature enable them.

Requirements:

- `serde`/`serde_json` remain absent from `eggfetch-core` when `json` is disabled unless another selected feature independently requires them;
- do not enable Serde derive unless the native helper API actually requires it; generic `Serialize`/`DeserializeOwned` bounds do not require derive support in eggfetch itself;
- keep the default core feature set unchanged unless the project explicitly decides native JSON is a default capability for Rust users. The default recommendation for this program is to keep it opt-in so minimal consumers pay nothing.

Acceptance:

- [ ] `cargo tree -e features` proves JSON dependencies are feature-owned.
- [ ] `cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,json` succeeds after the feature/TLS plan.
- [ ] Disabling `json` removes native Serde JSON helper code through normal `cfg(feature = "json")` boundaries.

# 2. Add request JSON serialization

Add an idiomatic request-builder helper equivalent in behavior to:

```rust
client
    .post(url)?
    .json(&payload)?
    .send()
    .await?;
```

Exact error timing may follow eggfetch's existing builder convention: either return `Result<Self>` or store a deferred builder error consistently with other fallible request-builder operations. Do not introduce a one-off error pattern.

Required semantics:

- serialize via `serde_json` exactly once into bytes;
- construct a byte-backed `RequestBody`, preserving replayability for redirects/retries;
- set `Content-Type: application/json` only when the request does not already contain an explicit Content-Type;
- preserve an explicit caller Content-Type, including vendor media types;
- do not set `Accept` automatically;
- do not double-encode an existing raw body;
- if body-setting APIs conflict, use the same last-call/explicit-conflict convention as existing builder methods and document it;
- serialization failure must be reported before network I/O.

Prefer the conventional `application/json` value without charset unless existing project policy requires otherwise.

Acceptance:

- [ ] structs/maps/scalars/arrays serialize correctly;
- [ ] explicit Content-Type is preserved;
- [ ] serialization failure opens no connection;
- [ ] resulting body remains replayable through existing retry/redirect rules.

# 3. Add response JSON deserialization

Add an optional response helper equivalent to:

```rust
let value: MyType = response.json().await?;
```

It must follow `Response::bytes()`/`text()` single-consumption semantics rather than adding an independent buffering model.

Required semantics:

- consume the response body once;
- apply existing response decompression and decoded-body limits exactly as `bytes()` does;
- deserialize from bytes via `serde_json::from_slice`, not through a lossy UTF-8 conversion;
- do not require `Content-Type` to be exactly `application/json`; many APIs return vendor JSON or incorrect/missing content types. JSON parsing should be explicit caller intent;
- after body consumption, subsequent body reads retain existing consumed/error semantics;
- trailers/history/final URL/metadata behavior remains consistent with `bytes()`.

Acceptance:

- [ ] valid JSON maps to `DeserializeOwned` targets;
- [ ] invalid JSON returns a structured JSON error;
- [ ] decoded-body limits still apply;
- [ ] compressed JSON follows the same decode path as buffered bytes;
- [ ] the response is consumed once and cannot be silently re-read.

# 4. Extend the error taxonomy narrowly

Introduce a native JSON error classification rather than flattening Serde failures into a generic body string.

Choose an error representation compatible with the crate's current cloneable `Error` design. Because `serde_json::Error` is not necessarily suitable for direct storage under all existing clone/source expectations, acceptable options include a structured string/category wrapper with stable `kind()` classification or an `Arc` source if appropriate.

Required properties:

- request serialization and response deserialization can be distinguished if that improves caller handling;
- `Error::kind()` receives stable names;
- secret-bearing JSON body content is never included in `Display`/`Debug`; report location/category/message without echoing payload content;
- Python compatibility error mapping is unchanged unless native error propagation actually reaches that boundary.

Do not create a broad serialization framework or generic codec trait.

Acceptance:

- [ ] JSON errors are distinguishable from transport/body I/O failures.
- [ ] error formatting does not leak request/response JSON payloads.

# 5. Add numeric content-length convenience if it stays allocation-free

The response already preserves the original wire `Content-Length` string. Add a native convenience such as:

```rust
pub fn content_length(&self) -> Option<u64>
```

only if it can be defined unambiguously.

Recommended semantics:

- parse the wire Content-Length header as unsigned decimal;
- return `None` when absent or invalid;
- describe this as wire/declared length, not decoded body length;
- retain `wire_content_length()` for callers needing exact header text/compatibility behavior;
- do not infer length for chunked/streamed responses that lack the header.

If duplicate/conflicting Content-Length semantics require more nuanced handling, reuse the parser/validation decision already made by the transport rather than inventing a second interpretation.

Acceptance:

- [ ] numeric helper never reports decompressed/collected body size as wire Content-Length.
- [ ] existing compatibility metadata remains untouched.

# 6. Evaluate request-scoped decoded-body limits

The client already supports `max_decoded_body_size` and `max_decompression_ratio`. Determine whether request-level overrides fit cleanly into existing `RequestParts`/prepared-request state.

If implementation is small and consistent, add request builder overrides such as:

```rust
request.max_decoded_body_size(n)
request.max_decompression_ratio(r)
```

with clear precedence:

```text
request override > client setting > unlimited/default
```

Requirements:

- preserve values across retries of the same request;
- preserve or deliberately carry them across redirects because they are response safety policy, not destination routing state;
- enforce both streaming and buffered response paths through the existing limit wrapper;
- avoid duplicating limit logic in `Response` methods.

If this requires invasive pipeline changes disproportionate to the benefit, record a deliberate deferral rather than expanding scope. Native JSON is the required part of this plan; request-scoped body limits are desirable but not a blocker to parent-program closure.

# 7. Builder/API consistency audit

While adding JSON helpers, inspect native request ergonomics for avoidable inconsistencies exposed directly by the implementation work, but do not conduct a broad API redesign.

Specifically verify:

- fallible builder methods handle errors consistently;
- header precedence is deterministic;
- body replacement/conflict semantics are documented;
- JSON byte bodies participate normally in content-length calculation;
- retry/redirect replay classification remains `Immutable`/replayable;
- Debug redaction still reports body length only.

# 8. Tests

Add focused tests for:

- request serialization of representative types;
- custom Content-Type preservation;
- serialization errors before I/O;
- JSON request replay across retry and permitted redirects;
- successful response deserialization;
- invalid/truncated JSON;
- JSON with non-UTF-8-invalid bytes returning JSON parse error without lossy conversion;
- compressed JSON when corresponding compression feature is enabled;
- decoded-body-limit interaction;
- response single-consumption behavior;
- JSON feature off compile check;
- numeric content-length helper if implemented;
- request-scoped body-limit precedence if implemented.

Avoid external network access.

# 9. Documentation

Update native Rust API/examples and the feature reference to show JSON as optional native capability. Keep Python JSON docs separate because Python continues to expose its own compatibility-shaped API over the same engine.

Document that explicit `json()` parsing does not validate media type and that it consumes the response body.

## Non-goals

- no reqwest API compatibility layer;
- no generic serializer/content-negotiation framework;
- no CBOR/MessagePack/XML codecs;
- no automatic response JSON decoding based on Content-Type;
- no provider-specific request types;
- no SSE/event parser;
- no changes to Python public JSON semantics unless required by shared-core regression correction;
- no default enabling of heavy convenience features merely for downstream migration ease.

## Required validation

Run native JSON tests and:

```sh
./scripts/check.sh
```

Run feature-off and feature-on compile checks. Run affected retry/redirect/body-limit/compression tests where relevant. Do not renew exact-SHA compatibility status here; later executable/qualification work follows.

## Exit criteria

- [ ] The reserved `json` feature has real native behavior and optional dependency ownership.
- [ ] Native request JSON serialization is replay-safe and respects header precedence.
- [ ] Native response JSON deserialization uses existing single-consumption/body-limit semantics.
- [ ] JSON errors are structured and do not leak payloads.
- [ ] Narrow additional ergonomics are implemented only where semantics remain clear and scope stays small.
- [ ] Default/non-JSON consumers do not pay for Serde JSON through this feature.
- [ ] Documentation accurately describes the native JSON contract.
