# Milestone R Plan: Response Compression and Decompression

## Objective

Implement configurable, streaming response decompression in `eggfetch-core` and expose predictable requests/httpx-like behavior through the Python clients.

The implementation must preserve backpressure, read-timeout semantics, pool/body lifecycle, and bounded memory use. Compression support should remain feature-gated by algorithm to preserve the project’s dependency and auditability goals.

## Prerequisites

Milestone Q should be complete or sufficiently isolated. The validation/polish pass must already have proven live response streaming and cancellation behavior.

Required stable foundations:

- live response body streams
- Python `StreamingResponse`
- per-chunk read timeout behavior
- response header mutation rules
- feature-matrix CI

## Scope

Milestone R includes:

- gzip decompression
- deflate decompression
- brotli decompression
- zstd decompression
- `Accept-Encoding` negotiation
- streaming decoder wrappers
- decoded/raw header policy
- buffered and live-streaming response integration
- decompression limits and error mapping
- Python configuration

Milestone R does not initially include:

- outgoing request compression
- content-type-specific transformations
- archive extraction
- transparent recompression

## Feature flags and dependencies

Use independent features:

```toml
compression-gzip = [...]
compression-brotli = [...]
compression-zstd = [...]
```

Deflate may share the gzip implementation crate if appropriate.

Evaluate small, mature async/stream-capable crates. Prefer decoders that can be wrapped around `AsyncRead` or body streams without buffering the full response.

Document each dependency and transitive tree.

## Client configuration

Add core configuration:

```rust
Client::builder()
    .automatic_decompression(true)
    .accept_encoding(...)
```

Default behavior for Python should likely enable all compiled-in decoders, matching mainstream client expectations.

Rust core default may also enable automatic decompression when compression features are enabled, but this must be explicit in docs.

Support per-request override if straightforward:

```rust
client.get(url).automatic_decompression(false)
```

Python target:

```python
Client(decompress=True)
client.get(url, decompress=False)
```

Do not add a public Python kwarg unless the semantics are stable and useful.

## Accept-Encoding generation

If automatic decompression is enabled and the user has not supplied `Accept-Encoding`, generate an ordered list of compiled-in algorithms.

Example:

```text
Accept-Encoding: gzip, deflate, br, zstd
```

Do not advertise algorithms that are not compiled in.

If the user supplies `Accept-Encoding`, define whether eggfetch automatically decodes matching encodings. Recommended policy:

- user header controls negotiation
- automatic decoding still occurs for supported received encodings unless decompression is disabled

Document this clearly.

## Content-Encoding parsing

Parse `Content-Encoding` as an ordered list.

Multiple encodings must be decoded in reverse application order.

Example:

```text
Content-Encoding: gzip, br
```

means decode brotli first, then gzip.

Reject or surface unsupported encodings deterministically rather than returning silently corrupted content.

Possible policy:

- if any content coding is unsupported, leave the body raw and expose headers unchanged
- or raise `UnsupportedContentEncoding`

Recommended for correctness: raise a structured error when automatic decompression is enabled and the server claims an unsupported encoding. If decompression is disabled, return raw bytes.

## Streaming decoder pipeline

Wrap the incoming `ResponseBody` stream in one or more decoder layers.

Requirements:

- no full-body buffering
- natural backpressure
- decoder errors map to `Error::Decompression`
- pool lease remains held through the decoded stream lifetime
- read timeout should apply to underlying network reads
- cancellation drops decoder and underlying stream

Do not release the pool lease merely because compressed bytes have been read into an unbounded decoder buffer. Keep buffers bounded.

## Header policy

After automatic decompression, decide how response headers are exposed.

Common client behavior removes or adjusts:

- `Content-Encoding`
- `Content-Length`

Recommended:

- remove `Content-Encoding` from the decoded response header view
- remove `Content-Length` because it describes compressed bytes
- preserve original values in internal metadata if future raw access is needed

Do not replace Content-Length with decoded length for streaming responses.

Add optional response metadata later if useful:

```python
response.original_headers
```

Not required for this milestone.

## Buffered responses

Buffered `get()`/`post()` responses should transparently expose decoded `.content`, `.text`, and `.json()`.

Decoding errors should occur during request completion/body buffering and map to Python decompression exceptions.

## Streaming responses

`client.stream()` should yield decoded bytes incrementally by default.

If raw compressed bytes are needed, provide a deliberate API rather than an ambiguous iterator. Options:

- `iter_raw()` / `aiter_raw()`
- `decompress=False`

Preferred initial route: `decompress=False` on the request/client and keep iterators consistently decoded otherwise.

Text decoding must occur after content decompression.

## Deflate compatibility

HTTP `deflate` is historically ambiguous between zlib-wrapped and raw DEFLATE streams.

Decide whether to support both forms. Mainstream compatibility often attempts zlib first and raw deflate fallback.

If fallback is implemented, keep it streaming-safe and tested. Document behavior.

## Resource limits and decompression bombs

Automatic decompression can create extreme expansion ratios.

Introduce configurable safeguards:

- optional maximum decoded bytes
- optional maximum expansion ratio after enough input is observed
- maximum decoder nesting count

Initial default can be unlimited decoded size for compatibility if streaming remains bounded, but production/security docs must explain the risk. Prefer adding at least a maximum nesting depth now.

Python may expose `max_response_size` later; do not overexpand API surface in this milestone unless already planned.

## Errors

Add structured errors:

- unsupported content encoding
- malformed compressed stream
- decoded-size limit exceeded
- excessive encoding nesting

Map to Python exceptions under `RequestError` or a dedicated `DecodingError` hierarchy.

Avoid including compressed payload data in errors.

## Redirect interaction

Redirect response bodies may be compressed. The redirect engine must drain/decode or safely discard them without leaking permits and within the total timeout budget.

It may be more efficient to discard compressed bytes without full decoding when the body is not retained. If doing so, ensure connection reuse remains safe.

History metadata should reflect the header policy consistently.

## Tests

### Algorithm tests

For each enabled algorithm:

- buffered response decodes correctly
- streaming response decodes incrementally
- empty body
- very small chunks
- truncated/corrupt stream errors
- cancellation mid-decode

### Negotiation tests

- generated Accept-Encoding matches compiled features
- user Accept-Encoding is preserved
- decompression-disabled request returns raw body
- unsupported encoding behavior matches policy
- multiple encodings decode in reverse order

### Header tests

- Content-Encoding removed after decode
- Content-Length removed after decode
- raw/decompression-disabled response preserves headers

### Security tests

- excessive nesting rejected
- decoded-size limit if implemented
- large expansion remains bounded-memory under streaming

### Python tests

- buffered `.content`, `.text`, `.json()` decoded
- sync live stream decoded
- async live stream decoded
- disable decompression
- malformed stream exception mapping
- sync/async parity

## Documentation

Document:

- enabled algorithms and feature flags
- default Accept-Encoding behavior
- decoded versus raw semantics
- header mutation
- deflate compatibility
- security/resource limits
- differences from requests/httpx

## Acceptance criteria

Milestone R is complete when:

- all compiled-in encodings decode in buffered and streaming APIs
- decoding preserves backpressure and lifecycle semantics
- headers accurately describe decoded content
- malformed/unsupported encodings fail deterministically
- compression features build independently
- Python sync/async behavior matches
- security limits/policy are documented

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps

cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-brotli
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-zstd

cd crates/eggfetch-python
maturin develop
python -m pytest
```

## Handoff note

Milestone S adds proxies at the connector layer. Keep decompression entirely above transport/proxy concerns so proxied and direct responses share identical decoding behavior.
