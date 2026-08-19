//! Python network stream wrappers.
//!
//! Provides sync and async wrappers for the core [`UpgradedStream`]
//! and [`ConnectionMetadata`] types. These establish the HTTPX/httpcore
//! method names for `network_stream` compatibility.
//!
//! The sync [`PyNetworkStream`] executes IO on the explicit runtime
//! handle carried by the wrapper, rather than relying on the ambient
//! [`tokio::runtime::Handle::current`]. The async [`PyAsyncNetworkStream`]
//! uses [`pyo3_async_runtimes::tokio::future_into_py`] to bridge Rust
//! futures into Python coroutines; the IO is dispatched on the
//! shared Tokio runtime that drives the client's async engine.

use std::sync::{Arc, Mutex};

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

use crate::errors::map_err;
use crate::streaming::RuntimeLease;

/// Classification of an [`UpgradedStream`] for [`start_tls`](UpgradedStream::start_tls) support.
///
/// Hyper's `Upgraded` adapter type is opaque and cannot be unwrapped to
/// a concrete `TcpStream`; only inner `Tcp` variants support `start_tls`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamVariant {
    /// Plain TCP — supports `start_tls`.
    Tcp,
    /// Already TLS — `start_tls` is rejected.
    Tls,
    /// Hyper opaque adapter — `start_tls` is rejected.
    Adapter,
}

impl From<eggfetch_core::network_stream::UpgradedStreamVariant> for StreamVariant {
    fn from(value: eggfetch_core::network_stream::UpgradedStreamVariant) -> Self {
        match value {
            eggfetch_core::network_stream::UpgradedStreamVariant::Tcp => StreamVariant::Tcp,
            eggfetch_core::network_stream::UpgradedStreamVariant::Tls => StreamVariant::Tls,
            eggfetch_core::network_stream::UpgradedStreamVariant::Adapter => StreamVariant::Adapter,
        }
    }
}

/// Type alias for the sync-side stream lock shared between the sync
/// and async wrappers. The sync wrapper takes a `std::Mutex` lock for
/// Type alias kept for documentation; the sync and async wrappers each
/// own their own `Arc<Mutex<Option<UpgradedStream>>>` (sync = std,
/// async = tokio). They cannot share the lock because `std::sync::MutexGuard`
/// is not `Send` across the async boundary.
#[allow(dead_code)]
pub(crate) type SharedStreamInner =
    Arc<Mutex<Option<eggfetch_core::network_stream::UpgradedStream>>>;

/// A Python-accessible network stream handle.
///
/// For upgraded connections (101 Switching Protocols), provides full
/// read/write/close access. For ordinary pooled connections, provides
/// read-only metadata.
///
/// This is the core of the HTTPX `network_stream` compatibility layer.
/// The sync wrapper carries an explicit runtime handle so it can drive
/// Tokio futures without relying on an ambient runtime.
#[pyclass(name = "NetworkStream")]
pub struct PyNetworkStream {
    /// Inner upgraded stream, wrapped in `Arc<Mutex<>>` so multiple
    /// clones share the same IO. ``None`` for metadata-only handles.
    inner: SharedStreamInner,
    /// Connection metadata.
    metadata: Option<Arc<eggfetch_core::network_stream::ConnectionMetadata>>,
    /// Concrete variant of the inner stream — used to refuse `start_tls`
    /// on unsupported types. `None` for metadata-only handles.
    variant: Option<StreamVariant>,
    /// Explicit runtime handle that drives all IO operations.
    runtime_handle: tokio::runtime::Handle,
    /// Optional runtime lease that keeps the runtime alive after the
    /// owning client is dropped. Only populated when the stream was
    /// constructed from a sync client that wants to outlive the client.
    runtime_lease: Option<RuntimeLease>,
}

impl std::fmt::Debug for PyNetworkStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let is_upgraded = self.inner.lock().is_ok_and(|g| g.is_some());
        f.debug_struct("PyNetworkStream")
            .field("is_upgraded", &is_upgraded)
            .field("metadata", &self.metadata)
            .field("variant", &self.variant)
            .field("runtime_handle", &"<elided>")
            .field("runtime_lease", &self.runtime_lease.is_some())
            .finish()
    }
}

impl Clone for PyNetworkStream {
    /// Cloning a network stream shares the underlying IO. The clone
    /// operates on the same `Arc<Mutex<Option<UpgradedStream>>>` so a
    /// upgrade performed on either handle is visible to the other.
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            metadata: self.metadata.clone(),
            variant: self.variant,
            runtime_handle: self.runtime_handle.clone(),
            runtime_lease: self.runtime_lease.clone(),
        }
    }
}

