//! Unix-domain transport connector.
//!
//! The connector only establishes the Unix stream. Hyper owns HTTP framing,
//! streaming, keep-alive, and response lifecycle exactly as it does for TCP.

#[cfg(unix)]
use std::future::Future;
#[cfg(unix)]
use std::pin::Pin;
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use std::task::{Context, Poll};

#[cfg(unix)]
use http::Uri;
#[cfg(unix)]
use tower_service::Service;

#[cfg(unix)]
use crate::body::{BoxBytesStream, ResponseBody};
#[cfg(unix)]
use crate::error::{Error, Result};
#[cfg(unix)]
use crate::response::Response;

#[cfg(unix)]
pub(crate) enum UdsStream {
    Plain(tokio::net::UnixStream),
    #[cfg(feature = "tls-rustls")]
    Tls(Box<tokio_rustls::client::TlsStream<tokio::net::UnixStream>>),
}

#[cfg(feature = "tls-rustls")]
type TlsConnector = tokio_rustls::TlsConnector;

#[cfg(not(feature = "tls-rustls"))]
type TlsConnector = ();

#[cfg(unix)]
impl tokio::io::AsyncRead for UdsStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_read(cx, buf),
            #[cfg(feature = "tls-rustls")]
            Self::Tls(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

#[cfg(unix)]
impl tokio::io::AsyncWrite for UdsStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_write(cx, bytes),
            #[cfg(feature = "tls-rustls")]
            Self::Tls(stream) => Pin::new(stream).poll_write(cx, bytes),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_flush(cx),
            #[cfg(feature = "tls-rustls")]
            Self::Tls(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(cx),
            #[cfg(feature = "tls-rustls")]
            Self::Tls(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

#[cfg(unix)]
impl hyper_util::client::legacy::connect::Connection for UdsStream {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
        // Inspect the negotiated ALPN protocol on TLS streams so hyper-util
        // knows whether the connection negotiated HTTP/2. Without this
        // signal, an HTTP/2-only legacy client could silently downgrade to
        // HTTP/1.1 even when ALPN selected `h2`.
        #[cfg(feature = "tls-rustls")]
        let mut connected = hyper_util::client::legacy::connect::Connected::new();
        #[cfg(not(feature = "tls-rustls"))]
        let connected = hyper_util::client::legacy::connect::Connected::new();
        #[cfg(feature = "tls-rustls")]
        if let Self::Tls(tls) = self {
            if let Some(alpn) = tls.get_ref().1.alpn_protocol() {
                if alpn == b"h2" {
                    connected = connected.negotiated_h2();
                }
            }
        }
        connected
    }
}

/// Hyper connector for a single Unix socket path.
#[cfg(unix)]
#[derive(Clone)]
pub(crate) struct UdsConnector {
    path: Arc<str>,
    tls: Option<Arc<TlsConnector>>,
    metrics: Option<Arc<crate::transport::metrics::TransportMetrics>>,
}

#[cfg(unix)]
impl UdsConnector {
    #[allow(
        dead_code,
        reason = "kept for tests without metrics; client paths use with_metrics"
    )]
    pub(crate) fn new(path: String, tls: Option<TlsConnector>) -> Self {
        Self {
            path: Arc::from(path),
            tls: tls.map(Arc::new),
            metrics: None,
        }
    }

    pub(crate) fn with_metrics(
        path: String,
        tls: Option<TlsConnector>,
        metrics: Arc<crate::transport::metrics::TransportMetrics>,
    ) -> Self {
        Self {
            path: Arc::from(path),
            tls: tls.map(Arc::new),
            metrics: Some(metrics),
        }
    }
}

