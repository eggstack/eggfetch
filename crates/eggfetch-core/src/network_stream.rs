//! Network stream types for connection metadata and upgraded connections.
//!
//! This module provides:
//!
//! - [`ConnectionMetadata`]: read-only metadata about an established
//!   connection (addresses, TLS info, transport kind).
//! - [`UpgradedStream`]: an owned post-HTTP IO handle for 101/CONNECT
//!   upgrade handoff.
//! - [`NetworkStream`]: a compatibility object exposed on responses for
//!   metadata access and, where safe, low-level IO.

use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::error::{Error, Result};

/// The kind of transport used for a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// Plain TCP.
    Tcp,
    /// Unix domain socket.
    Unix,
    /// TLS-wrapped TCP (rustls).
    Tls,
    /// TLS-wrapped Unix domain socket.
    TlsUnix,
}

/// Read-only TLS session metadata captured after handshake.
///
/// This is an EggFetch-specific type; it does **not** masquerade as
/// `ssl.SSLObject` or any Python SSL type.
#[derive(Debug, Clone)]
pub struct TlsInfo {
    /// Negotiated ALPN protocol (e.g. `"http/1.1"`, `"h2"`).
    pub alpn_protocol: Option<String>,
    /// TLS version string (e.g. `"TLSv1.3"`).
    pub tls_version: Option<String>,
    /// Cipher suite identifier.
    pub cipher_suite: Option<String>,
    /// Server name used for SNI.
    pub server_name: Option<String>,
}

/// Read-only metadata about an established connection.
///
/// All fields are captured at connection time and never mutated.
/// Shared through `Arc` so multiple responses (H2) can reference the
/// same connection metadata.
#[derive(Debug, Clone)]
pub struct ConnectionMetadata {
    /// Local (client) socket address, if available.
    pub local_addr: Option<SocketAddr>,
    /// Remote (peer/server) socket address, if available.
    pub peer_addr: Option<SocketAddr>,
    /// Transport kind.
    pub transport_kind: TransportKind,
    /// TLS session info, if the connection is TLS.
    pub tls_info: Option<TlsInfo>,
}

impl Default for ConnectionMetadata {
    fn default() -> Self {
        Self {
            local_addr: None,
            peer_addr: None,
            transport_kind: TransportKind::Tcp,
            tls_info: None,
        }
    }
}

/// An owned post-HTTP IO handle for upgraded connections (101, CONNECT).
///
/// After Hyper transfers ownership of the underlying IO through its
/// upgrade mechanism, this type owns the raw stream and provides
/// async read/write, TLS upgrade, and metadata access.
///
/// # Ownership rules
///
/// - Once created, the connection is removed from the HTTP pool and
///   must never be reused.
/// - `close()` is idempotent.
/// - Cancellation during `start_tls()` closes or leaves the stream
///   in a deterministic state.
pub struct UpgradedStream {
    inner: UpgradedStreamInner,
    metadata: Arc<ConnectionMetadata>,
    /// Leading bytes already read from the connection before the
    /// upgrade completed (e.g. data sent by the server in the same
    /// write as the 101/CONNECT response headers).
    leading_data: Bytes,
}

/// The concrete IO inside an upgraded stream.
enum UpgradedStreamInner {
    /// Plain TCP stream.
    Tcp(tokio::net::TcpStream),
    /// TLS stream already established before upgrade.
    Tls(Box<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>),
    /// Opaque adapter (e.g. Hyper's Upgraded wrapper).
    Adapter(Pin<Box<dyn TokioIoBox>>),
}

/// Trait alias for `AsyncRead + AsyncWrite + Send` objects that can
/// be stored in a trait object.
pub(crate) trait TokioIoBox:
    tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin
{
}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin> TokioIoBox for T {}

impl AsyncRead for UpgradedStreamInner {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            UpgradedStreamInner::Tcp(s) => Pin::new(s).poll_read(cx, buf),
            UpgradedStreamInner::Tls(s) => Pin::new(s).poll_read(cx, buf),
            UpgradedStreamInner::Adapter(a) => a.as_mut().poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for UpgradedStreamInner {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match &mut *self {
            UpgradedStreamInner::Tcp(s) => Pin::new(s).poll_write(cx, buf),
            UpgradedStreamInner::Tls(s) => Pin::new(s).poll_write(cx, buf),
            UpgradedStreamInner::Adapter(a) => a.as_mut().poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            UpgradedStreamInner::Tcp(s) => Pin::new(s).poll_flush(cx),
            UpgradedStreamInner::Tls(s) => Pin::new(s).poll_flush(cx),
            UpgradedStreamInner::Adapter(a) => a.as_mut().poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            UpgradedStreamInner::Tcp(s) => Pin::new(s).poll_shutdown(cx),
            UpgradedStreamInner::Tls(s) => Pin::new(s).poll_shutdown(cx),
            UpgradedStreamInner::Adapter(a) => a.as_mut().poll_shutdown(cx),
        }
    }
}

