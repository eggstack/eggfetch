//! Python bindings for eggfetch.

use pyo3::prelude::*;

mod async_client;
mod auth;
mod client;
mod conversion;
mod cookies;
mod errors;
mod extensions;
mod headers;
mod limits;
mod multipart;
mod network_stream;
mod proxy;
mod request_preparation;
mod response;
mod retry;
mod streaming;
mod timeout;
mod tls;
mod trace_bridge;

use async_client::PyAsyncClient;
use auth::{PyBasicAuth, PyBearerAuth, PyNoAuth};
use client::PyClient;
use cookies::PyCookies;
use errors::map_err;
use headers::PyHeaders;
use limits::PyLimits;
use multipart::PyFile;
use response::{
    PyResponse, PyResponseBytesIterator, PyResponseLinesIterator, PyResponseTextIterator,
};
use retry::PyRetry;
use streaming::{
    PyAsyncBytesIterator, PyAsyncLinesIterator, PyAsyncRawBytesIterator, PyAsyncTextIterator,
    PyBytesChunkIterator, PyLinesChunkIterator, PyRawBytesChunkIterator, PyStreamingResponse,
    PyTextChunkIterator,
};
use timeout::PyTimeout;

/// Send an HTTP request using a short-lived client.
///
/// Args:
///     method: HTTP method string.
///     url: Target URL.
///     headers: Request headers dict or sequence of pairs (optional).
///     params: Query parameters dict or sequence of pairs (optional).
///     content: Raw request body as bytes (optional).
///     data: Form data as dict or sequence of pairs (optional).
///     json: JSON-serializable object (optional).
///     files: File uploads as mapping or sequence of pairs (optional).
///     timeout: Request timeout in seconds, or Timeout object (optional).
///     cookies: Initial cookies as dict of name=value pairs (optional).
///     auth: Authentication credentials (optional).
///     `follow_redirects`: Whether to follow redirects (default False).
///     `max_redirects`: Maximum redirects to follow (default 20).
#[pyfunction]
#[pyo3(signature = (method, url, *, headers=None, params=None, content=None, data=None, json=None, files=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, limits=None))]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn request<'py>(
    py: Python<'py>,
    method: &str,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    content: Option<&Bound<'py, PyAny>>,
    data: Option<&Bound<'py, PyAny>>,
    json: Option<&Bound<'py, PyAny>>,
    files: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
    cookies: Option<&Bound<'py, PyAny>>,
    auth: Option<&Bound<'py, PyAny>>,
    follow_redirects: Option<bool>,
    max_redirects: Option<usize>,
    decompress: Option<bool>,
    proxy: Option<&Bound<'py, PyAny>>,
    verify: Option<&Bound<'py, PyAny>>,
    cert: Option<&Bound<'py, PyAny>>,
    retries: Option<&Bound<'py, PyAny>>,
    limits: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let prepared = request_preparation::prepare_request(
        py,
        method,
        url,
        headers,
        params,
        content,
        data,
        json,
        files,
        timeout,
        cookies,
        auth,
        follow_redirects,
        max_redirects,
        proxy,
        retries,
        None,
        false,
    )?;

    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let tls_config = tls::build_tls_config(verify, cert, None)?;

    let mut client_builder = eggfetch_core::Client::builder()
        .redirect_policy(eggfetch_core::redirect::RedirectPolicy::new(
            follow_redirects.unwrap_or(false),
            max_redirects.unwrap_or(20),
        ))
        .tls_config(tls_config);

    if let Some(l) = limits {
        let py_limits: PyLimits = l.extract()?;
        client_builder = client_builder.limits(py_limits.inner);
    }

    let client = client_builder.build();
    let dispatch = request_preparation::prepare_core_dispatch(
        &client,
        prepared,
        request_preparation::RequestDispatchDefaults {
            redirect_policy: eggfetch_core::redirect::RedirectPolicy::new(
                follow_redirects.unwrap_or(false),
                max_redirects.unwrap_or(20),
            ),
            decompress,
        },
    )?;
    let builder = dispatch.builder;

    let result = py.detach(|| {
        runtime.block_on(async {
            let mut response = Box::pin(builder.send()).await.map_err(map_err)?;
            let content = response.bytes().await.map_err(map_err)?;
            Ok::<_, PyErr>((response, content))
        })
    });

    let (mut response, content) = result?;
    // The top-level short-lived helpers do not own a persistent runtime,
    // so the network stream (if any) gets no explicit handle or lease.
    let py_response =
        PyResponse::from_core_response_with_body(&mut response, content, None, None, false)?;
    Ok(Py::new(py, py_response)?.into_bound(py).into_any())
}

