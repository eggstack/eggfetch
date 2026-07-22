//! Python bindings for eggfetch.

use pyo3::prelude::*;

mod async_client;
mod auth;
mod client;
mod conversion;
mod cookies;
mod errors;
mod headers;
mod limits;
mod multipart;
mod proxy;
mod response;
mod retry;
mod streaming;
mod timeout;
mod tls;

use async_client::PyAsyncClient;
use auth::{PyBasicAuth, PyBearerAuth, PyNoAuth};
use client::PyClient;
use cookies::PyCookies;
use errors::map_err;
use headers::PyHeaders;
use limits::PyLimits;
use multipart::PyFile;
use proxy::ProxyOverride;
use response::PyResponse;
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
#[allow(clippy::too_many_arguments)]
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
    let method_upper = method.to_uppercase();
    let http_method = http::Method::try_from(method_upper.as_str()).map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("invalid HTTP method: {method}"))
    })?;

    let mut target_url = url::Url::parse(url)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    if let Some(p) = params {
        conversion::python_params_to_url(py, &mut target_url, p)?;
    }
    let target_url = target_url;

    conversion::validate_body_kwargs_with_files(content, data, json, files)?;

    let mut rust_headers = if let Some(h) = headers {
        conversion::python_headers_to_rust(py, h)?
    } else {
        eggfetch_core::Headers::new()
    };

    let (body_bytes, auto_content_type): (Option<Vec<u8>>, Option<String>) = if let Some(f) = files
    {
        let (body, ct) = multipart::build_multipart_body(py, data, f)?;
        match body {
            eggfetch_core::RequestBody::Bytes(b) => (Some(b.to_vec()), Some(ct)),
            _ => (None, Some(ct)),
        }
    } else {
        let (bytes, ct) = conversion::build_request_body(py, content, data, json)?;
        (bytes, ct.map(String::from))
    };

    if let Some(ct) = &auto_content_type {
        if !rust_headers.contains("content-type") {
            rust_headers.insert("content-type", ct).map_err(map_err)?;
        }
    }

    let rust_timeout = conversion::parse_timeout(timeout)?;

    let redirect_policy = eggfetch_core::redirect::RedirectPolicy::new(
        follow_redirects.unwrap_or(false),
        max_redirects.unwrap_or(20),
    );

    if let Some(cookie_header) = conversion::python_cookies_to_header(cookies, &target_url)? {
        if !rust_headers.contains("cookie") {
            rust_headers
                .insert("cookie", &cookie_header)
                .map_err(map_err)?;
        }
    }

    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let auth_override = auth::parse_auth(auth)?;

    let proxy_override = proxy::parse_proxy(proxy)?;

    let retry_override = retry::parse_retry_option(retries)?;

    let tls_config = tls::build_tls_config(verify, cert)?;

    let mut builder = eggfetch_core::Client::builder()
        .redirect_policy(redirect_policy)
        .tls_config(tls_config);

    match auth_override {
        auth::AuthOverride::Inherit | auth::AuthOverride::Disable => {}
        auth::AuthOverride::Override(a) => {
            builder = builder.auth(a);
        }
    }

    if let ProxyOverride::Override(ref url) = proxy_override {
        let p = eggfetch_core::Proxy::all(url).map_err(map_err)?;
        builder = builder.proxy(p);
    }

    if let Some(l) = limits {
        let py_limits: PyLimits = l.extract()?;
        builder = builder.limits(py_limits.inner);
    }

    let client = builder.build();

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

            if let Some(d) = decompress {
                builder = builder.decompress(d);
            }

            match proxy_override {
                ProxyOverride::Inherit | ProxyOverride::Override(_) => {}
                ProxyOverride::Disable => {
                    builder = builder.without_proxy();
                }
            }

            if let Some(retry_policy) = retry_override.as_ref() {
                builder = builder.retry(retry_policy.clone());
            }

            let response = Box::pin(builder.send()).await.map_err(map_err)?;
            Ok::<_, PyErr>(response)
        })
    });

    let response = result?;
    let py_response = PyResponse::from_core_response(response)?;
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

/// Register the __all__ list on the module.
fn register_all(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let all_items = vec![
        "AsyncClient",
        "AsyncStreamingBytesIterator",
        "AsyncStreamingLinesIterator",
        "AsyncStreamingRawBytesIterator",
        "AsyncStreamingTextIterator",
        "Client",
        "Cookie",
        "Cookies",
        "File",
        "Headers",
        "Limits",
        "NoAuth",
        "NOAUTH",
        "Response",
        "Retry",
        "StreamingBytesIterator",
        "StreamingLinesIterator",
        "StreamingRawBytesIterator",
        "StreamingResponse",
        "StreamingTextIterator",
        "Timeout",
        "BasicAuth",
        "BearerAuth",
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
        "TooManyRedirects",
        "StreamConsumed",
        "StreamClosed",
        "ResponseNotRead",
        "DecompressionError",
        "UnsupportedContentEncoding",
        "ProxyError",
        "ProxyConnectError",
        "ProxyAuthError",
        "BodyNotReplayableForRetry",
        "RetryBudgetExhausted",
        "RetryNotConfigured",
        "Http2Error",
        "Http2GoAway",
        "Http2StreamReset",
        "Http2FlowControlError",
        "H3Error",
        "H3ConnectError",
        "H3ProtocolError",
    ];
    let py = m.py();
    let py_list = pyo3::types::PyList::new(py, &all_items)?;
    m.add("__all__", py_list)?;
    Ok(())
}

/// eggfetch - Python bindings for eggfetch.
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", "0.1.0")?;

    m.add_class::<PyAsyncClient>()?;
    m.add_class::<PyClient>()?;
    m.add_class::<cookies::PyCookie>()?;
    m.add_class::<PyCookies>()?;
    m.add_class::<PyHeaders>()?;
    m.add_class::<PyResponse>()?;
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

    register_all(m)?;

    Ok(())
}
