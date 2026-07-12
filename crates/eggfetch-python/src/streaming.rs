//! True streaming response types for the Python bindings.
//!
//! Iterator constructors accept `encoding_override` by value because the
//! Python binding layer already owns the string and passing it through is
//! the natural pyo3 convention. Explicit `'py` lifetimes are kept on pyo3
//! methods for clarity even when elidable. Channel types are wrapped in
//! `Mutex` to satisfy pyclass `Sync` requirements.

// Clippy pedantic lints suppressed for pyo3 binding patterns:
// - `needless_pass_by_value`: iterator constructors take `Option<String>` by value
//   because callers already own the value and passing by ref would be awkward.
// - `elidable_lifetime_names`: explicit `'py` lifetimes kept for pyo3 API clarity.
// - `module_name_repetitions`: types are prefixed for disambiguation in __all__.
// - `complex_type`: `tokio::sync::mpsc::Receiver` wrapped in Arc<Mutex<...>> is
//   the minimal type for async pyclass fields; factoring would obscure intent.
#![allow(
    clippy::needless_pass_by_value,
    clippy::elidable_lifetime_names,
    clippy::module_name_repetitions
)]

/// Type alias for the async byte channel used by async iterators.
type AsyncByteRx = Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Result<Vec<u8>, PyErr>>>>;
/// Type alias for the async text channel used by async iterators.
type AsyncTextRx = Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Result<String, PyErr>>>>;

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyString};

use crate::cookies::PyCookies;
use crate::errors::map_err;
use crate::headers::PyHeaders;
use crate::response::{extract_charset, version_to_string};

const STATE_STREAMING: u8 = 0;
const STATE_BUFFERED: u8 = 1;
const STATE_CONSUMED: u8 = 2;
const STATE_CLOSED: u8 = 3;

// ---------------------------------------------------------------------------
// PyStreamingResponse
// ---------------------------------------------------------------------------

/// A streaming HTTP response exposed to Python.
#[pyclass(name = "StreamingResponse")]
pub(crate) struct PyStreamingResponse {
    #[pyo3(get)]
    status_code: u16,
    #[pyo3(get)]
    headers: PyHeaders,
    #[pyo3(get)]
    url: String,
    #[pyo3(get)]
    history: Vec<crate::response::PyResponse>,
    #[pyo3(get)]
    cookies: PyCookies,
    body_state: AtomicU8,
    stream: std::sync::Mutex<Option<eggfetch_core::body::BoxBytesStream>>,
    encoding_name: Option<String>,
    cached_content: std::sync::Mutex<Option<Bytes>>,
    cached_text: std::sync::Mutex<Option<String>>,
}

impl PyStreamingResponse {
    pub fn from_core_response(
        py: Python<'_>,
        mut response: eggfetch_core::Response,
    ) -> PyResult<Bound<'_, Self>> {
        let status = response.status().as_u16();
        let headers = PyHeaders::from_header_map(response.headers().clone());
        let response_url = response.url().to_string();
        let encoding = extract_charset(response.headers());

        let core_history = std::mem::take(response.history_mut());
        let history: Vec<crate::response::PyResponse> = core_history
            .into_iter()
            .map(|entry| {
                let status = entry.status().as_u16();
                let headers = PyHeaders::from_header_map(entry.headers().clone());
                let url = entry.url().to_string();
                let reason_phrase = entry.reason_phrase().to_string();
                let http_version = version_to_string(entry.version());
                let enc = extract_charset(entry.headers());
                crate::response::PyResponse::from_parts(
                    status,
                    headers,
                    url,
                    Bytes::new(),
                    reason_phrase,
                    http_version,
                    enc,
                )
            })
            .collect();

        let jar = eggfetch_core::cookie::CookieJar::new();
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

        let stream = response.bytes_stream().map_err(map_err)?;