#[cfg(unix)]
impl Service<Uri> for UdsConnector {
    type Response = hyper_util::rt::TokioIo<UdsStream>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future =
        Pin<Box<dyn Future<Output = std::result::Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let path = Arc::clone(&self.path);
        #[cfg(feature = "tls-rustls")]
        let tls = self.tls.clone();
        let metrics = self.metrics.clone();
        Box::pin(async move {
            if let Some(ref m) = metrics {
                m.record_uds_attempt();
            }
            let stream = match tokio::net::UnixStream::connect(&*path).await {
                Ok(s) => s,
                Err(e) => {
                    if let Some(ref m) = metrics {
                        m.record_uds_failure();
                    }
                    return Err(Box::new(Error::Connect(format!(
                        "UDS connect to {path} failed: {e}"
                    )))
                        as Box<dyn std::error::Error + Send + Sync>);
                }
            };
            if dst.scheme_str() != Some("https") {
                return Ok(hyper_util::rt::TokioIo::new(UdsStream::Plain(stream)));
            }
            #[cfg(feature = "tls-rustls")]
            let connector = tls.ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
                Error::Tls("HTTPS over UDS requires a TLS connector".into()).into()
            })?;
            #[cfg(not(feature = "tls-rustls"))]
            return Err(Box::new(Error::Tls(
                "HTTPS over UDS requires the tls-rustls feature".into(),
            )) as Box<dyn std::error::Error + Send + Sync>);
            #[cfg(feature = "tls-rustls")]
            let host = dst
                .host()
                .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
                    Error::InvalidUrl("HTTPS over UDS requires an origin host".into()).into()
                })?;
            #[cfg(feature = "tls-rustls")]
            let name = crate::transport::direct_connector::tls_server_name(host).map_err(
                |e| -> Box<dyn std::error::Error + Send + Sync> {
                    Error::Tls(format!("invalid UDS TLS server name '{host}': {e}")).into()
                },
            )?;
            #[cfg(feature = "tls-rustls")]
            let stream = connector.connect(name, stream).await.map_err(
                |e| -> Box<dyn std::error::Error + Send + Sync> {
                    Error::Tls(format!("TLS handshake over UDS failed: {e}")).into()
                },
            )?;
            #[cfg(feature = "tls-rustls")]
            Ok(hyper_util::rt::TokioIo::new(UdsStream::Tls(Box::new(
                stream,
            ))))
        })
    }
}

/// Convert a Hyper response from the UDS client into the core response type.
///
/// Reuses the shared trace lifecycle helpers, [`wrap_incoming`] body
/// adapter, and connector-metadata upgrade helper from the direct path so
/// generic trace/header/body conversion cannot diverge. Error mapping is
/// unified through `map_send_error` so write-timeout and HTTP/2
/// classifications propagate identically.
///
/// UDS 101 upgrades are downcast to the concrete `UdsStream` where the
/// transport kind (`Unix`/`TlsUnix`) is observable without inventing IP
/// addresses; local/peer remain explicitly `None`.
#[cfg(unix)]
pub(crate) async fn send_request(
    client: &crate::transport::TimeoutUdsClient,
    request: http::Request<crate::transport::HyperRequestBody>,
    url: url::Url,
    trace: Option<&dyn crate::trace::TraceObserver>,
) -> Result<Response> {
    crate::transport::direct::emit_send_start(
        trace,
        request.method().as_str(),
        &request.uri().to_string(),
    )?;

    let result = client
        .request(request)
        .await
        .map_err(crate::transport::direct::map_send_error);

    match result {
        Ok(mut response) => {
            let status = response.status().as_u16();
            let version = response.version();
            let headers = response.headers().clone();

            crate::transport::direct::emit_receive_complete(trace, status);

            // Capture upgrade future before consuming the body (same
            // lifecycle as the direct path).
            let on_upgrade = hyper::upgrade::on(&mut response);
            let upgrading = crate::transport::direct::is_upgrade_status(status);

            if upgrading {
                let body = ResponseBody::buffered(bytes::Bytes::new());
                let mut core_response = Response::new(
                    http::StatusCode::from_u16(status).unwrap_or(http::StatusCode::OK),
                    version,
                    headers,
                    url,
                    body,
                );
                core_response.set_trailers(crate::body::SharedTrailers::new());
                if let Some(stream) = crate::transport::direct::await_upgrade(on_upgrade).await {
                    core_response
                        .set_network_stream(crate::network_stream::NetworkStream::Upgraded(stream));
                }
                Ok(core_response)
            } else {
                let trailers = crate::body::SharedTrailers::new();
                let body: BoxBytesStream =
                    crate::transport::direct::wrap_incoming(response.into_body(), trailers.clone());
                let mut core_response = Response::new(
                    http::StatusCode::from_u16(status).unwrap_or(http::StatusCode::OK),
                    version,
                    headers,
                    url,
                    ResponseBody::streaming(body),
                );
                core_response.set_trailers(trailers);
                Ok(core_response)
            }
        }
        Err(e) => {
            crate::transport::direct::emit_send_failed(trace);
            Err(e)
        }
    }
}

// Unix domain sockets are unsupported on this platform. The non-Unix
// dispatch site in `pipeline.rs` returns `Error::Unsupported` directly,
// so no fallback helper lives here.
