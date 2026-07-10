# Milestone O Plan: Cookie Subsystem

## Objective

Implement a first-class cookie subsystem in `eggfetch-core` and expose familiar, stateful cookie behavior through the Python clients and responses.

The initial goal is a correct HTTP-client cookie jar, not a complete browser cookie implementation. The design should cover ordinary server-managed sessions while keeping policy boundaries explicit enough to add stricter browser-style behavior later.

## Prerequisites

Milestone N must be complete.

In particular, the following should already be stable:

- redirect lifecycle and cross-origin handling
- response history ownership
- sync/async client state
- header multi-value access
- request/response API parity

## Scope

Milestone O includes:

- core `Cookie` representation
- `Set-Cookie` parsing
- request `Cookie` serialization
- stateful `CookieJar`
- domain/path/secure/expiry matching
- host-only cookies
- replacement/deletion behavior
- redirect integration
- client-level cookie state
- per-request cookie input
- Python `Cookies` wrapper
- `client.cookies`
- `response.cookies`
- comprehensive cookie tests

Milestone O does not initially include:

- public suffix list enforcement
- browser SameSite enforcement
- partitioned cookies
- cookie persistence to disk
- browser privacy policies
- JavaScript-related HttpOnly behavior beyond preserving the attribute

## Dependency decision

Before implementing parsing manually, evaluate a focused cookie crate.

Criteria:

- RFC 6265 parsing quality
- dependency size
- maintenance activity
- no browser framework coupling
- ability to preserve relevant attributes

Using a small mature parser is reasonable because cookie syntax has many edge cases. The jar policy and matching logic should remain under eggfetch control.

Document the dependency decision in the architecture dependency policy.

## Core data model

Suggested types:

```rust
pub struct Cookie {
    name: String,
    value: String,
    domain: String,
    host_only: bool,
    path: String,
    secure: bool,
    http_only: bool,
    same_site: Option<SameSite>,
    expires: Option<SystemTime>,
    persistent: bool,
    creation_index: u64,
}

pub struct CookieJar {
    // internal storage
}
```

The stored cookie must retain enough metadata to determine whether it should be sent for a URL.

## Parsing `Set-Cookie`

Parse each `Set-Cookie` header independently. Do not comma-split a combined value because Expires attributes may contain commas.

Handle:

- name/value
- Domain
- Path
- Secure
- HttpOnly
- SameSite
- Max-Age
- Expires

Required rules:

- invalid cookie names are rejected
- missing Domain creates a host-only cookie for the response host
- explicit Domain is normalized and validated against the response host
- invalid Domain attributes cause rejection
- Max-Age takes precedence over Expires
- Max-Age <= 0 removes the matching cookie
- expired cookies are not stored
- default path is derived from the response URL path

Unknown attributes may be ignored while preserving parser safety.

## Domain matching

Implement explicit host/domain rules.

Host-only cookie:

- sent only to the exact host that set it

Domain cookie:

- sent to the normalized domain and matching subdomains
- only accepted if the setting host domain-matches the Domain attribute

Normalize leading dots according to modern cookie rules rather than treating them as special wildcard syntax.

IP-address handling should be deliberate. Prefer host-only behavior and reject Domain attributes that attempt suffix matching on IP literals.

## Path matching

Implement RFC-style path matching.

Default path should be derived from the request path.

When multiple cookies with the same name match, serialization order should prefer longer paths, then earlier creation time/index as appropriate.

## Secure and scheme behavior

Secure cookies are sent only over HTTPS.

Do not send Secure cookies over HTTP, including during redirects.

Document localhost behavior rather than inventing browser exceptions initially.

## Expiry and cleanup

Expire cookies lazily during jar access and optionally through a cleanup method.

Avoid a background cleanup task for the initial implementation.

Use a clock abstraction or injectable `now` in tests so expiry tests are deterministic.

## Cookie replacement key

A stored cookie is identified by:

```text
name + domain + path
```

A newly received cookie with the same identity replaces the prior value and attributes.

Deletion through expired/zero Max-Age should remove the matching identity.

## Request integration

Before sending a request:

1. obtain matching cookies for the URL
2. serialize a `Cookie` header
3. merge with explicit request cookie behavior

Define interaction with a user-supplied `Cookie` header.

Recommended policy:

- explicit raw `Cookie` header wins and disables automatic jar injection for that request
- Python `cookies=` merges into a temporary request cookie set without mutating the client jar unless response cookies later update it

