//! Python async client wrapper over the `eggfetch-core` engine.

use pyo3::prelude::*;

use crate::cookies::PyCookies;
use crate::errors::map_err;
use crate::streaming::PyStreamingResponse;
use crate::trace_bridge::take_callback_error;

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
    client: std::sync::Mutex<Option<eggfetch_core::Client>>,
    decompress: Option<bool>,
    verify_disabled: bool,
    redirect_policy: eggfetch_core::redirect::RedirectPolicy,
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
        let verify_disabled = prepared.verify_disabled;
        let decompress = prepared.decompress;
        let redirect_policy = prepared.redirect_policy.clone();
        let client = crate::request_preparation::apply_client_config(prepared)?;

        Ok(Self {
            client: std::sync::Mutex::new(Some(client)),
            decompress,
            verify_disabled,
            redirect_policy,
        })
    }

    /// Send an HTTP request asynchronously.
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
                "verify and cert are client-level only; set them on the AsyncClient() constructor",
            ));
        }

        let prepared = crate::request_preparation::prepare_request(
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
            true,
        )?;

        let client = self.ensure_client()?;
        let dispatch = crate::request_preparation::prepare_core_dispatch(
            &client,
            prepared,
            crate::request_preparation::RequestDispatchDefaults {
                redirect_policy: self.redirect_policy.clone(),
                decompress: decompress.or(self.decompress),
            },
        )?;
        let trace_slot = dispatch.trace_error_slot;
        let builder = dispatch.builder;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let response_result = Box::pin(builder.send()).await;

            // Surface any trace-callback errors recorded during dispatch.
            // Check the slot regardless of whether the transport succeeded
            // or failed so that callback exceptions are never swallowed.
            if let Some(slot) = trace_slot {
                if let Some(err) = take_callback_error(&slot) {
                    return Err(err);
                }
            }

            let mut response = response_result.map_err(map_err)?;
            let content = response.bytes().await.map_err(map_err)?;
            // The async client backs the runtime through the ambient
            // handle; the network stream is constructed with no explicit
            // lease because the AsyncClient owns its runtime.
            let runtime_handle = tokio::runtime::Handle::current();
            crate::response::PyResponse::from_core_response_with_body(
                &mut response,
                content,
                Some(&runtime_handle),
                None,
                true,
            )
        })
    }

    /// Send a GET request asynchronously.
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

    /// Send a POST request asynchronously.
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

    /// Send a PUT request asynchronously.
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

    /// Send a PATCH request asynchronously.
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

    /// Send a DELETE request asynchronously.
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

    /// Send a HEAD request asynchronously.
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

    /// Send an OPTIONS request asynchronously.
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

        let prepared = crate::request_preparation::prepare_request(
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
            true,
        )?;

        let client = self.ensure_client()?;
        let dispatch = crate::request_preparation::prepare_core_dispatch(
            &client,
            prepared,
            crate::request_preparation::RequestDispatchDefaults {
                redirect_policy: self.redirect_policy.clone(),
                decompress: decompress.or(self.decompress),
            },
        )?;
        let trace_slot = dispatch.trace_error_slot;
        let builder = dispatch.builder;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let runtime_handle = tokio::runtime::Handle::current();
            let response_result = Box::pin(builder.send()).await;

            // Surface any trace-callback errors recorded during dispatch.
            // Check the slot regardless of whether the transport succeeded
            // or failed so that callback exceptions are never swallowed.
            if let Some(slot) = trace_slot {
                if let Some(err) = take_callback_error(&slot) {
                    return Err(err);
                }
            }

            let response = response_result.map_err(map_err)?;

            let obj: Py<PyAny> = Python::attach(|py| {
                PyStreamingResponse::from_core_response(py, response, runtime_handle, None, true)
                    .map(|r| r.unbind().into_any())
            })?;
            Ok(obj)
        })
    }

    /// Close the client and release all resources.
    ///
    /// Drops the underlying `eggfetch-core` client (closing idle connections).
    /// Subsequent requests raise `ValueError`. Idempotent.
    fn close(&self) {
        let mut guard = self
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = None;
    }

    /// Close the client through an awaitable API.
    fn aclose<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::PyAny>> {
        self.close();
        pyo3_async_runtimes::tokio::future_into_py(py, async { Ok(()) })
    }

    /// Returns True if the client has been closed.
    #[getter]
    fn is_closed(&self) -> bool {
        self.client.lock().map_or(true, |g| g.is_none())
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
        &self,
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
        let is_closed = self.client.lock().map_or(true, |g| g.is_none());
        if is_closed {
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
        let guard = self.client.lock().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("client lock poisoned")
        })?;
        if guard.is_none() {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "client is closed",
            ));
        }
        Ok(())
    }

    fn ensure_client(&self) -> PyResult<eggfetch_core::Client> {
        let guard = self.client.lock().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("client lock poisoned")
        })?;
        guard
            .clone()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("client is closed"))
    }
}
