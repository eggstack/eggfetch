//! Python response wrapper with buffered data.

use bytes::Bytes;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyString};

use crate::cookies::PyCookies;
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
        if let Some((name, charset)) = part.split_once('=') {
            if name.trim().eq_ignore_ascii_case("charset") {
                let charset = charset.trim().trim_matches(['"', '\'']);
                return Some(charset.to_string());
            }
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
    /// Redirect history (populated when `follow_redirects` is enabled).
    #[pyo3(get)]
    history: Vec<PyResponse>,
    /// Cookies set by the server via Set-Cookie headers.
    #[pyo3(get)]
    cookies: PyCookies,
    /// Whether the stream has been consumed (always `false` for buffered
    /// responses).
    #[pyo3(get)]
    _stream_consumed: bool,
    /// Original wire `Content-Encoding`, for the HTTPX compatibility facade.
    #[pyo3(get)]
    _wire_content_encoding: Option<String>,
    /// Original wire `Content-Length`, for the HTTPX compatibility facade.
    #[pyo3(get)]
    _wire_content_length: Option<String>,
}

impl PyResponse {
    /// Create a `PyResponse` from a core `Response`, buffering all data.
    ///
    /// Uses a short-lived tokio runtime to buffer the body. Not safe to
    /// call from within an existing async context — use
    /// [`from_core_response_with_body`] instead.
    pub fn from_core_response(mut response: eggfetch_core::Response) -> PyResult<Self> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "cannot synchronously buffer a response inside a Tokio runtime; use the async API",
            ));
        }
        let content = {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
            rt.block_on(response.bytes())
                .map_err(crate::errors::map_err)?
        };
        Self::from_core_response_with_body(response, content)
    }

    /// Create a `PyResponse` from a core `Response` with pre-buffered body
    /// bytes.
    ///
    /// This is safe to call from async contexts because it does not spawn
    /// a runtime. Redirect history is converted properly.
    #[allow(clippy::unnecessary_wraps)]
    pub fn from_core_response_with_body(
        mut response: eggfetch_core::Response,
        content: Bytes,
    ) -> PyResult<Self> {
        let status = response.status().as_u16();
        let headers = PyHeaders::from_header_map(response.headers().clone());
        let wire_content_encoding = response.wire_content_encoding().map(ToOwned::to_owned);
        let wire_content_length = response.wire_content_length().map(ToOwned::to_owned);
        let reason_phrase = response
            .status()
            .canonical_reason()
            .unwrap_or("")
            .to_string();
        let http_version = version_to_string(response.version());
        let encoding = extract_charset(response.headers());

        // Convert redirect history (metadata-only snapshots, no body).
        let core_history = std::mem::take(response.history_mut());
        let history: Vec<PyResponse> = core_history
            .into_iter()
            .map(|entry| {
                let status = entry.status().as_u16();
                let headers = PyHeaders::from_header_map(entry.headers().clone());
                let url = entry.url().to_string();
                let reason_phrase = entry.reason_phrase().to_string();
                let http_version = version_to_string(entry.version());
                let encoding = extract_charset(entry.headers());
                PyResponse::from_parts(
                    status,
                    headers,
                    url,
                    Bytes::new(),
                    reason_phrase,
                    http_version,
                    encoding,
                )
            })
            .collect();

        let text = decode_with_encoding(&content, encoding.as_deref());

        // Parse Set-Cookie headers into a Cookies mapping.
        let jar = eggfetch_core::cookie::CookieJar::new();
        let response_url = response.url();
        let set_cookie_headers: Vec<String> = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok().map(ToString::to_string))
            .collect();
        if !set_cookie_headers.is_empty() {
            jar.update_from_response(response_url, &set_cookie_headers);
        }
        let cookies = PyCookies::from_jar(jar);

        Ok(Self {
            status_code: status,
            headers,
            url: response_url.to_string(),
            content,
            text,
            reason_phrase,
            http_version,
            encoding,
            history,
            cookies,
            _stream_consumed: false,
            _wire_content_encoding: wire_content_encoding,
            _wire_content_length: wire_content_length,
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
            cookies: PyCookies::from_jar(eggfetch_core::cookie::CookieJar::new()),
            _stream_consumed: false,
            _wire_content_encoding: None,
            _wire_content_length: None,
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
        if chunk_size == 0 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "chunk_size must be greater than zero",
            ));
        }
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
        if chunk_size == 0 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "chunk_size must be greater than zero",
            ));
        }
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
