//! Proxy request handling for HTTP and HTTPS destinations.

use bytes::Bytes;

use crate::body::{BoxBytesStream, RequestBody, ResponseBody};
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::proxy::{ProxyAuth, ProxyConfig};
use crate::response::Response;
use crate::timeout::TimeoutPhase;

/// Combine one phase timeout with the optional native request deadline.
///
/// Compatibility callers provide only the four HTTPX phase budgets. Native
/// callers may additionally provide `deadline`; the latter is an outer cap
/// and is never allowed to extend a phase budget.
///
/// The remaining budget is measured against a freshly captured instant on
/// every call so that each sequential phase of a multi-phase proxy setup
/// (proxy connect → proxy TLS → CONNECT write/read → origin TLS →
/// write/read) sees only what is actually left of the monotonic request
/// deadline, never a stale snapshot from before earlier phases ran.
pub(crate) fn effective_timeout(
    deadline: Option<std::time::Instant>,
    configured: Option<std::time::Duration>,
) -> Result<Option<std::time::Duration>> {
    let now = std::time::Instant::now();
    let total = match deadline {
        Some(deadline) if deadline > now => Some(deadline.duration_since(now)),
        Some(_) => {
            return Err(Error::Timeout {
                phase: TimeoutPhase::Total,
                elapsed: std::time::Duration::ZERO,
            });
        }
        None => None,
    };

    Ok(match (configured, total) {
        (Some(phase), Some(total)) => Some(phase.min(total)),
        (Some(phase), None) => Some(phase),
        (None, Some(total)) => Some(total),
        (None, None) => None,
    })
}

pub(crate) struct ProxyRequestContext<'a> {
    pub(crate) remaining_total: Option<std::time::Duration>,
    pub(crate) deadline: Option<std::time::Instant>,
    pub(crate) connect_timeout: Option<std::time::Duration>,
    pub(crate) proxy_connect_timeout: Option<std::time::Duration>,
    pub(crate) proxy_tls_timeout: Option<std::time::Duration>,
    pub(crate) write_timeout: Option<std::time::Duration>,
    pub(crate) read_timeout: Option<std::time::Duration>,
    /// TLS configuration for the *origin* server (used after CONNECT
    /// tunnel establishment).
    pub(crate) origin_tls_config: Option<&'a crate::tls::TlsConfig>,
    /// TLS configuration for the *proxy* endpoint (used when the proxy
    /// endpoint itself is `https://`).  When `None`, the proxy endpoint
    /// uses the proxy/default trust roots.  The origin TLS
    /// configuration is independent and is never reused as a fallback
    /// for the proxy handshake.
    pub(crate) proxy_tls_config: Option<&'a crate::tls::TlsConfig>,
    pub(crate) socks_client: Option<crate::transport::TimeoutSocksClient>,
}

/// Client-to-proxy stream, optionally protected by TLS for an `https://`
/// proxy endpoint.
pub(crate) enum ProxyIo {
    /// Plain proxy TCP connection.
    Tcp(tokio::net::TcpStream),
    /// TLS-protected proxy connection.
    Tls(Box<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>),
}

impl tokio::io::AsyncRead for ProxyIo {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Tcp(stream) => std::pin::Pin::new(stream).poll_read(cx, buf),
            Self::Tls(stream) => std::pin::Pin::new(stream.as_mut()).poll_read(cx, buf),
        }
    }
}

impl tokio::io::AsyncWrite for ProxyIo {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bytes: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match &mut *self {
            Self::Tcp(stream) => std::pin::Pin::new(stream).poll_write(cx, bytes),
            Self::Tls(stream) => std::pin::Pin::new(stream.as_mut()).poll_write(cx, bytes),
        }
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Tcp(stream) => std::pin::Pin::new(stream).poll_flush(cx),
            Self::Tls(stream) => std::pin::Pin::new(stream.as_mut()).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Tcp(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
            Self::Tls(stream) => std::pin::Pin::new(stream.as_mut()).poll_shutdown(cx),
        }
    }
}

