//! Conversion utilities between Python and Rust types.

use pyo3::prelude::*;
use pyo3::types::PyTuple;

use crate::errors::map_err;

/// Convert a Python dict/Mapping to Rust `eggfetch_core::Headers`.
pub fn python_headers_to_rust(
    _py: Python,
    headers: &Bound<'_, PyAny>,
) -> PyResult<eggfetch_core::Headers> {
    let mut rust_headers = eggfetch_core::Headers::new();

    let items: Bound<'_, PyAny> = headers.call_method0("items")?;
    let iter = items.try_iter()?;
    for item in iter {
        let item = item?;
        let tuple: Bound<'_, PyTuple> = item.downcast_into::<PyTuple>()?;
        let key: String = tuple.get_item(0)?.extract()?;
        let value: String = tuple.get_item(1)?.extract()?;
        rust_headers.insert(&key, &value).map_err(map_err)?;
    }

    Ok(rust_headers)
}

/// Append query parameters from a Python dict to a `url::Url`.
pub fn python_params_to_url(url: &mut url::Url, params: &Bound<'_, PyAny>) -> PyResult<()> {
    let items: Bound<'_, PyAny> = params.call_method0("items")?;
    let iter = items.try_iter()?;
    for item in iter {
        let item = item?;
        let tuple: Bound<'_, PyTuple> = item.downcast_into::<PyTuple>()?;
        let key: String = tuple.get_item(0)?.extract()?;
        let value: String = tuple.get_item(1)?.extract()?;
        url.query_pairs_mut().append_pair(&key, &value);
    }
    Ok(())
}

/// Convert a Python timeout value to an optional Rust `eggfetch_core::Timeout`.
pub fn parse_timeout(
    py_timeout: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<eggfetch_core::Timeout>> {
    match py_timeout {
        None => Ok(None),
        Some(val) => {
            if val.is_none() {
                Ok(None)
            } else if let Ok(secs) = val.extract::<f64>() {
                if secs < 0.0 {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "timeout must be a non-negative number",
                    ));
                }
                let duration = std::time::Duration::from_secs_f64(secs);
                Ok(Some(eggfetch_core::Timeout {
                    pool: Some(duration),
                    connect: Some(duration),
                    write: Some(duration),
                    read: Some(duration),
                    total: None,
                }))
            } else if let Ok(py_timeout_obj) = val.extract::<crate::timeout::PyTimeout>() {
                Ok(Some(py_timeout_obj.inner))
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                    "timeout must be a float (seconds) or Timeout object",
                ))
            }
        }
    }
}
