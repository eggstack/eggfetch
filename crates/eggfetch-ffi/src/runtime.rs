//! Tokio runtime management for FFI.

use std::sync::OnceLock;

use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Get or initialize the global FFI runtime.
///
/// The runtime is created lazily on first call with multi-thread configuration.
/// It is never shut down — it lives for the process lifetime.
#[must_use]
pub(crate) fn ffi_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().expect("failed to create eggfetch-ffi tokio runtime"))
}

/// Block on an async future, safe to call from any context (tokio or not).
///
/// If we are already inside a tokio runtime (e.g. from napi-rs), we use
/// `block_in_place` to safely block the current worker thread while driving
/// the future on the runtime. Otherwise we call `block_on` directly on the
/// global FFI runtime.
pub(crate) fn blocking_send<
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
>(
    future: F,
) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // We're inside a tokio runtime (e.g. napi-rs async executor).
            // block_in_place converts the current worker thread into a
            // blocking context, allowing handle.block_on to run the future.
            tokio::task::block_in_place(move || handle.block_on(future))
        }
        Err(_) => {
            // Not inside a tokio runtime — safe to call block_on directly.
            ffi_runtime().block_on(future)
        }
    }
}