impl PyNetworkStream {
    /// Create a metadata-only network stream (no IO access).
    pub fn from_metadata(metadata: Arc<eggfetch_core::network_stream::ConnectionMetadata>) -> Self {
        let runtime_handle = tokio::runtime::Handle::current();
        Self {
            inner: Arc::new(Mutex::new(None)),
            metadata: Some(metadata),
            variant: None,
            runtime_handle,
            runtime_lease: None,
        }
    }

    /// Create from an upgraded stream (full IO access).
    ///
    /// Uses the ambient runtime handle; preferred for callers that do not
    /// need to outlive a particular client.
    pub fn from_upgraded(upgraded: eggfetch_core::network_stream::UpgradedStream) -> Self {
        let variant = upgraded.variant().into();
        let metadata = upgraded.metadata().clone();
        let runtime_handle = tokio::runtime::Handle::current();
        Self {
            inner: Arc::new(Mutex::new(Some(upgraded))),
            metadata: Some(metadata),
            variant: Some(variant),
            runtime_handle,
            runtime_lease: None,
        }
    }

    /// Create from an upgraded stream with an explicit runtime handle and
    /// optional lease to keep the runtime alive after the client is dropped.
    pub(crate) fn from_upgraded_with_handle(
        upgraded: eggfetch_core::network_stream::UpgradedStream,
        runtime_handle: tokio::runtime::Handle,
        runtime_lease: Option<RuntimeLease>,
    ) -> Self {
        let variant = upgraded.variant().into();
        let metadata = upgraded.metadata().clone();
        Self {
            inner: Arc::new(Mutex::new(Some(upgraded))),
            metadata: Some(metadata),
            variant: Some(variant),
            runtime_handle,
            runtime_lease,
        }
    }

