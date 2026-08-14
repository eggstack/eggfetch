//! Direct TCP connector with socket-level configuration.
//!
//! This connector provides the same DNS resolution + TCP connect flow as
//! `hyper_util::client::legacy::connect::HttpConnector`, but adds support
//! for:
//!
//! - **Socket options**: common TCP options (`TCP_NODELAY`, `SO_KEEPALIVE`,
//!   `SO_RCVBUF`, `SO_SNDBUF`) applied to the socket before connecting via
//!   `tokio::net::TcpSocket`.
//! - **Local address binding**: bind the outbound socket to a specific local
//!   address before connecting to the remote.
//!
//! When no advanced options are configured, callers should use the standard
//! `hyper_rustls::HttpsConnector` path for maximum compatibility and
//! performance.
//!
//! # Socket option representation
//!
//! Socket options are represented as `(level, option, value)` triples,
//! matching HTTPX's tuple-list format. The connector interprets common
//! TCP options by their integer constants:
//!
//! - `IPPROTO_TCP (6)` + `TCP_NODELAY (1)` → `set_nodelay`
//! - `SOL_SOCKET (1)` + `SO_KEEPALIVE (5)` → `set_keepalive`
//! - `SOL_SOCKET (1)` + `SO_RCVBUF (8)` → `set_recv_buffer_size`
//! - `SOL_SOCKET (1)` + `SO_SNDBUF (7)` → `set_send_buffer_size`
//!
//! Unrecognized option triples produce an error. This satisfies the plan
//! requirement that unsupported options are never silently ignored. Callers
//! that need platform-specific socket options beyond the recognized set
//! should use a custom transport.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use http::Uri;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tower_service::Service;

use crate::error::Error;

/// Socket options that the safe connector can apply portably.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketOptionKind {
    /// Disable Nagle's algorithm.
    TcpNoDelay,
    /// Enable or disable TCP keepalive.
    KeepAlive,
    /// Set the receive buffer size.
    ReceiveBuffer,
    /// Set the send buffer size.
    SendBuffer,
}

/// A socket option triple `(level, option, value)`.
///
/// Mirrors the Python `socket` module's tuple representation used by HTTPX.
/// `level` and `option` are integers matching OS socket option constants.
/// `value` is the raw bytes to set.
#[derive(Debug, Clone)]
pub struct SocketOption {
    /// Socket level (e.g., `IPPROTO_TCP`).
    pub level: i32,
    /// Option name within the level (e.g., `TCP_NODELAY`).
    pub option: i32,
    /// Raw option value bytes.
    pub value: Vec<u8>,
    /// Semantic classification performed by the compatibility boundary.
    /// `None` deliberately represents an unsupported tuple and is rejected
    /// before the socket is connected.
    pub kind: Option<SocketOptionKind>,
}

/// Configuration for a direct TCP connection with advanced socket options.
///
/// This is stored in [`ClientInner`](crate::client::ClientInner) when the
/// caller requests local-address binding, socket options, or both. The
/// connector is only used for requests that need these options; ordinary
/// requests continue through the standard hyper connector path.
#[derive(Debug, Clone)]
pub struct DirectConnectorConfig {
    /// Optional local address to bind before connecting.
    pub(crate) local_address: Option<SocketAddr>,
    /// Socket options to apply before connecting.
    pub(crate) socket_options: Vec<SocketOption>,
}

impl DirectConnectorConfig {}

