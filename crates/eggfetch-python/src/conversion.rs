//! Conversion utilities between Python and Rust types.

use pyo3::prelude::*;
use pyo3::types::PyTuple;

use crate::errors::map_err;

/// Iterate over key-value pairs from a Python Mapping or sequence of pairs.
///
/// For Mapping objects (dict, etc.), calls `.items()`.
/// For other iterables (list of tuples, etc.), iterates directly.
fn iter_kv_pairs(_py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    let items = if obj.get_type().hasattr("__getitem__")? && obj.hasattr("items")? {
        obj.call_method0("items")?
    } else {
        obj.clone()
    };
    let mut pairs = Vec::new();
    for item in items.try_iter()? {
        let item = item?;
        let tuple: Bound<'_, PyTuple> = item.downcast_into::<PyTuple>()?;
        if tuple.len() != 2 {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "expected a mapping or sequence of 2-tuples",
            ));
        }
        let key: String = tuple.get_item(0)?.extract()?;
        let value: String = tuple.get_item(1)?.extract()?;
        pairs.push((key, value));
    }
    Ok(pairs)
}

/// Convert a Python dict/Mapping/sequence-of-pairs to Rust `eggfetch_core::Headers`.
pub fn python_headers_to_rust(
    py: Python,
    headers: &Bound<'_, PyAny>,
) -> PyResult<eggfetch_core::Headers> {
    let mut rust_headers = eggfetch_core::Headers::new();
    let pairs = iter_kv_pairs(py, headers)?;
    for (key, value) in &pairs {
        rust_headers.insert(key, value).map_err(map_err)?;
    }
    Ok(rust_headers)
}

/// Append query parameters from a Python dict/Mapping/sequence-of-pairs to a `url::Url`.
pub fn python_params_to_url(
    py: Python,
    url: &mut url::Url,
    params: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let pairs = iter_kv_pairs(py, params)?;
    for (key, value) in &pairs {
        url.query_pairs_mut().append_pair(key, value);
    }
    Ok(())
}

/// Encode a Python Mapping or sequence-of-pairs as `application/x-www-form-urlencoded` bytes.
///
/// Uses `url::form_urlencoded` for proper percent-encoding of keys and values.
pub fn encode_form_body(py: Python, data: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    let pairs = iter_kv_pairs(py, data)?;
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in &pairs {
        serializer.append_pair(key, value);
    }
    Ok(serializer.finish().into_bytes())
}

/// Serialize a Python object to JSON bytes using Python's `json.dumps()`.
///
/// Returns the UTF-8 encoded JSON bytes.
pub fn encode_json_body(py: Python, obj: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    let json_mod = py.import("json")?;
    let json_str: String = json_mod.call_method1("dumps", (obj,))?.extract()?;
    Ok(json_str.into_bytes())
}

/// Validate that only one body kwarg is provided among `content`, `data`, `json`.
pub fn validate_body_kwargs(
    content: Option<&Bound<'_, PyAny>>,
    data: Option<&Bound<'_, PyAny>>,
    json: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let count = u8::from(content.is_some()) + u8::from(data.is_some()) + u8::from(json.is_some());
    if count > 1 {
        let mut provided = Vec::new();
        if content.is_some() {
            provided.push("content");
        }
        if data.is_some() {
            provided.push("data");
        }
        if json.is_some() {
            provided.push("json");
        }
        return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
            "only one of content, data, or json may be provided; got: {}",
            provided.join(", ")
        )));
    }
    Ok(())
}

/// Build a request body from the provided Python kwargs.
///
/// Returns `(body_bytes, content_type_override)` where `content_type_override`
/// is `Some(ct)` when the body was auto-typed (form or JSON).
pub fn build_request_body<'py>(
    py: Python<'py>,
    content: Option<&Bound<'py, PyAny>>,
    data: Option<&Bound<'py, PyAny>>,
    json: Option<&Bound<'py, PyAny>>,
) -> PyResult<(Option<Vec<u8>>, Option<&'static str>)> {
    if let Some(c) = content {
        let body_bytes: Vec<u8> = if let Ok(s) = c.extract::<String>() {
            s.into_bytes()
        } else {
            c.extract::<Vec<u8>>()?
        };
        Ok((Some(body_bytes), None))
    } else if let Some(d) = data {
        let body_bytes = encode_form_body(py, d)?;
        Ok((Some(body_bytes), Some("application/x-www-form-urlencoded")))
    } else if let Some(j) = json {
        let body_bytes = encode_json_body(py, j)?;
        Ok((Some(body_bytes), Some("application/json")))
    } else {
        Ok((None, None))
    }
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