/// Send a request through a proxy.
///
/// Routes to HTTP forwarding, CONNECT tunneling, or SOCKS5 tunneling
/// based on the proxy scheme and destination scheme.
#[allow(clippy::too_many_arguments)] // Transport hints added as a typed parameter.
pub(crate) async fn send_proxy_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    proxy_config: &ProxyConfig,
    transport_hints: &crate::request::TransportHints,
    ctx: &ProxyRequestContext<'_>,
) -> Result<Response> {
    if proxy_config.is_socks() {
        return send_socks_request(
            dest_url,
            method,
            headers,
            body,
            version,
            transport_hints,
            ctx,
        )
        .await;
    }

    match dest_url.scheme() {
        "http" => {
            send_http_proxy_request(
                dest_url,
                method,
                headers,
                body,
                version,
                proxy_config,
                transport_hints,
                ctx,
            )
            .await
        }
        "https" => {
            super::connect::send_https_connect_request(
                dest_url,
                method,
                headers,
                body,
                version,
                proxy_config,
                transport_hints,
                ctx,
            )
            .await
        }
        other => Err(Error::Unsupported(format!(
            "unsupported destination scheme '{other}' through proxy"
        ))),
    }
}

/// Connect to the proxy, returning a buffered TCP stream.
pub(crate) async fn connect_to_proxy(
    proxy_config: &ProxyConfig,
    proxy_connect_timeout: Option<std::time::Duration>,
    proxy_tls_timeout: Option<std::time::Duration>,
    deadline: Option<std::time::Instant>,
    proxy_tls_config: Option<&crate::tls::TlsConfig>,
) -> Result<tokio::io::BufReader<ProxyIo>> {
    let proxy_host = proxy_config.host().unwrap_or("127.0.0.1");
    let proxy_port = proxy_config.port();

    let connect_future = async {
        let stream = tokio::net::TcpStream::connect((proxy_host, proxy_port))
            .await
            .map_err(|e| Error::ProxyConnect(format!("failed to connect to proxy: {e}")))?;
        stream
            .set_nodelay(true)
            .map_err(|e| Error::ProxyConnect(format!("failed to set nodelay: {e}")))?;
        Ok::<_, Error>(stream)
    };

    let connect_timeout = effective_timeout(deadline, proxy_connect_timeout)?;
    let stream = match connect_timeout {
        Some(dur) => match tokio::time::timeout(dur, connect_future).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err(Error::Timeout {
                    phase: TimeoutPhase::ProxyConnect,
                    elapsed: dur,
                });
            }
        },
        None => connect_future.await?,
    };

    let stream = if proxy_config.scheme() == "https" {
        let rustls_config = if let Some(tc) = proxy_tls_config {
            tc.build_rustls_config()
                .map_err(|e| Error::Tls(format!("failed to build proxy TLS config: {e}")))?
        } else {
            let mut root_store = rustls::RootCertStore::empty();
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth()
        };
        let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(rustls_config));
        let domain = rustls::pki_types::ServerName::try_from(proxy_host.to_owned())
            .map_err(|e| Error::Tls(format!("invalid proxy TLS server name: {e}")))?;
        let handshake = connector.connect(domain, stream);
        let tls_timeout = effective_timeout(deadline, proxy_tls_timeout)?;
        let tls_stream = match tls_timeout {
            Some(dur) => match tokio::time::timeout(dur, handshake).await {
                Ok(Ok(stream)) => stream,
                Ok(Err(e)) => return Err(Error::Tls(format!("proxy TLS handshake failed: {e}"))),
                Err(_) => {
                    return Err(Error::Timeout {
                        phase: TimeoutPhase::ProxyTls,
                        elapsed: dur,
                    })
                }
            },
            None => handshake
                .await
                .map_err(|e| Error::Tls(format!("proxy TLS handshake failed: {e}")))?,
        };
        ProxyIo::Tls(Box::new(tls_stream))
    } else {
        ProxyIo::Tcp(stream)
    };

    Ok(tokio::io::BufReader::new(stream))
}