/// Send a GET request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, limits=None))]
#[allow(clippy::too_many_arguments)]
fn get<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
    cookies: Option<&Bound<'py, PyAny>>,
    auth: Option<&Bound<'py, PyAny>>,
    follow_redirects: Option<bool>,
    max_redirects: Option<usize>,
    decompress: Option<bool>,
    proxy: Option<&Bound<'py, PyAny>>,
    verify: Option<&Bound<'py, PyAny>>,
    cert: Option<&Bound<'py, PyAny>>,
    retries: Option<&Bound<'py, PyAny>>,
    limits: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(
        py,
        "GET",
        url,
        headers,
        params,
        None,
        None,
        None,
        None,
        timeout,
        cookies,
        auth,
        follow_redirects,
        max_redirects,
        decompress,
        proxy,
        verify,
        cert,
        retries,
        limits,
    )
}

/// Send a POST request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, content=None, data=None, json=None, files=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, limits=None))]
#[allow(clippy::too_many_arguments)]
fn post<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    content: Option<&Bound<'py, PyAny>>,
    data: Option<&Bound<'py, PyAny>>,
    json: Option<&Bound<'py, PyAny>>,
    files: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
    cookies: Option<&Bound<'py, PyAny>>,
    auth: Option<&Bound<'py, PyAny>>,
    follow_redirects: Option<bool>,
    max_redirects: Option<usize>,
    decompress: Option<bool>,
    proxy: Option<&Bound<'py, PyAny>>,
    verify: Option<&Bound<'py, PyAny>>,
    cert: Option<&Bound<'py, PyAny>>,
    retries: Option<&Bound<'py, PyAny>>,
    limits: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(
        py,
        "POST",
        url,
        headers,
        params,
        content,
        data,
        json,
        files,
        timeout,
        cookies,
        auth,
        follow_redirects,
        max_redirects,
        decompress,
        proxy,
        verify,
        cert,
        retries,
        limits,
    )
}

/// Send a PUT request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, content=None, data=None, json=None, files=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, limits=None))]
#[allow(clippy::too_many_arguments)]
fn put<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    content: Option<&Bound<'py, PyAny>>,
    data: Option<&Bound<'py, PyAny>>,
    json: Option<&Bound<'py, PyAny>>,
    files: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
    cookies: Option<&Bound<'py, PyAny>>,
    auth: Option<&Bound<'py, PyAny>>,
    follow_redirects: Option<bool>,
    max_redirects: Option<usize>,
    decompress: Option<bool>,
    proxy: Option<&Bound<'py, PyAny>>,
    verify: Option<&Bound<'py, PyAny>>,
    cert: Option<&Bound<'py, PyAny>>,
    retries: Option<&Bound<'py, PyAny>>,
    limits: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(
        py,
        "PUT",
        url,
        headers,
        params,
        content,
        data,
        json,
        files,
        timeout,
        cookies,
        auth,
        follow_redirects,
        max_redirects,
        decompress,
        proxy,
        verify,
        cert,
        retries,
        limits,
    )
}

/// Send a PATCH request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, content=None, data=None, json=None, files=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, limits=None))]
#[allow(clippy::too_many_arguments)]
fn patch<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    content: Option<&Bound<'py, PyAny>>,
    data: Option<&Bound<'py, PyAny>>,
    json: Option<&Bound<'py, PyAny>>,
    files: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
    cookies: Option<&Bound<'py, PyAny>>,
    auth: Option<&Bound<'py, PyAny>>,
    follow_redirects: Option<bool>,
    max_redirects: Option<usize>,
    decompress: Option<bool>,
    proxy: Option<&Bound<'py, PyAny>>,
    verify: Option<&Bound<'py, PyAny>>,
    cert: Option<&Bound<'py, PyAny>>,
    retries: Option<&Bound<'py, PyAny>>,
    limits: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(
        py,
        "PATCH",
        url,
        headers,
        params,
        content,
        data,
        json,
        files,
        timeout,
        cookies,
        auth,
        follow_redirects,
        max_redirects,
        decompress,
        proxy,
        verify,
        cert,
        retries,
        limits,
    )
}