        Py::new(
            py,
            Self {
                status_code: status,
                headers,
                url: response_url,
                history,
                cookies,
                body_state: AtomicU8::new(STATE_STREAMING),
                stream: std::sync::Mutex::new(Some(stream)),
                encoding_name: encoding,
                cached_content: std::sync::Mutex::new(None),
                cached_text: std::sync::Mutex::new(None),
            },
        )
        .map(|inner| inner.into_bound(py))
    }

    fn take_stream(&self) -> PyResult<eggfetch_core::body::BoxBytesStream> {
        self.stream
            .lock()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?
            .take()
            .ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>("stream already consumed")
            })
    }

    fn drain_all_bytes(&self) -> PyResult<Bytes> {
        let mut stream = self.take_stream()?;
        match tokio::runtime::Handle::try_current() {
            Ok(_handle) => {
                // Inside a tokio runtime. Use block_on via a new thread to avoid deadlock.
                // Actually, we can't create a new runtime here. Use a blocking task approach.
                // Since we can't easily block_on in this context, we'll use a temporary runtime
                // on a separate thread.
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
                    let result = rt.block_on(async {
                        let mut buf = BytesMut::new();
                        while let Some(chunk) = stream.next().await {
                            let chunk = chunk.map_err(map_err)?;
                            buf.extend_from_slice(&chunk);
                        }
                        Ok::<_, PyErr>(buf.freeze())
                    });
                    let _ = tx.send(result);
                });
                let bytes = rx
                    .recv()
                    .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?
                    .map_err(|e| e)?;
                self.body_state.store(STATE_BUFFERED, Ordering::SeqCst);
                if let Ok(mut cache) = self.cached_content.lock() {
                    *cache = Some(bytes.clone());
                }
                Ok(bytes)
            }
            Err(_) => {
                // No tokio runtime, create one directly.
                let rt = tokio::runtime::Runtime::new().map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string())
                })?;
                let bytes = rt.block_on(async {
                    let mut buf = BytesMut::new();
                    while let Some(chunk) = stream.next().await {
                        let chunk = chunk.map_err(map_err)?;
                        buf.extend_from_slice(&chunk);
                    }
                    Ok::<_, PyErr>(buf.freeze())
                })?;
                self.body_state.store(STATE_BUFFERED, Ordering::SeqCst);
                if let Ok(mut cache) = self.cached_content.lock() {
                    *cache = Some(bytes.clone());
                }
                Ok(bytes)
            }
        }
    }

    fn drain_all_text(&self) -> PyResult<String> {
        if let Ok(cache) = self.cached_text.lock() {
            if let Some(ref text) = *cache {
                return Ok(text.clone());
            }
        }
        // If the content is already buffered, decode from it instead of re-reading the stream.
        if let Ok(cache) = self.cached_content.lock() {
            if let Some(ref bytes) = *cache {
                let text = decode_bytes(self.encoding_name.as_deref(), bytes);
                if let Ok(mut tc) = self.cached_text.lock() {
                    *tc = Some(text.clone());
                }
                return Ok(text);
            }
        }
        let bytes = self.drain_all_bytes()?;
        let text = decode_bytes(self.encoding_name.as_deref(), &bytes);
        if let Ok(mut cache) = self.cached_text.lock() {
            *cache = Some(text.clone());
        }
        Ok(text)
    }

    fn ensure_streaming(&self) -> PyResult<()> {
        match self.body_state.load(Ordering::SeqCst) {
            STATE_STREAMING => Ok(()),
            STATE_BUFFERED => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "streaming body has already been buffered",
            )),
            STATE_CONSUMED => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "streaming body has been consumed",
            )),
            STATE_CLOSED => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "streaming body has been closed",
            )),
            _ => unreachable!(),
        }
    }

    fn drain_and_close(&self) -> PyResult<()> {
        if self.body_state.load(Ordering::SeqCst) == STATE_STREAMING {
            if let Some(mut stream) = self
                .stream
                .lock()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?
                .take()
            {
                // Try to get the current tokio runtime handle (if we're inside one).
                // If we are, drain without creating a new runtime. If not, create one.
                match tokio::runtime::Handle::try_current() {
                    Ok(_handle) => {
                        // We're inside a tokio runtime. We can't block_on here.
                        // The stream will be dropped (connection closed) when it goes out of scope.
                        // Just drop the stream to clean up.
                        drop(stream);
                    }
                    Err(_) => {
                        // No tokio runtime, create one.
                        let rt = tokio::runtime::Runtime::new().map_err(|e| {
                            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string())
                        })?;
                        rt.block_on(async { while stream.next().await.is_some() {} });
                    }
                }
            }
        }
        self.body_state.store(STATE_CLOSED, Ordering::SeqCst);
        Ok(())
    }

    fn take_stream_or_err(&self) -> PyResult<eggfetch_core::body::BoxBytesStream> {
        self.ensure_streaming()?;
        self.take_stream()
    }
}

