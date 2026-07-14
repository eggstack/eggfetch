//! HTTPS CONNECT tunnel support for proxy connections.

use bytes::Bytes;

use crate::body::{BoxBytesStream, RequestBody, ResponseBody};
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::proxy::ProxyConfig;
use crate::response::Response;
use crate::timeout::TimeoutPhase;

use super::proxy::{connect_to_proxy, read_proxy_response, write_proxy_request};

/// Send an HTTPS request through an HTTP proxy using CONNECT tunneling.
pub(crate) async fn send_https_connect_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    proxy_config: &ProxyConfig,
    remaining_total: Option<std::time::Duration>,
) -> Result<Response> {
    use tokio::io::AsyncWriteExt;

    let mut stream = connect_to_proxy(proxy_config, remaining_total).await?;

    // Send CONNECT request.
    let dest_host = dest_url
        .host_str()
        .ok_or_else(|| Error::InvalidUrl("destination URL has no host".into()))?;
    let dest_port = dest_url.port_or_known_default().unwrap_or(443);
    let connect_target = format!("{dest_host}:{dest_port}");

    let mut connect_req =
        format!("CONNECT {connect_target} HTTP/1.1\r\nHost: {connect_target}\r\n");
    if let Some(auth) = proxy_config.auth() {
        use std::fmt::Write;
        let _ = write!(
            connect_req,
            "Proxy-Authorization: {}\r\n",
            auth.header_value()
        );
    }
    connect_req.push_str("\r\n");

    stream
        .write_all(connect_req.as_bytes())
        .await
        .map_err(|e| Error::ProxyConnect(format!("failed to send CONNECT: {e}")))?;

    // Read the CONNECT response.
    let (status, _resp_headers, initial_buf) = read_proxy_response(&mut stream).await?;

    if status != 200 {
        let body_str = initial_buf.iter().take(256).copied().collect::<Vec<u8>>();
        let body_str = String::from_utf8_lossy(&body_str).into_owned();
        return Err(Error::ProxyConnectRejected {
            status,
            body: body_str,
        });
    }

    // The tunnel is established. Get the raw TCP stream.
    let tcp_stream = stream.into_inner();

    // Wrap with initial buffer for TLS.
    let tunnel = ProxyTunnel::new(initial_buf, tcp_stream);

    // Perform TLS handshake with the destination through the tunnel.
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let tls_connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(tls_config));

    let domain = rustls::pki_types::ServerName::try_from(dest_host.to_owned())
        .map_err(|e| Error::Tls(format!("invalid TLS server name: {e}")))?;

    let tls_handshake = tls_connector.connect(domain, tunnel);
    let tls_stream = match remaining_total {
        Some(dur) => match tokio::time::timeout(dur, tls_handshake).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                return Err(Error::Tls(format!(
                    "TLS handshake through tunnel failed: {e}"
                )))
            }
            Err(_) => {
                return Err(Error::Timeout {
                    phase: TimeoutPhase::ProxyTls,
                    elapsed: dur,
                });
            }
        },
        None => tls_handshake
            .await
            .map_err(|e| Error::Tls(format!("TLS handshake through tunnel failed: {e}")))?,
    };

    // Send the actual HTTP request over the TLS connection.
    let absolute_uri = dest_url.as_str();
    let mut tls_buf = tokio::io::BufReader::new(tls_stream);

    write_proxy_request(
        &mut tls_buf,
        method,
        absolute_uri,
        version,
        headers,
        None, // No proxy auth for the destination request.
        body,
    )
    .await?;

    // Read the response from the destination.
    let (status, resp_headers, initial_buf) = read_proxy_response(&mut tls_buf).await?;

    let url = dest_url.clone();
    let status = http::StatusCode::from_u16(status)
        .map_err(|e| Error::MalformedProxyResponse(format!("invalid status code: {e}")))?;

    let mut resp_headers_map = http::HeaderMap::new();
    for (name, value) in &resp_headers {
        let name = http::HeaderName::from_bytes(name.as_bytes())
            .map_err(|e| Error::MalformedProxyResponse(format!("invalid header name: {e}")))?;
        let value = http::HeaderValue::from_str(value)
            .map_err(|e| Error::MalformedProxyResponse(format!("invalid header value: {e}")))?;
        resp_headers_map.append(name, value);
    }

    let stream_reader = tls_buf.into_inner();
    // For TLS streams, we can't easily extract the inner stream.
    // Use the initial_buf approach with the TLS stream wrapped.
    let body_stream = TlsProxyResponseStream::new(initial_buf, stream_reader);
    let body_stream = Box::pin(body_stream) as BoxBytesStream;
    let body = ResponseBody::streaming(body_stream);

    Ok(Response::new(status, version, resp_headers_map, url, body))
}