/// Send a DELETE request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, limits=None))]
#[allow(clippy::too_many_arguments)]
fn delete<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
    cookies: Option<&Bound<'py, PyAny>>,
    auth: Option<&Bound<'py, PyAny>>,
    follow_redirects: Option<bool>,
    max_redirects: Option<usize>,
    decompress: Option<bool>,
    proxy: Option<&Bound<'py, PyAny>>,
    verify: Option<&Bound<'py, PyAny>>,
    cert: Option<&Bound<'py, PyAny>>,
    retries: Option<&Bound<'py, PyAny>>,
    limits: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(
        py,
        "DELETE",
        url,
        headers,
        params,
        None,
        None,
        None,
        None,
        timeout,
        cookies,
        auth,
        follow_redirects,
        max_redirects,
        decompress,
        proxy,
        verify,
        cert,
        retries,
        limits,
    )
}

/// Send a HEAD request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, limits=None))]
#[allow(clippy::too_many_arguments)]
fn head<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
    cookies: Option<&Bound<'py, PyAny>>,
    auth: Option<&Bound<'py, PyAny>>,
    follow_redirects: Option<bool>,
    max_redirects: Option<usize>,
    decompress: Option<bool>,
    proxy: Option<&Bound<'py, PyAny>>,
    verify: Option<&Bound<'py, PyAny>>,
    cert: Option<&Bound<'py, PyAny>>,
    retries: Option<&Bound<'py, PyAny>>,
    limits: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(
        py,
        "HEAD",
        url,
        headers,
        params,
        None,
        None,
        None,
        None,
        timeout,
        cookies,
        auth,
        follow_redirects,
        max_redirects,
        decompress,
        proxy,
        verify,
        cert,
        retries,
        limits,
    )
}

/// Send an OPTIONS request using a short-lived client.
#[pyfunction]
#[pyo3(signature = (url, *, headers=None, params=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, limits=None))]
#[allow(clippy::too_many_arguments)]
fn options<'py>(
    py: Python<'py>,
    url: &str,
    headers: Option<&Bound<'py, PyAny>>,
    params: Option<&Bound<'py, PyAny>>,
    timeout: Option<&Bound<'py, PyAny>>,
    cookies: Option<&Bound<'py, PyAny>>,
    auth: Option<&Bound<'py, PyAny>>,
    follow_redirects: Option<bool>,
    max_redirects: Option<usize>,
    decompress: Option<bool>,
    proxy: Option<&Bound<'py, PyAny>>,
    verify: Option<&Bound<'py, PyAny>>,
    cert: Option<&Bound<'py, PyAny>>,
    retries: Option<&Bound<'py, PyAny>>,
    limits: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    request(
        py,
        "OPTIONS",
        url,
        headers,
        params,
        None,
        None,
        None,
        None,
        timeout,
        cookies,
        auth,
        follow_redirects,
        max_redirects,
        decompress,
        proxy,
        verify,
        cert,
        retries,
        limits,
    )
}

