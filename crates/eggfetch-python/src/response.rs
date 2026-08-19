//! Python response wrapper with buffered data.

use bytes::Bytes;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyString};

use crate::cookies::PyCookies;
use crate::errors::HTTPStatusError;
use crate::headers::PyHeaders;
use crate::network_stream::PyNetworkStream;
use crate::streaming::RuntimeLease;

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
    /// Optional network stream handle for connection metadata and
    /// upgraded-connection IO. Returns `None` for buffered responses
    /// where the connection has been returned to the pool.
    #[pyo3(get)]
    _network_stream: Option<PyNetworkStream>,
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
        // The short-lived runtime is dropped here; any 101 upgrade
        // extracted from this path falls back to the ambient handle
        // (None at this point) and will not be usable for IO.
        Self::from_core_response_with_body(&mut response, content, None, None)
    }

    /// Create a `PyResponse` from a core `Response` with pre-buffered body
    /// bytes.
    ///
    /// This is safe to call from async contexts because it does not spawn
    /// a runtime. Redirect history is converted properly.
    ///
    /// `runtime_handle` and `runtime_lease` are propagated into the
    /// extracted network stream so the 101 upgrade wrapper can outlive
    /// the buffered response itself when the underlying client is closed.
    /// Pass `None` for both when the caller is not backed by a persistent
    /// runtime (e.g. internals that do not need to drive IO).
    #[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]
    pub(crate) fn from_core_response_with_body(
        response: &mut eggfetch_core::Response,
        content: Bytes,
        runtime_handle: Option<&tokio::runtime::Handle>,
        runtime_lease: Option<&RuntimeLease>,
    ) -> PyResult<Self> {
        let status = response.status().as_u16();
        let headers = PyHeaders::from_header_map(response.headers().clone());
        let wire_content_encoding = response.wire_content_encoding().map(ToOwned::to_owned);
        let wire_content_length = response.wire_content_length().map(ToOwned::to_owned);
        // Prefer the wire reason phrase as captured from the server.
        // Falls back to the canonical reason phrase from `http::StatusCode`
        // only when the wire reason was missing (e.g. HTTP/2 / HTTP/3).
        let reason_phrase = response
            .wire_reason_phrase()
            .map(ToOwned::to_owned)
            .or_else(|| response.status().canonical_reason().map(ToOwned::to_owned))
            .unwrap_or_default();
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
        let response_url = response.url().to_string();
        let set_cookie_headers: Vec<String> = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok().map(ToString::to_string))
            .collect();
        if !set_cookie_headers.is_empty() {
            jar.update_from_response(response.url(), &set_cookie_headers);
        }
        let cookies = PyCookies::from_jar(jar);

        // Extract network stream from core response if present.
        // For buffered responses, the connection has been returned to the
        // pool, so the network_stream is only meaningful for upgrade
        // responses or streaming responses.
        //
        // A buffered path that did not provide a runtime handle cannot
        // surface a working upgrade wrapper: the underlying connection is
        // already returned to the pool and the IR generator would have
        // produced a `Metadata` variant. The `Unreachable` arm documents
        // the invariant.
        let network_stream = response.take_network_stream().map(|ns| match ns {
            eggfetch_core::network_stream::NetworkStream::Upgraded(u) => {
                let handle = runtime_handle.cloned().unwrap_or_else(|| {
                    // Fall back to the ambient handle when the caller
                    // did not supply one. This is best-effort: the
                    // caller must arrange a runtime if IO is needed.
                    tokio::runtime::Handle::current()
                });
                PyNetworkStream::from_upgraded_with_handle(u, handle, runtime_lease.cloned())
            }
            eggfetch_core::network_stream::NetworkStream::Metadata(m) => {
                PyNetworkStream::from_metadata(m)
            }
        });

        Ok(Self {
            status_code: status,
            headers,
            url: response_url,
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
            _network_stream: network_stream,
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
            _network_stream: None,
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

    /// Snapshot wire-level response metadata (http version, reason
    /// phrase, network stream) into a dict compatible with HTTPX's
    /// `response.extensions`.
    ///
    /// For 101 Switching Protocols responses, the owned upgraded stream
    /// is exposed through `extensions["network_stream"]`. For ordinary
    /// buffered responses where the connection has been returned to the
    /// pool, the field is `None` — Hyper does not expose per-response
    /// socket metadata without endangering pool safety.
    #[getter]
    fn extensions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("http_version", self.http_version.clone())?;
        dict.set_item("reason_phrase", self.reason_phrase.clone())?;
        #[allow(clippy::used_underscore_binding)]
        match self._network_stream.as_ref() {
            Some(stream) if stream.is_upgraded() => {
                dict.set_item("network_stream", stream.clone())?;
            }
            _ => {
                dict.set_item("network_stream", py.None())?;
            }
        }
        Ok(dict)
    }

    fn __repr__(&self) -> String {
        format!("<Response [{} {}]>", self.status_code, self.reason_phrase)
    }
}

/// Snapshot the wire-level metadata of a [`eggfetch_core::Response`] into a
/// Python dict compatible with HTTPX's `response.extensions`.
///
/// Keys mirror HTTPX's vocabulary:
/// - `http_version`: e.g. `"HTTP/1.1"`, `"HTTP/2"`, `"HTTP/3"`.
/// - `reason_phrase`: wire reason phrase if present (HTTP/1.x), else
///   canonical reason phrase derived from the status code.
/// - `network_stream`: a [`PyNetworkStream`] wrapper for 101 upgrades,
///   or `None` for ordinary pooled responses where the connection has
///   been returned to the pool.
#[allow(dead_code)] // Wired into the HTTPX compatibility facade.
pub(crate) fn response_extensions_from_core<'py>(
    py: Python<'py>,
    response: &eggfetch_core::Response,
    network_stream: Option<&PyNetworkStream>,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("http_version", version_to_string(response.version()))?;
    let reason = response
        .wire_reason_phrase()
        .map(ToOwned::to_owned)
        .or_else(|| response.status().canonical_reason().map(ToOwned::to_owned))
        .unwrap_or_default();
    dict.set_item("reason_phrase", reason)?;
    match network_stream {
        Some(stream) if stream.is_upgraded() => {
            dict.set_item("network_stream", stream.clone())?;
        }
        _ => {
            dict.set_item("network_stream", py.None())?;
        }
    }
    Ok(dict)
}
