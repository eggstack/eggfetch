//! Conversion utilities between Python and Rust types.

use pyo3::prelude::*;
use pyo3::types::PyTuple;

use bytes::Bytes;

use crate::errors::map_err;

/// Convert request-local Python cookies into a destination-scoped header.
///
/// Request cookies are intentionally kept out of the client's persistent jar.
/// The core redirect pipeline can therefore strip the serialized header on a
/// cross-origin hop without accidentally replaying it from client state.
pub(crate) fn python_cookies_to_header(
    cookies: Option<&Bound<'_, PyAny>>,
    target_url: &url::Url,
) -> PyResult<Option<String>> {
    let Some(cookies) = cookies else {
        return Ok(None);
    };
    if cookies.is_none() {
        return Ok(None);
    }
    let jar = eggfetch_core::cookie::CookieJar::new();
    for (name, value) in iter_kv_pairs(cookies.py(), cookies)? {
        jar.set_default_cookie(name, value);
    }
    Ok(jar.cookies_for_url(target_url))
}

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

/// Validate body kwargs with optional `files=` support.
///
/// `files=` may be combined with `data=` (multipart fields), but conflicts
/// with `content=` and `json=`.
pub fn validate_body_kwargs_with_files(
    content: Option<&Bound<'_, PyAny>>,
    data: Option<&Bound<'_, PyAny>>,
    json: Option<&Bound<'_, PyAny>>,
    files: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    if files.is_some() {
        if content.is_some() {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "files= conflicts with content=",
            ));
        }
        if json.is_some() {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "files= conflicts with json=",
            ));
        }
    }
    validate_body_kwargs(content, data, json)
}

/// Build a request body from the provided Python kwargs.
///
/// Returns `(body_bytes, content_type_override)` where `content_type_override`
/// is `Some(ct)` when the body was auto-typed (form or JSON).
///
/// If `content` is a Python iterable/generator (not bytes or str), returns
/// `None` for `body_bytes` — the caller must handle it as a stream body.
pub fn build_request_body<'py>(
    py: Python<'py>,
    content: Option<&Bound<'py, PyAny>>,
    data: Option<&Bound<'py, PyAny>>,
    json: Option<&Bound<'py, PyAny>>,
) -> PyResult<(Option<Vec<u8>>, Option<&'static str>)> {
    if let Some(c) = content {
        // Try to extract as bytes or string first.
        if let Ok(s) = c.extract::<String>() {
            return Ok((Some(s.into_bytes()), None));
        }
        if let Ok(b) = c.extract::<Vec<u8>>() {
            return Ok((Some(b), None));
        }
        // If it's an iterable/generator, signal to caller to treat as stream.
        if c.hasattr("__iter__")? || c.hasattr("__aiter__")? {
            return Ok((None, None));
        }
        Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
            "content must be bytes, str, or an iterable of bytes",
        ))
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

/// Check if a Python object is an iterable/generator (not bytes or str).
pub fn is_python_iterable(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    if obj.extract::<Vec<u8>>().is_ok() || obj.extract::<String>().is_ok() {
        return Ok(false);
    }
    Ok(obj.hasattr("__iter__")? || obj.hasattr("__aiter__")?)
}

/// Create a `RequestBody` from a Python sync iterable.
///
/// The iterable is consumed lazily via a bounded channel bridge: a Tokio task
/// calls `next()` on the Python iterator and sends chunks through a bounded
/// channel. This avoids eagerly buffering the entire body.
pub fn python_iterable_to_request_body<'py>(
    _py: Python<'py>,
    iterable: &Bound<'py, PyAny>,
) -> PyResult<eggfetch_core::RequestBody> {
    use futures_util::stream;

    // Eagerly collect the iterable into a Vec of chunks on the Python thread.
    // This is necessary because the Python iterator is not Send and cannot be
    // moved into an async task. For true lazy iteration, the caller should
    // use the native client's streaming API directly.
    let mut chunks: Vec<Bytes> = Vec::new();
    for item in iterable.try_iter()? {
        let item = item?;
        let chunk: Vec<u8> = if let Ok(b) = item.extract::<Vec<u8>>() {
            b
        } else if let Ok(s) = item.extract::<String>() {
            s.into_bytes()
        } else {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "iterable must yield bytes or str items",
            ));
        };
        chunks.push(Bytes::from(chunk));
    }

    if chunks.is_empty() {
        return Ok(eggfetch_core::RequestBody::Empty);
    }

    let total_len: usize = chunks.iter().map(bytes::Bytes::len).sum();
    let stream = stream::iter(chunks.into_iter().map(Ok::<_, eggfetch_core::error::Error>));
    Ok(eggfetch_core::RequestBody::from_stream(
        Box::pin(stream),
        Some(total_len),
    ))
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
                if !secs.is_finite() || secs < 0.0 {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "timeout must be a finite, non-negative number",
                    ));
                }
                let duration = std::time::Duration::try_from_secs_f64(secs).map_err(|_| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "timeout is too large to represent",
                    )
                })?;
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