Do not produce multiple ambiguous Cookie headers.

## Response integration

After each response, including redirect hops:

- parse every `Set-Cookie`
- update the client jar before constructing the next redirect request

This is important for login/session redirects.

The final response should expose cookies received on that specific response. History responses should expose cookies from their own hop.

## Client state

`eggfetch_core::Client` should own or share a jar through its inner state.

Cloned Rust clients should share jar state if they share other client state.

Python sync and async wrappers for the same client object should expose the same underlying state model; they should not maintain Python-only duplicate jars.

Thread/concurrency safety is required.

## Python API

Target:

```python
client = eggfetch.Client(cookies={"session": "abc"})
client.cookies
client.cookies.get("session")
client.cookies.set("theme", "dark", domain="example.com", path="/")
client.cookies.delete("session")
client.cookies.clear()

response.cookies

eggfetch.get(url, cookies={"a": "1"})
```

Provide a `Cookies` mapping-like wrapper with:

- `get`
- `set`
- `delete`
- `clear`
- iteration
- length
- containment
- possibly `items()`

Ambiguous same-name cookies across domains/paths should not silently return an arbitrary value. Either require domain/path or raise an ambiguity error.

## Python constructor behavior

Accept client/request cookies as:

- mapping of name to value
- sequence of pairs
- existing `Cookies` instance

Mapping cookies without domain/path are request-local or bound deliberately to a URL when used in a request. For a client constructor without a base URL, define whether they become broad synthetic cookies or simple default Cookie header values.

Recommended approach:

- client constructor mapping is stored as default name/value cookies without domain restrictions only if a clear internal representation exists
- alternatively, expose `Cookies` and require domain-aware `.set()` for persistent client cookies while treating constructor mappings as default request cookies

Choose and document one coherent policy.

## Redirect behavior

Required:

- process Set-Cookie on intermediate redirects
- newly set cookie may be sent on the next matching hop
- host-only cookie is not sent cross-host
- Domain cookie follows matching rules
- Secure cookie is not sent after HTTPS-to-HTTP downgrade
- explicit raw Cookie header is stripped cross-origin according to redirect security policy
- jar then recomputes cookies for the destination

Never carry the prior serialized Cookie header blindly across a redirect.

## Error handling

Invalid server cookies should generally be ignored rather than failing the entire response, unless the parser encounters an internal error.

Invalid user-provided cookies should raise a request-building/Python value error.

Consider optional diagnostics/tracing for rejected server cookies later.

## Tests

### Core parsing tests

- basic cookie
- quoted values if supported
- Expires with comma
- Max-Age precedence
- immediate deletion
- invalid Domain rejection
- default Path
- Secure/HttpOnly/SameSite attributes

### Matching tests

- host-only exact match
- domain subdomain match
- no sibling-domain leakage
- path matching
- secure-only behavior
- expiry
- same-name different paths ordering

### Request/response tests

- Set-Cookie updates jar
- later request sends cookie
- intermediate redirect cookie is used on next hop
- cross-origin redirect does not leak host-only cookie
- HTTPS cookie not sent over HTTP
- explicit Cookie header policy works

### Python tests

- client cookie persistence
- request-local cookies
- response.cookies
- mapping-like methods
- deletion/clear
- sync/async parity
- ambiguous lookup policy

## Public API and feature gating

Use the existing `cookies` feature flag in `eggfetch-core`.

Decide whether Python wheels enable cookies by default. Given expected HTTP-client semantics, enabling cookies in the Python package by default is reasonable while keeping the Rust core feature-gated.

## Documentation

Document:

- current standards target
- host/domain/path matching
- Secure behavior
- constructor/request cookie semantics
- redirect handling
- known gaps versus browsers and HTTPX

## Acceptance criteria

Milestone O is complete when:

- core jar stores and matches cookies correctly
- Set-Cookie is processed on ordinary and redirect responses
- Cookie headers are recomputed safely for each destination
- client cookie state is concurrency-safe
- Python clients expose useful cookie APIs
- sync and async behavior match
- invalid server cookies do not break requests
- comprehensive deterministic tests pass

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

cd crates/eggfetch-python
maturin develop
python -m pytest
```

## Handoff note

Authentication is the next milestone. Cookie and auth redirect policies overlap, so keep credential and cookie header generation centralized in the core request pipeline.
