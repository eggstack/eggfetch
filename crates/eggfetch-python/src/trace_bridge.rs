//! Python trace callback bridge.
//!
//! HTTPX's `trace` extension expects a Python callable that receives
//! `(event_name: str, info: dict)` invocations for each lifecycle event.
//! `eggfetch-core` exposes a typed [`TraceEvent`] enum and a
//! [`TraceObserver`] callback trait.  This module bridges the two.
//!
//! Design constraints (per plan Corrective 02 §C):
//!
//! - Python callables never enter `eggfetch-core`.  Core only ever
//!   sees the [`Arc<PyTraceObserver>`] wrapper, which implements
//!   [`TraceObserver`].
//! - Callback execution happens while the GIL is held.
//! - Sync callbacks run inline on the transport thread.
//! - Async callbacks must be awaited; calling an async callable
//!   synchronously without awaiting is a contract violation and is
//!   reported as a clear error.
//! - Events are delivered in the order the core transport emits them.
//! - Callback exceptions are captured in a shared error slot and
//!   surface through the original request future.

use std::fmt;
use std::sync::{Arc, Mutex};

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};

use eggfetch_core::trace::{event_to_httpcore_name, TraceEvent, TraceObserver};

/// Errors raised by a user-supplied trace callback.
#[derive(Debug)]
pub(crate) enum TraceBridgeError {
    /// The Python callable raised an exception.
    Callback(PyErr),
    /// The callable was an async (coroutine) function invoked without
    /// awaiting, which violates the httpcore callback contract.
    NotAwaited,
}

impl fmt::Display for TraceBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Callback(_) => f.write_str("trace callback raised an exception"),
            Self::NotAwaited => f.write_str(
                "async trace callback was not awaited; sync transport cannot drive coroutines",
            ),
        }
    }
}

impl std::error::Error for TraceBridgeError {}

/// Shared error slot between the observer and the request future.
///
/// Core's `TraceObserver::on_event` returns `()` and cannot abort the
/// future directly, so callback errors are recorded here and read by
/// the request future after dispatch completes.
#[derive(Clone)]
pub(crate) struct CallbackErrorSlot {
    inner: Arc<Mutex<Option<TraceBridgeError>>>,
}

impl CallbackErrorSlot {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }

    fn record(&self, err: TraceBridgeError) {
        if let Ok(mut guard) = self.inner.lock() {
            // Preserve the first error so callers see the originating
            // failure rather than a later side-effect.
            if guard.is_none() {
                *guard = Some(err);
            }
        }
    }

    pub(crate) fn take(&self) -> Option<TraceBridgeError> {
        self.inner.lock().ok().and_then(|mut g| g.take())
    }
}

impl Default for CallbackErrorSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CallbackErrorSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallbackErrorSlot").finish()
    }
}

/// A trace observer that invokes a Python callable for each event.
pub(crate) struct PyTraceObserver {
    callback: Py<PyAny>,
    error_slot: CallbackErrorSlot,
}

impl fmt::Debug for PyTraceObserver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PyTraceObserver")
            .field("callback", &"<py callable>")
            .finish()
    }
}

impl PyTraceObserver {
    /// Wrap a Python callable as a trace observer with its own error
    /// slot.  The returned [`CallbackErrorSlot`] is the handle the
    /// request future uses to surface callback errors.
    ///
    /// Async callables (detected via `inspect.iscoroutinefunction`)
    /// cannot be driven from a sync request future, so we record a
    /// `NotAwaited` error eagerly.  The `AsyncClient` accepts them but
    /// must drive the coroutine itself.
    pub(crate) fn new(py: Python<'_>, callback: Bound<'_, PyAny>) -> (Self, CallbackErrorSlot) {
        let is_async = py
            .import("inspect")
            .ok()
            .and_then(|m| {
                m.call(
                    (PyString::new(py, "iscoroutinefunction"), callback.clone()),
                    None,
                )
                .ok()
            })
            .and_then(|r| r.is_truthy().ok())
            .unwrap_or(false);
        let error_slot = CallbackErrorSlot::new();
        if is_async {
            error_slot.record(TraceBridgeError::NotAwaited);
        }
        let observer = Self {
            callback: callback.unbind(),
            error_slot: error_slot.clone(),
        };
        (observer, error_slot)
    }
}

