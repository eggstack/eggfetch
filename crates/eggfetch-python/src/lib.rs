//! Python bindings for eggfetch.

use pyo3::prelude::*;

mod async_client;
mod client;
mod conversion;
mod errors;
mod headers;
mod response;
mod timeout;

use async_client::PyAsyncClient;
use client::PyClient;
use errors::map_err;
use headers::PyHeaders;
use response::PyResponse;
use timeout::PyTimeout;

/// Send an HTTP request using a short-lived client.
///
/// Args:
///     method: HTTP method string.
///     url: Target URL.
///     headers: Request headers dict (optional).
///     params: Query parameters dict (optional).
///     content: Request body as bytes (optional).
///     timeout: Request timeout in seconds, or Timeout object (optional).
#[pyfunction]
#[pyo3(signature = (method, url, *, headers=None, params=None, content=None, timeout=None))]
fn request<'py>(
    py: Python<'py>,
    method: &str,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    content: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let method_upper = method.to_uppercase();
    let http_method = http::Method::try_from(method_upper.as_str()).map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("invalid HTTP method: {method}"))
    })?;

    let mut target_url = url::Url::parse(url)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    if let Some(p) = params {
        conversion::python_params_to_url(&mut target_url, p)?;
    }
    let target_url = target_url;

    let rust_headers = if let Some(h) = headers {
        conversion::python_headers_to_rust(py, h)?
    } else {
        eggfetch_core::Headers::new()
    };

    let body_bytes: Option<Vec<u8>> = if let Some(c) = content {
        Some(c.extract::<Vec<u8>>()?)
    } else {
        None
    };

    let rust_timeout = conversion::parse_timeout(timeout)?;

    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
    let client = eggfetch_core::Client::new();

    let result = py.allow_threads(|| {
        runtime.block_on(async {
            let mut builder = client
                .request(http_method, target_url.as_str())
                .map_err(map_err)?;

            for (name, value) in rust_headers.iter() {
                builder = builder.header(name.as_str(), value.to_str().unwrap_or(""));
            }

            if let Some(bytes) = body_bytes {
                builder = builder.bytes(bytes);
            }

            if let Some(t) = rust_timeout {
                builder = builder.timeout(t);
            }

            let response = builder.send().await.map_err(map_err)?;
            Ok::<_, PyErr>(response)
        })
    });

    let response = result?;
    let py_response = PyResponse::from_core_response(response)?;
    Ok(Py::new(py, py_response)?.into_bound(py).into_any())
}

/// Send a GET request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, timeout=None))]
fn get<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(py, "GET", url, headers, params, None, timeout)
}

/// Send a POST request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, content=None, timeout=None))]
fn post<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    content: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(py, "POST", url, headers, params, content, timeout)
}

/// Send a PUT request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, content=None, timeout=None))]
fn put<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    content: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(py, "PUT", url, headers, params, content, timeout)
}

/// Send a PATCH request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, content=None, timeout=None))]
fn patch<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    content: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(py, "PATCH", url, headers, params, content, timeout)
}

/// Send a DELETE request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, timeout=None))]
fn delete<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(py, "DELETE", url, headers, params, None, timeout)
}

/// Send a HEAD request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, timeout=None))]
fn head<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(py, "HEAD", url, headers, params, None, timeout)
}

/// Send an OPTIONS request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, timeout=None))]
fn options<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(py, "OPTIONS", url, headers, params, None, timeout)
}

/// eggfetch - Python bindings for eggfetch.
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", "0.1.0")?;

    m.add_class::<PyAsyncClient>()?;
    m.add_class::<PyClient>()?;
    m.add_class::<PyHeaders>()?;
    m.add_class::<PyResponse>()?;
    m.add_class::<PyTimeout>()?;

    m.add("EggfetchError", m.py().get_type::<errors::EggfetchError>())?;
    m.add("RequestError", m.py().get_type::<errors::RequestError>())?;
    m.add("InvalidUrl", m.py().get_type::<errors::InvalidUrl>())?;
    m.add(
        "TimeoutException",
        m.py().get_type::<errors::TimeoutException>(),
    )?;
    m.add("PoolTimeout", m.py().get_type::<errors::PoolTimeout>())?;
    m.add(
        "ConnectTimeout",
        m.py().get_type::<errors::ConnectTimeout>(),
    )?;
    m.add("ReadTimeout", m.py().get_type::<errors::ReadTimeout>())?;
    m.add("WriteTimeout", m.py().get_type::<errors::WriteTimeout>())?;
    m.add("NetworkError", m.py().get_type::<errors::NetworkError>())?;
    m.add("ProtocolError", m.py().get_type::<errors::ProtocolError>())?;
    m.add("BodyError", m.py().get_type::<errors::BodyError>())?;
    m.add(
        "HTTPStatusError",
        m.py().get_type::<errors::HTTPStatusError>(),
    )?;
    m.add(
        "UnsupportedKwarg",
        m.py().get_type::<errors::UnsupportedKwarg>(),
    )?;

    m.add_function(wrap_pyfunction!(request, m)?)?;
    m.add_function(wrap_pyfunction!(get, m)?)?;
    m.add_function(wrap_pyfunction!(post, m)?)?;
    m.add_function(wrap_pyfunction!(put, m)?)?;
    m.add_function(wrap_pyfunction!(patch, m)?)?;
    m.add_function(wrap_pyfunction!(delete, m)?)?;
    m.add_function(wrap_pyfunction!(head, m)?)?;
    m.add_function(wrap_pyfunction!(options, m)?)?;

    let all_items = vec![
        "AsyncClient",
        "Client",
        "Headers",
        "Response",
        "Timeout",
        "request",
        "get",
        "post",
        "put",
        "patch",
        "delete",
        "head",
        "options",
        "EggfetchError",
        "RequestError",
        "InvalidUrl",
        "TimeoutException",
        "PoolTimeout",
        "ConnectTimeout",
        "ReadTimeout",
        "WriteTimeout",
        "NetworkError",
        "ProtocolError",
        "BodyError",
        "HTTPStatusError",
        "UnsupportedKwarg",
    ];
    let py = m.py();
    let py_list = pyo3::types::PyList::new(py, &all_items)?;
    m.add("__all__", py_list)?;

    Ok(())
}