fn decode_bytes(encoding_name: Option<&str>, bytes: &[u8]) -> String {
    if let Some(name) = encoding_name {
        if let Some(enc) = encoding_rs::Encoding::for_label(name.as_bytes()) {
            let (decoded, _, _) = enc.decode(bytes);
            return decoded.into_owned();
        }
    }
    String::from_utf8_lossy(bytes).to_string()
}

#[pymethods]
impl PyStreamingResponse {
    #[getter]
    fn status_code(&self) -> u16 {
        self.status_code
    }

    #[getter]
    fn encoding(&self) -> Option<String> {
        self.encoding_name.clone()
    }

    #[getter]
    fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }

    #[getter]
    fn is_redirect(&self) -> bool {
        (300..400).contains(&self.status_code)
    }

    #[getter]
    fn is_client_error(&self) -> bool {
        (400..500).contains(&self.status_code)
    }

    #[getter]
    fn is_server_error(&self) -> bool {
        (500..600).contains(&self.status_code)
    }

    #[getter]
    fn is_error(&self) -> bool {
        (400..600).contains(&self.status_code)
    }

    #[getter]
    fn is_informational(&self) -> bool {
        (100..200).contains(&self.status_code)
    }

    fn raise_for_status(&self) -> PyResult<()> {
        if self.status_code >= 400 {
            return Err(crate::errors::HTTPStatusError::new_err(format!(
                "{} for url '{}'",
                self.status_code, self.url
            )));
        }
        Ok(())
    }

    #[pyo3(signature = ())]
    fn iter_bytes<'py>(
        slf: Py<Self>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyBytesChunkIterator>> {
        PyBytesChunkIterator::new(slf, py)
    }

    #[pyo3(signature = (*, encoding=None))]
    fn iter_text<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        encoding: Option<&str>,
    ) -> PyResult<Bound<'py, PyTextChunkIterator>> {
        PyTextChunkIterator::new(slf, py, encoding.map(String::from))
    }

    #[pyo3(signature = (*, encoding=None))]
    fn iter_lines<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        encoding: Option<&str>,
    ) -> PyResult<Bound<'py, PyLinesChunkIterator>> {
        PyLinesChunkIterator::new(slf, py, encoding.map(String::from))
    }

    #[pyo3(signature = ())]
    fn aiter_bytes<'py>(
        slf: Py<Self>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAsyncBytesIterator>> {
        PyAsyncBytesIterator::new(slf, py)
    }

    #[pyo3(signature = (*, encoding=None))]
    fn aiter_text<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        encoding: Option<&str>,
    ) -> PyResult<Bound<'py, PyAsyncTextIterator>> {
        PyAsyncTextIterator::new(slf, py, encoding.map(String::from))
    }

    #[pyo3(signature = (*, encoding=None))]
    fn aiter_lines<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        encoding: Option<&str>,
    ) -> PyResult<Bound<'py, PyAsyncLinesIterator>> {
        PyAsyncLinesIterator::new(slf, py, encoding.map(String::from))
    }

    fn read(slf: Py<Self>, py: Python<'_>) -> PyResult<PyObject> {
        let bytes = {
            let borrowed = slf.borrow(py);
            borrowed.ensure_streaming()?;
            borrowed.drain_all_bytes()?
        };
        Ok(PyBytes::new(py, &bytes).into())
    }

    fn text(slf: Py<Self>, py: Python<'_>) -> PyResult<String> {
        let borrowed = slf.borrow(py);
        borrowed.drain_all_text()
    }

    fn aread<'py>(slf: Py<Self>, py: Python<'py>) -> PyResult<Bound<'py, pyo3::PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut stream = {
                Python::with_gil(|py| {
                    let borrowed = slf.borrow(py);
                    borrowed.take_stream_or_err()
                })?
            };
            let mut buf = BytesMut::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(map_err)?;
                buf.extend_from_slice(&chunk);
            }
            let bytes = buf.freeze();
            Python::with_gil(|py| {
                let borrowed = slf.borrow(py);
                borrowed.body_state.store(STATE_BUFFERED, Ordering::SeqCst);
                if let Ok(mut cache) = borrowed.cached_content.lock() {
                    *cache = Some(bytes.clone());
                }
                Ok::<_, PyErr>(())
            })?;
            Ok::<_, PyErr>(bytes.to_vec())
        })
    }

    fn close(slf: Py<Self>, py: Python<'_>) -> PyResult<()> {
        slf.borrow(py).drain_and_close()
    }

    fn aclose<'py>(slf: Py<Self>, py: Python<'py>) -> PyResult<Bound<'py, pyo3::PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let result: PyResult<()> = Python::with_gil(|py| slf.borrow(py).drain_and_close());
            result?;
            Ok(())
        })
    }

    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    #[pyo3(signature = (_exc_type=None, _exc_value=None, _traceback=None))]
    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        self.drain_and_close()?;
        Ok(false)
    }

    fn __aenter__(slf: Py<Self>, py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        let asyncio = py.import("asyncio")?;
        let future = asyncio.getattr("Future")?.call0()?;
        future.call_method1("set_result", (slf,))?;
        Ok(future)
    }

    #[pyo3(signature = (_exc_type=None, _exc_value=None, _traceback=None))]
    fn __aexit__<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'py, pyo3::PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let result: PyResult<bool> = Python::with_gil(|py| {
                slf.borrow(py).drain_and_close()?;
                Ok(false)
            });
            result
        })
    }

    fn __repr__(&self) -> String {
        format!("<StreamingResponse [{} {}]>", self.status_code, self.url)
    }
}