/// Apply a socket option to a `tokio::net::TcpSocket`.
///
/// Recognized options are applied via the socket's typed setters.
/// Unrecognized options return an error (the plan requires that
/// unsupported options are never silently ignored).
fn apply_socket_option(
    socket: &tokio::net::TcpSocket,
    opt: &SocketOption,
) -> std::result::Result<(), Error> {
    if opt.value.len() < 4 {
        return Err(Error::Connect(format!(
            "socket option value too short: expected at least 4 bytes, got {}",
            opt.value.len()
        )));
    }
    match opt.kind {
        // TCP_NODELAY: value is a 4-byte int (non-zero = enabled).
        Some(SocketOptionKind::TcpNoDelay) => {
            let val = i32::from_le_bytes([opt.value[0], opt.value[1], opt.value[2], opt.value[3]]);
            socket
                .set_nodelay(val != 0)
                .map_err(|e| Error::Connect(format!("failed to set TCP_NODELAY: {e}")))?;
        }
        // SO_KEEPALIVE: value is a 4-byte int (non-zero = enabled).
        Some(SocketOptionKind::KeepAlive) => {
            let val = i32::from_le_bytes([opt.value[0], opt.value[1], opt.value[2], opt.value[3]]);
            socket
                .set_keepalive(val != 0)
                .map_err(|e| Error::Connect(format!("failed to set SO_KEEPALIVE: {e}")))?;
        }
        // SO_RCVBUF: value is a 4-byte int (buffer size in bytes).
        Some(SocketOptionKind::ReceiveBuffer) => {
            let val = u32::from_le_bytes([opt.value[0], opt.value[1], opt.value[2], opt.value[3]]);
            socket
                .set_recv_buffer_size(val)
                .map_err(|e| Error::Connect(format!("failed to set SO_RCVBUF: {e}")))?;
        }
        // SO_SNDBUF: value is a 4-byte int (buffer size in bytes).
        Some(SocketOptionKind::SendBuffer) => {
            let val = u32::from_le_bytes([opt.value[0], opt.value[1], opt.value[2], opt.value[3]]);
            socket
                .set_send_buffer_size(val)
                .map_err(|e| Error::Connect(format!("failed to set SO_SNDBUF: {e}")))?;
        }
        // Unrecognized: return error (plan Track 2.4: never silently ignore).
        _ => {
            return Err(Error::Connect(format!(
                "unsupported socket option: level={}, option={}",
                opt.level, opt.option
            )));
        }
    }
    Ok(())
}

/// A connected stream that can be either a raw TCP stream or a TLS stream.
///
/// This enum allows the direct connector to return a single type that
/// implements both `AsyncRead` and `AsyncWrite`, satisfying hyper-util's
/// connector requirements.
pub(crate) enum DirectStream {
    /// A plain TCP stream (for HTTP).
    Tcp(tokio::net::TcpStream),
    /// A TLS-wrapped stream (for HTTPS), boxed to reduce enum size.
    Tls(Box<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>),
}

