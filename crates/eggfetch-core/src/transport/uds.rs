//! Unix-domain transport connector.
//!
//! The connector only establishes the Unix stream. Hyper owns HTTP framing,
//! streaming, keep-alive, and response lifecycle exactly as it does for TCP.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use http::Uri;
use tower_service::Service;

use crate::body::{BoxBytesStream, ResponseBody};
use crate::error::{Error, Result};
use crate::response::Response;

#[cfg(unix)]
pub(crate) enum UdsStream {
    Plain(tokio::net::UnixStream),
    Tls(Box<tokio_rustls::client::TlsStream<tokio::net::UnixStream>>),
}

#[cfg(unix)]
impl tokio::io::AsyncRead for UdsStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_read(cx, buf),
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
            Self::Tls(stream) => Pin::new(stream).poll_write(cx, bytes),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_flush(cx),
            Self::Tls(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Tls(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

#[cfg(unix)]
impl hyper_util::client::legacy::connect::Connection for UdsStream {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
        hyper_util::client::legacy::connect::Connected::new()
    }
}

/// Hyper connector for a single Unix socket path.
#[cfg(unix)]
#[derive(Clone)]
pub(crate) struct UdsConnector {
    path: Arc<str>,
    tls: Option<Arc<tokio_rustls::TlsConnector>>,
}

#[cfg(unix)]
impl UdsConnector {
    pub(crate) fn new(path: String, tls: Option<tokio_rustls::TlsConnector>) -> Self {
        Self {
            path: Arc::from(path),
            tls: tls.map(Arc::new),
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
        let tls = self.tls.clone();
        Box::pin(async move {
            let stream = tokio::net::UnixStream::connect(&*path).await.map_err(
                |e| -> Box<dyn std::error::Error + Send + Sync> {
                    Error::Connect(format!("UDS connect to {path} failed: {e}")).into()
                },
            )?;
            if dst.scheme_str() != Some("https") {
                return Ok(hyper_util::rt::TokioIo::new(UdsStream::Plain(stream)));
            }
            let connector = tls.ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
                Error::Tls("HTTPS over UDS requires a TLS connector".into()).into()
            })?;
            let host = dst
                .host()
                .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
                    Error::InvalidUrl("HTTPS over UDS requires an origin host".into()).into()
                })?;
            let name = tokio_rustls::rustls::pki_types::ServerName::try_from(host.to_owned())
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    Error::Tls(format!("invalid UDS TLS server name '{host}': {e}")).into()
                })?;
            let stream = connector.connect(name, stream).await.map_err(
                |e| -> Box<dyn std::error::Error + Send + Sync> {
                    Error::Tls(format!("TLS handshake over UDS failed: {e}")).into()
                },
            )?;
            Ok(hyper_util::rt::TokioIo::new(UdsStream::Tls(Box::new(
                stream,
            ))))
        })
    }
}

/// Convert a Hyper response from the UDS client into the core response type.
#[cfg(unix)]
pub(crate) async fn send_request(
    client: &crate::transport::TimeoutUdsClient,
    request: http::Request<crate::transport::HyperRequestBody>,
    url: url::Url,
    trace: Option<&dyn crate::trace::TraceObserver>,
) -> Result<Response> {
    use crate::trace::{TraceEvent, TracePhase};

    if let Some(observer) = trace {
        let method = request.method().as_str().to_owned();
        let target = request.uri().to_string();
        observer.on_event(&TraceEvent::SendRequestHeaders {
            phase: TracePhase::Started,
            method,
            target,
        });
    }

    let result = client
        .request(request)
        .await
        .map_err(|e| Error::HyperClient(Arc::new(e)));

    match result {
        Ok(response) => {
            let status = response.status().as_u16();
            let version = response.version();
            let headers = response.headers().clone();

            if let Some(observer) = trace {
                observer.on_event(&TraceEvent::ReceiveResponseHeaders {
                    phase: TracePhase::Complete,
                    status,
                });
            }

            let body: BoxBytesStream =
                crate::transport::direct::wrap_incoming(response.into_body());
            Ok(Response::new(
                http::StatusCode::from_u16(status).unwrap_or(http::StatusCode::OK),
                version,
                headers,
                url,
                ResponseBody::streaming(body),
            ))
        }
        Err(e) => {
            if let Some(observer) = trace {
                observer.on_event(&TraceEvent::SendRequestHeaders {
                    phase: TracePhase::Failed,
                    method: String::new(),
                    target: String::new(),
                });
            }
            Err(e)
        }
    }
}

#[cfg(not(unix))]
pub(crate) fn unsupported() -> Result<()> {
    Err(Error::Unsupported(
        "Unix domain sockets are not supported on this platform".into(),
    ))
}

#[allow(dead_code)]
fn _keep_bytes_import(_: Bytes) {}