    /// Create a sync wrapper that shares its `UpgradedStream` with an
    /// async wrapper. Both wrappers hold an `Arc<>` to the same
    /// `Mutex<Option<UpgradedStream>>` so the underlying IO is shared
    /// and one side can drive the read while the other side drives the
    /// write.
    #[allow(dead_code)]
    pub(crate) fn from_shared_inner(
        inner: SharedStreamInner,
        variant: StreamVariant,
        metadata: Arc<eggfetch_core::network_stream::ConnectionMetadata>,
        runtime_handle: tokio::runtime::Handle,
        runtime_lease: Option<RuntimeLease>,
    ) -> Self {
        Self {
            inner,
            metadata: Some(metadata),
            variant: Some(variant),
            runtime_handle,
            runtime_lease,
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

        let handle = self.runtime_handle.clone();
        let result = if let Some(secs) = timeout {
            let dur = std::time::Duration::from_secs_f64(secs);
            py.allow_threads(|| {
                handle.block_on(async { tokio::time::timeout(dur, inner.read(max_bytes)).await })
            })
        } else {
            Ok(py.allow_threads(|| handle.block_on(inner.read(max_bytes))))
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
    ///     `timeout`: Write timeout in seconds (optional).
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
        let handle = self.runtime_handle.clone();
        let result = if let Some(secs) = timeout {
            let dur = std::time::Duration::from_secs_f64(secs);
            py.allow_threads(|| {
                handle.block_on(async { tokio::time::timeout(dur, inner.write_all(&buf)).await })
            })
        } else {
            Ok(py.allow_threads(|| handle.block_on(inner.write_all(&buf))))
        };

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(map_err(e)),
            Err(_) => Err(pyo3::exceptions::PyTimeoutError::new_err("write timed out")),
        }
    }

    /// Close the stream (idempotent).
    fn close(&self, py: Python<'_>) {
        let Ok(mut guard) = self.inner.lock() else {
            return;
        };
        let Some(inner) = guard.as_mut() else {
            return;
        };
        let handle = self.runtime_handle.clone();
        let _ = py.allow_threads(|| handle.block_on(inner.close()));
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
    pub fn is_upgraded(&self) -> bool {
        self.inner.lock().is_ok_and(|g| g.is_some())
    }

    /// Upgrade this stream to TLS.
    ///
    /// Wraps the inner TCP stream with a new TLS layer. The supplied
    /// `ssl_context` is translated through the Corrective 01 safe
    /// `SSLContext` translation boundary; only the supported subset of
    /// `SSLContext` policies can be represented. Adapter-based streams
    /// (from Hyper's 101 upgrade) are rejected because the concrete
    /// type cannot be recovered.
    ///
    /// This method is provided for API compatibility with HTTPX's
    /// `network_stream.start_tls()`. It will return an error for
    /// streams obtained from 101 Switching Protocols responses,
    /// which use Hyper's opaque adapter internally.
    ///
    /// Args:
    ///     `ssl_context`: An `ssl.SSLContext` (or `None` for default).
    ///     `server_hostname`: TLS server name for SNI.
    ///     `timeout`: TLS handshake timeout in seconds (optional).
    ///
    /// Returns:
    ///     `NetworkStream`: A new TLS-wrapped network stream.
    ///
    /// Raises:
    ///     `ValueError`: If the stream does not support TLS upgrade.
    ///     `TypeError`: If the `SSLContext` cannot be translated.
    #[pyo3(signature = (ssl_context, server_hostname, *, timeout=None))]
    fn start_tls(
        &self,
        py: Python<'_>,
        ssl_context: Option<&Bound<'_, PyAny>>,
        server_hostname: &str,
        timeout: Option<f64>,
    ) -> PyResult<PyNetworkStream> {
        // Reject variants that cannot be safely upgraded.
        match self.variant {
            Some(StreamVariant::Tcp) => {}
            Some(StreamVariant::Tls) => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "cannot start TLS on an already-TLS-wrapped stream",
                ));
            }
            Some(StreamVariant::Adapter) => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "cannot start TLS on a Hyper adapter-backed stream; the underlying \
                     TCP socket is not recoverable from the upgrade future",
                ));
            }
            None => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "cannot start TLS on a metadata-only network stream",
                ));
            }
        }

        // Translate the SSLContext through Corrective 01: this applies
        // the registry fingerprint check, the post-construction mutation
        // detection, and the unsafe-representation rejection.
        let tls_config = match ssl_context {
            Some(ctx) if !ctx.is_none() => {
                let cfg = crate::tls::ssl_context_to_tls_config(py, Some(ctx))?;
                cfg.ok_or_else(|| {
                    pyo3::exceptions::PyTypeError::new_err(
                        "eggfetch cannot safely translate this ssl.SSLContext; \
                         use eggfetch.compat.httpx.create_ssl_context() or pass \
                         verify/cert kwargs directly",
                    )
                })?
            }
            _ => eggfetch_core::TlsConfig::builder().build(),
        };

        // Build a TlsConnector from the translated config.
        let connector = tls_config.tls_connector().map_err(map_err)?;

        // Take ownership of the inner stream irreversibly.
        let mut guard = self.inner.lock().map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("lock poisoned: {e}"))
        })?;
        let inner = guard.take().ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(
                "cannot start TLS on an already-closed or metadata-only network stream",
            )
        })?;

        let server_name_owned = server_hostname.to_owned();
        let handle = self.runtime_handle.clone();
        let result = py.allow_threads(|| {
            let dur = timeout.map(std::time::Duration::from_secs_f64);
            let handshake = inner.start_tls(&connector, &server_name_owned);
            match dur {
                Some(d) => handle.block_on(async { tokio::time::timeout(d, handshake).await }),
                None => Ok(handle.block_on(handshake)),
            }
        });

        match result {
            Ok(Ok(upgraded)) => Ok(PyNetworkStream::from_upgraded_with_handle(
                upgraded, handle, None,
            )),
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

/// Internal enum that holds either a sync or async network stream wrapper.
///
/// This allows callers to construct the correct wrapper based on whether
/// they are in a sync or async context, avoiding deadlocks when the
/// wrong wrapper type is used.
#[derive(Debug, Clone)]
pub(crate) enum EitherNetworkStream {
    /// Sync wrapper — uses `block_on` for IO; suitable for sync callers.
    Sync(PyNetworkStream),
    /// Async wrapper — uses `pyo3_async_runtimes` for IO; suitable for async callers.
    Async(PyAsyncNetworkStream),
}

impl EitherNetworkStream {
    /// Returns `true` if the inner stream is an upgraded connection with IO access.
    pub fn is_upgraded(&self) -> bool {
        match self {
            Self::Sync(s) => s.is_upgraded(),
            Self::Async(s) => s.is_upgraded(),
        }
    }

    /// Insert the inner stream object into a Python dict as `network_stream`.
    ///
    /// For sync streams, inserts the `PyNetworkStream` directly.
    /// For async streams, inserts the `PyAsyncNetworkStream` directly.
    pub fn insert_into_dict<'py>(
        &self,
        py: Python<'py>,
        dict: &Bound<'py, PyDict>,
    ) -> PyResult<()> {
        match self {
            Self::Sync(s) => {
                if s.is_upgraded() {
                    dict.set_item("network_stream", s.clone())?;
                } else {
                    dict.set_item("network_stream", py.None())?;
                }
            }
            Self::Async(s) => {
                if s.is_upgraded() {
                    dict.set_item("network_stream", s.clone())?;
                } else {
                    dict.set_item("network_stream", py.None())?;
                }
            }
        }
        Ok(())
    }
}

