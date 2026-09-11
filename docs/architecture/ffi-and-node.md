# FFI & Node Deep Dive

This document covers the C ABI bindings and the Node.js N-API prototype.

See also: [overview.md](overview.md).

## FFI (`eggfetch-ffi`)

### Architecture

- `unsafe_code = "allow"` — sole exception for FFI boundary.
- Depends only on `eggfetch-core`'s public API. Zero networking logic.
- Produces `cdylib`, `staticlib`, and `rlib` targets.
- Mirrors `eggfetch-core`'s feature flags.

### Handle Types

| Handle | Thread Safety | Lifetime |
|--------|--------------|----------|
| `ClientBuilderHandle` | Single-thread, single-use | Consumed by `build()` or freed |
| `ClientHandle` | `Send + Sync` (wraps `Client`, which is itself thread-safe) | Process-long, freed explicitly |
| `RequestHandle` | Single-thread, single-use | Consumed by `send()` or freed |
| `ResponseHandle` | Single-thread, single-use | Freed after body is read |
| `StreamingResponseHandle` | Thread-safe shared state (`Arc<StreamState>`; cancel from one thread while `next` blocks on another) | Freed after stream is consumed or cancelled |
| `ErrorHandle` | Single-thread, single-use | Freed after inspection |

All handles are opaque pointers (`*mut eggfetch_ffi_client`, etc.).

### Module Map

| Module | Purpose |
|--------|---------|
| `handle.rs` | Opaque handle type definitions |
| `client.rs` | Client creation and configuration |
| `request.rs` | Request building |
| `response.rs` | Response reading |
| `ffi_response.rs` | Response data extraction for FFI |
| `error.rs` | Error inspection |
| `builder.rs` | Builder configuration helpers |
| `runtime.rs` | Tokio runtime management |
| `streaming.rs` | Streaming body support |

### Runtime Bridge

`eggfetch-ffi` manages a global tokio runtime (`OnceLock<Runtime>`, multi-thread flavor, never shut down). The `blocking_send` helper picks a strategy from the ambient context:

- **Outside any runtime**: calls `ffi_runtime().block_on()` directly.
- **Inside a multi-thread runtime** (e.g. napi-rs): `tokio::task::block_in_place` on the ambient handle.
- **Inside a current-thread runtime**: spawns the future on the global FFI runtime and blocks the caller on a channel (avoids `block_in_place` panics).

This ensures the FFI works correctly from both sync C code and async-aware host runtimes.

### String and Memory Management

- Returned strings (`FfiString`) are heap-allocated C strings. Callers free with `eggfetch_string_free`.
- Body buffers are heap-allocated with `std::alloc`. Callers free with `eggfetch_body_free`.
- Null pointer inputs are treated as no-ops for all free functions.

### C API Pattern

```c
// Create client
ClientHandle *client = eggfetch_client_new();

// Build request (or use the verb helpers: eggfetch_client_get/post/...)
RequestHandle *req = eggfetch_client_request(client, "GET", "https://example.com");

// Send (blocking, consumes req); on failure *err_out holds an ErrorHandle
ResponseHandle *resp = eggfetch_client_send(client, req, &err);

// Read body (out-params; buffer allocated with std::alloc)
uint8_t *data; size_t len;
eggfetch_response_body(resp, &data, &len);

// Cleanup
eggfetch_body_free(data, len);
eggfetch_response_free(resp);
eggfetch_client_free(client);
```

Streaming uses `eggfetch_client_send_streaming` + `eggfetch_response_stream_next` (chunks are `StreamChunk`, freed with `eggfetch_stream_chunk_free`); errors are inspected via `eggfetch_error_kind`/`eggfetch_error_message` and freed with `eggfetch_error_free`.

## Node.js (`eggfetch-node`)

### Support status: experimental prototype

Per `plans/node-binding-maturation.md`, the Node crate closes as a
**deliberately experimental prototype**, not a supported binding. The
supported-binding path (direct async dispatch into `eggfetch-core`,
Rust-owned in-flight lifetimes, byte/stream bodies, incremental response
streaming, cancellation, structured errors, generated TypeScript
declarations) remains deferred work: each item is a new API surface with
its own ownership, backpressure, redaction, and semver commitments, and
landing them together immediately before the exact-SHA HTTPX
requalification freeze would widen that freeze for no parity benefit.
Direct dispatch is not blocked on feasibility — core `Client` is
`Clone + Send + Sync` — it is deferred on scope.

### Architecture

- Prototype using napi-rs to wrap `eggfetch-ffi`.
- `unsafe_code = "allow"` — sole exception for N-API.
- Modules: `client.rs`, `response.rs`, `lib.rs`.
- Ordinary requests execute the **blocking C ABI**
  (`eggfetch_client_send`) inside `spawn_blocking`; Node adds no HTTP
  behavior of its own and all I/O still originates in `eggfetch-core`
  via the FFI runtime bridge.
- The client handle is a raw FFI pointer stored as `usize`. In-flight
  safety currently relies on napi-derive's internal strong-reference
  codegen for async class methods (see the lifetime-safety comment on
  `EggfetchClient`), not on Rust ownership — this is acceptable for a
  prototype and must be replaced by `Arc`-owned state before any
  supported release.

### Narrow prototype guarantees

- Async `Client` request methods for the common verbs plus arbitrary
  methods via `request(method, url, body)`.
- UTF-8 string request bodies (`Option<String>`); empty body when `None`.
- Buffered responses only: status, URL, headers (duplicates preserved
  internally, joined in the `headers` object, individually via `getAll`),
  `text` / `bytes` / `json` accessors, `ok` flag.
- Rust-side compilation and unit surface covered by Tier 1
  (`cargo test -p eggfetch-node`); the JS surface (`test.js`) runs in
  Tier 1 only when `node` and a built `./eggfetch.node` artifact are
  present, and records an explicit skip otherwise.

### Explicitly unsupported (do not rely on these)

- Binary request bodies: `Buffer` / `Uint8Array` inputs have no
  lossless path; only UTF-8 strings are accepted.
- Response streaming: no async-iterator / Node-stream API; bodies are
  buffered eagerly (must fit in memory).
- Cancellation: no abort/close propagation to in-flight requests.
- Request configuration: no headers, timeout, redirect, TLS/CA, proxy,
  auth, or HTTP-version surface beyond FFI defaults.
- Structured errors: failures surface as a single
  `"eggfetch error [kind]: message"` string; assert on categories only
  through that string, and expect it to change.
- TypeScript declarations: `index.d.ts` is a stub, not generated output.
- Packaging: no npm publication pipeline; the `index.js` loader expects
  a manually placed `./eggfetch.node` artifact.
- Trailers: deferred like the other non-Python adapters (core retains
  them; see the workspace trailer policy in `AGENTS.md`).
