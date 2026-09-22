//! True streaming response types for the Python bindings.
//!
//! Iterator constructors accept `encoding_override` by value because the
//! Python binding layer already owns the string and passing it through is
//! the natural `pyo3` convention. Explicit `'py` lifetimes are kept on `pyo3`
//! methods for clarity even when elidable. Channel types are wrapped in
//! `Mutex` to satisfy `pyclass` `Sync` requirements.

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

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

use crate::cookies::PyCookies;
use crate::errors::map_err;
use crate::errors::{StreamClosed, StreamConsumed};
use crate::headers::PyHeaders;
use crate::network_stream::{EitherNetworkStream, PyAsyncNetworkStream, PyNetworkStream};
use crate::response::{extract_charset, version_to_string};

mod async_iterators;
mod decoding;
mod state;
mod sync_bridge;
mod sync_iterators;
pub(crate) use async_iterators::{
    PyAsyncBytesIterator, PyAsyncLinesIterator, PyAsyncRawBytesIterator, PyAsyncTextIterator,
};
use decoding::decode_bytes;
pub(crate) use state::RuntimeLease;
use state::{StreamMode, STATE_BUFFERED, STATE_CLOSED, STATE_CONSUMED, STATE_STREAMING};
use sync_bridge::SyncBridge;
pub(crate) use sync_iterators::{
    PyBytesChunkIterator, PyLinesChunkIterator, PyRawBytesChunkIterator, PyTextChunkIterator,
};

/// A streaming HTTP response exposed to Python.
#[pyclass(name = "StreamingResponse")]
pub(crate) struct PyStreamingResponse {
    #[pyo3(get)]
    status_code: u16,
    #[pyo3(get)]
    headers: PyHeaders,
    #[pyo3(get)]
    url: String,
    /// HTTP reason phrase (e.g. "OK", "Not Found").  Prefer the wire
    /// reason phrase captured from the server when present; fall back
    /// to the canonical phrase derived from the status code.
    #[pyo3(get)]
    reason_phrase: String,
    /// HTTP version string (e.g. "HTTP/1.1", "HTTP/2").
    #[pyo3(get)]
    http_version: String,
    /// Optional network stream handle for connection metadata and
    /// upgraded-connection IO.  Always `None` for non-101 streaming
    /// responses because the body iterator is the canonical accessor
    /// for the connection. For 101 Switching Protocols streaming
    /// responses, this holds the appropriate wrapper (sync or async)
    /// based on the caller's context.
    network_stream: Option<EitherNetworkStream>,
    #[pyo3(get)]
    history: Vec<crate::response::PyResponse>,
    #[pyo3(get)]
    cookies: PyCookies,
    /// Original wire `Content-Encoding`, for the HTTPX compatibility facade.
    #[pyo3(get)]
    _wire_content_encoding: Option<String>,
    /// Original wire `Content-Length`, for the HTTPX compatibility facade.
    #[pyo3(get)]
    _wire_content_length: Option<String>,
    body_state: Arc<AtomicU8>,
    response: Arc<std::sync::Mutex<Option<eggfetch_core::Response>>>,
    stream_cancel: tokio::sync::watch::Sender<bool>,
    runtime_handle: tokio::runtime::Handle,
    _runtime_lease: Option<RuntimeLease>,
    encoding_name: Option<String>,
    cached_content: Arc<std::sync::Mutex<Option<Bytes>>>,
    cached_text: Arc<std::sync::Mutex<Option<String>>>,
}

#[derive(Clone)]
struct StreamingBodyState {
    body_state: Arc<AtomicU8>,
    response: Arc<std::sync::Mutex<Option<eggfetch_core::Response>>>,
    stream_cancel: tokio::sync::watch::Sender<bool>,
    runtime_handle: tokio::runtime::Handle,
    encoding_name: Option<String>,
    cached_content: Arc<std::sync::Mutex<Option<Bytes>>>,
    cached_text: Arc<std::sync::Mutex<Option<String>>>,
}

