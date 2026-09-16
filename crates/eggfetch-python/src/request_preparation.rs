//! Shared Python argument normalization for sync and async dispatch.

use pyo3::prelude::*;

use crate::auth::{self, AuthOverride};
use crate::conversion::{
    classify_python_body, encode_form_body, encode_json_body, parse_timeout,
    python_async_iterable_to_request_body, python_cookies_to_header, python_headers_to_rust,
    python_iterable_to_request_body, python_params_to_url, validate_body_kwargs_with_files,
    PythonBodyKind,
};
use crate::errors::{map_err, InvalidUrl};
use crate::extensions::{extract_native_extensions, ExtractedExtensions};
use crate::proxy::{self, ProxyOverride};
use crate::retry;

/// Owned client arguments shared by the sync and async adapters.
///
/// This type deliberately contains no runtime state.  The synchronous
/// adapter adds its owned runtime after preparation, while the asynchronous
/// adapter dispatches on the ambient asyncio runtime.
pub(crate) struct PreparedClientConfig {
    pub tls_config: eggfetch_core::TlsConfig,
    pub http_version_policy: eggfetch_core::HttpVersionPolicy,
    pub limits: Option<eggfetch_core::Limits>,
    pub default_headers: eggfetch_core::Headers,
    pub timeout: Option<eggfetch_core::Timeout>,
    pub redirect_policy: eggfetch_core::redirect::RedirectPolicy,
    pub cookie_jar: eggfetch_core::cookie::CookieJar,
    pub auth: AuthOverride,
    pub proxy: ProxyOverride,
    pub proxy_headers: Option<eggfetch_core::Headers>,
    pub proxy_tls_config: Option<eggfetch_core::TlsConfig>,
    pub environment_proxies: Vec<eggfetch_core::Proxy>,
    pub retry: Option<eggfetch_core::RetryPolicy>,
    pub local_address: Option<std::net::SocketAddr>,
    pub socket_options: Vec<eggfetch_core::SocketOption>,
    pub uds: Option<String>,
    pub decompress: Option<bool>,
    pub verify_disabled: bool,
}

