//! HTTPS CONNECT tunnel support for proxy connections.

use bytes::{Bytes, BytesMut};

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
#[allow(clippy::too_many_arguments)] // Transport hints added as a typed parameter.
pub(crate) async fn send_https_connect_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    proxy_config: &ProxyConfig,
    transport_hints: &crate::request::TransportHints,
    ctx: &ProxyRequestContext<'_>,
) -> Result<Response> {
    use tokio::io::AsyncWriteExt;

    let mut stream = connect_to_proxy(
        proxy_config,
        ctx.proxy_connect_timeout,
        ctx.proxy_tls_timeout,
        ctx.deadline,
        ctx.proxy_tls_config,
    )
    .await?;

    // Send CONNECT request.
    let dest_host = dest_url
        .host_str()
        .ok_or_else(|| Error::InvalidUrl("destination URL has no host".into()))?;
    let dest_port = dest_url.port_or_known_default().unwrap_or(443);
    let connect_target = authority_form_target(dest_host, dest_port);

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
    // Write proxy-only headers on the CONNECT request.
    for (name, value) in proxy_config.proxy_headers().iter() {
        // Skip proxy-authorization — handled above from configured auth.
        if name.as_str().eq_ignore_ascii_case("proxy-authorization") {
            continue;
        }
        if let Ok(value_str) = value.to_str() {
            use std::fmt::Write;
            let _ = write!(connect_req, "{}: {value_str}\r\n", name.as_str());
        }
    }
    connect_req.push_str("\r\n");

    if connect_req.len() > crate::headers::MAX_REQUEST_HEADER_BYTES {
        return Err(Error::RequestBuild(format!(
            "request headers exceed maximum size of {} bytes",
            crate::headers::MAX_REQUEST_HEADER_BYTES
        )));
    }

    let write = stream.write_all(connect_req.as_bytes());
    match effective_timeout(ctx.deadline, ctx.write_timeout)? {
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
    let (status, resp_headers, initial_buf, _reason_phrase) =
        match effective_timeout(ctx.deadline, ctx.read_timeout)? {
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
    let rustls_config = if let Some(tc) = ctx.origin_tls_config {
        let rc = tc
            .build_rustls_config()
            .map_err(|e| Error::Tls(format!("failed to build TLS config for tunnel: {e}")))?;
        crate::client::configure_tls_alpn(
            rc,
            crate::http_version::HttpVersionPolicyEnabler::from_policy(ctx.http_version_policy),
        )
    } else {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let fallback = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        crate::client::configure_tls_alpn(
            fallback,
            crate::http_version::HttpVersionPolicyEnabler::from_policy(ctx.http_version_policy),
        )
    };
    let tls_connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(rustls_config));

    // Use sni_hostname override for TLS SNI and certificate verification
    // while TCP still connects to dest_host (the URL host).
    let sni_name = transport_hints.sni_hostname.as_deref().unwrap_or(dest_host);
    let domain = crate::transport::direct_connector::tls_server_name(sni_name)
        .map_err(|e| Error::Tls(format!("invalid TLS server name: {e}")))?;

    let tls_handshake = tls_connector.connect(domain, tunnel);
    let tls_timeout = effective_timeout(ctx.deadline, ctx.connect_timeout)?;
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
    // absolute-form.  Apply target override if present.
    let origin_uri = if let Some(ref target) = transport_hints.target {
        crate::pipeline::validate_target(target)?;
        std::str::from_utf8(target)
            .map_err(|_| Error::InvalidUrl("target extension is not valid UTF-8".into()))?
            .to_owned()
    } else {
        match dest_url.query() {
            Some(query) => format!("{}?{query}", dest_url.path()),
            None => dest_url.path().to_owned(),
        }
    };
    let mut tls_buf = tokio::io::BufReader::new(tls_stream);

    // No proxy headers inside the tunnel — only origin headers.
    let empty_proxy_headers = crate::headers::Headers::new();
    let write = write_proxy_request(
        &mut tls_buf,
        method,
        &origin_uri,
        version,
        headers,
        None, // No proxy auth for the destination request.
        &empty_proxy_headers,
        body,
    );
    match effective_timeout(ctx.deadline, ctx.write_timeout)? {
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
    let (status, resp_headers, initial_buf, reason_phrase) =
        match effective_timeout(ctx.deadline, ctx.read_timeout)? {
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
        let value = http::HeaderValue::from_bytes(value)
            .map_err(|e| Error::MalformedProxyResponse(format!("invalid header value: {e}")))?;
        resp_headers_map.append(name, value);
    }

    let stream_reader = tls_buf.into_inner();
    // For TLS streams, we can't easily extract the inner stream.
    // Use the initial_buf approach with the TLS stream wrapped.
    // The declared length lets the stream tell a truncated body apart
    // from a complete body followed by an abrupt close without TLS
    // `close_notify` (normal for real servers; see `response_content_length`).
    let expected_len = super::proxy::response_content_length(&resp_headers);
    let body_stream = TlsProxyResponseStream::new(initial_buf, stream_reader, expected_len);
    let body_stream = Box::pin(body_stream) as BoxBytesStream;
    let body = ResponseBody::streaming(body_stream);

    let mut response = Response::new(status, version, resp_headers_map, url, body);
    response.set_wire_reason_phrase(reason_phrase);
    Ok(response)
}

/// Build an authority-form `host:port` target for a CONNECT request.
///
/// Authority-form requires brackets around IPv6 literals; the url crate
/// strips them from `host_str()`, so they are restored here.
fn authority_form_target(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// Maximum characters retained from a proxy-controlled rejection body.
///
/// The rejection text is attacker-controllable when the proxy is hostile;
/// bounding and sanitizing it keeps credential-looking material and
/// terminal escape sequences out of logs and error displays.
const MAX_PROXY_REJECTION_BODY_CHARS: usize = 256;

fn proxy_rejection_body(headers: &[(String, Vec<u8>)], initial_buf: &[u8]) -> String {
    let raw = headers
        .iter()
        .find(|(name, _)| {
            name.eq_ignore_ascii_case("x-error-message")
                || name.eq_ignore_ascii_case("x-proxy-error")
        })
        .map_or_else(
            || String::from_utf8_lossy(initial_buf).into_owned(),
            |(_, value)| String::from_utf8_lossy(value).into_owned(),
        );
    let mut sanitized = String::new();
    let mut sanitized_chars = 0;
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.next_if_eq(&'[').is_some() {
                // Drop CSI sequences such as ESC[31m in their entirety.
                for sequence_char in chars.by_ref() {
                    if ('@'..='~').contains(&sequence_char) {
                        break;
                    }
                }
            }
            continue;
        }
        if !c.is_control() {
            sanitized.push(c);
            sanitized_chars += 1;
            if sanitized_chars >= MAX_PROXY_REJECTION_BODY_CHARS {
                break;
            }
        }
    }
    sanitized
}

/// Streaming response body from a TLS connection through a proxy tunnel.
///
/// Yields data from an initial buffer first, then reads from the
/// underlying TLS stream.
pub(crate) struct TlsProxyResponseStream<S> {
    initial_buf: std::io::Cursor<Vec<u8>>,
    inner: S,
    chunk: BytesMut,
    /// Declared `Content-Length`, if the response carried an explicit
    /// non-chunked length. Used only to tell a truncated body apart from
    /// a complete body followed by an abrupt close.
    expected_len: Option<u64>,
    /// Body bytes yielded so far.
    delivered: u64,
}

impl<S> TlsProxyResponseStream<S> {
    pub(crate) fn new(initial_buf: Vec<u8>, inner: S, expected_len: Option<u64>) -> Self {
        Self {
            initial_buf: std::io::Cursor::new(initial_buf),
            inner,
            chunk: BytesMut::with_capacity(8192),
            expected_len,
            delivered: 0,
        }
    }

    /// Returns `true` once every declared body byte has been delivered.
    fn body_is_complete(&self) -> bool {
        self.expected_len.is_some_and(|n| self.delivered >= n)
    }
}

impl<S: tokio::io::AsyncRead + Unpin> futures_core::Stream for TlsProxyResponseStream<S> {
    type Item = Result<Bytes>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::io::Read as _;
        let this = self.as_mut().get_mut();

        // Drain the initial buffer first.
        if this.initial_buf.position() < this.initial_buf.get_ref().len() as u64 {
            this.chunk.clear();
            this.chunk.resize(8192, 0);
            let n = match this.initial_buf.read(&mut this.chunk) {
                Ok(n) => n,
                Err(e) => {
                    return std::task::Poll::Ready(Some(Err(Error::Body(format!(
                        "failed to read initial buffer: {e}"
                    )))));
                }
            };
            if n > 0 {
                this.delivered += n as u64;
                return std::task::Poll::Ready(Some(Ok(this.chunk.split_to(n).freeze())));
            }
        }

        // Read from the inner stream.
        this.chunk.clear();
        this.chunk.resize(8192, 0);
        let mut read_buf = tokio::io::ReadBuf::new(&mut this.chunk);
        match tokio::io::AsyncRead::poll_read(
            std::pin::Pin::new(&mut this.inner),
            cx,
            &mut read_buf,
        ) {
            std::task::Poll::Ready(Ok(())) => {
                let n = read_buf.filled().len();
                if n > 0 {
                    this.delivered += n as u64;
                    std::task::Poll::Ready(Some(Ok(this.chunk.split_to(n).freeze())))
                } else {
                    std::task::Poll::Ready(None)
                }
            }
            std::task::Poll::Ready(Err(e)) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    // Truncated declared body: EOF before every
                    // `Content-Length` byte arrived. Otherwise the body is
                    // complete (abrupt closes without TLS `close_notify`
                    // are normal once all bytes arrived) or
                    // close-delimited (EOF is the framing), so end the
                    // stream cleanly.
                    if this.expected_len.is_some() && !this.body_is_complete() {
                        return std::task::Poll::Ready(Some(Err(Error::Body(format!(
                            "proxy closed connection unexpectedly: {e}"
                        )))));
                    }
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
            let Ok(pos_usize) = usize::try_from(pos) else {
                return std::task::Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "initial buffer position exceeds usize",
                )));
            };
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
    use super::{authority_form_target, proxy_rejection_body, TlsProxyResponseStream};
    use futures_util::StreamExt as _;

    /// Simulates a tunneled peer that sends `data` then closes abruptly
    /// (no TLS `close_notify`), surfacing `UnexpectedEof` like rustls does.
    struct AbruptReader {
        data: std::io::Cursor<Vec<u8>>,
    }

    impl AbruptReader {
        fn new(data: &[u8]) -> Self {
            Self {
                data: std::io::Cursor::new(data.to_vec()),
            }
        }
    }

    impl tokio::io::AsyncRead for AbruptReader {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            use std::io::Read as _;
            let mut tmp = [0u8; 1024];
            match self.data.read(&mut tmp) {
                Ok(0) => std::task::Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "peer closed connection without sending TLS close_notify",
                ))),
                Ok(n) => {
                    buf.put_slice(&tmp[..n]);
                    std::task::Poll::Ready(Ok(()))
                }
                Err(e) => std::task::Poll::Ready(Err(e)),
            }
        }
    }

    #[tokio::test]
    async fn tls_tunnel_complete_body_survives_abrupt_close() {
        // Full declared body arrives, then the origin closes without TLS
        // `close_notify` (normal for real servers): the stream ends
        // cleanly instead of failing the already-complete body.
        let mut stream =
            TlsProxyResponseStream::new(b"ok".to_vec(), AbruptReader::new(b""), Some(2));
        let first = stream.next().await.expect("body chunk").expect("no error");
        assert_eq!(first.as_ref(), b"ok");
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn tls_tunnel_truncated_body_still_errors() {
        // Only 1 of 2 declared bytes arrives before the abrupt close:
        // this is a genuine truncation and must stay an error.
        let mut stream = TlsProxyResponseStream::new(Vec::new(), AbruptReader::new(b"o"), Some(2));
        let first = stream.next().await.expect("body chunk").expect("no error");
        assert_eq!(first.as_ref(), b"o");
        assert!(stream.next().await.expect("stream item").is_err());
    }

    #[test]
    fn authority_form_target_brackets_ipv6() {
        assert_eq!(authority_form_target("::1", 8080), "[::1]:8080");
        assert_eq!(
            authority_form_target("2001:db8::1", 443),
            "[2001:db8::1]:443"
        );
        assert_eq!(authority_form_target("example.com", 80), "example.com:80");
        assert_eq!(authority_form_target("127.0.0.1", 9090), "127.0.0.1:9090");
    }

    #[test]
    fn proxy_rejection_body_prefers_diagnostic_header() {
        let headers = vec![("X-Error-Message".to_owned(), b"access denied".to_vec())];
        assert_eq!(proxy_rejection_body(&headers, b"ignored"), "access denied");
    }

    #[test]
    fn proxy_rejection_body_truncates_buffered_bytes() {
        let body = "a".repeat(300);
        let sanitized = proxy_rejection_body(&[], body.as_bytes());
        assert_eq!(sanitized.len(), 256);
        assert!(sanitized.chars().all(|c| c == 'a'));
    }

    #[test]
    fn proxy_rejection_body_strips_control_characters() {
        // Terminal escape sequences and CR/LF from a hostile proxy must
        // not survive into error Display/Debug output.
        let headers = vec![(
            "x-proxy-error".to_owned(),
            b"denied\x1b[31m\r\nsecret: hunter2".to_vec(),
        )];
        assert_eq!(
            proxy_rejection_body(&headers, b"ignored"),
            "deniedsecret: hunter2"
        );
    }
}
