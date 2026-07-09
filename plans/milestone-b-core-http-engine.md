# Milestone B Plan: Core Request/Response Model and Minimal HTTP Engine

## Objective

Implement the first usable async Rust HTTP client engine in `eggfetch-core` while preserving clean long-term architecture. This milestone should produce a minimal but real client capable of issuing HTTPS requests, returning buffered responses, and exposing an idiomatic Rust request builder.

Correctness is more important than breadth. The goal is not full requests/httpx parity yet; the goal is a reliable core model that later Python, CLI, streaming, redirects, pooling, and protocol work can build on.

## Scope

Milestone B includes:

- async `Client`
- `Request`
- `RequestBuilder`
- `Response`
- `Body`
- `Headers`
- method and URI handling
- query parameter handling
- byte request bodies
- buffered response bodies
- HTTPS support using Rustls
- basic error taxonomy
- local integration tests

Milestone B does not include:

- advanced pooling policy beyond what is required for a reusable client
- detailed timeout phases
- streaming user-facing API
- redirects
- cookies
- proxies
- decompression
- Python bindings
- CLI behavior

## Core API target

The Rust API should support:

```rust
use eggfetch_core::Client;

let client = Client::new();
let response = client
    .get("https://example.com")
    .header("user-agent", "eggfetch")
    .query("q", "test")
    .send()
    .await?;

assert!(response.status().is_success());
let bytes = response.bytes().await?;
```

Also support:

```rust
let response = client
    .post("https://example.com/api")
    .header("content-type", "application/json")
    .body(r#"{"a":1}"#)
    .send()
    .await?;
```

The final names may vary, but the ergonomics should be close.

## Module responsibilities

### client.rs

Owns `Client` and high-level request creation.

Required methods:

- `Client::new()`
- `Client::builder()`
- `Client::request(method, url)`
- `Client::get(url)`
- `Client::post(url)`
- `Client::put(url)`
- `Client::patch(url)`
- `Client::delete(url)`
- `Client::head(url)`
- `Client::options(url)`

The client should internally own the hyper client and shared configuration.

### request.rs

Owns request construction.

Required types:

- `Request`
- `RequestBuilder`
- re-export or wrap `http::Method`
- URI parsing helpers

Builder capabilities:

- set method
- set URL
- append query pairs
- set headers
- set body bytes
- build final request
- send request through associated client

### response.rs

Owns response metadata and body access.

Required fields or accessors:

- status
- version
- headers
- final URL if tracked
- body handle

Initial body behavior may buffer the whole body, but structure it so streaming can be introduced in Milestone E without replacing the type entirely.

### body.rs

Owns body representation.

Initial request body variants:

- empty
- bytes

Initial response body behavior:

- collect into bytes
- return bytes once or cache if response semantics require repeated access later

Avoid designing this as only `Vec<u8>`. The abstraction must leave room for streams.

### headers.rs

Use `http::HeaderMap` internally unless there is a strong reason to wrap from the start.

Add helper functions for:

- validating names and values
- appending repeated headers
- setting replacement headers
- case-insensitive lookup through `HeaderMap`

Do not flatten duplicate headers into a plain map.

### error.rs

Create a project error taxonomy early.

Suggested variants:

- invalid URL
- invalid method
- invalid header name
- invalid header value
- request build error
- DNS/connect error if distinguishable
- TLS error if distinguishable
- protocol error
- body error
- hyper error
- unsupported feature

Do not expose raw hyper errors as the public API, though source errors should be preserved.

## Hyper integration

Use `hyper` and `hyper-util` as the low-level engine. Prefer a design that makes the transport stack explicit and replaceable.

The initial transport should support:

- HTTP/1.1
- HTTPS via Rustls
- Tokio runtime

HTTP/2 should remain feature-gated or deferred unless it is trivial to enable without complicating the milestone.

## URL handling

Use the `url` crate for URL parsing and query manipulation.

Requirements:

- reject unsupported schemes cleanly
- accept `http` and `https`
- preserve path and query correctly
- support appending query pairs
- avoid lossy string manipulation

Test cases:

- existing query plus appended query
- empty path normalization
- percent-encoded query values
- invalid URLs
- unsupported schemes

## Header handling

Requirements:

- validate header names
- validate header values
- preserve duplicate headers
- provide set and append semantics
- do not lowercase values
- do not accidentally strip user headers except where protocol requires

Test cases:

- repeated `set-cookie`
- mixed-case names
- invalid newline in value
- invalid header name

## Body handling

Initial body support:

- empty body
- byte vector body
- string body as bytes
- static byte slice convenience if ergonomic

Request body rules:

- GET should not automatically prohibit bodies at the Rust layer
- Content-Length should be set if body size is known and hyper does not set it automatically
- Avoid chunked encoding unless the body model requires it

Response body rules:

- initial `bytes().await` may collect the body
- impose no default maximum body size in core unless a config option exists
- body collection errors should map into eggfetch errors

## Client configuration foundation

Add a `ClientBuilder` even if options are initially limited.

Initial options:

- default headers
- user agent optional default
- HTTP/1 only versus future HTTP/2 feature placeholder
- TLS config placeholder

Do not add timeouts here yet except as placeholders if needed. Milestone D owns timeout behavior.

## Tests

Create local integration tests using a small local HTTP server. Prefer avoiding heavyweight test dependencies if practical. A tiny hyper-based test server inside test utilities is acceptable.

Test categories:

- GET request returns status and body
- POST sends body correctly
- custom headers are received
- query params are serialized correctly
- repeated headers are handled correctly
- HTTPS smoke test if practical in CI, otherwise isolate certificate work for later
- invalid URL produces expected error
- unsupported scheme produces expected error
- response body collection works

If HTTPS test setup is too heavy for this milestone, document the gap and ensure the TLS stack compiles and is exercised by a smoke test where feasible.

## Public API hygiene

Before merging Milestone B, inspect the public API for accidental leakage of hyper internals. The public API may re-export standard `http` crate types if desirable, but it should not force users to understand hyper-specific body machinery.

## Acceptance criteria

Milestone B is complete when:

- `eggfetch-core` can perform basic async HTTP requests
- HTTPS support is wired through Rustls
- GET and POST work against local tests
- headers, query params, and byte bodies work
- responses expose status, headers, version, and buffered body bytes
- errors are mapped into eggfetch error types
- `cargo test -p eggfetch-core` passes
- docs show a minimal Rust usage example

## Risks

The main risk is coupling the response type too tightly to buffered bodies. Avoid that by separating response metadata from body representation even if the first implementation buffers.

Another risk is selecting convenience dependencies too early. Prefer explicit code for simple builder behavior unless a crate clearly reduces correctness risk.

## Handoff notes

After this milestone, the next implementer should not immediately add Python bindings. Milestones C, D, and E are foundational and should land before exposing a serious Python API, because Python compatibility will depend heavily on stable pooling, timeout, and streaming semantics.