/// Normalize constructor arguments while the GIL is held.
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::too_many_lines,
    reason = "the GIL-held constructor boundary keeps all shared policy in one place"
)]
pub(crate) fn prepare_client_config(
    py: Python<'_>,
    headers: Option<&Bound<'_, PyAny>>,
    timeout: Option<&Bound<'_, PyAny>>,
    follow_redirects: Option<bool>,
    max_redirects: Option<usize>,
    cookies: Option<&Bound<'_, PyAny>>,
    auth_value: Option<&Bound<'_, PyAny>>,
    decompress: Option<bool>,
    proxy_value: Option<&Bound<'_, PyAny>>,
    verify: Option<&Bound<'_, PyAny>>,
    cert: Option<&Bound<'_, PyAny>>,
    retries: Option<&Bound<'_, PyAny>>,
    http1: Option<bool>,
    http2: Option<bool>,
    http3: Option<bool>,
    limits: Option<&Bound<'_, PyAny>>,
    trust_env: Option<bool>,
    local_address: Option<&str>,
    socket_options: Option<&Bound<'_, PyAny>>,
    uds: Option<&str>,
) -> PyResult<PreparedClientConfig> {
    let tls_config = crate::tls::build_tls_config(verify, cert, trust_env)?;
    let http1_enabled = http1.unwrap_or(true);
    let http2_enabled = http2.unwrap_or(false);
    let http_version_policy = if http3 == Some(true) {
        eggfetch_core::HttpVersionPolicy::Http3Only
    } else if !http1_enabled && http2_enabled {
        eggfetch_core::HttpVersionPolicy::Http2Only
    } else if http1_enabled && http2_enabled {
        eggfetch_core::HttpVersionPolicy::Auto { allow_http3: false }
    } else if http1_enabled && !http2_enabled {
        eggfetch_core::HttpVersionPolicy::Http1Only
    } else {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "At least one of http1 or http2 must be True",
        ));
    };

    let default_headers = headers
        .map(|value| python_headers_to_rust(py, value))
        .transpose()?
        .unwrap_or_default();
    let timeout = timeout
        .map(|value| parse_timeout(Some(value)))
        .transpose()?
        .flatten();
    let redirect_policy = eggfetch_core::redirect::RedirectPolicy::new(
        follow_redirects.unwrap_or(false),
        max_redirects.unwrap_or(20),
    );

    let cookie_jar = eggfetch_core::cookie::CookieJar::new();
    if let Some(cookies) = cookies {
        if let Ok(dict) = cookies.cast::<pyo3::types::PyDict>() {
            for (key, value) in dict.iter() {
                let name: String = key.extract()?;
                let value: String = value.extract()?;
                cookie_jar
                    .set_default_cookie(name, value)
                    .map_err(|error| {
                        PyErr::new::<pyo3::exceptions::PyValueError, _>(error.to_string())
                    })?;
            }
        }
    }

    let auth = auth::parse_auth(auth_value)?;
    let proxy = proxy::parse_proxy(proxy_value)?;
    let (proxy_headers, proxy_tls_config) = proxy::extract_proxy_extras(py, proxy_value)?;
    let mut environment_proxies = Vec::new();

    if trust_env.unwrap_or(true) && proxy == ProxyOverride::Inherit {
        #[cfg(feature = "proxy")]
        for (scheme, env_proxy) in proxy::env_proxy_urls(py)? {
            let mut env = match scheme {
                "http" => eggfetch_core::Proxy::http(&env_proxy),
                "https" => eggfetch_core::Proxy::https(&env_proxy),
                _ => eggfetch_core::Proxy::all(&env_proxy),
            }
            .map_err(map_err)?;
            if let Some(no_proxy) = proxy::env_no_proxy(py)? {
                let rules =
                    eggfetch_core::NoProxy::parse_httpx(&no_proxy).map_err(
                        |error| match error {
                            eggfetch_core::Error::InvalidProxyUrl(message) => {
                                InvalidUrl::new_err(message)
                            }
                            other => map_err(other),
                        },
                    )?;
                env = env.no_proxy(rules);
            }
            environment_proxies.push(env);
        }
    }

    let local_address = local_address
        .map(crate::conversion::parse_local_address)
        .transpose()?;
    let socket_options = socket_options
        .map(crate::conversion::parse_socket_options)
        .transpose()?
        .unwrap_or_default();

    Ok(PreparedClientConfig {
        tls_config,
        http_version_policy,
        limits: match limits {
            Some(value) => Some(value.extract::<crate::limits::PyLimits>()?.inner),
            None => None,
        },
        default_headers,
        timeout,
        redirect_policy,
        cookie_jar,
        auth,
        proxy,
        proxy_headers,
        proxy_tls_config,
        environment_proxies,
        retry: retry::parse_retry_option(retries)?,
        local_address,
        socket_options,
        uds: uds.map(str::to_owned),
        decompress,
        verify_disabled: verify
            .and_then(|value| value.extract::<bool>().ok())
            .is_some_and(|value| !value),
    })
}

/// Apply prepared client configuration to the shared core builder.
pub(crate) fn apply_client_config(config: PreparedClientConfig) -> PyResult<eggfetch_core::Client> {
    let PreparedClientConfig {
        tls_config,
        http_version_policy,
        limits,
        default_headers,
        timeout,
        redirect_policy,
        cookie_jar,
        auth,
        proxy,
        proxy_headers,
        proxy_tls_config,
        environment_proxies,
        retry,
        local_address,
        socket_options,
        uds,
        ..
    } = config;

    let mut builder = eggfetch_core::Client::builder()
        .tls_config(tls_config)
        .http_version_policy(http_version_policy)
        .default_headers(default_headers)
        .redirect_policy(redirect_policy)
        .cookie_jar(cookie_jar);
    if let Some(limits) = limits {
        builder = builder.limits(limits);
    }
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    match auth {
        AuthOverride::Inherit | AuthOverride::Disable => {}
        AuthOverride::Override(auth) => builder = builder.auth(auth),
    }
    if let ProxyOverride::Override(url) = proxy {
        let mut value = eggfetch_core::Proxy::all_compat(&proxy::normalize_compat_proxy_url(&url))
            .map_err(map_err)?;
        if let Some(headers) = proxy_headers {
            value = value.proxy_headers(headers);
        }
        if let Some(tls_config) = proxy_tls_config {
            value = value.with_proxy_tls_config(tls_config);
        }
        builder = builder.proxy(value);
    }
    for proxy in environment_proxies {
        builder = builder.environment_proxy(proxy);
    }
    if let Some(retry) = retry {
        builder = builder.retry(retry);
    }
    if let Some(address) = local_address {
        builder = builder.local_address(address);
    }
    if !socket_options.is_empty() {
        builder = builder.socket_options(socket_options);
    }
    if let Some(path) = uds {
        builder = builder.uds_path(path);
    }
    Ok(builder.build())
}

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

