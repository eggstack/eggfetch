//! Shared Python argument normalization for sync and async dispatch.

use pyo3::prelude::*;

use crate::auth::{self, AuthOverride};
use crate::conversion::{
    build_request_body, is_async_iterable, is_sync_iterable, parse_timeout,
    python_async_iterable_to_request_body, python_cookies_to_header, python_headers_to_rust,
    python_iterable_to_request_body, python_params_to_url, reject_async_only_body,
    validate_body_kwargs_with_files,
};
use crate::errors::map_err;
use crate::extensions::{extract_native_extensions, ExtractedExtensions};
use crate::proxy::{self, ProxyOverride};
use crate::retry;

/// Owned request arguments ready for either runtime-specific dispatcher.
pub(crate) struct PreparedRequest {
    pub method: http::Method,
    pub url: url::Url,
    pub headers: eggfetch_core::Headers,
    pub body: Option<eggfetch_core::RequestBody>,
    pub timeout: Option<eggfetch_core::Timeout>,
    pub auth: AuthOverride,
    pub proxy: ProxyOverride,
    pub proxy_headers: Option<eggfetch_core::Headers>,
    pub proxy_tls_config: Option<eggfetch_core::TlsConfig>,
    pub retry: Option<eggfetch_core::RetryPolicy>,
    pub extensions: ExtractedExtensions,
    pub follow_redirects: Option<bool>,
    pub max_redirects: Option<usize>,
}

/// Normalize the common request arguments while the GIL is held.
// This boundary intentionally mirrors the public request signature so sync
// and async adapters cannot silently diverge in argument handling.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_request<'py>(
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
    auth_value: Option<&Bound<'py, PyAny>>,
    follow_redirects: Option<bool>,
    max_redirects: Option<usize>,
    proxy_value: Option<&Bound<'py, PyAny>>,
    retries: Option<&Bound<'py, PyAny>>,
    extensions: Option<&Bound<'py, PyAny>>,
    allow_async_body: bool,
) -> PyResult<PreparedRequest> {
    let method_upper = method.to_uppercase();
    let method = http::Method::try_from(method_upper.as_str()).map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("invalid HTTP method: {method}"))
    })?;

    let mut url = url::Url::parse(url)
        .map_err(|error| PyErr::new::<pyo3::exceptions::PyValueError, _>(error.to_string()))?;
    if let Some(params) = params {
        python_params_to_url(py, &mut url, params)?;
    }

    validate_body_kwargs_with_files(content, data, json, files)?;
    let mut headers = headers
        .map(|value| python_headers_to_rust(py, value))
        .transpose()?
        .unwrap_or_default();

    let mut body = None;
    let content_type = if let Some(files) = files {
        let (multipart_body, content_type) =
            crate::multipart::build_multipart_body(py, data, files)?;
        body = Some(multipart_body);
        Some(content_type)
    } else {
        let (body_bytes, content_type) = build_request_body(py, content, data, json)?;
        if let Some(bytes) = body_bytes {
            body = Some(eggfetch_core::RequestBody::Bytes(bytes.into()));
        }
        content_type.map(String::from)
    };

    if let Some(content) = content {
        let async_body_available = is_async_iterable(content)?;
        let sync_body_available = is_sync_iterable(content)?;
        if async_body_available && !sync_body_available && !allow_async_body {
            reject_async_only_body(content)?;
        }
        if body.is_none() && files.is_none() {
            if async_body_available && !sync_body_available {
                body = Some(python_async_iterable_to_request_body(py, content)?);
            } else if sync_body_available {
                body = Some(python_iterable_to_request_body(py, content)?);
            }
        }
    }

    if let Some(content_type) = content_type.as_deref() {
        if !headers.contains("content-type") {
            headers
                .insert("content-type", content_type)
                .map_err(map_err)?;
        }
    }
    if let Some(cookie_header) = python_cookies_to_header(cookies, &url)? {
        if !headers.contains("cookie") {
            headers.insert("cookie", &cookie_header).map_err(map_err)?;
        }
    }

    let (proxy_headers, proxy_tls_config) = proxy::extract_proxy_extras(py, proxy_value)?;

    Ok(PreparedRequest {
        method,
        url,
        headers,
        body,
        timeout: parse_timeout(timeout)?,
        auth: auth::parse_auth(auth_value)?,
        proxy: proxy::parse_proxy(proxy_value)?,
        proxy_headers,
        proxy_tls_config,
        retry: retry::parse_retry_option(retries)?,
        extensions: extract_native_extensions(py, extensions)?,
        follow_redirects,
        max_redirects,
    })
}
