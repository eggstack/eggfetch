//! Python async client wrapper over the `eggfetch-core` engine.

use pyo3::prelude::*;

use crate::auth;
use crate::conversion::{
    build_request_body, parse_timeout, python_cookies_to_header, python_headers_to_rust,
    python_params_to_url, validate_body_kwargs_with_files,
};
use crate::cookies::PyCookies;
use crate::errors::map_err;
use crate::limits::PyLimits;
use crate::proxy::{self, ProxyOverride};
use crate::retry;
use crate::streaming::PyStreamingResponse;

/// An async HTTP client exposed to Python.
///
/// Uses `pyo3-async-runtimes` to bridge Rust futures into Python
/// coroutines. No duplicate HTTP logic exists; all network I/O
/// goes through `eggfetch_core::Client`.
///
/// Only the `asyncio` backend is supported initially.
/// Trio/AnyIO support is planned for a later milestone.
#[pyclass(name = "AsyncClient")]
pub struct PyAsyncClient {
    client: Option<eggfetch_core::Client>,
    decompress: Option<bool>,
    verify_disabled: bool,
    #[allow(dead_code)]
    retry: Option<eggfetch_core::RetryPolicy>,
}

#[pymethods]
impl PyAsyncClient {
    /// Create a new async client.
    ///
    /// Args:
    ///     headers: Default headers dict or sequence of pairs (optional).
    ///     timeout: Default timeout in seconds, or Timeout object (optional).
    ///     `follow_redirects`: Whether to follow redirects (default False).
    ///     `max_redirects`: Maximum redirects to follow (default 20).
    #[allow(clippy::too_many_arguments)]
    #[allow(
        clippy::too_many_lines,
        reason = "constructor keeps shared binding configuration at one adapter boundary"
    )]
    #[new]
    #[pyo3(signature = (*, headers=None, timeout=None, follow_redirects=None, max_redirects=None, cookies=None, auth=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, http1=None, http2=None, http3=None, limits=None, trust_env=None, local_address=None, socket_options=None, uds=None))]
    fn new(
        py: Python<'_>,
        headers: Option<&Bound<'_, PyAny>>,
        timeout: Option<&Bound<'_, PyAny>>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
        cookies: Option<&Bound<'_, PyAny>>,
        auth: Option<&Bound<'_, PyAny>>,
        decompress: Option<bool>,
        proxy: Option<&Bound<'_, PyAny>>,
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
    ) -> PyResult<Self> {
        let verify_disabled = verify
            .and_then(|v| v.extract::<bool>().ok())
            .is_some_and(|b| !b);

        let tls_config = crate::tls::build_tls_config(verify, cert, trust_env)?;
        let mut builder = eggfetch_core::Client::builder().tls_config(tls_config);

        // Resolve the HTTP version policy from (http1, http2, http3) flags.
        // When only http1 is explicitly set (http1=True, http2=False/None),
        // use Http1Only. When only http2 is set (http1=False/None, http2=True),
        // use Http2Only for prior-knowledge mode. When both are true,
        // use Auto. http3=True always maps to Http3Only.
        let http1_enabled = http1.unwrap_or(true);
        let http2_enabled = http2.unwrap_or(false);
        if let Some(true) = http3 {
            builder = builder.http_version_policy(eggfetch_core::HttpVersionPolicy::Http3Only);
        } else if !http1_enabled && http2_enabled {
            builder = builder.http_version_policy(eggfetch_core::HttpVersionPolicy::Http2Only);
        } else if http1_enabled && http2_enabled {
            builder = builder
                .http_version_policy(eggfetch_core::HttpVersionPolicy::Auto { allow_http3: false });
        } else if http1_enabled && !http2_enabled {
            builder = builder.http_version_policy(eggfetch_core::HttpVersionPolicy::Http1Only);
        }

        if let Some(l) = limits {
            let py_limits: PyLimits = l.extract()?;
            builder = builder.limits(py_limits.inner);
        }

        if let Some(hdrs) = headers {
            let rust_headers = python_headers_to_rust(py, hdrs)?;
            for (name, value) in rust_headers.iter() {
                builder = builder
                    .default_header(name.as_str(), value.to_str().unwrap_or(""))
                    .map_err(map_err)?;
            }
        }

        if let Some(t) = timeout {
            if let Some(rust_timeout) = parse_timeout(Some(t))? {
                builder = builder.timeout(rust_timeout);
            }
        }

        let redirect = eggfetch_core::redirect::RedirectPolicy::new(
            follow_redirects.unwrap_or(false),
            max_redirects.unwrap_or(20),
        );
        builder = builder.redirect_policy(redirect);

        let jar = eggfetch_core::cookie::CookieJar::new();
        if let Some(c) = cookies {
            if let Ok(dict) = c.downcast::<pyo3::types::PyDict>() {
                for (key, value) in dict.iter() {
                    let name: String = key.extract()?;
                    let val: String = value.extract()?;
                    jar.set_default_cookie(name, val);
                }
            }
        }
        builder = builder.cookie_jar(jar);

        let auth_override = auth::parse_auth(auth)?;
        match auth_override {
            auth::AuthOverride::Inherit | auth::AuthOverride::Disable => {}
            auth::AuthOverride::Override(a) => {
                builder = builder.auth(a);
            }
        }

        let proxy_override = proxy::parse_proxy(proxy)?;
        if let ProxyOverride::Override(ref url) = proxy_override {
            let mut p = eggfetch_core::Proxy::all_compat(&proxy::normalize_compat_proxy_url(url))
                .map_err(map_err)?;
            if let Some(proxy_obj) = proxy {
                if let Ok(proxy_module) = py.import("eggfetch.compat.httpx._proxy") {
                    if let Ok(proxy_class) = proxy_module.getattr("Proxy") {
                        if proxy_obj.is_instance(&proxy_class).unwrap_or(false) {
                            if let Ok(headers) = proxy_obj.getattr("headers") {
                                let rust_headers =
                                    crate::conversion::python_headers_to_rust(py, &headers)?;
                                p = p.proxy_headers(rust_headers);
                            }
                        }
                    }
                }
            }
            builder = builder.proxy(p);
        }

        let trust_env = trust_env.unwrap_or(true);
        if trust_env && proxy_override == ProxyOverride::Inherit {
            #[cfg(feature = "proxy")]
            {
                for (scheme, env_proxy) in proxy::env_proxy_urls(py)? {
                    let mut p = match scheme {
                        "http" => eggfetch_core::Proxy::http(&env_proxy),
                        "https" => eggfetch_core::Proxy::https(&env_proxy),
                        _ => eggfetch_core::Proxy::all(&env_proxy),
                    }
                    .map_err(map_err)?;
                    if let Some(no_proxy) = proxy::env_no_proxy(py)? {
                        let rules =
                            eggfetch_core::NoProxy::parse_httpx(&no_proxy).map_err(map_err)?;
                        p = p.no_proxy(rules);
                    }
                    builder = builder.environment_proxy(p);
                }
            }
        }

        let retry_policy = retry::parse_retry_option(retries)?;
        if let Some(ref policy) = retry_policy {
            builder = builder.retry(policy.clone());
        }

        // Forward advanced transport options.
        if let Some(addr_str) = local_address {
            let addr = crate::conversion::parse_local_address(addr_str)?;
            builder = builder.local_address(addr);
        }
        if let Some(opts) = socket_options {
            let rust_opts = crate::conversion::parse_socket_options(opts)?;
            if !rust_opts.is_empty() {
                builder = builder.socket_options(rust_opts);
            }
        }
        if let Some(path) = uds {
            builder = builder.uds_path(path.to_owned());
        }

        let client = builder.build();

        Ok(Self {
            client: Some(client),
            decompress,
            verify_disabled,
            retry: retry_policy,
        })
    }

    /// Send an HTTP request asynchronously.
    #[pyo3(signature = (method, url, *, headers=None, params=None, content=None, data=None, json=None, files=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None))]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    fn request<'py>(
        &self,
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
    ) -> PyResult<Bound<'py, PyAny>> {
        self.ensure_not_closed()?;

        if verify.is_some() || cert.is_some() {
            return Err(PyErr::new::<crate::errors::UnsupportedKwarg, _>(
                "verify and cert are client-level only; set them on the AsyncClient() constructor",
            ));
        }

        let method_upper = method.to_uppercase();
        let http_method = http::Method::try_from(method_upper.as_str()).map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "invalid HTTP method: {method}"
            ))
        })?;

        let mut target_url = url::Url::parse(url)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

        if let Some(p) = params {
            python_params_to_url(py, &mut target_url, p)?;
        }
        let target_url = target_url;

        validate_body_kwargs_with_files(content, data, json, files)?;

        let mut rust_headers = if let Some(h) = headers {
            python_headers_to_rust(py, h)?
        } else {
            eggfetch_core::Headers::new()
        };

        let (body_bytes, auto_content_type): (Option<Vec<u8>>, Option<String>) =
            if let Some(f) = files {
                let (body, ct) = crate::multipart::build_multipart_body(py, data, f)?;
                match body {
                    eggfetch_core::RequestBody::Bytes(b) => (Some(b.to_vec()), Some(ct)),
                    _ => (None, Some(ct)),
                }
            } else {
                let (bytes, ct) = build_request_body(py, content, data, json)?;
                (bytes, ct.map(String::from))
            };

        // Check if content is a Python iterable (not bytes/str) — treat as stream body.
        let stream_body = if let Some(c) = content {
            if body_bytes.is_none() && files.is_none() && crate::conversion::is_python_iterable(c)?
            {
                Some(crate::conversion::python_iterable_to_request_body(py, c)?)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(ct) = &auto_content_type {
            if !rust_headers.contains("content-type") {
                rust_headers.insert("content-type", ct).map_err(map_err)?;
            }
        }

        if let Some(cookie_header) = python_cookies_to_header(cookies, &target_url)? {
            if !rust_headers.contains("cookie") {
                rust_headers
                    .insert("cookie", &cookie_header)
                    .map_err(map_err)?;
            }
        }

        let rust_timeout = parse_timeout(timeout)?;

        let auth_override = auth::parse_auth(auth)?;

        let proxy_override = proxy::parse_proxy(proxy)?;

        let retry_override = retry::parse_retry_option(retries)?;

        let effective_decompress = decompress.or(self.decompress);

        let client = self.ensure_client()?.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut builder = client
                .request(http_method, target_url.as_str())
                .map_err(map_err)?;

            for (name, value) in rust_headers.iter() {
                builder = builder.header(name.as_str(), value.to_str().unwrap_or(""));
            }

            if let Some(bytes) = body_bytes {
                builder = builder.bytes(bytes);
            } else if let Some(stream) = stream_body {
                builder = builder.body(stream);
            }

            if let Some(t) = rust_timeout {
                builder = builder.timeout(t);
            }

            if let Some(d) = effective_decompress {
                builder = builder.decompress(d);
            }

            match auth_override {
                auth::AuthOverride::Inherit => {}
                auth::AuthOverride::Disable => {
                    builder = builder.without_auth();
                }
                auth::AuthOverride::Override(a) => {
                    builder = builder.auth(a);
                }
            }

            match proxy_override {
                ProxyOverride::Inherit => {}
                ProxyOverride::Disable => {
                    builder = builder.without_proxy();
                }
                ProxyOverride::Override(url) => {
                    let p =
                        eggfetch_core::Proxy::all_compat(&proxy::normalize_compat_proxy_url(&url))
                            .map_err(map_err)?;
                    builder = builder.proxy(&p);
                }
            }

            if follow_redirects.is_some() || max_redirects.is_some() {
                let mut redirect = eggfetch_core::redirect::RedirectPolicy::default();
                if let Some(f) = follow_redirects {
                    redirect.follow = f;
                }
                if let Some(m) = max_redirects {
                    redirect.max_redirects = m;
                }
                builder = builder.redirect_policy(redirect);
            }

            if let Some(retry_policy) = retry_override.as_ref() {
                builder = builder.retry(retry_policy.clone());
            }

            let mut response = Box::pin(builder.send()).await.map_err(map_err)?;
            let content = response.bytes().await.map_err(map_err)?;
            crate::response::PyResponse::from_core_response_with_body(response, content)
        })
    }

    /// Send a GET request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None))]
    #[allow(clippy::too_many_arguments)]
    fn get<'py>(
        &self,
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
    ) -> PyResult<Bound<'py, PyAny>> {
        self.request(
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
        )
    }

    /// Send a POST request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, content=None, data=None, json=None, files=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None))]
    #[allow(clippy::too_many_arguments)]
    fn post<'py>(
        &self,
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
    ) -> PyResult<Bound<'py, PyAny>> {
        self.request(
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
        )
    }

    /// Send a PUT request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, content=None, data=None, json=None, files=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None))]
    #[allow(clippy::too_many_arguments)]
    fn put<'py>(
        &self,
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
    ) -> PyResult<Bound<'py, PyAny>> {
        self.request(
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
        )
    }

    /// Send a PATCH request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, content=None, data=None, json=None, files=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None))]
    #[allow(clippy::too_many_arguments)]
    fn patch<'py>(
        &self,
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
    ) -> PyResult<Bound<'py, PyAny>> {
        self.request(
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
        )
    }

    /// Send a DELETE request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None))]
    #[allow(clippy::too_many_arguments)]
    fn delete<'py>(
        &self,
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
    ) -> PyResult<Bound<'py, PyAny>> {
        self.request(
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
        )
    }

    /// Send a HEAD request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None))]
    #[allow(clippy::too_many_arguments)]
    fn head<'py>(
        &self,
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
    ) -> PyResult<Bound<'py, PyAny>> {
        self.request(
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
        )
    }

    /// Send an OPTIONS request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None))]
    #[allow(clippy::too_many_arguments)]
    fn options<'py>(
        &self,
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
    ) -> PyResult<Bound<'py, PyAny>> {
        self.request(
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
        )
    }

    /// Send a streaming HTTP request asynchronously.
    #[pyo3(signature = (method, url, *, headers=None, params=None, content=None, data=None, json=None, files=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, extensions=None))]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    fn stream<'py>(
        &self,
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
        extensions: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.ensure_not_closed()?;

        if verify.is_some() || cert.is_some() {
            return Err(PyErr::new::<crate::errors::UnsupportedKwarg, _>(
                "verify and cert are client-level only; set them on the AsyncClient() constructor",
            ));
        }

        let method_upper = method.to_uppercase();
        let http_method = http::Method::try_from(method_upper.as_str()).map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "invalid HTTP method: {method}"
            ))
        })?;

        let mut target_url = url::Url::parse(url)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

        if let Some(p) = params {
            python_params_to_url(py, &mut target_url, p)?;
        }
        let target_url = target_url;

        validate_body_kwargs_with_files(content, data, json, files)?;

        let mut rust_headers = if let Some(h) = headers {
            python_headers_to_rust(py, h)?
        } else {
            eggfetch_core::Headers::new()
        };

        let (body_bytes, auto_content_type): (Option<Vec<u8>>, Option<String>) =
            if let Some(f) = files {
                let (body, ct) = crate::multipart::build_multipart_body(py, data, f)?;
                match body {
                    eggfetch_core::RequestBody::Bytes(b) => (Some(b.to_vec()), Some(ct)),
                    _ => (None, Some(ct)),
                }
            } else {
                let (bytes, ct) = build_request_body(py, content, data, json)?;
                (bytes, ct.map(String::from))
            };

        // Check if content is a Python iterable (not bytes/str) — treat as stream body.
        let stream_body = if let Some(c) = content {
            if body_bytes.is_none() && files.is_none() && crate::conversion::is_python_iterable(c)?
            {
                Some(crate::conversion::python_iterable_to_request_body(py, c)?)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(ct) = &auto_content_type {
            if !rust_headers.contains("content-type") {
                rust_headers.insert("content-type", ct).map_err(map_err)?;
            }
        }

        if let Some(cookie_header) = python_cookies_to_header(cookies, &target_url)? {
            if !rust_headers.contains("cookie") {
                rust_headers
                    .insert("cookie", &cookie_header)
                    .map_err(map_err)?;
            }
        }

        let rust_timeout = parse_timeout(timeout)?;
        let auth_override = auth::parse_auth(auth)?;

        let proxy_override = proxy::parse_proxy(proxy)?;

        // Extract proxy headers from a Python Proxy object before
        // entering the async closure.
        let proxy_headers = if let Some(proxy_obj) = proxy {
            if let Ok(proxy_module) = py.import("eggfetch.compat.httpx._proxy") {
                if let Ok(proxy_class) = proxy_module.getattr("Proxy") {
                    if proxy_obj.is_instance(&proxy_class).unwrap_or(false) {
                        if let Ok(headers) = proxy_obj.getattr("headers") {
                            Some(crate::conversion::python_headers_to_rust(py, &headers)?)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let retry_override = retry::parse_retry_option(retries)?;

        // Extract transport hints from extensions dict before the async block.
        let transport_hints = if let Some(ext) = extensions {
            if let Ok(dict) = ext.downcast::<pyo3::types::PyDict>() {
                let mut hints = eggfetch_core::TransportHints::default();
                if let Some(target_val) = dict.get_item("target").ok().flatten() {
                    if let Ok(b) = target_val.extract::<Vec<u8>>() {
                        hints.target = Some(bytes::Bytes::from(b));
                    } else if let Ok(s) = target_val.extract::<String>() {
                        hints.target = Some(bytes::Bytes::from(s.into_bytes()));
                    }
                }
                if let Some(sni_val) = dict.get_item("sni_hostname").ok().flatten() {
                    if let Ok(s) = sni_val.extract::<String>() {
                        hints.sni_hostname = Some(s);
                    }
                }
                if hints.target.is_some() || hints.sni_hostname.is_some() {
                    Some(hints)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let effective_decompress = decompress.or(self.decompress);

        let client = self.ensure_client()?.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let runtime_handle = tokio::runtime::Handle::current();
            let mut builder = client
                .request(http_method, target_url.as_str())
                .map_err(map_err)?;

            for (name, value) in rust_headers.iter() {
                builder = builder.header(name.as_str(), value.to_str().unwrap_or(""));
            }

            if let Some(bytes) = body_bytes {
                builder = builder.bytes(bytes);
            } else if let Some(stream) = stream_body {
                builder = builder.body(stream);
            }

            if let Some(t) = rust_timeout {
                builder = builder.timeout(t);
            }

            if let Some(d) = effective_decompress {
                builder = builder.decompress(d);
            }

            match auth_override {
                auth::AuthOverride::Inherit => {}
                auth::AuthOverride::Disable => {
                    builder = builder.without_auth();
                }
                auth::AuthOverride::Override(a) => {
                    builder = builder.auth(a);
                }
            }

            match proxy_override {
                ProxyOverride::Inherit => {}
                ProxyOverride::Disable => {
                    builder = builder.without_proxy();
                }
                ProxyOverride::Override(url) => {
                    let mut p =
                        eggfetch_core::Proxy::all_compat(&proxy::normalize_compat_proxy_url(&url))
                            .map_err(map_err)?;
                    if let Some(ref hdrs) = proxy_headers {
                        p = p.proxy_headers(hdrs.clone());
                    }
                    builder = builder.proxy(&p);
                }
            }

            if follow_redirects.is_some() || max_redirects.is_some() {
                let mut redirect = eggfetch_core::redirect::RedirectPolicy::default();
                if let Some(f) = follow_redirects {
                    redirect.follow = f;
                }
                if let Some(m) = max_redirects {
                    redirect.max_redirects = m;
                }
                builder = builder.redirect_policy(redirect);
            }

            if let Some(retry_policy) = retry_override.as_ref() {
                builder = builder.retry(retry_policy.clone());
            }

            // Apply pre-extracted transport hints.
            if let Some(hints) = transport_hints {
                builder = builder.transport_hints(hints);
            }

            let response = Box::pin(builder.send()).await.map_err(map_err)?;
            let obj: PyObject = Python::with_gil(|py| {
                PyStreamingResponse::from_core_response(py, response, runtime_handle, None)
                    .map(|r| r.unbind().into_any())
            })?;
            Ok(obj)
        })
    }

    /// Close the client and release all resources.
    ///
    /// Drops the underlying `eggfetch-core` client (closing idle connections).
    /// Subsequent requests raise `ValueError`. Idempotent.
    fn close(&mut self) {
        self.client = None;
    }

    /// Close the client through an awaitable API.
    fn aclose<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::PyAny>> {
        self.close();
        pyo3_async_runtimes::tokio::future_into_py(py, async { Ok(()) })
    }

    /// Returns True if the client has been closed.
    #[getter]
    fn is_closed(&self) -> bool {
        self.client.is_none()
    }

    /// The client's cookie jar.
    #[getter]
    fn cookies(&self) -> PyResult<PyCookies> {
        let client = self.ensure_client()?;
        Ok(PyCookies::from_jar(client.cookies().clone()))
    }

    /// Async context manager: enter. Returns an awaitable that resolves to self.
    fn __aenter__(slf: Py<Self>, py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        let asyncio = py.import("asyncio")?;
        let future = asyncio.getattr("Future")?.call0()?;
        future.call_method1("set_result", (slf,))?;
        Ok(future)
    }

    /// Async context manager: exit. Closes the client.
    #[pyo3(signature = (_exc_type=None, _exc_value=None, _traceback=None))]
    fn __aexit__<'py>(
        &mut self,
        py: Python<'py>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.close();
        let asyncio = py.import("asyncio")?;
        let future = asyncio.getattr("Future")?.call0()?;
        future.call_method1("set_result", (false,))?;
        Ok(future)
    }

    fn __repr__(&self) -> String {
        if self.client.is_none() {
            "AsyncClient(closed=true)".to_string()
        } else if self.verify_disabled {
            "AsyncClient(verify=False) [UNSAFE: TLS verification disabled]".to_string()
        } else {
            "AsyncClient()".to_string()
        }
    }
}

impl PyAsyncClient {
    fn ensure_not_closed(&self) -> PyResult<()> {
        if self.client.is_none() {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "client is closed",
            ));
        }
        Ok(())
    }

    fn ensure_client(&self) -> PyResult<&eggfetch_core::Client> {
        self.client
            .as_ref()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("client is closed"))
    }
}
