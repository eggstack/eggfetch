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
| `ClientHandle` | `Send + Sync` (shared via `Arc`) | Process-long, freed explicitly |
| `RequestHandle` | Single-thread, single-use | Consumed by `send()` or freed |
| `ResponseHandle` | Single-thread, single-use | Freed after body is read |
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

`eggfetch-ffi` manages a global tokio runtime (`OnceLock<Runtime>`). The `blocking_send` helper:

- **Outside tokio**: calls `ffi_runtime().block_on()` directly.
- **Inside tokio**: spawns a dedicated thread with its own runtime to avoid nested `block_on` panics.

This ensures the FFI works correctly from both sync C code and async-aware host runtimes.

### String and Memory Management

- Returned strings (`FfiString`) are heap-allocated C strings. Callers free with `eggfetch_string_free`.
- Body buffers are heap-allocated with `std::alloc`. Callers free with `eggfetch_body_free`.
- Null pointer inputs are treated as no-ops for all free functions.

### C API Pattern

```c
// Create client
eggfetch_ffi_client *client = eggfetch_client_new();

// Build request
eggfetch_ffi_request *req = eggfetch_request_new(client, "GET", "https://example.com");

// Send (blocking)
eggfetch_ffi_response *resp = eggfetch_request_send(req);

// Read body
eggfetch_ffi_body_chunk chunk = eggfetch_response_read_body(resp);

// Cleanup
eggfetch_body_free(chunk.data);
eggfetch_response_free(resp);
eggfetch_request_free(req);
eggfetch_client_free(client);
```

## Node.js (`eggfetch-node`)

### Architecture

- Prototype using napi-rs to wrap `eggfetch-ffi`.
- `unsafe_code = "allow"` — sole exception for N-API.
- Modules: `client.rs`, `response.rs`, `lib.rs`.

### API

```javascript
const { EggfetchClient } = require('eggfetch');

const client = new EggfetchClient();
const response = await client.get('https://example.com');
console.log(response.statusCode, response.text());
```

### Limitations

- Prototype stage — API surface may change.
- Wraps FFI rather than calling core directly.
- No streaming support yet.