/// Type alias for the async-side stream lock.
///
/// `tokio::sync::Mutex` is used (rather than `std::sync::Mutex`) so the
/// guard can be held across `await` points inside the async futures.
/// The inner stream is `Send` so the lock itself is `Send`.
type AsyncStreamLock =
    Arc<tokio::sync::Mutex<Option<eggfetch_core::network_stream::UpgradedStream>>>;

/// An async Python network stream handle.
///
/// Provides genuinely awaitable `read`, `write`, `aclose` methods that
/// bridge Tokio futures into Python coroutines through
/// [`pyo3_async_runtimes::tokio::future_into_py`]. The Tokio IO is
/// dispatched on the shared runtime handle carried by the wrapper; the
/// inner stream is locked for the duration of the awaited futures.
#[pyclass(name = "AsyncNetworkStream")]
#[derive(Clone)]
pub struct PyAsyncNetworkStream {
    /// Inner upgraded stream, wrapped in `Arc<tokio::sync::Mutex<>>` so
    /// the async future can hold the lock while it is in-flight on the
    /// Tokio runtime. `None` for metadata-only handles.
    inner: AsyncStreamLock,
    /// Connection metadata.
    metadata: Option<Arc<eggfetch_core::network_stream::ConnectionMetadata>>,
    /// Concrete variant of the inner stream — used to refuse `start_tls`
    /// on unsupported types. `None` for metadata-only handles.
    variant: Option<StreamVariant>,
    /// Runtime handle that drives the underlying async engine. Reserved
    /// for future hook points (e.g. dispatching across an explicit
    /// runtime); the `tokio::sync::Mutex` lock path currently picks
    /// up the ambient runtime through `pyo3_async_runtimes`.
    #[allow(dead_code)]
    runtime_handle: tokio::runtime::Handle,
}

impl std::fmt::Debug for PyAsyncNetworkStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let is_upgraded = self.inner.try_lock().is_ok_and(|g| g.is_some());
        f.debug_struct("PyAsyncNetworkStream")
            .field("is_upgraded", &is_upgraded)
            .field("metadata", &self.metadata)
            .field("variant", &self.variant)
            .field("runtime_handle", &"<elided>")
            .finish()
    }
}