impl TraceObserver for PyTraceObserver {
    fn on_event(&self, event: &TraceEvent) {
        // Acquire the GIL only at the callback delivery point.  We do
        // not block on user code from outside the GIL.  All event-name
        // and info construction happens inside the GIL block.
        let result = Python::with_gil(|py| {
            let (_prefix, name) = event_to_httpcore_name(event);
            let info_dict = match build_info_dict(py, event) {
                Ok(d) => d,
                Err(e) => {
                    return Err(TraceBridgeError::Callback(e));
                }
            };
            let cb = self.callback.bind(py);
            let call_result = cb.call1((name.as_str(), info_dict.unbind()));
            match call_result {
                Ok(_) => Ok(()),
                Err(e) => Err(TraceBridgeError::Callback(e)),
            }
        });

        if let Err(err) = result {
            self.error_slot.record(err);
        }
    }
}

/// Build the info dict for a [`TraceEvent`], matching the
/// `httpcore 1.0.9` vocabulary documented in
/// `compat/httpx/0.28.1/trace-vocabulary.md`.
///
/// Only the keys the vocabulary defines for that event are emitted.
/// Unknown keys (e.g. ``request`` / ``return_value``) are deliberately
/// not synthesized because the native binding does not hold an HTTPX
/// `Request` object — passing Python objects into `eggfetch-core` is
/// forbidden by the hard rule in `AGENTS.md`.
fn build_info_dict<'py>(py: Python<'py>, event: &TraceEvent) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    match event {
        TraceEvent::ConnectTcp { host, port, .. } => {
            dict.set_item("host", host.clone())?;
            dict.set_item("port", *port)?;
        }
        TraceEvent::ConnectUnixSocket { path, .. } => {
            dict.set_item("path", path.clone())?;
        }
        TraceEvent::StartTls {
            server_hostname, ..
        } => {
            dict.set_item("server_hostname", server_hostname.clone())?;
        }
        TraceEvent::Retry { delay_ms, .. } => {
            dict.set_item("delay_ms", *delay_ms)?;
        }
        TraceEvent::Close { .. }
        | TraceEvent::SendRequestBody { .. }
        | TraceEvent::ReceiveResponseBody { .. }
        | TraceEvent::ResponseClosed { .. } => {}
        TraceEvent::SendRequestHeaders { method, target, .. } => {
            dict.set_item("method", method.clone())?;
            dict.set_item("target", target.clone())?;
        }
        TraceEvent::ReceiveResponseHeaders { status, .. } => {
            dict.set_item("status", *status)?;
        }
    }
    Ok(dict)
}

/// Convert a [`TraceBridgeError`] into a Python exception suitable for
/// surfacing through the request future.
///
/// Callback errors map to `RuntimeError` with the captured message;
/// the not-awaited error is reported as `TypeError` because the bug is
/// in how the callable was supplied.
pub(crate) fn bridge_error_to_pyerr(err: TraceBridgeError) -> PyErr {
    match err {
        TraceBridgeError::Callback(pyerr) => pyerr,
        TraceBridgeError::NotAwaited => PyTypeError::new_err(
            "async trace callback was not awaited; sync transport cannot drive coroutines. \
             Use AsyncClient for async trace callbacks.",
        ),
    }
}

/// Drain a callback error slot into a Python exception, if any.
pub(crate) fn take_callback_error(slot: &CallbackErrorSlot) -> Option<PyErr> {
    slot.take().map(bridge_error_to_pyerr)
}

// Rust-level unit tests for this module require linking the Python
// interpreter, which is not available when running `cargo test --lib`
// against the eggfetch-python crate (the `pyo3/extension-module` feature
// prevents normal linkage).  Equivalent coverage is provided by the
// Python tests in
// `crates/eggfetch-python/tests/compat/test_corrective_02_extensions_and_wire_metadata.py`,
// which drive the same code paths through the public Python API.