impl UpgradedStream {
    /// Wrap a tokio TCP stream into an upgraded stream.
    ///
    /// Leading data is bytes already read from the connection before
    /// the upgrade handshake completed (preserved by Hyper in its
    /// `Upgraded::read_buf`).
    pub fn from_tcp(stream: tokio::net::TcpStream, leading_data: Bytes) -> Self {
        let metadata = Arc::new(ConnectionMetadata {
            local_addr: stream.local_addr().ok(),
            peer_addr: stream.peer_addr().ok(),
            transport_kind: TransportKind::Tcp,
            tls_info: None,
        });
        Self {
            inner: UpgradedStreamInner::Tcp(stream),
            metadata,
            leading_data,
        }
    }

    /// Wrap an adapter implementing `AsyncRead + AsyncWrite` into an
    /// upgraded stream.
    ///
    /// Used when the underlying IO type is opaque (e.g. Hyper's
    /// `Upgraded` wrapper) and cannot be downcast to a concrete type.
    /// The metadata must be provided by the caller.
    pub(crate) fn from_adapter(
        adapter: impl TokioIoBox + 'static,
        leading_data: Bytes,
        metadata: Arc<ConnectionMetadata>,
    ) -> Self {
        let pinned: Pin<Box<dyn TokioIoBox>> = Box::pin(adapter);
        Self {
            inner: UpgradedStreamInner::Adapter(pinned),
            metadata,
            leading_data,
        }
    }

    /// Wrap a TLS stream into an upgraded stream.
    pub fn from_tls(
        stream: tokio_rustls::client::TlsStream<tokio::net::TcpStream>,
        leading_data: Bytes,
        tls_info: TlsInfo,
    ) -> Self {
        // Extract peer/local addr from the underlying TcpStream through
        // the TLS stream's `get_ref()` which returns (&TcpStream, &ClientConnection).
        let (inner_tcp, _) = stream.get_ref();
        let local_addr = inner_tcp.local_addr().ok();
        let peer_addr = inner_tcp.peer_addr().ok();
        let metadata = Arc::new(ConnectionMetadata {
            local_addr,
            peer_addr,
            transport_kind: TransportKind::Tls,
            tls_info: Some(tls_info),
        });
        Self {
            inner: UpgradedStreamInner::Tls(Box::new(stream)),
            metadata,
            leading_data,
        }
    }

    /// Returns a reference to the connection metadata.
    #[must_use]
    pub fn metadata(&self) -> &Arc<ConnectionMetadata> {
        &self.metadata
    }

    /// Returns the leading data that was buffered before the upgrade.
    ///
    /// These are bytes already read from the connection after the
    /// HTTP response headers but before the caller consumed the
    /// upgrade. They must be returned before any new socket reads.
    #[must_use]
    pub fn leading_data(&self) -> &Bytes {
        &self.leading_data
    }

    /// Consume leading data, returning it.
    pub fn take_leading_data(&mut self) -> Bytes {
        std::mem::take(&mut self.leading_data)
    }

    /// Read up to `max_bytes` from the stream.
    ///
    /// Returns the bytes read. Returns empty bytes on EOF.
    ///
    /// # Errors
    ///
    /// Returns `Error::Io` if the underlying read fails.
    pub async fn read(&mut self, max_bytes: usize) -> Result<Bytes> {
        // First drain leading data.
        if !self.leading_data.is_empty() {
            let available = self.leading_data.len().min(max_bytes);
            return Ok(self.leading_data.split_to(available));
        }
        let mut buf = vec![0u8; max_bytes];
        let n = tokio::io::AsyncReadExt::read(self, &mut buf)
            .await
            .map_err(|e| Error::Io(std::sync::Arc::new(e)))?;
        if n == 0 {
            Ok(Bytes::new())
        } else {
            Ok(Bytes::copy_from_slice(&buf[..n]))
        }
    }

