# Milestone Z Plan: Additional Language Bindings and Framework Use

## Objective

Treat `eggfetch-core` as a reusable, language-neutral HTTP engine and define a controlled path for additional bindings and internal framework consumers. This milestone should validate that the core API is sufficiently stable and ergonomic outside Python without fragmenting semantics across languages.

## Preconditions

Begin only after:

- Rust core public API is versioned and stable
- TLS/retry/HTTP2 behavior is settled
- release engineering is functional
- compatibility and security documentation are mature
- a C-compatible boundary strategy has been reviewed

## Scope

Evaluate and prototype:

- Node.js via N-API
- Ruby via magnus or C ABI
- Perl via XS/C ABI
- Java/Kotlin via JNI or generated C ABI
- Zig/C consumers
- internal eggstack framework integrations

Do not commit to supporting every target. Select one secondary binding as the reference based on demand and maintenance cost.

## Binding architecture

All bindings must:

- invoke `eggfetch-core`
- preserve the async-first engine
- expose idiomatic host-language sync/async adapters
- map errors without losing categories
- preserve streaming and cancellation
- avoid reimplementing cookies/auth/redirects/retries
- maintain secret redaction

Consider an intermediate `eggfetch-ffi` crate only if multiple C-ABI consumers justify it.

## Stable core surface review

Before binding work, audit:

- ownership/lifetimes across FFI
- body stream handles
- callback/cancellation semantics
- runtime ownership
- thread safety
- client close/drop behavior
- error serialization
- header multi-values
- configuration serialization

Introduce internal façade types rather than exposing hyper/tokio/PyO3 details.

## Reference binding selection

Score candidates by:

- user demand
- async runtime compatibility
- package distribution complexity
- binary ABI stability
- test infrastructure
- maintainer expertise

Recommended initial reference candidates are Node/N-API or a minimal C ABI consumed from Zig, but the final decision should be evidence-based.

## Host-language API policy

Do not mechanically copy Python names. Preserve familiar conventions in each ecosystem while documenting semantic equivalence.

Each binding should expose:

- persistent client
- one-shot helpers where idiomatic
- buffered and streaming responses
- timeouts
- redirects
- cookies/auth
- multipart
- proxy/TLS/retries
- protocol selection

## Runtime integration

Define one runtime strategy per binding:

- shared internal Tokio runtime for sync/foreign async ecosystems
- bridge to host event loop where mature integration exists
- no runtime-per-request behavior

Cancellation propagation must be tested.

## Distribution

Plan package artifacts, supported platforms, ABI targets, signing, and CI before public release. Avoid expanding release matrices beyond maintainable limits.

## Internal eggstack consumers

Add integration examples for other Rust projects using `eggfetch-core` directly. Gather API pain points before freezing broader FFI façades.

## Tests

For any selected binding:

- shared behavioral fixture suite
- error/timeout parity
- streaming/cancellation
- binary body/header behavior
- cookies/auth/redirects
- TLS/proxy
- packaging smoke tests on supported platforms

Reuse protocol fixtures from core rather than duplicating servers.

## Acceptance criteria

- core façade is language-neutral and documented
- one reference secondary binding is prototyped or shipped
- semantics remain centralized in core
- distribution and maintenance commitments are explicit
- unsupported bindings remain exploratory rather than implied promises
