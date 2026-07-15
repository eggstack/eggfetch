# C ABI and FFI Binding Guide

This directory documents the C ABI boundary exposed by `eggfetch-ffi` for
consumption by foreign language bindings.

## Overview

`eggfetch-ffi` provides a stable C ABI layer over `eggfetch-core`. It is
designed for use by Node.js (N-API), Ruby (FFI/magnus), Zig, Java (JNI),
and any language that can call C-compatible functions.

## Key Concepts

### Opaque Handles

All complex types are represented as opaque pointers:

- `ClientHandle` — thread-safe, may be shared across threads
- `ClientBuilderHandle` — single-thread, consumed by build or freed
- `RequestHandle` — single-thread, consumed by `send()` or freed
- `ResponseHandle` — single-thread, freed after body is read
- `StreamingResponseHandle` — single-thread, body read via `stream_next`
- `ErrorHandle` — single-thread, freed after inspection

### Blocking API

All FFI functions are blocking. The internal tokio runtime handles async I/O
transparently. The API is safe to call from any thread, including within
existing tokio contexts (uses `block_in_place` to avoid nested `block_on`
panics).

### Memory Management

| Function | Purpose |
|----------|---------|
| `eggfetch_client_new` | Create a client with default settings |
| `eggfetch_client_free` | Free a client handle |
| `eggfetch_client_builder_new` | Create a client builder |
| `eggfetch_client_builder_build` | Build a client from the builder (consumes builder) |
| `eggfetch_client_builder_free` | Free a client builder |
| `eggfetch_request_free` | Free a request handle |
| `eggfetch_response_free` | Free a buffered response handle |
| `eggfetch_response_stream_free` | Free a streaming response handle |
| `eggfetch_stream_chunk_free` | Free a stream chunk |
| `eggfetch_error_free` | Free an error handle |
| `eggfetch_string_free` | Free a returned C string |
| `eggfetch_body_free` | Free a returned body buffer |

All free functions accept null pointers as a no-op.

### Configuration

The `ClientBuilder` API exposes full client configuration:

```c
eggfetch_client_builder *builder = eggfetch_client_builder_new();
eggfetch_client_builder_timeout(builder, 30);
eggfetch_client_builder_connect_timeout(builder, 5);
eggfetch_client_builder_follow_redirects(builder, 1);
eggfetch_client_builder_max_redirects(builder, 10);
eggfetch_client_builder_user_agent(builder, "my-app/1.0");
eggfetch_client_builder_http_version(builder, 2); // HTTP/2
eggfetch_client *client = eggfetch_client_builder_build(builder);
```

Per-request overrides are available on `RequestHandle`:

```c
eggfetch_request_timeout(req, 10);          // 10 second timeout
eggfetch_request_auth_basic(req, user, pass); // basic auth
eggfetch_request_decompress(req, 0);         // disable decompression
eggfetch_request_redirect_policy(req, 1, 5); // follow up to 5 redirects
```

### Streaming

For large responses, use the streaming API:

```c
eggfetch_streaming_response *resp = eggfetch_client_send_streaming(client, req, &err);
// Status and headers available immediately
uint16_t status = eggfetch_response_stream_status(resp);
// Read body chunks (blocks until chunk is available)
eggfetch_stream_chunk *chunk;
while ((chunk = eggfetch_response_stream_next(resp)) != NULL) {
    // process chunk->data, chunk->len
    eggfetch_stream_chunk_free(chunk);
}
eggfetch_response_stream_free(resp);
```

Cancel an in-progress stream:

```c
eggfetch_response_stream_cancel(resp); // subsequent next() calls return NULL
```

### Error Handling

Errors are returned via output parameters:

```c
eggfetch_error *err = NULL;
eggfetch_response *resp = eggfetch_client_send(client, req, &err);
if (resp == NULL) {
    // err is set — inspect with eggfetch_error_kind/message
    char *kind = eggfetch_error_kind(err);
    char *msg = eggfetch_error_message(err);
    // ... use kind and msg ...
    eggfetch_string_free(kind);
    eggfetch_string_free(msg);
    eggfetch_error_free(err);
}
```

### Feature Flags

The FFI crate mirrors `eggfetch-core`'s feature flags. Enable features via
Cargo features:

```toml
[dependencies]
eggfetch-ffi = { version = "0.1", features = ["http2", "cookies", "proxy"] }
```

## Node.js Binding

See `crates/eggfetch-node/` for a prototype Node.js binding using `napi-rs`.

## Exploratory Bindings

The following binding targets are **exploratory** and do not imply commitment
to maintain or ship:

- **Ruby** (via magnus or C ABI)
- **Perl** (via XS/C ABI)
- **Java/Kotlin** (via JNI or generated C ABI)
- **Zig/C consumers**

The only reference binding shipped as a prototype is **Node.js (N-API)**.
Other targets may be explored based on demand but should not be assumed
stable or supported.