impl PyAsyncNetworkStream {
    /// Create a metadata-only async network stream (no IO access).
    pub fn from_metadata(metadata: Arc<eggfetch_core::network_stream::ConnectionMetadata>) -> Self {
        let runtime_handle = tokio::runtime::Handle::current();
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(None)),
            metadata: Some(metadata),
            variant: None,
            runtime_handle,
        }
    }

    /// Create from an upgraded stream (full IO access).
    pub fn from_upgraded(upgraded: eggfetch_core::network_stream::UpgradedStream) -> Self {
        let variant = upgraded.variant().into();
        let metadata = upgraded.metadata().clone();
        let runtime_handle = tokio::runtime::Handle::current();
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(Some(upgraded))),
            metadata: Some(metadata),
            variant: Some(variant),
            runtime_handle,
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
    ///     A coroutine resolving to bytes (or empty bytes on EOF).
    #[pyo3(signature = (max_bytes=65536, timeout=None))]
    fn read<'py>(
        &self,
        py: Python<'py>,
        max_bytes: usize,
        timeout: Option<f64>,
    ) -> PyResult<Bound<'py, pyo3::PyAny>> {
        let inner = self.inner.clone();
        let dur = timeout.map(std::time::Duration::from_secs_f64);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            // Acquire the lock inside the async future so the GIL is
            // released while the IO is in-flight.
            let mut guard = inner.lock().await;
            let stream = guard.as_mut().ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(
                    "cannot read from a metadata-only or already-closed network stream",
                )
            })?;
            let result = match dur {
                Some(d) => tokio::time::timeout(d, stream.read(max_bytes))
                    .await
                    .map_err(|_| eggfetch_core::Error::Timeout {
                        phase: eggfetch_core::TimeoutPhase::Read,
                        elapsed: d,
                    })
                    .and_then(|r| r),
                None => stream.read(max_bytes).await,
            };
            let data = result.map_err(crate::errors::map_err)?;
            Ok::<_, PyErr>(data.to_vec())
        })
    }

    /// Write all supplied bytes to the stream (async).
    ///
    /// Args:
    ///     data: Bytes to write.
    ///     `timeout`: Write timeout in seconds (optional).
    #[pyo3(signature = (data, timeout=None))]
    fn write<'py>(
        &self,
        py: Python<'py>,
        data: &Bound<'_, PyBytes>,
        timeout: Option<f64>,
    ) -> PyResult<Bound<'py, pyo3::PyAny>> {
        let buf = data.as_bytes().to_vec();
        let inner = self.inner.clone();
        let dur = timeout.map(std::time::Duration::from_secs_f64);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut guard = inner.lock().await;
            let stream = guard.as_mut().ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(
                    "cannot write to a metadata-only or already-closed network stream",
                )
            })?;
            let result = match dur {
                Some(d) => tokio::time::timeout(d, stream.write_all(&buf))
                    .await
                    .map_err(|_| eggfetch_core::Error::Timeout {
                        phase: eggfetch_core::TimeoutPhase::Write,
                        elapsed: d,
                    })
                    .and_then(|r| r),
                None => stream.write_all(&buf).await,
            };
            result.map_err(crate::errors::map_err)?;
            Ok::<_, PyErr>(())
        })
    }

    /// Async close (idempotent).  Drops the inner stream so subsequent
    /// I/O methods raise a stable mapped error.
    fn aclose<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut guard = inner.lock().await;
            if let Some(mut stream) = guard.take() {
                let _ = stream.close().await;
            }
            Ok::<_, PyErr>(())
        })
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
    pub fn is_upgraded(&self) -> bool {
        self.inner.try_lock().is_ok_and(|g| g.is_some())
    }

    /// Upgrade this stream to TLS (async).
    ///
    /// The supplied `ssl_context` is translated through the Corrective 01
    /// safe boundary before any irreversible ownership transition. The
    /// returned future resolves to a dict with the new stream's metadata;
    /// the caller is expected to wrap it in a new `AsyncNetworkStream`
    /// through the `_wrap_after_tls` helper.
    #[pyo3(signature = (ssl_context, server_hostname, *, timeout=None))]
    fn start_tls<'py>(
        &self,
        py: Python<'py>,
        ssl_context: Option<&Bound<'_, PyAny>>,
        server_hostname: &str,
        timeout: Option<f64>,
    ) -> PyResult<Bound<'py, pyo3::PyAny>> {
        // Reject variants that cannot be safely upgraded.
        match self.variant {
            Some(StreamVariant::Tcp) => {}
            Some(StreamVariant::Tls) => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "cannot start TLS on an already-TLS-wrapped stream",
                ));
            }
            Some(StreamVariant::Adapter) => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "cannot start TLS on a Hyper adapter-backed stream",
                ));
            }
            None => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "cannot start TLS on a metadata-only network stream",
                ));
            }
        }

        let tls_config = match ssl_context {
            Some(ctx) if !ctx.is_none() => {
                let cfg = crate::tls::ssl_context_to_tls_config(py, Some(ctx))?;
                cfg.ok_or_else(|| {
                    pyo3::exceptions::PyTypeError::new_err(
                        "eggfetch cannot safely translate this ssl.SSLContext",
                    )
                })?
            }
            _ => eggfetch_core::TlsConfig::builder().build(),
        };

        let connector = tls_config.tls_connector().map_err(map_err)?;
        let inner = self.inner.clone();
        let server_name_owned = server_hostname.to_owned();
        let dur = timeout.map(std::time::Duration::from_secs_f64);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut guard = inner.lock().await;
            let stream = guard.take().ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(
                    "cannot start TLS on an already-closed or metadata-only network stream",
                )
            })?;
            let handshake = stream.start_tls(&connector, &server_name_owned);
            let result = match dur {
                Some(d) => tokio::time::timeout(d, handshake)
                    .await
                    .map_err(|_| eggfetch_core::Error::Timeout {
                        phase: eggfetch_core::TimeoutPhase::Connect,
                        elapsed: d,
                    })
                    .and_then(|r| r),
                None => handshake.await,
            };
            let upgraded = result.map_err(crate::errors::map_err)?;
            // Wrap the upgraded stream in a new AsyncNetworkStream and
            // return it as a Python object.
            Python::with_gil(|py| {
                let new_stream = PyAsyncNetworkStream::from_upgraded(upgraded);
                Bound::new(py, new_stream).map(|b| b.into_any().unbind())
            })
        })
    }

    fn __repr__(&self) -> String {
        let is_upgraded = self.inner.try_lock().is_ok_and(|g| g.is_some());
        if is_upgraded {
            "<AsyncNetworkStream (upgraded)>".to_string()
        } else {
            "<AsyncNetworkStream (metadata-only)>".to_string()
        }
    }
}
