//! Python network stream wrappers.
//!
//! Provides sync and async wrappers for the core `UpgradedStream`
//! and `ConnectionMetadata` types. These establish the HTTPX/httpcore
//! method names for `network_stream` compatibility.

use std::sync::Mutex;

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

use crate::errors::map_err;

/// A Python-accessible network stream handle.
///
/// For upgraded connections (101/CONNECT), provides full read/write/close
/// access. For ordinary pooled connections, provides read-only metadata.
///
/// This is the core of the HTTPX `network_stream` compatibility layer.
#[pyclass(name = "NetworkStream")]
pub struct PyNetworkStream {
    /// Inner upgraded stream, wrapped in Mutex for Sync.
    /// `None` for metadata-only handles.
    inner: Mutex<Option<eggfetch_core::network_stream::UpgradedStream>>,
    /// Connection metadata.
    metadata: Option<std::sync::Arc<eggfetch_core::network_stream::ConnectionMetadata>>,
}

impl std::fmt::Debug for PyNetworkStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let is_upgraded = self.inner.lock().is_ok_and(|g| g.is_some());
        f.debug_struct("PyNetworkStream")
            .field("is_upgraded", &is_upgraded)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl Clone for PyNetworkStream {
    /// Cloning a network stream creates a metadata-only copy.
    /// The underlying IO cannot be cloned; the clone loses IO access.
    fn clone(&self) -> Self {
        Self {
            inner: Mutex::new(None),
            metadata: self.metadata.clone(),
        }
    }
}

impl PyNetworkStream {
    /// Create a metadata-only network stream (no IO access).
    pub fn from_metadata(
        metadata: std::sync::Arc<eggfetch_core::network_stream::ConnectionMetadata>,
    ) -> Self {
        Self {
            inner: Mutex::new(None),
            metadata: Some(metadata),
        }
    }

    /// Create from an upgraded stream (full IO access).
    pub fn from_upgraded(upgraded: eggfetch_core::network_stream::UpgradedStream) -> Self {
        let metadata = upgraded.metadata().clone();
        Self {
            inner: Mutex::new(Some(upgraded)),
            metadata: Some(metadata),
        }
    }
}

#[pymethods]
impl PyNetworkStream {
    /// Read up to `max_bytes` from the stream.
    ///
    /// Args:
    ///     `max_bytes`: Maximum bytes to read (default 65536).
    ///     `timeout`: Read timeout in seconds (optional).
    ///
    /// Returns:
    ///     bytes: Data read from the stream, or empty bytes on EOF.
    #[pyo3(signature = (max_bytes=65536, timeout=None))]
    fn read<'py>(
        &self,
        py: Python<'py>,
        max_bytes: usize,
        timeout: Option<f64>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let mut guard = self.inner.lock().map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("lock poisoned: {e}"))
        })?;
        let inner = guard.as_mut().ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(
                "cannot read from a metadata-only network stream",
            )
        })?;

        let result = if let Some(secs) = timeout {
            let dur = std::time::Duration::from_secs_f64(secs);
            py.allow_threads(|| {
                tokio::runtime::Handle::current()
                    .block_on(async { tokio::time::timeout(dur, inner.read(max_bytes)).await })
            })
        } else {
            py.allow_threads(|| {
                Ok(tokio::runtime::Handle::current().block_on(inner.read(max_bytes)))
            })
        };

        match result {
            Ok(Ok(data)) => Ok(PyBytes::new(py, &data)),
            Ok(Err(e)) => Err(map_err(e)),
            Err(_) => Err(pyo3::exceptions::PyTimeoutError::new_err("read timed out")),
        }
    }

    /// Write all supplied bytes to the stream.
    ///
    /// Args:
    ///     data: Bytes to write.
    ///     timeout: Write timeout in seconds (optional).
    #[pyo3(signature = (data, timeout=None))]
    fn write(
        &self,
        py: Python<'_>,
        data: &Bound<'_, PyBytes>,
        timeout: Option<f64>,
    ) -> PyResult<()> {
        let mut guard = self.inner.lock().map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("lock poisoned: {e}"))
        })?;
        let inner = guard.as_mut().ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(
                "cannot write to a metadata-only network stream",
            )
        })?;

        let buf = data.as_bytes().to_vec();
        let result = if let Some(secs) = timeout {
            let dur = std::time::Duration::from_secs_f64(secs);
            py.allow_threads(|| {
                tokio::runtime::Handle::current()
                    .block_on(async { tokio::time::timeout(dur, inner.write_all(&buf)).await })
            })
        } else {
            py.allow_threads(|| {
                Ok(tokio::runtime::Handle::current().block_on(inner.write_all(&buf)))
            })
        };

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(map_err(e)),
            Err(_) => Err(pyo3::exceptions::PyTimeoutError::new_err("write timed out")),
        }
    }

    /// Close the stream (idempotent).
    fn close(&self, py: Python<'_>) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(ref mut inner) = *guard {
                let _ =
                    py.allow_threads(|| tokio::runtime::Handle::current().block_on(inner.close()));
            }
        }
    }

    /// Get extra information about the connection.
    ///
    /// Supported keys:
    /// - `"client_addr"`: local socket address as string.
    /// - `"server_addr"`: remote socket address as string.
    ///
    /// Args:
    ///     key: Information key string.
    ///
    /// Returns:
    ///     str or None: The requested information, or None if not available.
    fn get_extra_info(&self, key: &str) -> Option<String> {
        let meta = self.metadata.as_ref()?;
        match key {
            "client_addr" => meta.local_addr.map(|a| a.to_string()),
            "server_addr" => meta.peer_addr.map(|a| a.to_string()),
            _ => None,
        }
    }

    /// Returns whether this is an upgraded stream with IO access.
    #[getter]
    fn is_upgraded(&self) -> bool {
        self.inner.lock().is_ok_and(|g| g.is_some())
    }

    fn __repr__(&self) -> String {
        let is_upgraded = self.inner.lock().is_ok_and(|g| g.is_some());
        if is_upgraded {
            "<NetworkStream (upgraded)>".to_string()
        } else {
            "<NetworkStream (metadata-only)>".to_string()
        }
    }
}

/// A Python dict representing `get_extra_info()` results.
///
/// Mirrors the shape of HTTPX's `network_stream.get_extra_info()` return
/// value for compatibility testing.
#[allow(dead_code)]
#[pyfunction]
fn extra_info_dict<'py>(
    py: Python<'py>,
    metadata: &PyNetworkStream,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    if let Some(ref meta) = metadata.metadata {
        if let Some(addr) = meta.local_addr {
            dict.set_item("client_addr", addr.to_string())?;
        }
        if let Some(addr) = meta.peer_addr {
            dict.set_item("server_addr", addr.to_string())?;
        }
        dict.set_item("transport_kind", format!("{:?}", meta.transport_kind))?;
        if let Some(ref tls) = meta.tls_info {
            let tls_dict = PyDict::new(py);
            if let Some(ref proto) = tls.alpn_protocol {
                tls_dict.set_item("alpn_protocol", proto)?;
            }
            if let Some(ref ver) = tls.tls_version {
                tls_dict.set_item("tls_version", ver)?;
            }
            if let Some(ref cipher) = tls.cipher_suite {
                tls_dict.set_item("cipher_suite", cipher)?;
            }
            if let Some(ref name) = tls.server_name {
                tls_dict.set_item("server_name", name)?;
            }
            dict.set_item("tls_info", tls_dict)?;
        }
    }
    Ok(dict)
}
