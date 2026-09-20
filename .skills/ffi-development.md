# FFI & Node Development Skill

Use this skill when working on the eggfetch-ffi or eggfetch-node crates.

## Workflow

1. Read `docs/architecture/ffi-and-node.md` for the architecture.
2. Read `docs/ffi/` for the user-facing FFI documentation.
3. Read existing FFI source in `crates/eggfetch-ffi/src/` and `crates/eggfetch-node/src/` for conventions.

## Key Constraints

- Both crates use `unsafe_code = "allow"` — the sole exceptions to the workspace `forbid`.
- All HTTP logic lives in eggfetch-core (CONNECT wire in `eggfetch-http-connect`). FFI and Node are adapters.
- FFI uses opaque handle pattern. Consumers never see internal struct layouts.
- FFI functions are `extern "C"` with `#[repr(C)]` types.
- Null pointer inputs are treated as no-ops for all free functions.
- Node.js is an explicitly experimental prototype that wraps FFI via
  napi-rs (blocking C ABI in `spawn_blocking`, string-only bodies,
  buffered responses, unstructured errors, stub `index.d.ts`), not core
  directly. Do not present it as a supported binding; the contract and
  unsupported list live in `docs/architecture/ffi-and-node.md`.

## Handle Types

| Handle | Thread Safety | Lifetime |
|--------|--------------|----------|
| `ClientBuilderHandle` | Single-thread, single-use | Consumed by `build()` or freed |
| `ClientHandle` | `Send + Sync` (shared via `Arc`) | Process-long, freed explicitly |
| `RequestHandle` | Single-thread, single-use | Consumed by `send()` or freed |
| `ResponseHandle` | Single-thread, single-use | Freed after body is read |
| `StreamingResponseHandle` | Thread-safe shared state (`Arc<StreamState>`; cancel from one thread while `next` blocks on another) | Freed after stream is consumed or cancelled |
| `ErrorHandle` | Single-thread, single-use | Freed after inspection |

## Runtime Bridge

FFI manages a global tokio runtime (`OnceLock<Runtime>`, multi-thread flavor,
never shut down). The `blocking_send` helper picks a strategy from the ambient
context (`crates/eggfetch-ffi/src/runtime.rs`):

- Inside a multi-thread runtime (e.g. napi-rs): `tokio::task::block_in_place`
  on the ambient handle.
- Otherwise (no runtime, or a current-thread runtime where `block_in_place`
  would panic): spawns the future on the global FFI runtime via
  `try_ffi_runtime()` and blocks the caller on a channel (preserving panic
  payloads through `catch_unwind`).

## Architecture References

- FFI & Node: `docs/architecture/ffi-and-node.md`
- FFI guide: `docs/ffi/`