impl PyStreamingResponse {
    pub fn from_core_response(
        py: Python<'_>,
        mut response: eggfetch_core::Response,
        runtime_handle: tokio::runtime::Handle,
        runtime_lease: Option<RuntimeLease>,
        is_async: bool,
    ) -> PyResult<Bound<'_, Self>> {
        let status = response.status().as_u16();
        let response_url = response.url().to_string();
        let encoding = extract_charset(response.headers());
        let wire_content_encoding = response.wire_content_encoding().map(ToOwned::to_owned);
        let wire_content_length = response.wire_content_length().map(ToOwned::to_owned);

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
        // The body-state wrapper retains the core response, but it never
        // needs the ordinary response HeaderMap after construction. Move it
        // into PyHeaders after all wire metadata/cookie work is complete.
        let headers = PyHeaders::from_header_map(std::mem::take(response.headers_mut()));

        let (stream_cancel, _) = tokio::sync::watch::channel(false);

        // Capture reason phrase / http version / network stream before
        // we move the response into the inner Mutex so they stay available
        // through the lifetime of the streaming response.
        let reason_phrase = response
            .wire_reason_phrase()
            .map(ToOwned::to_owned)
            .or_else(|| response.status().canonical_reason().map(ToOwned::to_owned))
            .unwrap_or_default();
        let http_version = version_to_string(response.version());

        // For streaming responses we do not pre-extract the network
        // stream because the underlying connection is still owned by
        // the body stream.  Ordinary streaming responses expose
        // connection metadata through `into_network_stream()` after the
        // body has been drained.  Upgraded (101) responses expose their
        // upgraded IO via `into_upgraded_stream()`.
        //
        // For 101 streaming responses, create the wrapper that matches
        // the caller's context:
        // - Sync callers get `PyNetworkStream` (uses `block_on` for IO).
        // - Async callers get `PyAsyncNetworkStream` (uses `pyo3_async_runtimes`).
        //
        // Using the wrong wrapper type would deadlock: the sync wrapper's
        // `block_on` cannot run inside a Tokio runtime, and the async
        // wrapper's `future_into_py` cannot be awaited from sync code.
        //
        // `take_network_stream` removes the stream from the response
        // without consuming the response itself, so the body iterator
        // can still be driven from the rest of the response.
        let network_stream = match response.take_network_stream() {
            Some(eggfetch_core::network_stream::NetworkStream::Upgraded(u)) => {
                if is_async {
                    Some(EitherNetworkStream::Async(
                        PyAsyncNetworkStream::from_upgraded(u),
                    ))
                } else {
                    Some(EitherNetworkStream::Sync(
                        PyNetworkStream::from_upgraded_with_handle(
                            u,
                            runtime_handle.clone(),
                            runtime_lease.clone(),
                        ),
                    ))
                }
            }
            _ => None,
        };

        Py::new(
            py,
            Self {
                status_code: status,
                headers,
                url: response_url,
                reason_phrase,
                http_version,
                network_stream,
                history,
                cookies,
                _wire_content_encoding: wire_content_encoding,
                _wire_content_length: wire_content_length,
                body_state: Arc::new(AtomicU8::new(STATE_STREAMING)),
                response: Arc::new(std::sync::Mutex::new(Some(response))),
                stream_cancel,
                runtime_handle,
                _runtime_lease: runtime_lease,
                encoding_name: encoding,
                cached_content: Arc::new(std::sync::Mutex::new(None)),
                cached_text: Arc::new(std::sync::Mutex::new(None)),
            },
        )
        .map(|inner| inner.into_bound(py))
    }

    fn take_stream(&self, mode: StreamMode) -> PyResult<eggfetch_core::body::BoxBytesStream> {
        self.body_state().take_stream(mode)
    }

    fn body_state(&self) -> StreamingBodyState {
        StreamingBodyState {
            body_state: self.body_state.clone(),
            response: self.response.clone(),
            stream_cancel: self.stream_cancel.clone(),
            runtime_handle: self.runtime_handle.clone(),
            encoding_name: self.encoding_name.clone(),
            cached_content: self.cached_content.clone(),
            cached_text: self.cached_text.clone(),
        }
    }

    fn ensure_streaming(&self) -> PyResult<()> {
        match self.body_state.load(Ordering::SeqCst) {
            STATE_STREAMING => Ok(()),
            STATE_BUFFERED => Err(StreamConsumed::new_err(
                "streaming body has already been buffered",
            )),
            STATE_CONSUMED => Err(StreamConsumed::new_err("streaming body has been consumed")),
            STATE_CLOSED => Err(StreamClosed::new_err("streaming body has been closed")),
            // Unknown states must surface as an error, not a panic.
            _ => Err(StreamClosed::new_err(
                "streaming body is in an invalid state",
            )),
        }
    }

    fn drain_and_close(&self) -> PyResult<()> {
        // Wake any reader/iterator that already owns the body. Its select
        // branch will drop the stream and release the core pool lease.
        let _ = self.stream_cancel.send(true);
        let mut response_guard = self
            .response
            .lock()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        if self
            .body_state
            .compare_exchange(
                STATE_STREAMING,
                STATE_CLOSED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            // Dropping the live body is sufficient to release the core lease
            // and lets the transport discard the unread response. Blocking
            // here to drain an untrusted server would defeat close().
            drop(response_guard.take());
        } else {
            // Closing is terminal even when a reader or iterator has already
            // moved the body out of the response object.
            self.body_state.store(STATE_CLOSED, Ordering::Release);
        }
        Ok(())
    }

    fn take_stream_or_err(&self) -> PyResult<eggfetch_core::body::BoxBytesStream> {
        self.ensure_streaming()?;
        self.take_stream(StreamMode::Decoded)
    }
}