/// Send an HTTP request through an HTTP forward proxy.
#[allow(clippy::too_many_arguments)] // Forwarding needs the request and phase-specific proxy context.
async fn send_http_proxy_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    proxy_config: &ProxyConfig,
    transport_hints: &crate::request::TransportHints,
    ctx: &ProxyRequestContext<'_>,
) -> Result<Response> {
    let mut stream = connect_to_proxy(
        proxy_config,
        ctx.proxy_connect_timeout,
        ctx.proxy_tls_timeout,
        ctx.deadline,
        ctx.proxy_tls_config,
    )
    .await?;

    // Write the proxy request with absolute-form URI.
    // Apply target override if present.
    let absolute_uri = if let Some(ref target) = transport_hints.target {
        crate::pipeline::validate_target(target)?;
        std::str::from_utf8(target)
            .map_err(|_| Error::InvalidUrl("target extension is not valid UTF-8".into()))?
    } else {
        dest_url.as_str()
    };
    let write = write_proxy_request(
        &mut stream,
        method,
        absolute_uri,
        version,
        headers,
        proxy_config.auth(),
        proxy_config.proxy_headers(),
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

    // Read the response from the proxy.
    let read = read_proxy_response(&mut stream);
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
        let value = http::HeaderValue::from_str(value)
            .map_err(|e| Error::MalformedProxyResponse(format!("invalid header value: {e}")))?;
        resp_headers_map.append(name, value);
    }

    // Return the body as a streaming response.
    let stream_reader = stream.into_inner();
    let body_stream = ProxyResponseStream::new(initial_buf, stream_reader);
    let body_stream = Box::pin(body_stream) as BoxBytesStream;
    let body = ResponseBody::streaming(body_stream);

    let mut response = Response::new(status, version, resp_headers_map, url, body);
    response.set_wire_reason_phrase(reason_phrase);
    Ok(response)
}

/// Append one header line, writing the value's raw bytes.
///
/// Values may carry HTTP/1.1 obs-text (0x80..=0xFF); dropping them would
/// silently diverge from hyper's direct path, which preserves the bytes.
fn push_header_line(out: &mut Vec<u8>, name: &str, value: &[u8]) {
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(b": ");
    out.extend_from_slice(value);
    out.extend_from_slice(b"\r\n");
}

/// Write an HTTP request to a stream.
#[allow(clippy::too_many_arguments)] // Proxy headers channel added as a typed parameter.
pub(crate) async fn write_proxy_request<S: tokio::io::AsyncWrite + Unpin>(
    stream: &mut S,
    method: &http::Method,
    uri: &str,
    version: http::Version,
    headers: &Headers,
    proxy_auth: Option<&ProxyAuth>,
    proxy_headers: &Headers,
    body: RequestBody,
) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    let version_str = match version {
        http::Version::HTTP_10 => "HTTP/1.0",
        _ => "HTTP/1.1",
    };

    // The head is assembled as raw bytes: header values may carry valid
    // HTTP/1.1 obs-text that must reach the wire unchanged, matching
    // hyper's direct path.
    let mut request = Vec::new();
    request.extend_from_slice(format!("{method} {uri} {version_str}\r\n").as_bytes());

    // Add Host header if not present.
    if !headers.contains("host") {
        if let Ok(parsed) = url::Url::parse(uri) {
            let host = if let Some(port) = parsed.port() {
                format!("{}:{port}", parsed.host_str().unwrap_or(""))
            } else {
                parsed.host_str().unwrap_or("").to_string()
            };
            request.extend_from_slice(format!("Host: {host}\r\n").as_bytes());
        }
    }

    // Write regular headers.
    for (name, value) in headers.iter() {
        // Skip proxy-authorization from destination headers.
        if name.as_str().eq_ignore_ascii_case("proxy-authorization") {
            continue;
        }
        push_header_line(&mut request, name.as_str(), value.as_bytes());
    }

    // Write proxy-only headers.  Skip proxy-authorization (handled
    // separately below from configured auth) and any header that
    // already appeared in the origin headers to avoid duplication.
    for (name, value) in proxy_headers.iter() {
        if name.as_str().eq_ignore_ascii_case("proxy-authorization") {
            continue;
        }
        if headers.contains(name.as_str()) {
            continue;
        }
        push_header_line(&mut request, name.as_str(), value.as_bytes());
    }

    // Add Proxy-Authorization if configured.
    if let Some(auth) = proxy_auth {
        request.extend_from_slice(
            format!("Proxy-Authorization: {}\r\n", auth.header_value()).as_bytes(),
        );
    }

    request.extend_from_slice(b"\r\n");

    if request.len() > crate::headers::MAX_REQUEST_HEADER_BYTES {
        return Err(Error::RequestBuild(format!(
            "request headers exceed maximum size of {} bytes",
            crate::headers::MAX_REQUEST_HEADER_BYTES
        )));
    }

    stream
        .write_all(&request)
        .await
        .map_err(|e| Error::ProxyConnect(format!("failed to write request: {e}")))?;

    // Write the body.
    match body {
        RequestBody::Empty => {}
        RequestBody::Bytes(bytes) => {
            stream
                .write_all(&bytes)
                .await
                .map_err(|e| Error::ProxyConnect(format!("failed to write body: {e}")))?;
        }
        RequestBody::Stream {
            stream: mut body_stream,
            ..
        } => {
            use bytes::BytesMut;
            use futures_util::StreamExt;
            let mut buf = BytesMut::with_capacity(8192);
            while let Some(chunk) = body_stream.next().await {
                let chunk = chunk.map_err(|e| Error::Body(e.to_string()))?;
                buf.extend_from_slice(&chunk);
                if buf.len() >= 8192 {
                    stream
                        .write_all(&buf)
                        .await
                        .map_err(|e| Error::ProxyConnect(format!("failed to write body: {e}")))?;
                    buf.clear();
                }
            }
            if !buf.is_empty() {
                stream
                    .write_all(&buf)
                    .await
                    .map_err(|e| Error::ProxyConnect(format!("failed to write body: {e}")))?;
            }
        }
    }

    stream
        .flush()
        .await
        .map_err(|e| Error::ProxyConnect(format!("failed to flush: {e}")))?;

    Ok(())
}