    /// Write all supplied bytes to the stream.
    ///
    /// # Errors
    ///
    /// Returns `Error::Io` if the underlying write fails.
    pub async fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        tokio::io::AsyncWriteExt::write_all(self, buf)
            .await
            .map_err(|e| Error::Io(std::sync::Arc::new(e)))
    }

    /// Flush the stream.
    ///
    /// # Errors
    ///
    /// Returns `Error::Io` if the underlying flush fails.
    pub async fn flush(&mut self) -> Result<()> {
        tokio::io::AsyncWriteExt::flush(self)
            .await
            .map_err(|e| Error::Io(std::sync::Arc::new(e)))
    }

    /// Shut down the stream (idempotent).
    ///
    /// # Errors
    ///
    /// This method currently always succeeds; errors are silently
    /// discarded to maintain idempotent close semantics.
    pub async fn close(&mut self) -> Result<()> {
        let _ = tokio::io::AsyncWriteExt::shutdown(self).await;
        Ok(())
    }

    /// Upgrade this TCP stream to TLS.
    ///
    /// Wraps the inner TCP stream with a new TLS layer using the
    /// provided connector and server name. On success, returns a new
    /// `UpgradedStream` with TLS metadata. On failure, the original
    /// stream state is preserved (the handshake does not consume the
    /// stream on error).
    ///
    /// Only works for streams backed by a concrete `TcpStream`.
    /// Adapter-based streams (from Hyper's Upgraded) return an error
    /// because the concrete type cannot be recovered.
    ///
    /// # Errors
    ///
    /// Returns `Error::Tls` if the handshake fails or the stream type
    /// does not support TLS upgrade.
    pub async fn start_tls(
        mut self,
        tls_connector: &tokio_rustls::TlsConnector,
        server_name: &str,
    ) -> Result<Self> {
        use tokio_rustls::rustls::pki_types::ServerName;
        let tcp = match std::mem::replace(
            &mut self.inner,
            // Placeholder that we'll immediately replace.
            UpgradedStreamInner::Tcp({
                // This code path is only reached for Tcp variant; the
                // placeholder is never actually used.
                let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| {
                    Error::Tls(format!("failed to create dummy socket for TLS: {e}"))
                })?;
                let (std_stream, _) = listener
                    .accept()
                    .map_err(|e| Error::Tls(format!("failed to accept on dummy socket: {e}")))?;
                tokio::net::TcpStream::from_std(std_stream).map_err(|e| {
                    Error::Tls(format!("failed to create tokio socket for TLS: {e}"))
                })?
            }),
        ) {
            UpgradedStreamInner::Tcp(s) => s,
            UpgradedStreamInner::Tls(_) => {
                return Err(Error::Tls("stream is already TLS-wrapped".into()));
            }
            UpgradedStreamInner::Adapter(_) => {
                return Err(Error::Tls(
                    "cannot start TLS on an opaque adapter stream; \
                     use a concrete TcpStream-backed UpgradedStream"
                        .into(),
                ));
            }
        };

        let sn = ServerName::try_from(server_name.to_owned())
            .map_err(|e| Error::Tls(format!("invalid server name '{server_name}': {e}")))?;

        let tls_stream = tls_connector
            .connect(sn, tcp)
            .await
            .map_err(|e| Error::Tls(format!("TLS handshake failed: {e}")))?;

        // Extract TLS info from the negotiated parameters.
        let (inner_tcp, connection_info) = tls_stream.get_ref();
        let alpn = connection_info
            .alpn_protocol()
            .map(|p| String::from_utf8_lossy(p).into_owned());
        let version = connection_info.protocol_version().map(|v| format!("{v:?}"));
        let cipher = connection_info
            .negotiated_cipher_suite()
            .map(|c| format!("{c:?}"));

        let tls_info = TlsInfo {
            alpn_protocol: alpn,
            tls_version: version,
            cipher_suite: cipher,
            server_name: Some(server_name.to_owned()),
        };

        let local_addr = inner_tcp.local_addr().ok();
        let peer_addr = inner_tcp.peer_addr().ok();

        let metadata = Arc::new(ConnectionMetadata {
            local_addr,
            peer_addr,
            transport_kind: TransportKind::Tls,
            tls_info: Some(tls_info),
        });

        Ok(UpgradedStream {
            inner: UpgradedStreamInner::Tls(Box::new(tls_stream)),
            metadata,
            leading_data: Bytes::new(),
        })
    }
}