impl AsyncRead for DirectStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            DirectStream::Tcp(s) => Pin::new(s).poll_read(cx, buf),
            DirectStream::Tls(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for DirectStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match &mut *self {
            DirectStream::Tcp(s) => Pin::new(s).poll_write(cx, buf),
            DirectStream::Tls(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            DirectStream::Tcp(s) => Pin::new(s).poll_flush(cx),
            DirectStream::Tls(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            DirectStream::Tcp(s) => Pin::new(s).poll_shutdown(cx),
            DirectStream::Tls(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

impl hyper_util::client::legacy::connect::Connection for DirectStream {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
        hyper_util::client::legacy::connect::Connected::new()
    }
}

/// A tower service connector that establishes TCP connections with optional
/// socket-level pre-configuration.
///
/// This is used by the hyper client for requests that require advanced
/// socket options or local-address binding. It performs:
///
/// 1. DNS resolution (via `tokio::net::lookup_host`)
/// 2. Socket creation with `tokio::net::TcpSocket`
/// 3. Application of recognized socket options
/// 4. Optional local-address binding
/// 5. TCP connect
/// 6. Optional TLS handshake (for HTTPS)
///
/// The connector is `Clone`-able and can be shared across requests.
#[derive(Clone)]
pub(crate) struct DirectConnector {
    config: DirectConnectorConfig,
    tls: Option<Arc<tokio_rustls::TlsConnector>>,
}

impl DirectConnector {
    /// Create a new direct connector with the given configuration.
    pub(crate) fn new(
        config: DirectConnectorConfig,
        tls: Option<tokio_rustls::TlsConnector>,
    ) -> Self {
        Self {
            config,
            tls: tls.map(Arc::new),
        }
    }
}

impl Service<Uri> for DirectConnector {
    type Response = hyper_util::rt::TokioIo<DirectStream>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let config = self.config.clone();
        let tls = self.tls.clone();

        Box::pin(async move {
            let host = dst
                .host()
                .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
                    Error::Connect("no hostname in URI".into()).into()
                })?
                .to_owned();

            // Determine port: use explicit port or default for scheme.
            let port = dst.port_u16().unwrap_or_else(|| {
                if dst.scheme_str() == Some("https") {
                    443
                } else {
                    80
                }
            });

            let is_https = dst.scheme_str() == Some("https");

            let addresses = tokio::net::lookup_host((host.as_str(), port))
                .await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    Error::Connect(format!("DNS resolution failed for {host}: {e}")).into()
                })?;
            let mut last_error = None;
            let mut tokio_stream = None;
            for addr in addresses {
                if let Some(local_addr) = config.local_address {
                    if local_addr.is_ipv4() != addr.is_ipv4() {
                        continue;
                    }
                }
                let socket = if addr.is_ipv4() {
                    tokio::net::TcpSocket::new_v4()
                } else {
                    tokio::net::TcpSocket::new_v6()
                }
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    Error::Connect(format!("failed to create socket: {e}")).into()
                })?;
                for opt in &config.socket_options {
                    if let Err(error) = apply_socket_option(&socket, opt) {
                        return Err(error.into());
                    }
                }
                if let Some(local_addr) = config.local_address {
                    if let Err(error) = socket.bind(local_addr) {
                        last_error = Some(error.to_string());
                        continue;
                    }
                }
                match socket.connect(addr).await {
                    Ok(stream) => {
                        tokio_stream = Some(stream);
                        break;
                    }
                    Err(error) => last_error = Some(format!("{addr}: {error}")),
                }
            }
            let tokio_stream =
                tokio_stream.ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
                    Error::Connect(format!(
                        "TCP connect to {host}:{port} failed: {}",
                        last_error.unwrap_or_else(|| "no compatible addresses".into())
                    ))
                    .into()
                })?;

            // Set TCP_NODELAY after connect (applies even if not in socket_options).
            tokio_stream.set_nodelay(true).map_err(
                |e| -> Box<dyn std::error::Error + Send + Sync> {
                    Error::Connect(format!("failed to set TCP_NODELAY: {e}")).into()
                },
            )?;

            let direct_stream = if is_https {
                let tls_connector =
                    tls.ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
                        Error::Connect("HTTPS requested but TLS connector not configured".into())
                            .into()
                    })?;

                let server_name = tokio_rustls::rustls::pki_types::ServerName::try_from(
                    host.clone(),
                )
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    Error::Tls(format!("invalid server name '{host}': {e}")).into()
                })?;

                let stream = tls_connector
                    .connect(server_name, tokio_stream)
                    .await
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                        Error::Tls(format!("TLS handshake failed: {e}")).into()
                    })?;

                DirectStream::Tls(Box::new(stream))
            } else {
                DirectStream::Tcp(tokio_stream)
            };
            Ok(hyper_util::rt::TokioIo::new(direct_stream))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_config_is_clone() {
        let config = DirectConnectorConfig {
            local_address: None,
            socket_options: vec![SocketOption {
                level: 0,
                option: 0,
                value: 1i32.to_ne_bytes().to_vec(),
                kind: Some(SocketOptionKind::TcpNoDelay),
            }],
        };
        let _cloned = config.clone();
    }

    #[test]
    fn socket_option_stores_raw_value() {
        let opt = SocketOption {
            level: 0,
            option: 0,
            value: 1i32.to_ne_bytes().to_vec(),
            kind: Some(SocketOptionKind::TcpNoDelay),
        };
        assert_eq!(opt.value.len(), 4);
    }

    #[test]
    fn apply_socket_option_nodelay() {
        let socket = tokio::net::TcpSocket::new_v4().unwrap();
        let opt = SocketOption {
            level: 0,
            option: 0,
            value: 1i32.to_ne_bytes().to_vec(),
            kind: Some(SocketOptionKind::TcpNoDelay),
        };
        apply_socket_option(&socket, &opt).unwrap();
    }

    #[test]
    fn apply_socket_option_keepalive() {
        let socket = tokio::net::TcpSocket::new_v4().unwrap();
        let opt = SocketOption {
            level: 0,
            option: 0,
            value: 1i32.to_ne_bytes().to_vec(),
            kind: Some(SocketOptionKind::KeepAlive),
        };
        apply_socket_option(&socket, &opt).unwrap();
    }

    #[test]
    fn apply_socket_option_unrecognized_returns_error() {
        let socket = tokio::net::TcpSocket::new_v4().unwrap();
        let opt = SocketOption {
            level: 999,
            option: 999,
            value: vec![0, 1, 2, 3],
            kind: None,
        };
        let err = apply_socket_option(&socket, &opt).unwrap_err();
        assert_eq!(err.kind(), "connect");
        assert!(err.to_string().contains("unsupported socket option"));
    }

    #[test]
    fn apply_socket_option_short_value_returns_error() {
        let socket = tokio::net::TcpSocket::new_v4().unwrap();
        let opt = SocketOption {
            level: 0,
            option: 0,
            value: vec![1],
            kind: Some(SocketOptionKind::TcpNoDelay),
        };
        let err = apply_socket_option(&socket, &opt).unwrap_err();
        assert_eq!(err.kind(), "connect");
        assert!(err.to_string().contains("too short"));
    }
}
