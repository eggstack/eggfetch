# FFI Architecture

> User-facing overview. The canonical internal reference is
> [`docs/architecture/ffi-and-node.md`](../architecture/ffi-and-node.md).

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

`eggfetch-ffi` and `eggfetch-node` are the only crates in the workspace where
`unsafe` code is permitted (workspace `unsafe_code = "forbid"` with narrow
crate-level `allow` for their C/N-API boundaries). All `unsafe` is confined to:

1. `extern "C"` function definitions (required by the ABI)
2. Pointer dereferencing (with null checks)
3. `std::ptr::read` / `std::mem::forget` (for ownership transfer)

The crate validates all pointer inputs before delegating to `eggfetch-core`'s
safe API.

## Runtime Strategy

A global `OnceLock<Runtime>` provides a shared multi-thread tokio runtime. The
`blocking_send` helper (`crates/eggfetch-ffi/src/runtime.rs`) detects the caller's context:

- **Multi-thread tokio thread** (e.g. napi-rs): uses `tokio::task::block_in_place`
  on the ambient handle
- **Anything else** (non-tokio thread, or current-thread runtime where
  `block_in_place` would panic): spawns the future on the global FFI runtime
  via `try_ffi_runtime()` and blocks the caller on a channel

## Response Lifecycle

1. `eggfetch_client_send` performs the request synchronously (blocking)
2. Status, URL, and headers are extracted into `ResponseHandle`
3. The body is fully buffered into a `Vec<u8>`
4. The original `Response` (and its streaming body/pool lease) is dropped
5. The `ResponseHandle` is returned to the caller
6. The caller reads body via `eggfetch_response_body` or `eggfetch_response_text`
7. The caller frees the handle with `eggfetch_response_free`
