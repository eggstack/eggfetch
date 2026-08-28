//! Python-side extension extraction helpers.
//!
//! HTTPX's `request.extensions` dict carries arbitrary user data alongside a
//! few typed transport hints the engine can act on.  This module is the
//! single boundary that converts the Python dict into the typed
//! [`eggfetch_core::TransportHints`] used by `eggfetch-core`.
//!
//! Behavior:
//!
//! - `target` (str or bytes) is forwarded as the wire request target.
//! - `sni_hostname` (str) is forwarded as the TLS SNI override.
//! - `trace` (callable) is split off and converted into a
//!   [`TraceObserver`](eggfetch_core::trace::TraceObserver) implementation
//!   that bridges core events to the Python callable.  Python callables
//!   never enter `eggfetch-core`.
//! - All other keys are accepted but ignored at this layer; the HTTPX
//!   compatibility facade keeps the original dict on the `Request` object
//!   so callers can read them back from `response.request.extensions`.

use std::sync::Arc;

use bytes::Bytes;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use eggfetch_core::trace::TraceObserver;
use eggfetch_core::TransportHints;

use crate::trace_bridge::{CallbackErrorSlot, PyTraceObserver};

/// The result of converting a Python `extensions` dict into native
/// transport hints plus any auxiliary bridges that must remain alive for
/// the lifetime of the request.
pub(crate) struct ExtractedExtensions {
    /// Typed native hints that should be installed on the
    /// `eggfetch_core::Request` before dispatch.
    pub hints: TransportHints,
    /// Handle for surfacing callback errors after dispatch.
    pub trace_error_slot: Option<CallbackErrorSlot>,
}

/// Convert a Python `extensions` dict (or `None`) into native transport
/// hints plus any auxiliary bridges.
///
/// This is the single point where Python extension dicts are mapped to
/// typed engine inputs.  All native request entry points (sync buffered,
/// async buffered, sync streaming, async streaming) MUST call this
/// helper so behavior stays consistent.
pub(crate) fn extract_native_extensions(
    py: Python<'_>,
    extensions: Option<&Bound<'_, PyAny>>,
) -> PyResult<ExtractedExtensions> {
    let Some(ext) = extensions else {
        return Ok(ExtractedExtensions {
            hints: TransportHints::default(),
            trace_error_slot: None,
        });
    };

    // Accept `None` explicitly — equivalent to omitting the kwarg.
    if ext.is_none() {
        return Ok(ExtractedExtensions {
            hints: TransportHints::default(),
            trace_error_slot: None,
        });
    }

    let dict = ext.downcast::<PyDict>().map_err(|_| {
        PyTypeError::new_err(
            "extensions must be a dict containing supported keys: target, sni_hostname, trace",
        )
    })?;

    let mut hints = TransportHints::default();
    let mut trace_error_slot: Option<CallbackErrorSlot> = None;

    for (key_obj, value_obj) in dict.iter() {
        let key: String = key_obj.extract()?;
        match key.as_str() {
            "target" => {
                if let Some(existing) = hints.target.as_ref() {
                    return Err(PyTypeError::new_err(format!(
                        "target extension already supplied ({existing:?}); multiple target values are not supported"
                    )));
                }
                let bytes = if let Ok(b) = value_obj.extract::<Vec<u8>>() {
                    Bytes::from(b)
                } else if let Ok(s) = value_obj.extract::<String>() {
                    Bytes::from(s.into_bytes())
                } else {
                    return Err(PyTypeError::new_err(
                        "target extension must be a str or bytes value",
                    ));
                };
                // Pre-validate the target to surface smuggling characters
                // before any wire dispatch is attempted.  This mirrors
                // `eggfetch_core::pipeline::validate_target` (not exported)
                // so the same set of bytes is rejected at the binding
                // boundary and at the wire boundary.
                if bytes.is_empty() {
                    return Err(PyTypeError::new_err("target extension must not be empty"));
                }
                if bytes.iter().any(|&b| b < 0x20 || b == 0x7f) {
                    return Err(PyTypeError::new_err(
                        "target extension contains forbidden characters (C0 controls/DEL; includes CR/LF/NUL)",
                    ));
                }
                hints.target = Some(bytes);
            }
            "sni_hostname" => {
                if let Some(existing) = hints.sni_hostname.as_ref() {
                    return Err(PyTypeError::new_err(format!(
                        "sni_hostname extension already supplied ({existing:?}); multiple values are not supported"
                    )));
                }
                let s: String = value_obj.extract()?;
                if s.is_empty() {
                    return Err(PyTypeError::new_err(
                        "sni_hostname extension must not be empty",
                    ));
                }
                hints.sni_hostname = Some(s);
            }
            "trace" => {
                if trace_error_slot.is_some() {
                    return Err(PyTypeError::new_err(
                        "trace extension already supplied; only one trace callable is supported",
                    ));
                }
                if value_obj.is_none() {
                    // Treat `extensions={"trace": None}` as no observer.
                    continue;
                }
                let (observer, slot) = PyTraceObserver::new(py, value_obj.clone());
                let arc: Arc<dyn TraceObserver> = Arc::new(observer);
                hints.trace = Some(arc);
                trace_error_slot = Some(slot);
            }
            _ => {
                // Unknown keys are passed through at the HTTPX facade layer
                // and remain on the request object.  The core engine does
                // not see them.
            }
        }
    }

    Ok(ExtractedExtensions {
        hints,
        trace_error_slot,
    })
}

// Rust-level unit tests require linking the Python interpreter, which is
// not available when running `cargo test --lib` against the eggfetch-python
// crate.  Equivalent coverage is provided by the Python tests in
// `crates/eggfetch-python/tests/compat/test_corrective_02_extensions_and_wire_metadata.py`,
// which drive the same code paths through the public Python API.
