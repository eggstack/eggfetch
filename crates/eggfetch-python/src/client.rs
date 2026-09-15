//! Python client wrapper.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use pyo3::prelude::*;

use crate::auth;
use crate::cookies::PyCookies;
use crate::errors::map_err;
use crate::proxy::{self, ProxyOverride};
use crate::response::PyResponse;
use crate::streaming::PyStreamingResponse;
use crate::trace_bridge::take_callback_error;

/// Shared owner for the synchronous client's runtime.
pub(crate) struct RuntimeState {
    runtime: Mutex<Option<tokio::runtime::Runtime>>,
    shutdown_requested: AtomicBool,
}

impl RuntimeState {
    fn new(runtime: tokio::runtime::Runtime) -> Self {
        Self {
            runtime: Mutex::new(Some(runtime)),
            shutdown_requested: AtomicBool::new(false),
        }
    }

    fn handle(&self) -> Option<tokio::runtime::Handle> {
        self.runtime
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(tokio::runtime::Runtime::handle)
            .cloned()
    }

    fn request_shutdown(self: &Arc<Self>) {
        self.shutdown_requested.store(true, Ordering::Release);
        self.try_shutdown();
    }

    fn try_shutdown(self: &Arc<Self>) {
        if !self.shutdown_requested.load(Ordering::Acquire) {
            return;
        }
        if Arc::strong_count(self) != 1 {
            return;
        }
        // Re-check after acquiring the runtime lock to narrow the race where
        // another thread clones the Arc between the count check and the lock.
        let mut guard = self
            .runtime
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if Arc::strong_count(self) != 1 {
            return;
        }
        if let Some(runtime) = guard.take() {
            runtime.shutdown_background();
        }
    }
}

/// A runtime reference held for the duration of one dispatch.
#[derive(Clone)]
pub(crate) struct RuntimeGuard(Arc<RuntimeState>);

impl Drop for RuntimeGuard {
    fn drop(&mut self) {
        self.0.try_shutdown();
    }
}

/// A synchronous HTTP client exposed to Python.
///
/// Owns a `tokio` runtime and an `eggfetch-core` client. Releases the GIL
/// during network I/O. Thread-safe: multiple Python threads may call request
/// methods concurrently on the same `Client` instance.
#[pyclass(name = "Client")]
pub struct PyClient {
    runtime: std::sync::Mutex<Option<Arc<RuntimeState>>>,
    client: Mutex<Option<eggfetch_core::Client>>,
    decompress: Option<bool>,
    verify_disabled: bool,
}

