//! Python async client wrapper over the eggfetch-core engine.

use pyo3::prelude::*;

use crate::conversion::{
    build_request_body, parse_timeout, python_headers_to_rust, python_params_to_url,
    validate_body_kwargs,
};
use crate::cookies::PyCookies;
use crate::errors::map_err;

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
    client: eggfetch_core::Client,
    closed: bool,
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
    #[new]
    #[pyo3(signature = (*, headers=None, timeout=None, follow_redirects=None, max_redirects=None, cookies=None))]
    fn new(
        py: Python<'_>,
        headers: Option<&Bound<'_, PyAny>>,
        timeout: Option<&Bound<'_, PyAny>>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
        cookies: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let mut builder = eggfetch_core::Client::builder();

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

        let client = builder.build();

        Ok(Self {
            client,
            closed: false,
        })
    }

    /// Send an HTTP request asynchronously.
    #[pyo3(signature = (method, url, *, headers=None, params=None, content=None, data=None, json=None, timeout=None, follow_redirects=None, max_redirects=None))]
    #[allow(clippy::too_many_arguments)]
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
        timeout: Option<&Bound<'py, PyAny>>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.ensure_not_closed()?;

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

        validate_body_kwargs(content, data, json)?;

        let mut rust_headers = if let Some(h) = headers {
            python_headers_to_rust(py, h)?
        } else {
            eggfetch_core::Headers::new()
        };

        let (body_bytes, auto_content_type) = build_request_body(py, content, data, json)?;

        if let Some(ct) = auto_content_type {
            if !rust_headers.contains("content-type") {
                rust_headers.insert("content-type", ct).map_err(map_err)?;
            }
        }

        let rust_timeout = parse_timeout(timeout)?;

        let client = self.client.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
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

            let mut response = builder.send().await.map_err(map_err)?;
            let content = response.bytes().await.map_err(map_err)?;
            crate::response::PyResponse::from_core_response_with_body(response, content)
        })
    }

    /// Send a GET request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, timeout=None, follow_redirects=None, max_redirects=None))]
    #[allow(clippy::too_many_arguments)]
    fn get<'py>(
        &self,
        py: Python<'py>,
        url: &str,
        headers: Option<&Bound<'py, PyAny>>,
        params: Option<&Bound<'py, PyAny>>,
        timeout: Option<&Bound<'py, PyAny>>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
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
            timeout,
            follow_redirects,
            max_redirects,
        )
    }

    /// Send a POST request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, content=None, data=None, json=None, timeout=None, follow_redirects=None, max_redirects=None))]
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
        timeout: Option<&Bound<'py, PyAny>>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
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
            timeout,
            follow_redirects,
            max_redirects,
        )
    }

    /// Send a PUT request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, content=None, data=None, json=None, timeout=None, follow_redirects=None, max_redirects=None))]
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
        timeout: Option<&Bound<'py, PyAny>>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
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
            timeout,
            follow_redirects,
            max_redirects,
        )
    }

    /// Send a PATCH request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, content=None, data=None, json=None, timeout=None, follow_redirects=None, max_redirects=None))]
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
        timeout: Option<&Bound<'py, PyAny>>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
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
            timeout,
            follow_redirects,
            max_redirects,
        )
    }

    /// Send a DELETE request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, timeout=None, follow_redirects=None, max_redirects=None))]
    #[allow(clippy::too_many_arguments)]
    fn delete<'py>(
        &self,
        py: Python<'py>,
        url: &str,
        headers: Option<&Bound<'py, PyAny>>,
        params: Option<&Bound<'py, PyAny>>,
        timeout: Option<&Bound<'py, PyAny>>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
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
            timeout,
            follow_redirects,
            max_redirects,
        )
    }

    /// Send a HEAD request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, timeout=None, follow_redirects=None, max_redirects=None))]
    #[allow(clippy::too_many_arguments)]
    fn head<'py>(
        &self,
        py: Python<'py>,
        url: &str,
        headers: Option<&Bound<'py, PyAny>>,
        params: Option<&Bound<'py, PyAny>>,
        timeout: Option<&Bound<'py, PyAny>>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
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
            timeout,
            follow_redirects,
            max_redirects,
        )
    }

    /// Send an OPTIONS request asynchronously.
    #[pyo3(signature = (url, *, headers=None, params=None, timeout=None, follow_redirects=None, max_redirects=None))]
    #[allow(clippy::too_many_arguments)]
    fn options<'py>(
        &self,
        py: Python<'py>,
        url: &str,
        headers: Option<&Bound<'py, PyAny>>,
        params: Option<&Bound<'py, PyAny>>,
        timeout: Option<&Bound<'py, PyAny>>,
        follow_redirects: Option<bool>,
        max_redirects: Option<usize>,
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
            timeout,
            follow_redirects,
            max_redirects,
        )
    }

    /// Close the client. Idempotent.
    fn close(&mut self) {
        if !self.closed {
            self.closed = true;
        }
    }

    /// Returns True if the client has been closed.
    #[getter]
    fn is_closed(&self) -> bool {
        self.closed
    }

    /// The client's cookie jar.
    #[getter]
    fn cookies(&self) -> PyCookies {
        PyCookies::from_jar(self.client.cookies().clone())
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
        if self.closed {
            "AsyncClient(closed=true)".to_string()
        } else {
            "AsyncClient()".to_string()
        }
    }
}

impl PyAsyncClient {
    fn ensure_not_closed(&self) -> PyResult<()> {
        if self.closed {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "client is closed",
            ));
        }
        Ok(())
    }
}
