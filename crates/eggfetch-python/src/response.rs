//! Python response wrapper with buffered data.

use bytes::Bytes;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::errors::HTTPStatusError;
use crate::headers::PyHeaders;

/// A buffered HTTP response exposed to Python.
///
/// All data is buffered at creation time so Python code can access it synchronously.
#[pyclass(name = "Response")]
#[derive(Debug, Clone)]
pub struct PyResponse {
    /// HTTP status code.
    #[pyo3(get)]
    status_code: u16,
    /// Response headers.
    #[pyo3(get)]
    headers: PyHeaders,
    /// Final URL after any redirects.
    #[pyo3(get)]
    url: String,
    /// Raw response body bytes.
    content: Bytes,
    /// Decoded text of the response body.
    text: String,
}

impl PyResponse {
    /// Create a `PyResponse` from a core `Response`, buffering all data.
    pub fn from_core_response(mut response: eggfetch_core::Response) -> PyResult<Self> {
        let status = response.status().as_u16();
        let headers = PyHeaders::from_header_map(response.headers().clone());
        let url = response.url().to_string();

        // Buffer body bytes synchronously using a short-lived tokio runtime.
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        let content = rt
            .block_on(response.bytes())
            .map_err(crate::errors::map_err)?;

        // Decode text from bytes.
        let text = String::from_utf8_lossy(&content).to_string();

        Ok(Self {
            status_code: status,
            headers,
            url,
            content,
            text,
        })
    }

    /// Create a `PyResponse` from pre-buffered parts without spawning a runtime.
    pub fn from_parts(status: u16, headers: PyHeaders, url: String, content: Bytes) -> Self {
        let text = String::from_utf8_lossy(&content).to_string();
        Self {
            status_code: status,
            headers,
            url,
            content,
            text,
        }
    }
}

#[pymethods]
impl PyResponse {
    /// Raise an exception if the status code indicates an error (4xx/5xx).
    fn raise_for_status(&self) -> PyResult<()> {
        if self.status_code >= 400 {
            return Err(HTTPStatusError::new_err(format!(
                "HTTP {} for url: {}",
                self.status_code, self.url
            )));
        }
        Ok(())
    }

    /// Returns True if the status code is 2xx.
    #[getter]
    fn is_success(&self) -> bool {
        self.status_code >= 200 && self.status_code < 300
    }

    /// Returns the raw response body bytes.
    #[getter]
    fn content<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.content)
    }

    /// Returns the decoded text of the response body.
    #[getter]
    fn text(&self) -> &str {
        &self.text
    }

    fn __repr__(&self) -> String {
        format!("Response(status={}, url='{}')", self.status_code, self.url)
    }
}
