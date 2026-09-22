//! Private asynchronous streaming iterators.

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures_util::StreamExt;
use pyo3::prelude::*;

use super::decoding::{complete_lines, final_line, IncrementalDecoder};
use super::{PyStreamingResponse, StreamMode};

type AsyncByteRx = Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Result<Bytes, PyErr>>>>;
type AsyncTextRx = Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Result<String, PyErr>>>>;

// ---------------------------------------------------------------------------

#[pyclass(name = "AsyncStreamingBytesIterator")]
pub(crate) struct PyAsyncBytesIterator {
    rx: AsyncByteRx,
    producer: Option<tokio::task::JoinHandle<()>>,
    chunk_size: usize,
    pending: Arc<Mutex<Option<Bytes>>>,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyAsyncBytesIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    pub(super) fn new<'py>(
        resp: Py<PyStreamingResponse>,
        py: Python<'py>,
        chunk_size: usize,
    ) -> PyResult<Bound<'py, Self>> {
        let (stream, mut cancellation) = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            Ok::<_, PyErr>((
                borrowed.take_stream(StreamMode::Decoded)?,
                borrowed.stream_cancel.subscribe(),
            ))?
        };

        let (tx, rx) = tokio::sync::mpsc::channel(16);

        let producer = pyo3_async_runtimes::tokio::get_runtime().spawn(async move {
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
                            if tx.send(result).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        });

        Py::new(
            py,
            Self {
                rx: Arc::new(tokio::sync::Mutex::new(rx)),
                producer: Some(producer),
                chunk_size,
                pending: Arc::new(Mutex::new(None)),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

impl Drop for PyAsyncBytesIterator {
    fn drop(&mut self) {
        if let Some(producer) = self.producer.take() {
            producer.abort();
        }
    }
}

#[pymethods]
impl PyAsyncBytesIterator {
    fn __aiter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __anext__<'py>(slf: Py<Self>, py: Python<'py>) -> PyResult<Bound<'py, pyo3::PyAny>> {
        let rx = Arc::clone(&slf.borrow(py).rx);
        let chunk_size = slf.borrow(py).chunk_size;
        let pending = Arc::clone(&slf.borrow(py).pending);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            {
                let mut pending_guard = pending.lock().map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string())
                })?;
                if let Some(bytes) = pending_guard.as_mut() {
                    let take = bytes.len().min(chunk_size);
                    let chunk = bytes.split_to(take);
                    if bytes.is_empty() {
                        *pending_guard = None;
                    }
                    return Ok(chunk.to_vec());
                }
            }
            let mut rx_guard = rx.lock().await;
            match rx_guard.recv().await {
                Some(Ok(mut bytes)) => {
                    if bytes.len() > chunk_size {
                        let chunk = bytes.split_to(chunk_size);
                        let mut pending_guard = pending.lock().map_err(|e| {
                            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string())
                        })?;
                        *pending_guard = Some(bytes);
                        Ok(chunk.to_vec())
                    } else {
                        Ok(bytes.to_vec())
                    }
                }
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
    producer: Option<tokio::task::JoinHandle<()>>,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyAsyncTextIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    pub(super) fn new<'py>(
        resp: Py<PyStreamingResponse>,
        py: Python<'py>,
        _chunk_size: usize,
        encoding_override: Option<String>,
    ) -> PyResult<Bound<'py, Self>> {
        let (stream, enc_name, mut cancellation) = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            let stream = borrowed.take_stream(StreamMode::Decoded)?;
            let enc_name = encoding_override.or_else(|| borrowed.encoding_name.clone());
            (stream, enc_name, borrowed.stream_cancel.subscribe())
        };

        let (tx, rx) = tokio::sync::mpsc::channel(16);

        let producer = pyo3_async_runtimes::tokio::get_runtime().spawn(async move {
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
                            if tx.send(Ok(text)).await.is_err() {
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            let _ = tx.send(Err(crate::errors::map_err(e))).await;
                            break;
                        }
                        None => break,
                    }
                }
            }
            let tail = decoder.finish();
            if !tail.is_empty() {
                let _ = tx.send(Ok(tail)).await;
            }
        });

        Py::new(
            py,
            Self {
                rx: Arc::new(tokio::sync::Mutex::new(rx)),
                producer: Some(producer),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

impl Drop for PyAsyncTextIterator {
    fn drop(&mut self) {
        if let Some(producer) = self.producer.take() {
            producer.abort();
        }
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
    producer: Option<tokio::task::JoinHandle<()>>,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyAsyncLinesIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    pub(super) fn new<'py>(
        resp: Py<PyStreamingResponse>,
        py: Python<'py>,
        _chunk_size: usize,
        encoding_override: Option<String>,
    ) -> PyResult<Bound<'py, Self>> {
        let (stream, enc_name, mut cancellation) = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            let stream = borrowed.take_stream(StreamMode::Decoded)?;
            let enc_name = encoding_override.or_else(|| borrowed.encoding_name.clone());
            (stream, enc_name, borrowed.stream_cancel.subscribe())
        };

        let (tx, rx) = tokio::sync::mpsc::channel(16);

        let producer = pyo3_async_runtimes::tokio::get_runtime().spawn(async move {
            let mut stream = stream;
            let mut decoder = IncrementalDecoder::new(enc_name.as_deref());
            let mut line_buffer = String::new();

            loop {
                tokio::select! {
                    changed = cancellation.changed() => {
                        if changed.is_err() || *cancellation.borrow() {
                            return;
                        }
                    }
                    chunk = stream.next() => match chunk {
                        Some(Ok(chunk)) => {
                            line_buffer.push_str(&decoder.decode(&chunk, false));
                            for line in complete_lines(&mut line_buffer) {
                                if tx.send(Ok(line)).await.is_err() {
                                    return;
                                }
                            }
                        }
                        Some(Err(e)) => {
                            let _ = tx.send(Err(crate::errors::map_err(e))).await;
                            break;
                        }
                        None => break,
                    }
                }
            }

            line_buffer.push_str(&decoder.finish());
            for line in complete_lines(&mut line_buffer) {
                if tx.send(Ok(line)).await.is_err() {
                    return;
                }
            }
            if let Some(line) = final_line(&mut line_buffer) {
                let _ = tx.send(Ok(line)).await;
            }
        });

        Py::new(
            py,
            Self {
                rx: Arc::new(tokio::sync::Mutex::new(rx)),
                producer: Some(producer),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

impl Drop for PyAsyncLinesIterator {
    fn drop(&mut self) {
        if let Some(producer) = self.producer.take() {
            producer.abort();
        }
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

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------

#[pyclass(name = "AsyncStreamingRawBytesIterator")]
pub(crate) struct PyAsyncRawBytesIterator {
    rx: AsyncByteRx,
    producer: Option<tokio::task::JoinHandle<()>>,
    chunk_size: Option<usize>,
    pending: Arc<Mutex<Option<Bytes>>>,
    _keep_alive: Py<PyStreamingResponse>,
}

impl PyAsyncRawBytesIterator {
    #[allow(clippy::needless_pass_by_value, clippy::elidable_lifetime_names)]
    pub(super) fn new<'py>(
        resp: Py<PyStreamingResponse>,
        py: Python<'py>,
        chunk_size: Option<usize>,
    ) -> PyResult<Bound<'py, Self>> {
        let (stream, mut cancellation) = {
            let borrowed = resp.borrow(py);
            borrowed.ensure_streaming()?;
            Ok::<_, PyErr>((
                borrowed.take_stream(StreamMode::Raw)?,
                borrowed.stream_cancel.subscribe(),
            ))?
        };

        let (tx, rx) = tokio::sync::mpsc::channel(16);

        let producer = pyo3_async_runtimes::tokio::get_runtime().spawn(async move {
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
                            if tx.send(result).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        });

        Py::new(
            py,
            Self {
                rx: Arc::new(tokio::sync::Mutex::new(rx)),
                producer: Some(producer),
                chunk_size,
                pending: Arc::new(Mutex::new(None)),
                _keep_alive: resp,
            },
        )
        .map(|inner| inner.into_bound(py))
    }
}

impl Drop for PyAsyncRawBytesIterator {
    fn drop(&mut self) {
        if let Some(producer) = self.producer.take() {
            producer.abort();
        }
    }
}

#[pymethods]
impl PyAsyncRawBytesIterator {
    fn __aiter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __anext__<'py>(slf: Py<Self>, py: Python<'py>) -> PyResult<Bound<'py, pyo3::PyAny>> {
        let rx = Arc::clone(&slf.borrow(py).rx);
        let chunk_size = slf.borrow(py).chunk_size;
        let pending = Arc::clone(&slf.borrow(py).pending);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            {
                let mut pending_guard = pending.lock().map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string())
                })?;
                if let Some(bytes) = pending_guard.as_mut() {
                    let Some(chunk_size) = chunk_size else {
                        return Ok(std::mem::take(bytes).to_vec());
                    };
                    let chunk = bytes.split_to(bytes.len().min(chunk_size));
                    if bytes.is_empty() {
                        *pending_guard = None;
                    }
                    return Ok(chunk.to_vec());
                }
            }
            let mut rx_guard = rx.lock().await;
            match rx_guard.recv().await {
                Some(Ok(mut bytes)) => {
                    let Some(chunk_size) = chunk_size else {
                        return Ok(bytes.to_vec());
                    };
                    if bytes.len() > chunk_size {
                        let chunk = bytes.split_to(chunk_size);
                        let mut pending_guard = pending.lock().map_err(|e| {
                            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string())
                        })?;
                        *pending_guard = Some(bytes);
                        Ok(chunk.to_vec())
                    } else {
                        Ok(bytes.to_vec())
                    }
                }
                Some(Err(err)) => Err(err),
                None => Err(pyo3::exceptions::PyStopAsyncIteration::new_err("")),
            }
        })
    }
}