async fn read_bounded_line<S: tokio::io::AsyncRead + Unpin>(
    stream: &mut tokio::io::BufReader<S>,
    max_len: usize,
) -> Result<String> {
    use tokio::io::AsyncReadExt;

    let mut buf = Vec::with_capacity(256);
    let mut byte = [0u8; 1];
    loop {
        let n = stream
            .read(&mut byte)
            .await
            .map_err(|e| Error::ProxyConnect(format!("failed to read proxy response: {e}")))?;
        if n == 0 {
            // EOF before a newline: the response (status line or header
            // section) was truncated. Accepting the partial buffer here
            // would treat e.g. a bare "HTTP/1.1 200" from a dying proxy
            // as a successful response.
            return Err(Error::MalformedProxyResponse(
                "proxy closed connection before end of line".into(),
            ));
        }
        if byte[0] == b'\n' {
            break;
        }
        buf.push(byte[0]);
        if buf.len() > max_len {
            return Err(Error::MalformedProxyResponse(format!(
                "proxy response line exceeded maximum length of {max_len} bytes"
            )));
        }
    }
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    String::from_utf8(buf)
        .map_err(|_| Error::MalformedProxyResponse("proxy response contains invalid UTF-8".into()))
}

/// Read an HTTP response from a proxy or destination.
///
/// Returns `(status_code, headers, remaining_initial_bytes)`.
pub(crate) async fn read_proxy_response<S: tokio::io::AsyncRead + Unpin>(
    stream: &mut tokio::io::BufReader<S>,
) -> Result<(u16, Vec<(String, String)>, Vec<u8>, Option<String>)> {
    use tokio::io::AsyncBufReadExt;

    const MAX_STATUS_LINE_LEN: usize = 4096;
    const MAX_HEADER_COUNT: usize = 100;
    const MAX_HEADER_LINE_LEN: usize = 8192;
    const MAX_TOTAL_HEADER_BYTES: usize = 65536;

    let status_line = read_bounded_line(stream, MAX_STATUS_LINE_LEN).await?;

    if status_line.is_empty() {
        return Err(Error::MalformedProxyResponse(
            "proxy closed connection before response".into(),
        ));
    }

    let mut parts = status_line.splitn(3, ' ');
    let _version = parts.next();
    let status_code = parts
        .next()
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| {
            Error::MalformedProxyResponse(format!("invalid status line: {status_line}"))
        })?;
    // The third part (if present) is the reason phrase.
    let reason_phrase = parts.next().map(str::to_owned);

    let mut headers = Vec::new();
    let mut total_header_bytes: usize = 0;
    loop {
        let line = read_bounded_line(stream, MAX_HEADER_LINE_LEN).await?;

        if line.is_empty() {
            break;
        }

        total_header_bytes += line.len();
        if total_header_bytes > MAX_TOTAL_HEADER_BYTES {
            return Err(Error::MalformedProxyResponse(format!(
                "proxy response headers exceeded maximum total size of {MAX_TOTAL_HEADER_BYTES} bytes"
            )));
        }

        if headers.len() >= MAX_HEADER_COUNT {
            return Err(Error::MalformedProxyResponse(format!(
                "proxy response exceeded maximum header count of {MAX_HEADER_COUNT}"
            )));
        }

        headers.push(
            line.split_once(':')
                .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
                .ok_or_else(|| {
                    Error::MalformedProxyResponse(format!("invalid header line: {line}"))
                })?,
        );
    }

    let mut initial_buf = Vec::new();
    let buf_ref = stream.buffer();
    if !buf_ref.is_empty() {
        initial_buf.extend_from_slice(buf_ref);
        let consumed = buf_ref.len();
        stream.consume(consumed);
    }

    Ok((status_code, headers, initial_buf, reason_phrase))
}

