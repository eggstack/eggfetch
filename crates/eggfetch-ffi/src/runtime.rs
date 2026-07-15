//! Tokio runtime management for FFI.

use std::sync::OnceLock;

use tokio::runtime::Runtime;
use tokio::sync::oneshot;

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
/// If we are already inside a tokio runtime, we spawn the future on a
/// dedicated background thread with its own runtime to avoid nested
/// `block_on` panics. Otherwise we call `block_on` directly.
pub(crate) fn blocking_send<
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
>(
    future: F,
) -> T {
    // Check if we're inside a tokio runtime by trying to get a handle.
    // Handle::current() returns Err if not inside a runtime.
    match tokio::runtime::Handle::try_current() {
        Ok(_) => {
            // We're inside a tokio runtime. Spawn on a background thread
            // with its own runtime to avoid nested block_on.
            let (tx, rx) = oneshot::channel();
            std::thread::spawn(move || {
                let local_rt = Runtime::new().expect("failed to create blocking runtime");
                let result = local_rt.block_on(future);
                let _ = tx.send(result);
            });
            rx.blocking_recv().expect("blocking_send: channel closed")
        }
        Err(_) => {
            // Not inside a tokio runtime — safe to call block_on directly.
            ffi_runtime().block_on(future)
        }
    }
}
