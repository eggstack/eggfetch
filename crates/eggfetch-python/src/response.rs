//! Python response wrapper with buffered data.

use bytes::Bytes;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyString};

use crate::errors::HTTPStatusError;
use crate::headers::PyHeaders;

/// Map `http::Version` to a human-readable string.
pub(crate) fn version_to_string(version: http::Version) -> String {
    match version {
        http::Version::HTTP_10 => "HTTP/1.0".to_string(),
        http::Version::HTTP_11 => "HTTP/1.1".to_string(),
        http::Version::HTTP_2 => "HTTP/2".to_string(),
        http::Version::HTTP_3 => "HTTP/3".to_string(),
        other => format!("{other:?}"),
    }
}

/// Extract the `charset` parameter from a `Content-Type` header value.
pub(crate) fn extract_charset(headers: &http::HeaderMap) -> Option<String> {
    let content_type = headers.get("content-type")?;
    let ct_str = content_type.to_str().ok()?;
    for part in ct_str.split(';').skip(1) {
        let part = part.trim();
        if let Some(charset) = part.strip_prefix("charset=") {
            let charset = charset.trim().trim_matches('"');
            return Some(charset.to_string());
        }
    }
    None
}

/// Decode bytes to a `String` using the given encoding, falling back to UTF-8.
fn decode_with_encoding(content: &[u8], encoding: Option<&str>) -> String {
    if let Some(enc_name) = encoding {
        if let Some(enc) = encoding_rs::Encoding::for_label(enc_name.as_bytes()) {
            let (decoded, _, _) = enc.decode(content);
            return decoded.into_owned();
        }
    }
    String::from_utf8_lossy(content).to_string()
}

/// A buffered HTTP response exposed to Python.
///
/// All data is buffered at creation time so Python code can access it
/// synchronously.
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
    /// HTTP reason phrase (e.g. "OK", "Not Found").
    #[pyo3(get)]
    reason_phrase: String,
    /// HTTP version string (e.g. "HTTP/1.1", "HTTP/2").
    #[pyo3(get)]
    http_version: String,
    /// Character encoding detected from the Content-Type header, if any.
    #[pyo3(get)]
    encoding: Option<String>,
    /// Redirect history (empty; reserved for future redirect support).
    #[pyo3(get)]
    history: Vec<PyResponse>,
    /// Whether the stream has been consumed (always `false` for buffered
    /// responses).
    #[pyo3(get)]
    _stream_consumed: bool,
}

impl PyResponse {
    /// Create a `PyResponse` from a core `Response`, buffering all data.
    pub fn from_core_response(mut response: eggfetch_core::Response) -> PyResult<Self> {
        let status = response.status().as_u16();
        let headers = PyHeaders::from_header_map(response.headers().clone());
        let url = response.url().to_string();
        let reason_phrase = response
            .status()
            .canonical_reason()
            .unwrap_or("")
            .to_string();
        let http_version = version_to_string(response.version());
        let encoding = extract_charset(response.headers());

        // Buffer body bytes synchronously using a short-lived tokio runtime.
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        let content = rt
            .block_on(response.bytes())
            .map_err(crate::errors::map_err)?;

        let text = decode_with_encoding(&content, encoding.as_deref());

        Ok(Self {
            status_code: status,
            headers,
            url,
            content,
            text,
            reason_phrase,
            http_version,
            encoding,
            history: Vec::new(),
            _stream_consumed: false,
        })
    }

    /// Create a `PyResponse` from pre-buffered parts without spawning a
    /// runtime.
    pub fn from_parts(
        status: u16,
        headers: PyHeaders,
        url: String,
        content: Bytes,
        reason_phrase: String,
        http_version: String,
        encoding: Option<String>,
    ) -> Self {
        let text = decode_with_encoding(&content, encoding.as_deref());
        Self {
            status_code: status,
            headers,
            url,
            content,
            text,
            reason_phrase,
            http_version,
            encoding,
            history: Vec::new(),
            _stream_consumed: false,
        }
    }
}