/// Streaming response body from an HTTP proxy.
///
/// Yields data from an initial buffer first, then reads from the
/// underlying TCP stream.
struct ProxyResponseStream<S> {
    initial_buf: std::io::Cursor<Vec<u8>>,
    inner: S,
}

fn map_socks_send_error(err: hyper_util::client::legacy::Error) -> Error {
    let mapped = super::direct::map_send_error(err);
    if mapped.kind() == "hyper_client" {
        Error::ProxyConnect("proxy connection failed".into())
    } else {
        mapped
    }
}

/// Send a SOCKS request through Hyper's normal origin HTTP machinery.
async fn send_socks_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    transport_hints: &crate::request::TransportHints,
    ctx: &ProxyRequestContext<'_>,
) -> Result<Response> {
    let client = ctx
        .socks_client
        .as_ref()
        .ok_or_else(|| Error::ProxyConnect("persistent SOCKS client was not initialized".into()))?;
    let uri: http::Uri = if let Some(ref target) = transport_hints.target {
        crate::pipeline::validate_target(target)?;
        std::str::from_utf8(target)
            .map_err(|_| Error::InvalidUrl("target extension is not valid UTF-8".into()))?
            .parse()
            .map_err(|e| Error::InvalidUrl(format!("failed to convert target to URI: {e}")))?
    } else {
        dest_url
            .as_str()
            .parse()
            .map_err(|e| Error::InvalidUrl(format!("failed to convert url to URI: {e}")))?
    };
    let mut request = http::Request::builder()
        .method(method)
        .uri(uri)
        .version(version);
    for (name, value) in headers.iter() {
        request = request.header(name, value);
    }
    let request = request
        .body(body.into_http_body())
        .map_err(|e| Error::RequestBuild(e.to_string()))?;
    let response = match ctx.remaining_total {
        Some(duration) => tokio::time::timeout(duration, client.request(request))
            .await
            .map_err(|_| Error::Timeout {
                phase: TimeoutPhase::Total,
                elapsed: duration,
            })?
            .map_err(map_socks_send_error)?,
        None => client
            .request(request)
            .await
            .map_err(map_socks_send_error)?,
    };
    let status = response.status();
    let response_version = response.version();
    let response_headers = response.headers().clone();
    let stream = super::direct::wrap_incoming(response.into_body());
    Ok(Response::new(
        status,
        response_version,
        response_headers,
        dest_url.clone(),
        ResponseBody::streaming(stream),
    ))
}

impl<S> ProxyResponseStream<S> {
    fn new(initial_buf: Vec<u8>, inner: S) -> Self {
        Self {
            initial_buf: std::io::Cursor::new(initial_buf),
            inner,
        }
    }
}

impl<S: tokio::io::AsyncRead + Unpin> futures_core::Stream for ProxyResponseStream<S> {
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

        // Read from the proxy stream using poll_read.
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

#[cfg(test)]
mod tests {
    use super::{effective_timeout, read_proxy_response};
    use crate::error::Error;
    use crate::timeout::TimeoutPhase;
    use std::time::{Duration, Instant};

    #[test]
    fn effective_timeout_uses_the_smaller_phase_or_total_budget() {
        let deadline = Instant::now() + Duration::from_secs(2);
        let timeout = effective_timeout(Some(deadline), Some(Duration::from_secs(5)))
            .expect("deadline is in the future");
        assert!(timeout.is_some_and(|value| value <= Duration::from_secs(2)));
    }

    #[test]
    fn effective_timeout_uses_phase_budget_without_total() {
        assert_eq!(
            effective_timeout(None, Some(Duration::from_secs(3))).expect("phase budget is valid"),
            Some(Duration::from_secs(3))
        );
    }

    #[test]
    fn expired_total_budget_is_classified_as_total() {
        let error = effective_timeout(
            Some(
                Instant::now()
                    .checked_sub(Duration::from_millis(1))
                    .expect("a millisecond is representable before now"),
            ),
            Some(Duration::from_secs(3)),
        )
        .expect_err("expired total must fail before the phase starts");
        assert!(matches!(
            error,
            Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            }
        ));
    }

