//! Python network stream wrappers.
//!
//! Provides sync and async wrappers for the core `UpgradedStream`
//! and `ConnectionMetadata` types. These establish the HTTPX/httpcore
//! method names for `network_stream` compatibility.
//!
//! The sync `NetworkStream` uses `block_on` with GIL release for
//! blocking IO operations. The async `AsyncNetworkStream` uses
//! `pyo3_async_runtimes` to bridge Rust futures into Python coroutines
//! without nested runtimes.

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
    /// - `"ssl_version"`: TLS version string (e.g. `"TLSv1.3"`), or None.
    /// - `"ssl_cipher"`: negotiated cipher suite, or None.
    /// - `"ssl_alpn"`: negotiated ALPN protocol, or None.
    /// - `"ssl_server_name"`: SNI server name, or None.
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
            "ssl_version" => meta.tls_info.as_ref()?.tls_version.clone(),
            "ssl_cipher" => meta.tls_info.as_ref()?.cipher_suite.clone(),
            "ssl_alpn" => meta.tls_info.as_ref()?.alpn_protocol.clone(),
            "ssl_server_name" => meta.tls_info.as_ref()?.server_name.clone(),
            _ => None,
        }
    }

    /// Returns whether this is an upgraded stream with IO access.
    #[getter]
    fn is_upgraded(&self) -> bool {
        self.inner.lock().is_ok_and(|g| g.is_some())
    }

    /// Upgrade this stream to TLS.
    ///
    /// Wraps the inner TCP stream with a new TLS layer. Only works
    /// for upgraded streams backed by a concrete `TcpStream`. Adapter-based
    /// streams (from Hyper's 101 upgrade) return an error because the
    /// concrete type cannot be recovered.
    ///
    /// This method is provided for API compatibility with HTTPX's
    /// `network_stream.start_tls()`. It will return an error for
    /// streams obtained from 101 Switching Protocols responses,
    /// which use Hyper's opaque adapter internally.
    ///
    /// Args:
    ///     `ssl_context`: Reserved for future use (currently unused).
    ///     `server_hostname`: TLS server name for SNI.
    ///     `timeout`: TLS handshake timeout in seconds (optional).
    ///
    /// Returns:
    ///     `NetworkStream`: A new TLS-wrapped network stream.
    ///
    /// Raises:
    ///     `ValueError`: If the stream does not support TLS upgrade.
    #[pyo3(signature = (server_hostname, *, timeout=None))]
    fn start_tls(
        &self,
        py: Python<'_>,
        server_hostname: &str,
        timeout: Option<f64>,
    ) -> PyResult<PyNetworkStream> {
        let mut guard = self.inner.lock().map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("lock poisoned: {e}"))
        })?;
        let inner = guard.take().ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(
                "cannot start TLS on a metadata-only network stream",
            )
        })?;

        // Build a default TLS connector with system root certificates.
        let mut root_store = tokio_rustls::rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls_config = tokio_rustls::rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        let tls_connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(tls_config));

        let server_name_owned = server_hostname.to_owned();
        let result = py.allow_threads(|| {
            let dur = timeout.map(std::time::Duration::from_secs_f64);
            let handshake = inner.start_tls(&tls_connector, &server_name_owned);
            match dur {
                Some(d) => tokio::runtime::Handle::current()
                    .block_on(async { tokio::time::timeout(d, handshake).await }),
                None => Ok(tokio::runtime::Handle::current().block_on(handshake)),
            }
        });

        match result {
            Ok(Ok(upgraded)) => Ok(PyNetworkStream::from_upgraded(upgraded)),
            Ok(Err(e)) => Err(crate::errors::map_err(e)),
            Err(_) => Err(pyo3::exceptions::PyTimeoutError::new_err(
                "TLS handshake timed out",
            )),
        }
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

// ---------------------------------------------------------------------------
// AsyncNetworkStream
// ---------------------------------------------------------------------------

/// An async Python network stream handle.
///
/// Provides the same interface as `NetworkStream` but with async methods
/// that bridge into Python coroutines. Internally delegates to the sync
/// operations because `UpgradedStream` is not `Send` (it contains a
/// pinned trait object), so it cannot cross tokio spawn boundaries.
///
/// For upgraded connections (101/CONNECT), provides full async
/// read/write/close access. For ordinary pooled connections, provides
/// read-only metadata.
#[pyclass(name = "AsyncNetworkStream")]
pub struct PyAsyncNetworkStream {
    /// Inner upgraded stream, wrapped in Mutex for Sync.
    /// `None` for metadata-only handles.
    inner: Mutex<Option<eggfetch_core::network_stream::UpgradedStream>>,
    /// Connection metadata.
    metadata: Option<std::sync::Arc<eggfetch_core::network_stream::ConnectionMetadata>>,
}