/// Register all exception types on the module.
fn register_exceptions(m: &Bound<'_, PyModule>) -> PyResult<()> {
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
    m.add(
        "TooManyRedirects",
        m.py().get_type::<errors::TooManyRedirects>(),
    )?;
    m.add(
        "StreamConsumed",
        m.py().get_type::<errors::StreamConsumed>(),
    )?;
    m.add("StreamClosed", m.py().get_type::<errors::StreamClosed>())?;
    m.add(
        "ResponseNotRead",
        m.py().get_type::<errors::ResponseNotRead>(),
    )?;
    m.add(
        "DecompressionError",
        m.py().get_type::<errors::DecompressionError>(),
    )?;
    m.add(
        "UnsupportedContentEncoding",
        m.py().get_type::<errors::UnsupportedContentEncoding>(),
    )?;
    m.add("ProxyError", m.py().get_type::<errors::ProxyError>())?;
    m.add(
        "ProxyConnectError",
        m.py().get_type::<errors::ProxyConnectError>(),
    )?;
    m.add(
        "ProxyAuthError",
        m.py().get_type::<errors::ProxyAuthError>(),
    )?;
    m.add(
        "BodyNotReplayableForRetry",
        m.py().get_type::<errors::BodyNotReplayableForRetry>(),
    )?;
    m.add(
        "RetryBudgetExhausted",
        m.py().get_type::<errors::RetryBudgetExhausted>(),
    )?;
    m.add(
        "RetryNotConfigured",
        m.py().get_type::<errors::RetryNotConfigured>(),
    )?;
    m.add("Http2Error", m.py().get_type::<errors::Http2Error>())?;
    m.add("Http2GoAway", m.py().get_type::<errors::Http2GoAway>())?;
    m.add(
        "Http2StreamReset",
        m.py().get_type::<errors::Http2StreamReset>(),
    )?;
    m.add(
        "Http2FlowControlError",
        m.py().get_type::<errors::Http2FlowControlError>(),
    )?;
    m.add("H3Error", m.py().get_type::<errors::H3Error>())?;
    m.add(
        "H3ConnectError",
        m.py().get_type::<errors::H3ConnectError>(),
    )?;
    m.add(
        "H3ProtocolError",
        m.py().get_type::<errors::H3ProtocolError>(),
    )?;
    Ok(())
}

/// eggfetch - Python bindings for eggfetch.
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    m.add_class::<PyAsyncClient>()?;
    m.add_class::<PyClient>()?;
    m.add_class::<cookies::PyCookie>()?;
    m.add_class::<PyCookies>()?;
    m.add_class::<PyHeaders>()?;
    m.add_class::<PyResponse>()?;
    m.add_class::<PyResponseBytesIterator>()?;
    m.add_class::<PyResponseTextIterator>()?;
    m.add_class::<PyResponseLinesIterator>()?;
    m.add_class::<PyStreamingResponse>()?;
    m.add_class::<PyBytesChunkIterator>()?;
    m.add_class::<PyTextChunkIterator>()?;
    m.add_class::<PyLinesChunkIterator>()?;
    m.add_class::<PyRawBytesChunkIterator>()?;
    m.add_class::<PyAsyncBytesIterator>()?;
    m.add_class::<PyAsyncTextIterator>()?;
    m.add_class::<PyAsyncLinesIterator>()?;
    m.add_class::<PyAsyncRawBytesIterator>()?;
    m.add_class::<PyTimeout>()?;
    m.add_class::<PyBasicAuth>()?;
    m.add_class::<PyBearerAuth>()?;
    m.add_class::<PyNoAuth>()?;
    m.add_class::<PyFile>()?;
    m.add_class::<PyRetry>()?;
    m.add_class::<PyLimits>()?;
    m.add_class::<network_stream::PyNetworkStream>()?;
    m.add_class::<network_stream::PyAsyncNetworkStream>()?;

    // Create the NOAUTH singleton instance.
    let noauth_obj = Py::new(m.py(), PyNoAuth)?;
    m.add("NOAUTH", noauth_obj.bind(m.py()).clone())?;

    register_exceptions(m)?;

    m.add_function(wrap_pyfunction!(request, m)?)?;
    m.add_function(wrap_pyfunction!(get, m)?)?;
    m.add_function(wrap_pyfunction!(post, m)?)?;
    m.add_function(wrap_pyfunction!(put, m)?)?;
    m.add_function(wrap_pyfunction!(patch, m)?)?;
    m.add_function(wrap_pyfunction!(delete, m)?)?;
    m.add_function(wrap_pyfunction!(head, m)?)?;
    m.add_function(wrap_pyfunction!(options, m)?)?;

    // PyO3 automatically builds a module __all__ from every add/add_class
    // call. The pure-Python eggfetch package owns the supported export
    // contract, so do not expose a second contract from this private module.
    m.delattr("__all__")?;

    Ok(())
}