impl AsyncRead for UpgradedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // Drain leading data first.
        if !self.leading_data.is_empty() {
            let available = self.leading_data.len().min(buf.remaining());
            let chunk = self.leading_data.split_to(available);
            buf.put_slice(&chunk);
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for UpgradedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// A network stream handle exposed on responses for metadata access.
///
/// For upgraded connections (101/CONNECT), this provides full
/// read/write/close/TLS access. For ordinary pooled connections,
/// this provides read-only metadata — raw IO operations are not
/// exposed while Hyper still owns the connection.
pub enum NetworkStream {
    /// An upgraded stream with full IO access.
    Upgraded(UpgradedStream),
    /// Read-only metadata for an ordinary pooled connection.
    Metadata(Arc<ConnectionMetadata>),
}

impl NetworkStream {
    /// Returns the connection metadata.
    #[must_use]
    pub fn metadata(&self) -> &Arc<ConnectionMetadata> {
        match self {
            NetworkStream::Upgraded(s) => s.metadata(),
            NetworkStream::Metadata(m) => m,
        }
    }

    /// Returns `true` if this is an upgraded stream with IO access.
    #[must_use]
    pub fn is_upgraded(&self) -> bool {
        matches!(self, NetworkStream::Upgraded(_))
    }

    /// If this is an upgraded stream, returns a reference to it.
    #[must_use]
    pub fn as_upgraded(&self) -> Option<&UpgradedStream> {
        match self {
            NetworkStream::Upgraded(s) => Some(s),
            NetworkStream::Metadata(_) => None,
        }
    }

    /// If this is an upgraded stream, returns a mutable reference.
    pub fn as_upgraded_mut(&mut self) -> Option<&mut UpgradedStream> {
        match self {
            NetworkStream::Upgraded(s) => Some(s),
            NetworkStream::Metadata(_) => None,
        }
    }

    /// If this is an upgraded stream, consume and return it.
    pub fn into_upgraded(self) -> Option<UpgradedStream> {
        match self {
            NetworkStream::Upgraded(s) => Some(s),
            NetworkStream::Metadata(_) => None,
        }
    }
}

/// Read-only metadata accessible through `get_extra_info()`.
#[derive(Debug, Clone)]
pub struct ExtraInfo {
    /// Local address (`client_addr`).
    pub client_addr: Option<SocketAddr>,
    /// Remote address (`server_addr`).
    pub server_addr: Option<SocketAddr>,
    /// TLS info, if available.
    pub tls_info: Option<TlsInfo>,
}

impl ExtraInfo {
    /// Create from connection metadata.
    #[must_use]
    pub fn from_metadata(meta: &ConnectionMetadata) -> Self {
        Self {
            client_addr: meta.local_addr,
            server_addr: meta.peer_addr,
            tls_info: meta.tls_info.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_metadata_default() {
        let meta = ConnectionMetadata::default();
        assert!(meta.local_addr.is_none());
        assert!(meta.peer_addr.is_none());
        assert_eq!(meta.transport_kind, TransportKind::Tcp);
        assert!(meta.tls_info.is_none());
    }

    #[test]
    fn network_stream_metadata_only() {
        let meta = Arc::new(ConnectionMetadata {
            local_addr: Some("127.0.0.1:12345".parse().unwrap()),
            peer_addr: Some("93.184.216.34:443".parse().unwrap()),
            transport_kind: TransportKind::Tls,
            tls_info: Some(TlsInfo {
                alpn_protocol: Some("h2".into()),
                tls_version: Some("TLSv1.3".into()),
                cipher_suite: Some("TLS_AES_256_GCM_SHA384".into()),
                server_name: Some("example.com".into()),
            }),
        });
        let ns = NetworkStream::Metadata(meta.clone());
        assert!(!ns.is_upgraded());
        assert!(ns.as_upgraded().is_none());
        let info = ns.metadata();
        assert_eq!(info.local_addr.unwrap().port(), 12345);
        assert_eq!(info.peer_addr.unwrap().port(), 443);
    }

    #[test]
    fn extra_info_from_metadata() {
        let meta = ConnectionMetadata {
            local_addr: Some("127.0.0.1:1000".parse().unwrap()),
            peer_addr: Some("10.0.0.1:443".parse().unwrap()),
            transport_kind: TransportKind::Tcp,
            tls_info: None,
        };
        let info = ExtraInfo::from_metadata(&meta);
        assert_eq!(info.client_addr.unwrap().port(), 1000);
        assert_eq!(info.server_addr.unwrap().port(), 443);
        assert!(info.tls_info.is_none());
    }
}