impl StreamingBodyState {
    fn take_stream(&self, mode: StreamMode) -> PyResult<eggfetch_core::body::BoxBytesStream> {
        let mut response_guard = self
            .response
            .lock()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        let state = self.body_state.load(Ordering::Acquire);
        if state != STATE_STREAMING {
            return Err(match state {
                STATE_CLOSED => StreamClosed::new_err("streaming body has been closed"),
                STATE_BUFFERED | STATE_CONSUMED => {
                    StreamConsumed::new_err("streaming body has already been consumed")
                }
                // Unknown states must surface as an error, not a panic.
                _ => StreamClosed::new_err("streaming body is in an invalid state"),
            });
        }
        self.body_state
            .compare_exchange(
                STATE_STREAMING,
                STATE_CONSUMED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map_err(|state| match state {
                STATE_CLOSED => StreamClosed::new_err("streaming body has been closed"),
                STATE_BUFFERED | STATE_CONSUMED => {
                    StreamConsumed::new_err("streaming body has already been consumed")
                }
                _ => StreamClosed::new_err("streaming body is in an invalid state"),
            })?;
        let mut response = response_guard
            .take()
            .ok_or_else(|| StreamConsumed::new_err("streaming body has already been consumed"))?;
        let stream = match mode {
            StreamMode::Decoded => response.bytes_stream(),
            StreamMode::Raw => response.raw_bytes_stream(),
        }
        .map_err(map_err)?;
        Ok(stream)
    }

    fn drain_all_bytes(&self) -> PyResult<Bytes> {
        let mut stream = self.take_stream(StreamMode::Decoded)?;
        let mut cancellation = self.stream_cancel.subscribe();
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        self.runtime_handle.spawn(async move {
            let result: PyResult<Bytes> = async {
                let mut buf = BytesMut::new();
                loop {
                    tokio::select! {
                        changed = cancellation.changed() => {
                            if changed.is_err() || *cancellation.borrow() {
                                return Err(StreamClosed::new_err("streaming body has been closed"));
                            }
                        }
                        chunk = stream.next() => match chunk {
                            Some(chunk) => {
                                let chunk = chunk.map_err(map_err)?;
                                buf.extend_from_slice(&chunk);
                            }
                            None => break,
                        }
                    }
                }
                Ok(buf.freeze())
            }
            .await;
            let _ = tx.send(result).await;
        });
        let bytes = rx.blocking_recv().ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "streaming body drain task ended without a result",
            )
        })??;
        // Transition CONSUMED -> BUFFERED so later reads hit the cache.
        // A concurrent close may already have flipped the state to CLOSED
        // between the drain completing and this transition; the bytes
        // were fully received, so they must still be returned rather than
        // discarded with a spurious StreamClosed error.
        let _ = self.body_state.compare_exchange(
            STATE_CONSUMED,
            STATE_BUFFERED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        if let Ok(mut cache) = self.cached_content.lock() {
            *cache = Some(bytes.clone());
        }
        Ok(bytes)
    }

    fn drain_all_text(&self) -> PyResult<String> {
        if self.body_state.load(Ordering::Acquire) == STATE_CLOSED {
            return Err(StreamClosed::new_err("streaming body has been closed"));
        }
        if let Ok(cache) = self.cached_text.lock() {
            if let Some(ref text) = *cache {
                return Ok(text.clone());
            }
        }
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
                "{} {} for url '{}'",
                self.status_code,
                self.reason_phrase,
                safe_url_for_display(&self.url)
            )));
        }
        Ok(())
    }

    #[pyo3(signature = (*, chunk_size=8192))]
    fn iter_bytes<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        chunk_size: usize,
    ) -> PyResult<Bound<'py, PyBytesChunkIterator>> {
        PyBytesChunkIterator::new(slf, py, chunk_size)
    }

    /// Snapshot wire-level response metadata (http version, reason
    /// phrase, network stream) into a dict compatible with HTTPX's
    /// `response.extensions`.
    ///
    /// For 101 Switching Protocols streaming responses, the upgraded
    /// stream is exposed through `extensions["network_stream"]`.
    /// For ordinary streaming responses the connection is held in the
    /// body iterator; the `network_stream` field is `None` because the
    /// dedicated `network_stream` getter is the canonical accessor.
    #[getter]
    fn extensions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("http_version", self.http_version.clone())?;
        dict.set_item("reason_phrase", self.reason_phrase.clone())?;
        match self.network_stream.as_ref() {
            Some(stream) if stream.is_upgraded() => {
                stream.insert_into_dict(py, &dict)?;
            }
            _ => {
                dict.set_item("network_stream", py.None())?;
            }
        }
        Ok(dict)
    }

    /// Return the network stream handle for this response.
    ///
    /// For 101 Switching Protocols responses, returns the appropriate
    /// wrapper (sync `NetworkStream` or async `AsyncNetworkStream`) based
    /// on the caller's context. Returns `None` for non-101 responses.
    #[getter]
    fn network_stream<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAny>> {
        match self.network_stream.as_ref() {
            Some(EitherNetworkStream::Sync(s)) => Ok(Py::new(py, s.clone())?.into_any()),
            Some(EitherNetworkStream::Async(s)) => Ok(Py::new(py, s.clone())?.into_any()),
            None => Ok(py.None()),
        }
    }

    #[pyo3(signature = (*, chunk_size=8192, encoding=None))]
    fn iter_text<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        chunk_size: usize,
        encoding: Option<&str>,
    ) -> PyResult<Bound<'py, PyTextChunkIterator>> {
        PyTextChunkIterator::new(slf, py, chunk_size, encoding.map(String::from))
    }

    #[pyo3(signature = (*, chunk_size=8192, encoding=None))]
    fn iter_lines<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        chunk_size: usize,
        encoding: Option<&str>,
    ) -> PyResult<Bound<'py, PyLinesChunkIterator>> {
        PyLinesChunkIterator::new(slf, py, chunk_size, encoding.map(String::from))
    }

    #[pyo3(signature = (*, chunk_size=None))]
    fn iter_raw<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        chunk_size: Option<usize>,
    ) -> PyResult<Bound<'py, PyRawBytesChunkIterator>> {
        PyRawBytesChunkIterator::new(slf, py, chunk_size)
    }

    #[pyo3(signature = (*, chunk_size=8192))]
    fn aiter_bytes<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        chunk_size: usize,
    ) -> PyResult<Bound<'py, PyAsyncBytesIterator>> {
        PyAsyncBytesIterator::new(slf, py, chunk_size)
    }

    #[pyo3(signature = (*, chunk_size=8192, encoding=None))]
    fn aiter_text<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        chunk_size: usize,
        encoding: Option<&str>,
    ) -> PyResult<Bound<'py, PyAsyncTextIterator>> {
        PyAsyncTextIterator::new(slf, py, chunk_size, encoding.map(String::from))
    }

    #[pyo3(signature = (*, chunk_size=8192, encoding=None))]
    fn aiter_lines<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        chunk_size: usize,
        encoding: Option<&str>,
    ) -> PyResult<Bound<'py, PyAsyncLinesIterator>> {
        PyAsyncLinesIterator::new(slf, py, chunk_size, encoding.map(String::from))
    }

    #[pyo3(signature = (*, chunk_size=None))]
    fn aiter_raw<'py>(
        slf: Py<Self>,
        py: Python<'py>,
        chunk_size: Option<usize>,
    ) -> PyResult<Bound<'py, PyAsyncRawBytesIterator>> {
        PyAsyncRawBytesIterator::new(slf, py, chunk_size)
    }

    fn read(slf: Py<Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let state = Python::attach(|py| {
            let borrowed = slf.borrow(py);
            borrowed.ensure_streaming()?;
            Ok::<_, PyErr>(borrowed.body_state())
        })?;
        let bytes = py.detach(|| state.drain_all_bytes())?;
        Ok(PyBytes::new(py, &bytes).into())
    }

    fn text(slf: Py<Self>, py: Python<'_>) -> PyResult<String> {
        let state = Python::attach(|py| Ok::<_, PyErr>(slf.borrow(py).body_state()))?;
        py.detach(|| state.drain_all_text())
    }

    fn aread<'py>(slf: Py<Self>, py: Python<'py>) -> PyResult<Bound<'py, pyo3::PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (mut stream, mut cancellation) = {
                Python::attach(|py| {
                    let borrowed = slf.borrow(py);
                    Ok::<_, PyErr>((
                        borrowed.take_stream_or_err()?,
                        borrowed.stream_cancel.subscribe(),
                    ))
                })?
            };
            let mut buf = BytesMut::new();
            loop {
                tokio::select! {
                    changed = cancellation.changed() => {
                        if changed.is_err() || *cancellation.borrow_and_update() {
                            return Err(StreamClosed::new_err("streaming body has been closed"));
                        }
                    }
                    chunk = stream.next() => match chunk {
                        Some(chunk) => {
                            let chunk = chunk.map_err(map_err)?;
                            buf.extend_from_slice(&chunk);
                        }
                        None => break,
                    }
                }
            }
            let bytes = buf.freeze();
            let result = Python::attach(|py| {
                let borrowed = slf.borrow(py);
                let _ = borrowed.body_state.compare_exchange(
                    STATE_CONSUMED,
                    STATE_BUFFERED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                if let Ok(mut cache) = borrowed.cached_content.lock() {
                    *cache = Some(bytes.clone());
                }
                Ok::<_, PyErr>(PyBytes::new(py, &bytes).unbind().into_any())
            })?;
            Ok::<_, PyErr>(result)
        })
    }

    fn close(slf: Py<Self>, py: Python<'_>) -> PyResult<()> {
        slf.borrow(py).drain_and_close()
    }

    fn aclose<'py>(slf: Py<Self>, py: Python<'py>) -> PyResult<Bound<'py, pyo3::PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let result: PyResult<()> = Python::attach(|py| slf.borrow(py).drain_and_close());
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
        // HTTPX discards unread body on context exit — do NOT drain.
        let _ = self.stream_cancel.send(true);
        let mut response_guard = self
            .response
            .lock()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        // Drop the stream to release the pool lease without draining.
        drop(response_guard.take());
        self.body_state.store(STATE_CLOSED, Ordering::Release);
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
            // HTTPX discards unread body on context exit — do NOT drain.
            let result: PyResult<()> = Python::attach(|py| {
                let borrowed = slf.borrow(py);
                let _ = borrowed.stream_cancel.send(true);
                let mut response_guard = borrowed.response.lock().map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string())
                })?;
                drop(response_guard.take());
                borrowed.body_state.store(STATE_CLOSED, Ordering::Release);
                Ok(())
            });
            result?;
            Ok(false)
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "<StreamingResponse [{} {}]>",
            self.status_code,
            safe_url_for_display(&self.url)
        )
    }
}

pub(crate) fn safe_url_for_display(url: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return "<invalid-url>".to_owned();
    };
    let _ = parsed.set_username("");
    let _ = parsed.set_password(None);
    parsed.set_query(None);
    parsed.set_fragment(None);
    parsed.to_string()
}
