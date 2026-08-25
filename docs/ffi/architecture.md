# FFI Architecture

## Crate Layout

```
crates/
  eggfetch-core/    # Async HTTP engine (safe Rust)
  eggfetch-ffi/     # C ABI boundary (unsafe Rust, allowed by lint override)
  eggfetch-node/    # Node.js N-API binding (thin wrapper over eggfetch-ffi)
  eggfetch-cli/     # CLI adapter (thin wrapper over eggfetch-core)
  eggfetch-python/  # Python bindings via PyO3
```

## Unsafe Boundary

`eggfetch-ffi` is the only crate in the workspace where `unsafe` code is
permitted. All `unsafe` is confined to:

1. `extern "C"` function definitions (required by the ABI)
2. Pointer dereferencing (with null checks)
3. `std::ptr::read` / `std::mem::forget` (for ownership transfer)

The crate validates all pointer inputs before delegating to `eggfetch-core`'s
safe API.

## Runtime Strategy

A global `OnceLock<Runtime>` provides a shared multi-thread tokio runtime. The
`blocking_send` helper detects the caller's context:

- **Non-tokio thread**: calls `ffi_runtime().block_on()` directly
- **Multi-thread tokio thread** (e.g. napi-rs): uses `tokio::task::block_in_place`
  on the ambient handle
- **Current-thread tokio runtime**: spawns the future on the global FFI runtime
  and blocks the caller on a channel, preventing nested `block_on` panics

## Response Lifecycle

1. `eggfetch_client_send` performs the request synchronously (blocking)
2. Status, URL, and headers are extracted into `ResponseHandle`
3. The body is fully buffered into a `Vec<u8>`
4. The original `Response` (and its streaming body/pool lease) is dropped
5. The `ResponseHandle` is returned to the caller
6. The caller reads body via `eggfetch_response_body` or `eggfetch_response_text`
7. The caller frees the handle with `eggfetch_response_free`
