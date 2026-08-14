//! HTTPS CONNECT tunnel support for proxy connections.

use bytes::Bytes;

use crate::body::{BoxBytesStream, RequestBody, ResponseBody};
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::proxy::ProxyConfig;
use crate::response::Response;
use crate::timeout::TimeoutPhase;

use super::proxy::{
    connect_to_proxy, effective_timeout, read_proxy_response, write_proxy_request,
    ProxyRequestContext,
};

/// Send an HTTPS request through an HTTP proxy using CONNECT tunneling.
#[allow(clippy::too_many_lines)] // CONNECT owns the ordered proxy/tunnel/origin phases.
pub(crate) async fn send_https_connect_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    proxy_config: &ProxyConfig,
    ctx: &ProxyRequestContext<'_>,
) -> Result<Response> {
    use tokio::io::AsyncWriteExt;

    let mut stream = connect_to_proxy(
        proxy_config,
        ctx.proxy_connect_timeout,
        ctx.proxy_tls_timeout,
        ctx.deadline,
        ctx.now,
        ctx.tls_config,
    )
    .await?;

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

    let write = stream.write_all(connect_req.as_bytes());
    match effective_timeout(ctx.deadline, ctx.write_timeout, ctx.now)? {
        Some(duration) => tokio::time::timeout(duration, write)
            .await
            .map_err(|_| Error::Timeout {
                phase: TimeoutPhase::Write,
                elapsed: duration,
            })?
            .map_err(|e| Error::ProxyConnect(format!("failed to send CONNECT: {e}")))?,
        None => write
            .await
            .map_err(|e| Error::ProxyConnect(format!("failed to send CONNECT: {e}")))?,
    }

    // Read the CONNECT response.
    let read = read_proxy_response(&mut stream);
    let (status, resp_headers, initial_buf) =
        match effective_timeout(ctx.deadline, ctx.read_timeout, ctx.now)? {
            Some(duration) => {
                tokio::time::timeout(duration, read)
                    .await
                    .map_err(|_| Error::Timeout {
                        phase: TimeoutPhase::Read,
                        elapsed: duration,
                    })??
            }
            None => read.await?,
        };

    if status != 200 {
        return Err(Error::ProxyConnectRejected {
            status,
            body: proxy_rejection_body(&resp_headers, &initial_buf),
        });
    }

    // The tunnel is established. Get the raw TCP stream.
    let tcp_stream = stream.into_inner();

    // Wrap with initial buffer for TLS.
    let tunnel = ProxyTunnel::new(initial_buf, tcp_stream);

    // Perform TLS handshake with the destination through the tunnel.
    let rustls_config = if let Some(tc) = ctx.tls_config {
        tc.build_rustls_config()
            .map_err(|e| Error::Tls(format!("failed to build TLS config for tunnel: {e}")))?
    } else {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth()
    };
    let tls_connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(rustls_config));

    let domain = rustls::pki_types::ServerName::try_from(dest_host.to_owned())
        .map_err(|e| Error::Tls(format!("invalid TLS server name: {e}")))?;

    let tls_handshake = tls_connector.connect(domain, tunnel);
    let tls_timeout = effective_timeout(ctx.deadline, ctx.connect_timeout, ctx.now)?;
    let tls_stream = match tls_timeout {
        Some(dur) => match tokio::time::timeout(dur, tls_handshake).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                return Err(Error::Tls(format!(
                    "TLS handshake through tunnel failed: {e}"
                )))
            }
            Err(_) => {
                return Err(Error::Timeout {
                    phase: TimeoutPhase::Connect,
                    elapsed: dur,
                });
            }
        },
        None => tls_handshake
            .await
            .map_err(|e| Error::Tls(format!("TLS handshake through tunnel failed: {e}")))?,
    };

    // Send the actual HTTP request over the TLS connection.
    // CONNECT switches the request to the origin connection. HTTPX/httpcore
    // therefore uses origin-form here, while forward proxy requests retain
    // absolute-form.
    let origin_uri = match dest_url.query() {
        Some(query) => format!("{}?{query}", dest_url.path()),
        None => dest_url.path().to_owned(),
    };
    let mut tls_buf = tokio::io::BufReader::new(tls_stream);

    let write = write_proxy_request(
        &mut tls_buf,
        method,
        &origin_uri,
        version,
        headers,
        None, // No proxy auth for the destination request.
        body,
    );
    match effective_timeout(ctx.deadline, ctx.write_timeout, ctx.now)? {
        Some(duration) => {
            tokio::time::timeout(duration, write)
                .await
                .map_err(|_| Error::Timeout {
                    phase: TimeoutPhase::Write,
                    elapsed: duration,
                })??;
        }
        None => write.await?,
    }

    // Read the response from the destination.
    let read = read_proxy_response(&mut tls_buf);
    let (status, resp_headers, initial_buf) =
        match effective_timeout(ctx.deadline, ctx.read_timeout, ctx.now)? {
            Some(duration) => {
                tokio::time::timeout(duration, read)
                    .await
                    .map_err(|_| Error::Timeout {
                        phase: TimeoutPhase::Read,
                        elapsed: duration,
                    })??
            }
            None => read.await?,
        };

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

fn proxy_rejection_body(headers: &[(String, String)], initial_buf: &[u8]) -> String {
    headers
        .iter()
        .find(|(name, _)| {
            name.eq_ignore_ascii_case("x-error-message")
                || name.eq_ignore_ascii_case("x-proxy-error")
        })
        .map_or_else(
            || String::from_utf8_lossy(initial_buf).into_owned(),
            |(_, value)| value.clone(),
        )
}

/// Streaming response body from a TLS connection through a proxy tunnel.
///
/// Yields data from an initial buffer first, then reads from the
/// underlying TLS stream.
pub(crate) struct TlsProxyResponseStream<S> {
    initial_buf: std::io::Cursor<Vec<u8>>,
    inner: S,
}

impl<S> TlsProxyResponseStream<S> {
    pub(crate) fn new(initial_buf: Vec<u8>, inner: S) -> Self {
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
            std::task::Poll::Ready(Err(e)) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    return std::task::Poll::Ready(None);
                }
                std::task::Poll::Ready(Some(Err(Error::Body(format!(
                    "proxy stream read error: {e}"
                )))))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

/// IO wrapper for CONNECT tunnels that holds initial buffered bytes.
///
/// After the CONNECT handshake, the proxy may have sent some bytes
/// that are part of the TLS stream. This wrapper preserves them.
pub(crate) struct ProxyTunnel<S> {
    initial_buf: std::io::Cursor<Vec<u8>>,
    inner: S,
}

impl<S> ProxyTunnel<S> {
    pub(crate) fn new(initial_buf: Vec<u8>, inner: S) -> Self {
        Self {
            initial_buf: std::io::Cursor::new(initial_buf),
            inner,
        }
    }
}

impl<S: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for ProxyTunnel<S> {
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

impl<S: tokio::io::AsyncWrite + Unpin> tokio::io::AsyncWrite for ProxyTunnel<S> {
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

#[cfg(test)]
mod tests {
    use super::proxy_rejection_body;

    #[test]
    fn proxy_rejection_body_prefers_diagnostic_header() {
        let headers = vec![("X-Error-Message".to_owned(), "access denied".to_owned())];
        assert_eq!(proxy_rejection_body(&headers, b"ignored"), "access denied");
    }

    #[test]
    fn proxy_rejection_body_keeps_all_buffered_bytes() {
        let body = "a".repeat(300);
        assert_eq!(proxy_rejection_body(&[], body.as_bytes()), body);
    }
}