#[pymethods]
impl PyResponse {
    /// Raise an exception if the status code indicates an error (4xx/5xx).
    fn raise_for_status(&self) -> PyResult<()> {
        if self.status_code >= 400 {
            return Err(HTTPStatusError::new_err(format!(
                "{} {} for url '{}'",
                self.status_code, self.reason_phrase, self.url
            )));
        }
        Ok(())
    }

    /// Returns `True` if the status code is 1xx (informational).
    #[getter]
    fn is_informational(&self) -> bool {
        (100..200).contains(&self.status_code)
    }

    /// Returns `True` if the status code is 2xx (success).
    #[getter]
    fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }

    /// Returns `True` if the status code is 3xx (redirect).
    #[getter]
    fn is_redirect(&self) -> bool {
        (300..400).contains(&self.status_code)
    }

    /// Returns `True` if the status code is 4xx (client error).
    #[getter]
    fn is_client_error(&self) -> bool {
        (400..500).contains(&self.status_code)
    }

    /// Returns `True` if the status code is 5xx (server error).
    #[getter]
    fn is_server_error(&self) -> bool {
        (500..600).contains(&self.status_code)
    }

    /// Returns `True` if the status code indicates an error (4xx or 5xx).
    #[getter]
    fn is_error(&self) -> bool {
        (400..600).contains(&self.status_code)
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

    /// Parse the response body as JSON.
    #[pyo3(signature = (**kwargs))]
    fn json(&self, py: Python<'_>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<PyObject> {
        let json_module = py.import("json")?;
        let text_obj = PyString::new(py, &self.text);
        let loads = json_module.getattr("loads")?;
        match kwargs {
            Some(kw) => loads.call((text_obj,), Some(kw)).map(Into::into),
            None => loads.call1((text_obj,)).map(Into::into),
        }
    }

    /// Iterate over response body in byte chunks.
    #[pyo3(signature = (chunk_size=8192))]
    fn iter_bytes(&self, py: Python<'_>, chunk_size: usize) -> PyResult<PyObject> {
        let chunks: Vec<PyObject> = self
            .content
            .chunks(chunk_size)
            .map(|c| Ok(PyBytes::new(py, c).into()))
            .collect::<PyResult<Vec<_>>>()?;
        let list = PyList::new(py, chunks)?;
        py.import("builtins")?
            .getattr("iter")?
            .call1((list,))
            .map(Into::into)
    }

    /// Iterate over response body in text chunks.
    #[pyo3(signature = (chunk_size=8192))]
    fn iter_text(&self, py: Python<'_>, chunk_size: usize) -> PyResult<PyObject> {
        let chars: Vec<char> = self.text.chars().collect();
        let chunks: Vec<PyObject> = chars
            .chunks(chunk_size)
            .map(|c| {
                let s: String = c.iter().collect();
                Ok(PyString::new(py, &s).into())
            })
            .collect::<PyResult<Vec<_>>>()?;
        let list = PyList::new(py, chunks)?;
        py.import("builtins")?
            .getattr("iter")?
            .call1((list,))
            .map(Into::into)
    }

    /// Iterate over response body lines.
    fn iter_lines(&self, py: Python<'_>) -> PyResult<PyObject> {
        let lines: Vec<PyObject> = self
            .text
            .lines()
            .map(|l| Ok(PyString::new(py, l).into()))
            .collect::<PyResult<Vec<_>>>()?;
        let list = PyList::new(py, lines)?;
        py.import("builtins")?
            .getattr("iter")?
            .call1((list,))
            .map(Into::into)
    }

    /// Close the response (no-op for buffered responses).
    #[allow(clippy::unused_self)] // Intentional no-op: Python instance method for API compatibility.
    fn close(&self) {}

    /// Async close (no-op for buffered responses).
    #[allow(clippy::unused_self)] // Intentional no-op: Python async instance method for API compatibility.
    fn aclose(&self) {}

    fn __repr__(&self) -> String {
        format!("<Response [{} {}]>", self.status_code, self.reason_phrase)
    }
}
