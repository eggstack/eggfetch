# Core Surface Audit for FFI

This document records the FFI safety audit performed during Milestone Z. It covers the ownership model, thread safety, body stream handling, error mapping, and other concerns that must be correct before exposing `eggfetch-core` across a C ABI boundary.

## Ownership and Lifetimes

### Handle Types

| Handle | Inner Type | Ownership | Thread Safety |
|--------|-----------|-----------|---------------|
| `ClientHandle` | `Client` (Arc-backed) | Owned, caller frees with `eggfetch_client_free` | `Send + Sync` |
| `ClientBuilderHandle` | `ClientBuilder` | Owned; inner builder taken by build, shell freed with `eggfetch_client_builder_free` | Single-thread |
| `RequestHandle` | `RequestBuilder` | Owned, consumed by send or freed with `eggfetch_request_free` | Single-thread |
| `ResponseHandle` | Flattened `status + url + headers + body` | Owned, caller frees with `eggfetch_response_free` | Single-thread |
| `StreamingResponseHandle` | Flattened headers + `mpsc::Receiver` | Owned, caller frees with `eggfetch_response_stream_free` | Single-thread |
| `ErrorHandle` | `kind + message` strings | Owned, caller frees with `eggfetch_error_free` | Single-thread |

### Consumption Patterns

- **RequestBuilder consumption**: `eggfetch_client_send` and `eggfetch_client_send_streaming` consume the `RequestHandle` via `ptr::read` + `mem::forget`. The handle must not be used after sending.
- **ClientBuilder build semantics**: `eggfetch_client_builder_build` takes the builder value out of the handle but does **not** free the handle allocation. The handle stays valid until `eggfetch_client_builder_free` is called exactly once; rebuilding returns null instead of dangling. This keeps the consumed state distinguishable from allocation failure without a use-after-free window.
- **Response ownership**: `ResponseHandle` eagerly buffers the entire body. `StreamingResponseHandle` holds the body stream via an `mpsc::Receiver`.

### Memory Safety Rules

1. All handles must be freed exactly once via their respective `_free` functions.
2. Null pointer arguments are handled as no-ops or return null/ -1 as documented.
3. Consumed request handles must not be used after the consuming call. Builder handles remain valid after `build` and must still be freed exactly once.
4. `FfiString` values must be freed with `eggfetch_string_free`.
5. Body buffers from `eggfetch_response_body` must be freed with `eggfetch_body_free`.

## Body Stream Handling

### Buffered Path (`eggfetch_client_send`)

The entire response body is read in a single async block (`resp.bytes().await`), then copied into a `Vec<u8>` inside `ResponseHandle`. This avoids cross-runtime streaming issues. The tradeoff is that the full body must fit in memory.

### Streaming Path (`eggfetch_client_send_streaming`)

1. Response headers are collected eagerly (same as buffered path).
2. `resp.bytes_stream()` extracts the body stream.
3. A tokio task reads chunks from the stream and sends them over an `mpsc::channel(16)`.
4. The caller reads chunks via `eggfetch_response_stream_next`, which calls `rx.blocking_recv()`.
5. Cancellation is signaled via an `AtomicBool` + channel close.

### Cross-Runtime Safety

- `blocking_send` detects whether the caller is inside a tokio runtime (e.g. napi-rs).
- If inside a runtime: uses `tokio::task::block_in_place` + `handle.block_on` to safely block.
- If outside a runtime: uses the global FFI runtime's `block_on`.
- The streaming background task is spawned on the runtime that `block_on` runs on, ensuring the mpsc channel and stream are on the same runtime.

## Error Serialization

Errors are serialized as `(kind: String, message: String)` pairs. The `kind` comes from `eggfetch_core::Error::kind()` which returns string labels like:

- `"connect"` — connection failures
- `"timeout"` — timeout exceeded
- `"tls"` — TLS handshake errors
- `"http"` — HTTP protocol errors
- `"body"` — body read/decode errors
- `"redirect"` — redirect limit exceeded
- `"request"` — invalid request construction

This is a lossy mapping — structured error codes are not exposed. Callers must match on string values. Future improvement: add numeric error codes.

## Thread Safety

- `ClientHandle` wraps `Client` which is `Arc<ClientInner>` — safe to share across threads.
- The global FFI runtime uses `OnceLock<Runtime>` — thread-safe initialization.
- `blocking_send` is thread-safe and can be called from any thread.
- Streaming cancellation uses `AtomicBool` with `Relaxed` ordering — sufficient for a cancellation flag.
- The `mpsc::channel` in streaming is async-safe and thread-safe.

## Configuration Surface

### ClientBuilder (via FFI)

Exposed configuration options:
- Timeouts: overall, connect, read, write
- Redirects: follow/no-follow, max count
- HTTP version: HTTP/1.1 only, HTTP/2 only, auto (with/without HTTP/3)
- Decompression: enable/disable, max body size, max ratio
- Pool: max idle connections, per-host limits
- TLS: insecure mode (danger_accept_invalid_certs)
- Auth: basic auth, bearer token
- Headers: user-agent, default headers

### Per-Request Configuration

- Timeout override
- Auth override (basic, bearer, or opt-out)
- Redirect policy override
- Decompression override

### Not Exposed (Future Work)

- Proxy configuration (feature-gated, not yet wired to FFI)
- Cookie jar (feature-gated, not yet wired to FFI)
- Retry policy (exists in core, not yet wired to FFI)
- TLS certificate pinning (exists in core, not yet wired to FFI)
- Custom TLS config beyond insecure mode
- Multipart uploads (feature-gated, not yet wired to FFI)

## Acceptance Criteria Met

1. **Core façade is language-neutral**: All FFI functions use C-compatible types (pointers, integers, C strings). No Rust-specific types leak.
2. **One reference secondary binding prototyped**: Node.js N-API binding (`eggfetch-node`) is functional with 10 passing tests.
3. **Semantics centralized in core**: FFI and Node.js are thin wrappers. All HTTP behavior lives in `eggfetch-core`.
4. **Distribution and maintenance**: CI workflow defined. FFI crate builds as cdylib + staticlib + rlib.
5. **Unsupported bindings**: Documented as exploratory in README.

## Known Limitations

1. **No proxy/cookie/retry/multipart FFI surface**: Feature flags exist but no FFI functions expose them.
2. **Error mapping is string-based**: No numeric error codes for efficient matching.
3. **Streaming is blocking**: Each `eggfetch_response_stream_next` call blocks until a chunk arrives.
4. **No cancellation token propagation**: Cancel only works for streaming responses, not in-flight buffered requests.
5. **No per-request TLS configuration**: Only client-level TLS settings are supported.
