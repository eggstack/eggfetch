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
| `ClientHandle` | `Send + Sync` (shared via `Arc`) | Process-long, freed explicitly |
| `RequestHandle` | Single-thread, single-use | Consumed by `send()` or freed |
| `ResponseHandle` | Single-thread, single-use | Freed after body is read |
| `ErrorHandle` | Single-thread, single-use | Freed after inspection |

## Runtime Bridge

FFI manages a global tokio runtime (`OnceLock<Runtime>`). The `blocking_send` helper:
- Outside tokio: calls `ffi_runtime().block_on()` directly.
- Inside tokio: spawns a dedicated thread with its own runtime.

## Architecture References

- FFI & Node: `docs/architecture/ffi-and-node.md`
- FFI guide: `docs/ffi/`
