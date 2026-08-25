# FFI & Node Development Skill

Use this skill when working on the eggfetch-ffi or eggfetch-node crates.

## Workflow

1. Read `docs/architecture/ffi-and-node.md` for the architecture.
2. Read `docs/ffi/` for the user-facing FFI documentation.
3. Read existing FFI source in `crates/eggfetch-ffi/src/` and `crates/eggfetch-node/src/` for conventions.

## Key Constraints

- Both crates use `unsafe_code = "allow"` — the sole exceptions to the workspace `forbid`.
- All HTTP logic lives in eggfetch-core. FFI and Node are adapters.
- FFI uses opaque handle pattern. Consumers never see internal struct layouts.
- FFI functions are `extern "C"` with `#[repr(C)]` types.
- Null pointer inputs are treated as no-ops for all free functions.
- Node.js wraps FFI via napi-rs, not core directly.

## Handle Types

| Handle | Thread Safety | Lifetime |
|--------|--------------|----------|
| `ClientBuilderHandle` | Single-thread, single-use | Consumed by `build()` or freed |
| `ClientHandle` | `Send + Sync` (shared via `Arc`) | Process-long, freed explicitly |
| `RequestHandle` | Single-thread, single-use | Consumed by `send()` or freed |
| `ResponseHandle` | Single-thread, single-use | Freed after body is read |
| `StreamingResponseHandle` | Single-thread, single-use | Freed after final chunk is read |
| `ErrorHandle` | Single-thread, single-use | Freed after inspection |

## Runtime Bridge

FFI manages a global tokio runtime (`OnceLock<Runtime>`, multi-thread flavor,
never shut down). The `blocking_send` helper picks a strategy from the ambient
context (`crates/eggfetch-ffi/src/runtime.rs`):

- Outside any runtime: calls `ffi_runtime().block_on()` directly.
- Inside a multi-thread runtime (e.g. napi-rs): `tokio::task::block_in_place`
  on the ambient handle.
- Inside a current-thread runtime: spawns the future on the global FFI runtime
  and blocks the caller on a channel (avoids `block_in_place` panics).

## Architecture References

- FFI & Node: `docs/architecture/ffi-and-node.md`
- FFI guide: `docs/ffi/`
