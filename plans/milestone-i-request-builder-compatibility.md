# Milestone I Plan: Request Builder Compatibility Surface

## Objective

Expand Python request construction toward familiar requests/httpx semantics while preserving a clean Rust-core model. This milestone focuses on Python-facing arguments and configuration merging: `params`, `headers`, `content`, `data`, `json`, simple forms, client defaults, and request-level overrides.

The goal is not full HTTPX parity. The goal is a coherent compatibility layer for common requests that maps cleanly into `eggfetch-core`.

## Prerequisites

Milestones F, G, and H should be complete enough that sync/async clients and response handling are stable.

The core must have honest upload streaming semantics before exposing generator/file-like upload APIs. If true streaming upload is not implemented, do not expose streaming upload surfaces.

## Scope

Milestone I includes:

- Python request kwarg normalization
- client default configuration merging
- `params`
- `headers`
- `content`
- `data` for simple form/body cases
- `json`
- simple timeout normalization
- simple request object if needed
- request-side tests against local server

Milestone I does not include:

- multipart `files`
- cookie jar behavior
- auth flows
- redirects
- proxies
- SSL verification customization
- event hooks
- custom transports
- full HTTPX `Request` object parity

## Public API target

Support:

```python
client = eggfetch.Client(
    headers={"user-agent": "eggfetch"},
    timeout=5.0,
)

response = client.post(
    "https://example.com/api",
    params={"q": "test"},
    headers={"x-request-id": "1"},
    json={"hello": "world"},
)
```

Supported kwargs for `request()` and method helpers:

- `params`
- `headers`
- `content`
- `data`
- `json`
- `timeout`

Unsupported kwargs should raise a clear error. Do not silently ignore common HTTPX/requests kwargs like `cookies`, `auth`, `files`, `allow_redirects`, `follow_redirects`, `proxies`, `verify`, or `cert`.

## Argument normalization layer

Create a Python-side or Rust/PyO3 normalization layer that converts Python inputs to owned Rust request pieces before releasing the GIL or awaiting futures.

Suggested internal type:

```rust
struct PyRequestOptions {
    params: Option<Vec<(String, String)>>,
    headers: Option<Vec<(String, String)>>,
    body: Option<PyBodySpec>,
    timeout: Option<Timeout>,
}
```

Body variants:

```rust
enum PyBodySpec {
    Content(Bytes),
    Form(Vec<(String, String)>),
    Json(String or Bytes),
}
```

Keep conversions deterministic and explicit.

## Params handling

Accept:

- mapping: `{"a": "1"}`
- sequence of pairs: `[("a", "1"), ("a", "2")]`
- values convertible to string
- `None` meaning no params

Preserve repeated keys.

Do not sort query parameters unless documented. Preserve user-provided order for sequences of pairs. Mapping order follows Python dict insertion order.

Tests:

- mapping params
- repeated key params
- existing query plus params
- percent encoding
- `None` values policy

Decide `None` policy explicitly:

- either omit `None` values like many libraries often do, or
- encode as `None` string only if that is deliberate.

Preferred: reject `None` values initially unless compatibility requirements justify omission.

## Headers handling

Accept:

- mapping
- sequence of pairs
- existing `Headers` wrapper if implemented

Header names and values should be converted to strings or bytes only if safe. Reject newline characters and invalid header names/values through core validation.

Merging policy:

- client default headers apply first
- request-level headers override same-name defaults for normal set semantics
- support duplicate append only if the Python API exposes it deliberately

Do not collapse or mishandle `Set-Cookie` on response side. Request-side duplicate headers are less common but should be deterministic.

Tests:

- client default header reaches server
- request header overrides client default
- invalid header value with CR/LF raises
- mixed-case lookup is preserved or normalized as documented

## Body argument precedence

Define explicit precedence among `content`, `data`, and `json`.

Recommended policy:

- Only one of `content`, `data`, or `json` may be provided.
- Passing more than one raises `TypeError`.

This avoids surprising behavior.

## content

Accept:

- `bytes`
- `bytearray`
- `memoryview`
- `str` encoded as UTF-8

Do not accept Python iterators/generators unless true streaming upload is implemented.

Set `Content-Length` through core when known.

Do not set `Content-Type` automatically for raw `content` unless documented.

## data

For Milestone I, support simple form encoding:

```python
data={"a": "1", "b": "2"}
data=[("a", "1"), ("a", "2")]
```

Encode as `application/x-www-form-urlencoded` and set `Content-Type` if absent.

Also decide whether `data=b"raw"` is an alias for `content=b"raw"`. Requests permits several forms; eggfetch can keep this narrower.

Preferred:

- `data` mapping/sequence means form encoding
- `content` means raw body
- reject ambiguous `data` types initially

Tests:

- form mapping body reaches server
- repeated form keys preserved
- content-type auto-set for form when absent
- user content-type is not overwritten

## json

Implement JSON body support.

Options:

1. Use Python `json.dumps()` in the binding layer.
2. Use Rust `serde_json` behind a feature.

Preferred for Python compatibility: use Python `json.dumps()` so Python objects serialize according to Python semantics. Convert the resulting string to UTF-8 bytes.

Set `Content-Type: application/json` if absent.

Set `Content-Length` through core.

Tests:

- dict JSON reaches server
- list JSON reaches server
- custom unserializable object raises Python `TypeError`
- user content-type is preserved if provided

## Client defaults and merging

`Client` should support defaults:

- `headers`
- `timeout`
- possibly `base_url` only if desired later; defer for this milestone unless easy

Request-level values override client defaults.

Do not add `base_url` unless URL joining behavior is fully specified and tested.

Tests:

- default headers applied
- request override works
- request timeout override works
- client remains reusable after failed request-building conversion

## Request object

A minimal Python `Request` object may be useful but is not strictly required.

If added, keep it simple:

- `method`
- `url`
- `headers`
- `content`

Do not expose a mutable complex request object unless needed. HTTPX has rich request objects, but this can wait.

## Error behavior

Bad user arguments should raise Python `TypeError` or `ValueError` before calling Rust networking.

Protocol/build errors from Rust should map to eggfetch exceptions.

Unsupported kwargs should include the unsupported argument name in the message.

## Tests

Required tests:

- `params` mapping
- `params` repeated pairs
- existing query plus params
- invalid params type raises
- headers mapping
- request header overrides client default
- invalid header raises
- raw content bytes
- raw content str
- form `data` mapping
- form repeated pairs
- JSON body dict/list
- JSON serialization error
- conflict between `content` and `json` raises
- unsupported kwarg raises
- timeout scalar accepted
- request-level timeout override accepted

Run tests for both sync `Client` and `AsyncClient` where practical.

## Documentation

Document supported kwargs in README or Python docs.

Include a compatibility table:

```text
params: supported
headers: supported
content: supported
data: simple forms supported
json: supported
files: not yet
cookies: not yet
auth: not yet
redirects: Milestone J
proxies: not yet
```

## Acceptance criteria

Milestone I is complete when:

- supported kwargs behave consistently across top-level helpers, `Client`, and `AsyncClient`
- body precedence is explicit and tested
- params preserve repeated keys
- form and JSON body behavior works
- unsupported kwargs fail loudly
- client default merging is deterministic
- docs reflect the actual supported surface

Required validation:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cd crates/eggfetch-python && maturin develop && python -m pytest
```

## Handoff notes

Do not add multipart files here unless the core upload streaming path is mature. Multipart will become a separate compatibility and correctness task because it touches streaming, content length, filenames, content types, and memory behavior.
