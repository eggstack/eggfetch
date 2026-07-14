//! TLS configuration helpers for Python bindings.

use pyo3::prelude::*;
use pyo3::types::PyTuple;

use crate::errors::map_err;

/// Build a `TlsConfig` from Python `verify` and `cert` kwargs.
///
/// - `verify`: `None`/`True` (default), `False` (skip verification), or `str` (CA bundle path).
/// - `cert`: `None` (default), `str` (combined PEM), or `tuple(str, str)` (cert, key).
pub fn build_tls_config(
    verify: Option<&Bound<'_, PyAny>>,
    cert: Option<&Bound<'_, PyAny>>,
) -> PyResult<eggfetch_core::TlsConfig> {
    let mut builder = eggfetch_core::TlsConfig::builder();

    if let Some(v) = verify {
        if let Ok(b) = v.extract::<bool>() {
            if !b {
                builder = builder.danger_accept_invalid_certs(true);
            }
        } else if let Ok(path) = v.extract::<String>() {
            builder = builder.ca_certificate_path(&path).map_err(map_err)?;
        } else {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "verify must be a bool or str",
            ));
        }
    }

    if let Some(c) = cert {
        if let Ok(path) = c.extract::<String>() {
            builder = builder.client_cert_path(&path, &path).map_err(map_err)?;
        } else if let Ok(tuple) = c.downcast::<PyTuple>() {
            if tuple.len() != 2 {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "cert tuple must be (cert_path, key_path)",
                ));
            }
            let cert_path: String = tuple.get_item(0)?.extract()?;
            let key_path: String = tuple.get_item(1)?.extract()?;
            builder = builder
                .client_cert_path(&cert_path, &key_path)
                .map_err(map_err)?;
        } else {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "cert must be a str or tuple(cert_path, key_path)",
            ));
        }
    }

    Ok(builder.build())
}
