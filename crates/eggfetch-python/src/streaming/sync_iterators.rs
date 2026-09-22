//! Private synchronous streaming iterators.

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures_util::StreamExt;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyString};

use super::decoding::{complete_lines, final_line, IncrementalDecoder};
use super::{PyStreamingResponse, StreamMode, SyncBridge};

// ---------------------------------------------------------------------------
// Sync byte iterator
// ---------------------------------------------------------------------------

#[pyclass(name = "StreamingBytesIterator")]
pub(crate) struct PyBytesChunkIterator {
    rx: Arc<SyncBridge<Result<Bytes, PyErr>>>,
    cancel: Option<tokio::sync::watch::Sender<bool>>,
    producer: Option<tokio::task::JoinHandle<()>>,
    chunk_size: usize,
    pending: Mutex<Option<Bytes>>,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyBytesChunkIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    pub(super) fn new<'py>(
        resp: Py<PyStreamingResponse>,
        py: Python<'py>,
        chunk_size: usize,
    ) -> PyResult<Bound<'py, Self>> {
        let (stream, runtime_handle) = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            let stream = borrowed.take_stream(StreamMode::Decoded)?;
            (stream, borrowed.runtime_handle.clone())
        };
        let (cancel, mut cancellation) = tokio::sync::watch::channel(false);

        if chunk_size == 0 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "chunk_size must be greater than zero",
            ));
        }
        let bridge = SyncBridge::new(16);
        let producer_bridge = Arc::clone(&bridge);

        let producer = runtime_handle.spawn(async move {
            let mut stream = stream;
            loop {
                tokio::select! {
                    changed = cancellation.changed() => {
                        if changed.is_err() || *cancellation.borrow() {
                            break;
                        }
                    }
                    chunk = stream.next() => match chunk {
                        Some(chunk_result) => {
                            let result = match chunk_result {
                                Ok(bytes) => Ok(bytes),
                                Err(e) => Err(crate::errors::map_err(e)),
                            };
                            if !producer_bridge.send(result).await {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
            producer_bridge.close();
        });

        Py::new(
            py,
            Self {
                rx: bridge,
                cancel: Some(cancel),
                producer: Some(producer),
                chunk_size,
                pending: Mutex::new(None),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

impl Drop for PyBytesChunkIterator {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
        self.rx.close();
        if let Some(producer) = self.producer.take() {
            producer.abort();
        }
    }
}

#[pymethods]
impl PyBytesChunkIterator {
    fn __iter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        {
            let mut pending = self
                .pending
                .lock()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
            if let Some(bytes) = pending.as_mut() {
                let take = bytes.len().min(self.chunk_size);
                let chunk = bytes.split_to(take);
                if bytes.is_empty() {
                    *pending = None;
                }
                return Ok(Some(PyBytes::new(py, &chunk).into()));
            }
        }
        let result = py.detach(|| Ok::<_, PyErr>(self.rx.recv_blocking()));
        match result {
            Ok(Some(Ok(mut bytes))) => {
                if bytes.len() > self.chunk_size {
                    let chunk = bytes.split_to(self.chunk_size);
                    let mut pending = self.pending.lock().map_err(|e| {
                        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string())
                    })?;
                    *pending = Some(bytes);
                    return Ok(Some(PyBytes::new(py, &chunk).into()));
                }
                Ok(Some(PyBytes::new(py, &bytes).into()))
            }
            Ok(Some(Err(err))) => Err(err),
            Ok(None) | Err(_) => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Sync text iterator
// ---------------------------------------------------------------------------

#[pyclass(name = "StreamingTextIterator")]
pub(crate) struct PyTextChunkIterator {
    rx: Arc<SyncBridge<Result<String, PyErr>>>,
    cancel: Option<tokio::sync::watch::Sender<bool>>,
    producer: Option<tokio::task::JoinHandle<()>>,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyTextChunkIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    pub(super) fn new<'py>(
        resp: Py<PyStreamingResponse>,
        py: Python<'py>,
        _chunk_size: usize,
        encoding_override: Option<String>,
    ) -> PyResult<Bound<'py, Self>> {
        let (stream, enc_name, runtime_handle) = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            let stream = borrowed.take_stream(StreamMode::Decoded)?;
            let enc_name = encoding_override.or_else(|| borrowed.encoding_name.clone());
            let runtime_handle = borrowed.runtime_handle.clone();
            (stream, enc_name, runtime_handle)
        };
        let (cancel, mut cancellation) = tokio::sync::watch::channel(false);

        let bridge = SyncBridge::new(16);
        let producer_bridge = Arc::clone(&bridge);

        let producer = runtime_handle.spawn(async move {
            let mut stream = stream;
            let mut decoder = IncrementalDecoder::new(enc_name.as_deref());
            loop {
                tokio::select! {
                    changed = cancellation.changed() => {
                        if changed.is_err() || *cancellation.borrow() {
                            break;
                        }
                    }
                    chunk = stream.next() => match chunk {
                        Some(Ok(chunk)) => {
                            let text = decoder.decode(&chunk, false);
                            if !producer_bridge.send(Ok(text)).await {
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            let _ = producer_bridge.send(Err(crate::errors::map_err(e))).await;
                            break;
                        }
                        None => break,
                    }
                }
            }
            let tail = decoder.finish();
            if !tail.is_empty() {
                let _ = producer_bridge.send(Ok(tail)).await;
            }
            producer_bridge.close();
        });

        Py::new(
            py,
            Self {
                rx: bridge,
                cancel: Some(cancel),
                producer: Some(producer),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

impl Drop for PyTextChunkIterator {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
        self.rx.close();
        if let Some(producer) = self.producer.take() {
            producer.abort();
        }
    }
}

#[pymethods]
impl PyTextChunkIterator {
    fn __iter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let result = py.detach(|| Ok::<_, PyErr>(self.rx.recv_blocking()));
        match result {
            Ok(Some(Ok(text))) => Ok(Some(PyString::new(py, &text).into())),
            Ok(Some(Err(err))) => Err(err),
            Ok(None) | Err(_) => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Sync lines iterator
// ---------------------------------------------------------------------------

#[pyclass(name = "StreamingLinesIterator")]
pub(crate) struct PyLinesChunkIterator {
    rx: Arc<SyncBridge<Result<String, PyErr>>>,
    cancel: Option<tokio::sync::watch::Sender<bool>>,
    producer: Option<tokio::task::JoinHandle<()>>,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyLinesChunkIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    pub(super) fn new<'py>(
        resp: Py<PyStreamingResponse>,
        py: Python<'py>,
        _chunk_size: usize,
        encoding_override: Option<String>,
    ) -> PyResult<Bound<'py, Self>> {
        let (stream, enc_name, runtime_handle) = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            let stream = borrowed.take_stream(StreamMode::Decoded)?;
            let enc_name = encoding_override.or_else(|| borrowed.encoding_name.clone());
            let runtime_handle = borrowed.runtime_handle.clone();
            (stream, enc_name, runtime_handle)
        };
        let (cancel, mut cancellation) = tokio::sync::watch::channel(false);

        let bridge = SyncBridge::new(16);
        let producer_bridge = Arc::clone(&bridge);

        let producer = runtime_handle.spawn(async move {
            let mut stream = stream;
            let mut decoder = IncrementalDecoder::new(enc_name.as_deref());
            let mut line_buffer = String::new();

            loop {
                tokio::select! {
                    changed = cancellation.changed() => {
                        if changed.is_err() || *cancellation.borrow() {
                            producer_bridge.close();
                            return;
                        }
                    }
                    chunk = stream.next() => match chunk {
                        Some(Ok(chunk)) => {
                            line_buffer.push_str(&decoder.decode(&chunk, false));
                            for line in complete_lines(&mut line_buffer) {
                                if !producer_bridge.send(Ok(line)).await {
                                    producer_bridge.close();
                                    return;
                                }
                            }
                        }
                        Some(Err(e)) => {
                            let _ = producer_bridge.send(Err(crate::errors::map_err(e))).await;
                            break;
                        }
                        None => break,
                    }
                }
            }

            line_buffer.push_str(&decoder.finish());
            for line in complete_lines(&mut line_buffer) {
                if !producer_bridge.send(Ok(line)).await {
                    producer_bridge.close();
                    return;
                }
            }
            if let Some(line) = final_line(&mut line_buffer) {
                let _ = producer_bridge.send(Ok(line)).await;
            }
            producer_bridge.close();
        });

        Py::new(
            py,
            Self {
                rx: bridge,
                cancel: Some(cancel),
                producer: Some(producer),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

impl Drop for PyLinesChunkIterator {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
        self.rx.close();
        if let Some(producer) = self.producer.take() {
            producer.abort();
        }
    }
}

#[pymethods]
impl PyLinesChunkIterator {
    fn __iter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let result = py.detach(|| Ok::<_, PyErr>(self.rx.recv_blocking()));
        match result {
            Ok(Some(Ok(text))) => Ok(Some(PyString::new(py, &text).into())),
            Ok(Some(Err(err))) => Err(err),
            Ok(None) | Err(_) => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------

#[pyclass(name = "StreamingRawBytesIterator")]
pub(crate) struct PyRawBytesChunkIterator {
    rx: Arc<SyncBridge<Result<Bytes, PyErr>>>,
    cancel: Option<tokio::sync::watch::Sender<bool>>,
    producer: Option<tokio::task::JoinHandle<()>>,
    chunk_size: Option<usize>,
    pending: Mutex<Option<Bytes>>,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyRawBytesChunkIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    pub(super) fn new<'py>(
        resp: Py<PyStreamingResponse>,
        py: Python<'py>,
        chunk_size: Option<usize>,
    ) -> PyResult<Bound<'py, Self>> {
        let stream = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            borrowed.take_stream(StreamMode::Raw)?
        };
        let runtime_handle = resp.borrow(py).runtime_handle.clone();
        let (cancel, mut cancellation) = tokio::sync::watch::channel(false);

        let bridge = SyncBridge::new(16);
        let producer_bridge = Arc::clone(&bridge);

        let producer = runtime_handle.spawn(async move {
            let mut stream = stream;
            loop {
                tokio::select! {
                    changed = cancellation.changed() => {
                        if changed.is_err() || *cancellation.borrow() {
                            break;
                        }
                    }
                    chunk = stream.next() => match chunk {
                        Some(chunk_result) => {
                            let result = match chunk_result {
                                Ok(bytes) => Ok(bytes),
                                Err(e) => Err(crate::errors::map_err(e)),
                            };
                            if !producer_bridge.send(result).await {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
            producer_bridge.close();
        });

        Py::new(
            py,
            Self {
                rx: bridge,
                cancel: Some(cancel),
                producer: Some(producer),
                chunk_size,
                pending: Mutex::new(None),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

impl Drop for PyRawBytesChunkIterator {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
        self.rx.close();
        if let Some(producer) = self.producer.take() {
            producer.abort();
        }
    }
}

#[pymethods]
impl PyRawBytesChunkIterator {
    fn __iter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        {
            let mut pending = self
                .pending
                .lock()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
            if let Some(bytes) = pending.as_mut() {
                let chunk = if let Some(chunk_size) = self.chunk_size {
                    bytes.split_to(bytes.len().min(chunk_size))
                } else {
                    std::mem::take(bytes)
                };
                if bytes.is_empty() {
                    *pending = None;
                }
                return Ok(Some(PyBytes::new(py, &chunk).into()));
            }
        }
        let result = py.detach(|| Ok::<_, PyErr>(self.rx.recv_blocking()));
        match result {
            Ok(Some(Ok(mut bytes))) => {
                let Some(chunk_size) = self.chunk_size else {
                    return Ok(Some(PyBytes::new(py, &bytes).into()));
                };
                if bytes.len() > chunk_size {
                    let chunk = bytes.split_to(chunk_size);
                    let mut pending = self.pending.lock().map_err(|e| {
                        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string())
                    })?;
                    *pending = Some(bytes);
                    return Ok(Some(PyBytes::new(py, &chunk).into()));
                }
                Ok(Some(PyBytes::new(py, &bytes).into()))
            }
            Ok(Some(Err(err))) => Err(err),
            Ok(None) | Err(_) => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