#[pymethods]
impl PyClient {
    /// Create a new client.
    ///
    /// Args:
    ///     headers: Default headers dict or sequence of pairs (optional).
    ///     timeout: Default timeout in seconds, or Timeout object (optional).
    ///     `follow_redirects`: Whether to follow redirects (default False).
    ///     `max_redirects`: Maximum redirects to follow (default 20).
    #[allow(clippy::too_many_arguments)]
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
        let prepared = crate::request_preparation::prepare_client_config(
            py,
            headers,
            timeout,
            follow_redirects,
            max_redirects,
            cookies,
            auth,
            decompress,
            proxy,
            verify,
            cert,
            retries,
            http1,
            http2,
            http3,
            limits,
            trust_env,
            local_address,
            socket_options,
            uds,
        )?;
        let runtime = Arc::new(RuntimeState::new(tokio::runtime::Runtime::new().map_err(
            |e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()),
        )?));
        let verify_disabled = prepared.verify_disabled;
        let decompress = prepared.decompress;
        let client = crate::request_preparation::apply_client_config(prepared)?;

        Ok(Self {
            runtime: std::sync::Mutex::new(Some(runtime)),
            client: Mutex::new(Some(client)),
            decompress,
            verify_disabled,
        })
    }

    /// Send an HTTP request.
    #[pyo3(signature = (method, url, *, headers=None, params=None, content=None, data=None, json=None, files=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, extensions=None))]
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
        extensions: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.ensure_not_closed()?;

        if verify.is_some() || cert.is_some() {
            return Err(PyErr::new::<crate::errors::UnsupportedKwarg, _>(
                "verify and cert are client-level only; set them on the Client() constructor",
            ));
        }

        let crate::request_preparation::PreparedRequest {
            method: http_method,
            url: target_url,
            headers: rust_headers,
            body: request_body,
            timeout: rust_timeout,
            auth: auth_override,
            proxy: proxy_override,
            proxy_headers,
            proxy_tls_config,
            retry: retry_override,
            extensions: extracted,
            follow_redirects: prepared_follow_redirects,
            max_redirects: prepared_max_redirects,
        } = crate::request_preparation::prepare_request(
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
            extensions,
            false,
        )?;
        let transport_hints = extracted.hints;

        let client = self.clone_client()?;
        let trace_slot = extracted.trace_error_slot.clone();
        let (runtime_guard, runtime_handle) = self.runtime_for_dispatch()?;
        let effective_decompress = decompress.or(self.decompress);
        let result = py.detach(|| {
            runtime_handle.block_on(async {
                let mut builder = client
                    .request(http_method, target_url.as_str())
                    .map_err(map_err)?;

                builder = builder.headers(rust_headers);

                if let Some(body) = request_body {
                    builder = builder.body(body);
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
                        let mut p = eggfetch_core::Proxy::all_compat(
                            &proxy::normalize_compat_proxy_url(&url),
                        )
                        .map_err(map_err)?;
                        if let Some(ref hdrs) = proxy_headers {
                            p = p.proxy_headers(hdrs.clone());
                        }
                        if let Some(ref tls) = proxy_tls_config {
                            p = p.with_proxy_tls_config(tls.clone());
                        }
                        builder = builder.proxy(&p);
                    }
                }

                if prepared_follow_redirects.is_some() || prepared_max_redirects.is_some() {
                    let mut redirect = eggfetch_core::redirect::RedirectPolicy::default();
                    if let Some(f) = prepared_follow_redirects {
                        redirect.follow = f;
                    }
                    if let Some(m) = prepared_max_redirects {
                        redirect.max_redirects = m;
                    }
                    builder = builder.redirect_policy(redirect);
                }

                if let Some(retry_policy) = retry_override.as_ref() {
                    builder = builder.retry(retry_policy.clone());
                }

                // Install transport hints from the Python `extensions`
                // dict.  When no hints are supplied this is a no-op.
                builder = builder.transport_hints(transport_hints.clone());

                let mut response = Box::pin(builder.send()).await.map_err(map_err)?;
                // Consume the body on the client's persistent runtime.  The
                // response owns transport state (including the pool lease),
                // so moving it to a short-lived runtime after `send()` can
                // strand the next pooled HTTP/1 connection.
                let content = response.bytes().await.map_err(map_err)?;
                Ok::<_, PyErr>((response, content))
            })
        });

        // Surface any trace-callback errors recorded during dispatch, BEFORE
        // unwrapping the transport result so that callback errors are not
        // shadowed by network failures.
        if let Some(slot) = trace_slot {
            if let Some(err) = take_callback_error(&slot) {
                return Err(err);
            }
        }

        let (mut response, content) = result?;
        let runtime_lease = crate::streaming::RuntimeLease::new(runtime_guard);
        let py_response = PyResponse::from_core_response_with_body(
            &mut response,
            content,
            Some(&runtime_handle),
            Some(&runtime_lease),
            false,
        )?;
        Ok(Py::new(py, py_response)?.into_bound(py).into_any())
    }

    /// Send a GET request.
    #[pyo3(signature = (url, *, headers=None, params=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, extensions=None))]
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
        extensions: Option<&Bound<'py, PyAny>>,
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
            extensions,
        )
    }

    /// Send a POST request.
    #[pyo3(signature = (url, *, headers=None, params=None, content=None, data=None, json=None, files=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, extensions=None))]
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
        extensions: Option<&Bound<'py, PyAny>>,
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
            extensions,
        )
    }

    /// Send a PUT request.
    #[pyo3(signature = (url, *, headers=None, params=None, content=None, data=None, json=None, files=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, extensions=None))]
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
        extensions: Option<&Bound<'py, PyAny>>,
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
            extensions,
        )
    }

    /// Send a PATCH request.
    #[pyo3(signature = (url, *, headers=None, params=None, content=None, data=None, json=None, files=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, extensions=None))]
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
        extensions: Option<&Bound<'py, PyAny>>,
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
            extensions,
        )
    }

    /// Send a DELETE request.
    #[pyo3(signature = (url, *, headers=None, params=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, extensions=None))]
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
        extensions: Option<&Bound<'py, PyAny>>,
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
            extensions,
        )
    }

    /// Send a HEAD request.
    #[pyo3(signature = (url, *, headers=None, params=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, extensions=None))]
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
        extensions: Option<&Bound<'py, PyAny>>,
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
            extensions,
        )
    }

    /// Send an OPTIONS request.
    #[pyo3(signature = (url, *, headers=None, params=None, timeout=None, cookies=None, auth=None, follow_redirects=None, max_redirects=None, decompress=None, proxy=None, verify=None, cert=None, retries=None, extensions=None))]
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
        extensions: Option<&Bound<'py, PyAny>>,
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
            extensions,
        )
    }

    /// Send a streaming HTTP request.
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
    ) -> PyResult<Bound<'py, PyStreamingResponse>> {
        self.ensure_not_closed()?;

        if verify.is_some() || cert.is_some() {
            return Err(PyErr::new::<crate::errors::UnsupportedKwarg, _>(
                "verify and cert are client-level only; set them on the Client() constructor",
            ));
        }

        let crate::request_preparation::PreparedRequest {
            method: http_method,
            url: target_url,
            headers: rust_headers,
            body: request_body,
            timeout: rust_timeout,
            auth: auth_override,
            proxy: proxy_override,
            proxy_headers,
            proxy_tls_config,
            retry: retry_override,
            extensions: extracted,
            follow_redirects: prepared_follow_redirects,
            max_redirects: prepared_max_redirects,
        } = crate::request_preparation::prepare_request(
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
            extensions,
            false,
        )?;
        let transport_hints = extracted.hints;
        let trace_slot = extracted.trace_error_slot.clone();

        let client = self.clone_client()?;
        let (runtime_guard, runtime_handle) = self.runtime_for_dispatch()?;
        let effective_decompress = decompress.or(self.decompress);
        let result = py.detach(|| {
            runtime_handle.block_on(async {
                let mut builder = client
                    .request(http_method, target_url.as_str())
                    .map_err(map_err)?;

                builder = builder.headers(rust_headers);

                if let Some(body) = request_body {
                    builder = builder.body(body);
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
                        let mut p = eggfetch_core::Proxy::all_compat(
                            &proxy::normalize_compat_proxy_url(&url),
                        )
                        .map_err(map_err)?;
                        if let Some(ref hdrs) = proxy_headers {
                            p = p.proxy_headers(hdrs.clone());
                        }
                        if let Some(ref tls) = proxy_tls_config {
                            p = p.with_proxy_tls_config(tls.clone());
                        }
                        builder = builder.proxy(&p);
                    }
                }

                if prepared_follow_redirects.is_some() || prepared_max_redirects.is_some() {
                    let mut redirect = eggfetch_core::redirect::RedirectPolicy::default();
                    if let Some(f) = prepared_follow_redirects {
                        redirect.follow = f;
                    }
                    if let Some(m) = prepared_max_redirects {
                        redirect.max_redirects = m;
                    }
                    builder = builder.redirect_policy(redirect);
                }

                if let Some(retry_policy) = retry_override.as_ref() {
                    builder = builder.retry(retry_policy.clone());
                }

                // Apply pre-extracted transport hints (no-op when none were
                // supplied via `extensions=`).
                builder = builder.transport_hints(transport_hints.clone());

                let response = Box::pin(builder.send()).await.map_err(map_err)?;
                Ok::<_, PyErr>(response)
            })
        });

        // Surface any trace-callback errors recorded during dispatch.
        if let Some(slot) = trace_slot {
            if let Some(err) = take_callback_error(&slot) {
                return Err(err);
            }
        }

        let response = result?;
        PyStreamingResponse::from_core_response(
            py,
            response,
            runtime_handle,
            Some(crate::streaming::RuntimeLease::new(runtime_guard)),
            false,
        )
    }

    /// Close the client and release all resources.
    ///
    /// Drops the underlying `eggfetch-core` client (closing idle connections)
    /// and shuts down the tokio runtime. Subsequent requests raise
    /// `ValueError`. Idempotent.
    fn close(&self) {
        let mut guard = self
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if guard.is_some() {
            *guard = None;
            drop(guard);
            if let Some(runtime) = self
                .runtime
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                runtime.request_shutdown();
            }
        }
    }

    /// Returns True if the client has been closed.
    #[getter]
    fn is_closed(&self) -> bool {
        self.client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none()
    }

    /// The client's cookie jar.
    #[getter]
    fn cookies(&self) -> PyResult<PyCookies> {
        let client = self.clone_client()?;
        Ok(PyCookies::from_jar(client.cookies().clone()))
    }

    /// Context manager: enter.
    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    /// Context manager: exit.
    #[pyo3(signature = (_exc_type=None, _exc_value=None, _traceback=None))]
    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> bool {
        self.close();
        false
    }

    fn __repr__(&self) -> String {
        if self
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none()
        {
            "Client(closed=true)".to_string()
        } else if self.verify_disabled {
            "Client(verify=False) [UNSAFE: TLS verification disabled]".to_string()
        } else {
            "Client()".to_string()
        }
    }
}

impl PyClient {
    fn ensure_not_closed(&self) -> PyResult<()> {
        let guard = self
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if guard.is_none() {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "client is closed",
            ));
        }
        Ok(())
    }

    fn clone_client(&self) -> PyResult<eggfetch_core::Client> {
        let guard = self
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("client is closed"))
    }

    /// Clone the shared tokio runtime and its handle for dispatch.
    ///
    /// This must be the *only* runtime fetch on the request path: it
    /// happens once, before `detach`, so a concurrent `close()`
    /// taking the runtime afterwards cannot race a second fetch. A
    /// closed client raises `ValueError` instead of panicking.
    fn runtime_for_dispatch(&self) -> PyResult<(RuntimeGuard, tokio::runtime::Handle)> {
        let guard = self.runtime.lock().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("client runtime lock poisoned")
        })?;
        let state = guard
            .as_ref()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("client is closed"))?;
        let state = state.clone();
        let handle = state.handle().ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("client runtime is shut down")
        })?;
        Ok((RuntimeGuard(state), handle))
    }
}