    #[test]
    fn effective_timeout_rejects_deadline_at_now() {
        let error =
            effective_timeout(Some(Instant::now()), Some(Duration::from_secs(3))).unwrap_err();
        assert!(matches!(
            error,
            Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn read_proxy_response_returns_buffered_body_bytes() {
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(
            b"HTTP/1.1 200 OK\r\n\r\nbody".to_vec(),
        ));
        let (_, _, initial, reason) = read_proxy_response(&mut reader).await.unwrap();
        assert_eq!(initial, b"body");
        assert_eq!(reason.as_deref(), Some("OK"));
    }

    #[tokio::test]
    async fn truncated_status_line_is_rejected() {
        // A proxy that emits a bare status line and dies must not be
        // treated as a successful response.
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(b"HTTP/1.1 200".to_vec()));
        let error = read_proxy_response(&mut reader).await.unwrap_err();
        assert!(matches!(error, Error::MalformedProxyResponse(_)));
    }

    #[tokio::test]
    async fn truncated_header_section_is_rejected() {
        // EOF where the blank terminator line should be: the header
        // section is incomplete.
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(
            b"HTTP/1.1 200 OK\r\nX-Only: one".to_vec(),
        ));
        let error = read_proxy_response(&mut reader).await.unwrap_err();
        assert!(matches!(error, Error::MalformedProxyResponse(_)));
    }

    #[tokio::test]
    async fn header_count_limit_admits_exactly_max() {
        use std::fmt::Write as _;

        const MAX_HEADER_COUNT: usize = 100;
        let mut wire = String::from("HTTP/1.1 200 OK\r\n");
        for i in 0..MAX_HEADER_COUNT {
            let _ = write!(wire, "X-H{i}: v\r\n");
        }
        wire.push_str("\r\n");
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(wire.into_bytes()));
        let (_, headers, _, _) = read_proxy_response(&mut reader).await.unwrap();
        assert_eq!(headers.len(), MAX_HEADER_COUNT);

        // One more header must be rejected outright.
        let mut wire = String::from("HTTP/1.1 200 OK\r\n");
        for i in 0..=MAX_HEADER_COUNT {
            let _ = write!(wire, "X-H{i}: v\r\n");
        }
        wire.push_str("\r\n");
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(wire.into_bytes()));
        let error = read_proxy_response(&mut reader).await.unwrap_err();
        assert!(
            matches!(error, Error::MalformedProxyResponse(ref msg) if msg.contains("header count")),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn read_proxy_response_extracts_reason_phrase() {
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(
            b"HTTP/1.1 404 Not Found\r\n\r\n".to_vec(),
        ));
        let (status, _, _, reason) = read_proxy_response(&mut reader).await.unwrap();
        assert_eq!(status, 404);
        assert_eq!(reason.as_deref(), Some("Not Found"));
    }

    #[tokio::test]
    async fn write_proxy_request_preserves_obs_text_header_values() {
        use crate::body::RequestBody;
        use crate::headers::Headers;
        use tokio::io::AsyncReadExt;

        // obs-text (0x80..=0xFF) is valid HTTP/1.1; the manually
        // serialized proxy request must emit the raw bytes rather than
        // silently dropping the header (hyper's direct path preserves
        // them).
        let mut map = http::HeaderMap::new();
        map.insert(
            http::HeaderName::from_static("x-note"),
            http::HeaderValue::from_bytes(&[b'h', b'i', 0x80]).expect("obs-text is a valid value"),
        );
        let headers = Headers::from(map);

        let (mut client_io, mut server_io) = tokio::io::duplex(1024);
        super::write_proxy_request(
            &mut client_io,
            &http::Method::GET,
            "http://origin.example/path",
            http::Version::HTTP_11,
            &headers,
            None,
            &Headers::new(),
            RequestBody::Empty,
        )
        .await
        .unwrap();
        drop(client_io);

        let mut wire = Vec::new();
        server_io.read_to_end(&mut wire).await.unwrap();
        let needle: &[u8] = b"x-note: hi\x80\r\n";
        assert!(
            wire.windows(needle.len()).any(|w| w == needle),
            "raw obs-text value must reach the wire, got: {:?}",
            String::from_utf8_lossy(&wire)
        );
    }
}