// ---------------------------------------------------------------------------
// Sync byte iterator
// ---------------------------------------------------------------------------

#[pyclass(name = "StreamingBytesIterator")]
pub(crate) struct PyBytesChunkIterator {
    rx: std::sync::Mutex<std::sync::mpsc::Receiver<Result<Bytes, PyErr>>>,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyBytesChunkIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    fn new<'py>(resp: Py<PyStreamingResponse>, py: Python<'py>) -> PyResult<Bound<'py, Self>> {
        let stream = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            borrowed.take_stream()?
        };

        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                        e.to_string(),
                    )));
                    return;
                }
            };
            rt.block_on(async {
                let mut stream = stream;
                while let Some(chunk_result) = stream.next().await {
                    let result = match chunk_result {
                        Ok(bytes) => Ok(bytes),
                        Err(e) => Err(crate::errors::map_err(e)),
                    };
                    if tx.send(result).is_err() {
                        break;
                    }
                }
            });
        });

        Py::new(
            py,
            Self {
                rx: std::sync::Mutex::new(rx),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

#[pymethods]
impl PyBytesChunkIterator {
    fn __iter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        let result = py.allow_threads(|| {
            let rx = self
                .rx
                .lock()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
            rx.recv()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        });
        match result {
            Ok(Ok(bytes)) => Ok(Some(PyBytes::new(py, &bytes).into())),
            Ok(Err(err)) => Err(err),
            Err(_) => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Sync text iterator
// ---------------------------------------------------------------------------

#[pyclass(name = "StreamingTextIterator")]
pub(crate) struct PyTextChunkIterator {
    rx: std::sync::Mutex<std::sync::mpsc::Receiver<Result<String, PyErr>>>,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyTextChunkIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    fn new<'py>(
        resp: Py<PyStreamingResponse>,
        py: Python<'py>,
        encoding_override: Option<String>,
    ) -> PyResult<Bound<'py, Self>> {
        let (stream, enc_name) = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            let stream = borrowed.take_stream()?;
            let enc_name = encoding_override.or_else(|| borrowed.encoding_name.clone());
            (stream, enc_name)
        };

        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                        e.to_string(),
                    )));
                    return;
                }
            };
            rt.block_on(async move {
                let mut stream = stream;
                while let Some(chunk_result) = stream.next().await {
                    match chunk_result {
                        Ok(chunk) => {
                            let text = decode_bytes(enc_name.as_deref(), &chunk);
                            if tx.send(Ok(text)).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Err(crate::errors::map_err(e)));
                            break;
                        }
                    }
                }
            });
        });

        Py::new(
            py,
            Self {
                rx: std::sync::Mutex::new(rx),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

#[pymethods]
impl PyTextChunkIterator {
    fn __iter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        let result = py.allow_threads(|| {
            let rx = self
                .rx
                .lock()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
            rx.recv()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        });
        match result {
            Ok(Ok(text)) => Ok(Some(PyString::new(py, &text).into())),
            Ok(Err(err)) => Err(err),
            Err(_) => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Sync lines iterator
// ---------------------------------------------------------------------------

#[pyclass(name = "StreamingLinesIterator")]
pub(crate) struct PyLinesChunkIterator {
    rx: std::sync::Mutex<std::sync::mpsc::Receiver<Result<String, PyErr>>>,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyLinesChunkIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    fn new<'py>(
        resp: Py<PyStreamingResponse>,
        py: Python<'py>,
        encoding_override: Option<String>,
    ) -> PyResult<Bound<'py, Self>> {
        let (stream, enc_name) = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            let stream = borrowed.take_stream()?;
            let enc_name = encoding_override.or_else(|| borrowed.encoding_name.clone());
            (stream, enc_name)
        };

        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                        e.to_string(),
                    )));
                    return;
                }
            };
            rt.block_on(async move {
                let mut stream = stream;
                let mut line_buffer = BytesMut::new();

                while let Some(chunk_result) = stream.next().await {
                    match chunk_result {
                        Ok(chunk) => {
                            let decoded = decode_bytes(enc_name.as_deref(), &chunk);
                            line_buffer.extend_from_slice(decoded.as_bytes());

                            while let Some(pos) = line_buffer.iter().position(|&b| b == b'\n') {
                                let line_bytes = line_buffer.split_to(pos + 1);
                                let mut line = &line_bytes[..line_bytes.len() - 1];
                                if line.ends_with(&[b'\r'][..]) {
                                    line = &line[..line.len() - 1];
                                }
                                let line_str = String::from_utf8(line.to_vec()).unwrap_or_default();
                                if tx.send(Ok(line_str)).is_err() {
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Err(crate::errors::map_err(e)));
                            break;
                        }
                    }
                }

                if !line_buffer.is_empty() {
                    let remaining = std::mem::take(&mut line_buffer);
                    let mut line = &remaining[..];
                    if line.ends_with(&[b'\r'][..]) {
                        line = &line[..line.len() - 1];
                    }
                    let line_str = String::from_utf8(line.to_vec()).unwrap_or_default();
                    let _ = tx.send(Ok(line_str));
                }
            });
        });

        Py::new(
            py,
            Self {
                rx: std::sync::Mutex::new(rx),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

#[pymethods]
impl PyLinesChunkIterator {
    fn __iter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        let result = py.allow_threads(|| {
            let rx = self
                .rx
                .lock()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
            rx.recv()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        });
        match result {
            Ok(Ok(text)) => Ok(Some(PyString::new(py, &text).into())),
            Ok(Err(err)) => Err(err),
            Err(_) => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Async byte iterator
// ---------------------------------------------------------------------------

#[pyclass(name = "AsyncStreamingBytesIterator")]
pub(crate) struct PyAsyncBytesIterator {
    rx: AsyncByteRx,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyAsyncBytesIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    fn new<'py>(resp: Py<PyStreamingResponse>, py: Python<'py>) -> PyResult<Bound<'py, Self>> {
        let stream = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            borrowed.take_stream()?
        };

        let (tx, rx) = tokio::sync::mpsc::channel(16);

        pyo3_async_runtimes::tokio::get_runtime().spawn(async move {
            let mut stream = stream;
            while let Some(chunk_result) = stream.next().await {
                let result = match chunk_result {
                    Ok(bytes) => Ok(bytes.to_vec()),
                    Err(e) => Err(crate::errors::map_err(e)),
                };
                if tx.send(result).await.is_err() {
                    break;
                }
            }
        });

        Py::new(
            py,
            Self {
                rx: Arc::new(tokio::sync::Mutex::new(rx)),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

#[pymethods]
impl PyAsyncBytesIterator {
    fn __aiter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __anext__<'py>(slf: Py<Self>, py: Python<'py>) -> PyResult<Bound<'py, pyo3::PyAny>> {
        let rx = Arc::clone(&slf.borrow(py).rx);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut rx_guard = rx.lock().await;
            match rx_guard.recv().await {
                Some(Ok(bytes)) => Ok(bytes),
                Some(Err(err)) => Err(err),
                None => Err(pyo3::exceptions::PyStopAsyncIteration::new_err("")),
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Async text iterator
// ---------------------------------------------------------------------------

#[pyclass(name = "AsyncStreamingTextIterator")]
pub(crate) struct PyAsyncTextIterator {
    rx: AsyncTextRx,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyAsyncTextIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    fn new<'py>(
        resp: Py<PyStreamingResponse>,
        py: Python<'py>,
        encoding_override: Option<String>,
    ) -> PyResult<Bound<'py, Self>> {
        let (stream, enc_name) = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            let stream = borrowed.take_stream()?;
            let enc_name = encoding_override.or_else(|| borrowed.encoding_name.clone());
            (stream, enc_name)
        };

        let (tx, rx) = tokio::sync::mpsc::channel(16);

        pyo3_async_runtimes::tokio::get_runtime().spawn(async move {
            let mut stream = stream;
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        let text = decode_bytes(enc_name.as_deref(), &chunk);
                        if tx.send(Ok(text)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(crate::errors::map_err(e))).await;
                        break;
                    }
                }
            }
        });

        Py::new(
            py,
            Self {
                rx: Arc::new(tokio::sync::Mutex::new(rx)),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

#[pymethods]
impl PyAsyncTextIterator {
    fn __aiter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __anext__<'py>(slf: Py<Self>, py: Python<'py>) -> PyResult<Bound<'py, pyo3::PyAny>> {
        let rx = Arc::clone(&slf.borrow(py).rx);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut rx_guard = rx.lock().await;
            match rx_guard.recv().await {
                Some(Ok(text)) => Ok(text),
                Some(Err(err)) => Err(err),
                None => Err(pyo3::exceptions::PyStopAsyncIteration::new_err("")),
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Async lines iterator
// ---------------------------------------------------------------------------

#[pyclass(name = "AsyncStreamingLinesIterator")]
pub(crate) struct PyAsyncLinesIterator {
    rx: AsyncTextRx,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyAsyncLinesIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    fn new<'py>(
        resp: Py<PyStreamingResponse>,
        py: Python<'py>,
        encoding_override: Option<String>,
    ) -> PyResult<Bound<'py, Self>> {
        let (stream, enc_name) = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            let stream = borrowed.take_stream()?;
            let enc_name = encoding_override.or_else(|| borrowed.encoding_name.clone());
            (stream, enc_name)
        };

        let (tx, rx) = tokio::sync::mpsc::channel(16);

        pyo3_async_runtimes::tokio::get_runtime().spawn(async move {
            let mut stream = stream;
            let mut line_buffer = BytesMut::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        let decoded = decode_bytes(enc_name.as_deref(), &chunk);
                        line_buffer.extend_from_slice(decoded.as_bytes());

                        while let Some(pos) = line_buffer.iter().position(|&b| b == b'\n') {
                            let line_bytes = line_buffer.split_to(pos + 1);
                            let mut line = &line_bytes[..line_bytes.len() - 1];
                            if line.ends_with(&[b'\r'][..]) {
                                line = &line[..line.len() - 1];
                            }
                            let line_str = String::from_utf8(line.to_vec()).unwrap_or_default();
                            if tx.send(Ok(line_str)).await.is_err() {
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(crate::errors::map_err(e))).await;
                        break;
                    }
                }
            }

            if !line_buffer.is_empty() {
                let remaining = std::mem::take(&mut line_buffer);
                let mut line = &remaining[..];
                if line.ends_with(&[b'\r'][..]) {
                    line = &line[..line.len() - 1];
                }
                let line_str = String::from_utf8(line.to_vec()).unwrap_or_default();
                let _ = tx.send(Ok(line_str)).await;
            }
        });

        Py::new(
            py,
            Self {
                rx: Arc::new(tokio::sync::Mutex::new(rx)),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

#[pymethods]
impl PyAsyncLinesIterator {
    fn __aiter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __anext__<'py>(slf: Py<Self>, py: Python<'py>) -> PyResult<Bound<'py, pyo3::PyAny>> {
        let rx = Arc::clone(&slf.borrow(py).rx);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut rx_guard = rx.lock().await;
            match rx_guard.recv().await {
                Some(Ok(text)) => Ok(text),
                Some(Err(err)) => Err(err),
                None => Err(pyo3::exceptions::PyStopAsyncIteration::new_err("")),
            }
        })
    }
}
