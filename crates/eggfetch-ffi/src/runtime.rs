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
/// If we are already inside a multi-thread tokio runtime (e.g. from
/// napi-rs), we use `block_in_place` to safely block the current worker
/// thread while driving the future on the runtime. Inside a
/// current-thread runtime `block_in_place` would panic, so the future is
/// instead spawned on the global FFI runtime and awaited through a
/// channel; the calling thread blocks on the channel while the global
/// runtime's workers make progress. Outside any runtime we call
/// `block_on` directly on the global FFI runtime.
pub(crate) fn blocking_send<
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
>(
    future: F,
) -> eggfetch_core::Result<T> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle)
            if matches!(
                handle.runtime_flavor(),
                tokio::runtime::RuntimeFlavor::MultiThread
            ) =>
        {
            // We're inside a multi-thread tokio runtime (e.g. napi-rs
            // async executor). block_in_place converts the current worker
            // thread into a blocking context, allowing handle.block_on to
            // run the future.
            Ok(tokio::task::block_in_place(move || handle.block_on(future)))
        }
        _ => {
            // Not inside a multi-thread tokio runtime — either no runtime
            // at all (safe to block_on directly) or a current-thread
            // runtime (drive the future on the dedicated FFI runtime so
            // the caller's single worker stays responsive).
            let (tx, rx) = std::sync::mpsc::channel();
            ffi_runtime().spawn(async move {
                let _ = tx.send(Ok(future.await));
            });
            rx.recv().unwrap_or_else(|e| {
                Err(eggfetch_core::Error::Io(std::sync::Arc::new(
                    std::io::Error::other(format!(
                        "eggfetch-ffi blocking_send: runtime task dropped ({e})"
                    )),
                )))
            })
        }
    }
}
