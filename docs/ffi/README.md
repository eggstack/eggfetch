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
- `RequestHandle` — single-thread, consumed by `send()` or freed
- `ResponseHandle` — single-thread, freed after body is read
- `ErrorHandle` — single-thread, freed after inspection

### Blocking API

All FFI functions are blocking. The internal tokio runtime handles async I/O
transparently. The API is safe to call from any thread, including within
existing tokio contexts (a background thread is spawned to avoid nested
`block_on` panics).

### Memory Management

| Function | Purpose |
|----------|---------|
| `eggfetch_client_free` | Free a client handle |
| `eggfetch_request_free` | Free a request handle |
| `eggfetch_response_free` | Free a response handle |
| `eggfetch_error_free` | Free an error handle |
| `eggfetch_string_free` | Free a returned C string |
| `eggfetch_body_free` | Free a returned body buffer |

All free functions accept null pointers as a no-op.

### Error Handling

Errors are returned via output parameters:

```c
eggfetch_error *err = NULL;
eggfetch_response *resp = eggfetch_client_send(client, req, &err);
if (resp == NULL) {
    // err is set — inspect with eggfetch_error_kind/message
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