/// Streaming response body from a TLS connection through a proxy tunnel.
///
/// Yields data from an initial buffer first, then reads from the
/// underlying TLS stream.
struct TlsProxyResponseStream<S> {
    initial_buf: std::io::Cursor<Vec<u8>>,
    inner: S,
}

impl<S> TlsProxyResponseStream<S> {
    fn new(initial_buf: Vec<u8>, inner: S) -> Self {
        Self {
            initial_buf: std::io::Cursor::new(initial_buf),
            inner,
        }
    }
}

impl<S: tokio::io::AsyncRead + Unpin> futures_core::Stream for TlsProxyResponseStream<S> {
    type Item = Result<Bytes>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::io::Read as _;

        // Drain the initial buffer first.
        if self.initial_buf.position() < self.initial_buf.get_ref().len() as u64 {
            let mut chunk = vec![0u8; 8192];
            let n = match self.initial_buf.read(&mut chunk) {
                Ok(n) => n,
                Err(e) => {
                    return std::task::Poll::Ready(Some(Err(Error::Body(format!(
                        "failed to read initial buffer: {e}"
                    )))));
                }
            };
            if n > 0 {
                chunk.truncate(n);
                return std::task::Poll::Ready(Some(Ok(Bytes::from(chunk))));
            }
        }

        // Read from the inner stream.
        let mut chunk = vec![0u8; 8192];
        let mut read_buf = tokio::io::ReadBuf::new(&mut chunk);
        match tokio::io::AsyncRead::poll_read(
            std::pin::Pin::new(&mut self.inner),
            cx,
            &mut read_buf,
        ) {
            std::task::Poll::Ready(Ok(())) => {
                let n = read_buf.filled().len();
                if n > 0 {
                    chunk.truncate(n);
                    std::task::Poll::Ready(Some(Ok(Bytes::from(chunk))))
                } else {
                    std::task::Poll::Ready(None)
                }
            }
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Some(Err(Error::Body(
                format!("proxy stream read error: {e}"),
            )))),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

/// IO wrapper for CONNECT tunnels that holds initial buffered bytes.
///
/// After the CONNECT handshake, the proxy may have sent some bytes
/// that are part of the TLS stream. This wrapper preserves them.
struct ProxyTunnel {
    initial_buf: std::io::Cursor<Vec<u8>>,
    inner: tokio::net::TcpStream,
}

impl ProxyTunnel {
    fn new(initial_buf: Vec<u8>, inner: tokio::net::TcpStream) -> Self {
        Self {
            initial_buf: std::io::Cursor::new(initial_buf),
            inner,
        }
    }
}

impl tokio::io::AsyncRead for ProxyTunnel {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // Drain initial buffer first.
        let pos = self.initial_buf.position();
        let total = self.initial_buf.get_ref().len() as u64;
        if pos < total {
            let unfilled = buf.initialize_unfilled();
            let pos_usize = usize::try_from(pos).unwrap_or(usize::MAX);
            let remaining = &self.initial_buf.get_ref()[pos_usize..];
            let n = std::cmp::min(remaining.len(), unfilled.len());
            unfilled[..n].copy_from_slice(&remaining[..n]);
            self.initial_buf.set_position(pos + n as u64);
            buf.advance(n);
            return std::task::Poll::Ready(Ok(()));
        }

        // Delegate to inner stream.
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for ProxyTunnel {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