/// Runtime-neutral defaults needed while applying one prepared request.
pub(crate) struct RequestDispatchDefaults {
    pub redirect_policy: eggfetch_core::redirect::RedirectPolicy,
    pub decompress: Option<bool>,
}

/// A core request builder plus callback state that must be checked by the
/// runtime-specific dispatcher after the send future completes.
pub(crate) struct PreparedDispatch {
    pub builder: eggfetch_core::RequestBuilder,
    pub trace_error_slot: Option<crate::trace_bridge::CallbackErrorSlot>,
}

/// Apply normalized Python request state to a core builder.
///
/// This function deliberately performs no I/O and contains no Python or
/// runtime handling. Sync and async adapters share this mapping while keeping
/// their own GIL, runtime, response, and callback-error boundaries.
pub(crate) fn prepare_core_dispatch(
    client: &eggfetch_core::Client,
    request: PreparedRequest,
    defaults: RequestDispatchDefaults,
) -> PyResult<PreparedDispatch> {
    let PreparedRequest {
        method,
        url,
        headers,
        body,
        timeout,
        auth,
        proxy,
        proxy_headers,
        proxy_tls_config,
        retry,
        extensions,
        follow_redirects,
        max_redirects,
    } = request;
    let RequestDispatchDefaults {
        redirect_policy,
        decompress,
    } = defaults;

    let mut builder = client.request(method, url.as_str()).map_err(map_err)?;
    builder = builder.headers(headers);
    if let Some(body) = body {
        builder = builder.body(body);
    }
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    if let Some(decompress) = decompress {
        builder = builder.decompress(decompress);
    }

    match auth {
        AuthOverride::Inherit => {}
        AuthOverride::Disable => builder = builder.without_auth(),
        AuthOverride::Override(auth) => builder = builder.auth(auth),
    }

    match proxy {
        ProxyOverride::Inherit => {}
        ProxyOverride::Disable => builder = builder.without_proxy(),
        ProxyOverride::Override(url) => {
            let mut proxy =
                eggfetch_core::Proxy::all_compat(&proxy::normalize_compat_proxy_url(&url))
                    .map_err(map_err)?;
            if let Some(headers) = proxy_headers {
                proxy = proxy.proxy_headers(headers);
            }
            if let Some(tls_config) = proxy_tls_config {
                proxy = proxy.with_proxy_tls_config(tls_config);
            }
            builder = builder.proxy(&proxy);
        }
    }

    if follow_redirects.is_some() || max_redirects.is_some() {
        let mut redirect = redirect_policy;
        if let Some(follow) = follow_redirects {
            redirect.follow = follow;
        }
        if let Some(max) = max_redirects {
            redirect.max_redirects = max;
        }
        builder = builder.redirect_policy(redirect);
    }

    if let Some(retry) = retry {
        builder = builder.retry(retry);
    }
    builder = builder.transport_hints(extensions.hints);

    Ok(PreparedDispatch {
        builder,
        trace_error_slot: extensions.trace_error_slot,
    })
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
    } else if let Some(content) = content {
        match classify_python_body(content)? {
            PythonBodyKind::Buffered(bytes) => {
                body = Some(eggfetch_core::RequestBody::Bytes(bytes.into()));
            }
            PythonBodyKind::Sync(iterator) => {
                body = Some(python_iterable_to_request_body(iterator));
            }
            PythonBodyKind::Async(_iterator) if !allow_async_body => {
                return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                    "async iterable request bodies are supported only by AsyncClient",
                ));
            }
            PythonBodyKind::Async(iterator) => {
                body = Some(python_async_iterable_to_request_body(py, iterator)?);
            }
        }
        None
    } else if data.is_some() {
        Some(String::from("application/x-www-form-urlencoded"))
    } else if json.is_some() {
        Some(String::from("application/json"))
    } else {
        None
    };

    if body.is_none() {
        if let Some(data) = data {
            body = Some(eggfetch_core::RequestBody::Bytes(
                encode_form_body(py, data)?.into(),
            ));
        } else if let Some(json) = json {
            body = Some(eggfetch_core::RequestBody::Bytes(
                encode_json_body(py, json)?.into(),
            ));
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