impl PyAsyncNetworkStream {
    /// Create a metadata-only async network stream (no IO access).
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
impl PyAsyncNetworkStream {
    /// Read up to `max_bytes` from the stream (async).
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
        let data: bytes::Bytes = self.with_inner(|inner| {
            if let Some(secs) = timeout {
                let dur = std::time::Duration::from_secs_f64(secs);
                tokio::runtime::Handle::current()
                    .block_on(async { tokio::time::timeout(dur, inner.read(max_bytes)).await })
                    .map_err(|_| eggfetch_core::Error::Timeout {
                        phase: eggfetch_core::TimeoutPhase::Read,
                        elapsed: dur,
                    })
                    .and_then(|r| r)
            } else {
                tokio::runtime::Handle::current().block_on(inner.read(max_bytes))
            }
        })?;
        Ok(PyBytes::new(py, &data))
    }

    /// Write all supplied bytes to the stream (async).
    ///
    /// Args:
    ///     data: Bytes to write.
    ///     timeout: Write timeout in seconds (optional).
    #[pyo3(signature = (data, timeout=None))]
    fn write(
        &self,
        _py: Python<'_>,
        data: &Bound<'_, PyBytes>,
        timeout: Option<f64>,
    ) -> PyResult<()> {
        let buf = data.as_bytes().to_vec();
        self.with_inner(|inner| {
            if let Some(secs) = timeout {
                let dur = std::time::Duration::from_secs_f64(secs);
                tokio::runtime::Handle::current()
                    .block_on(async { tokio::time::timeout(dur, inner.write_all(&buf)).await })
                    .map_err(|_| eggfetch_core::Error::Timeout {
                        phase: eggfetch_core::TimeoutPhase::Write,
                        elapsed: dur,
                    })
                    .and_then(|r| r)
            } else {
                tokio::runtime::Handle::current().block_on(inner.write_all(&buf))
            }
        })
    }

    /// Close the stream (async, idempotent).
    fn close(&self) -> PyResult<()> {
        self.with_inner(|inner| tokio::runtime::Handle::current().block_on(inner.close()))
    }

    /// Get extra information about the connection.
    ///
    /// Supported keys:
    /// - `"client_addr"`: local socket address as string.
    /// - `"server_addr"`: remote socket address as string.
    /// - `"ssl_version"`: TLS version string, or None.
    /// - `"ssl_cipher"`: negotiated cipher suite, or None.
    /// - `"ssl_alpn"`: negotiated ALPN protocol, or None.
    /// - `"ssl_server_name"`: SNI server name, or None.
    fn get_extra_info(&self, key: &str) -> Option<String> {
        let meta = self.metadata.as_ref()?;
        match key {
            "client_addr" => meta.local_addr.map(|a| a.to_string()),
            "server_addr" => meta.peer_addr.map(|a| a.to_string()),
            "ssl_version" => meta.tls_info.as_ref()?.tls_version.clone(),
            "ssl_cipher" => meta.tls_info.as_ref()?.cipher_suite.clone(),
            "ssl_alpn" => meta.tls_info.as_ref()?.alpn_protocol.clone(),
            "ssl_server_name" => meta.tls_info.as_ref()?.server_name.clone(),
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
            "<AsyncNetworkStream (upgraded)>".to_string()
        } else {
            "<AsyncNetworkStream (metadata-only)>".to_string()
        }
    }
}

impl PyAsyncNetworkStream {
    /// Helper to acquire the lock and access the inner stream.
    ///
    /// Unlike the sync `PyNetworkStream`, this does NOT release the GIL
    /// because `UpgradedStream` is `!Send` (contains `Pin<Box<dyn TokioIoBox>>`)
    /// and cannot cross the `allow_threads` boundary.
    fn with_inner<T>(
        &self,
        f: impl FnOnce(&mut eggfetch_core::network_stream::UpgradedStream) -> eggfetch_core::Result<T>,
    ) -> PyResult<T> {
        let mut guard = self.inner.lock().map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("lock poisoned: {e}"))
        })?;
        let inner = guard.as_mut().ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "cannot perform IO on a metadata-only network stream",
            )
        })?;
        f(inner).map_err(map_err)
    }
}
