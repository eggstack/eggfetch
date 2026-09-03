//! Proxy request handling for HTTP and HTTPS destinations.

use bytes::{Bytes, BytesMut};

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
    /// HTTP version policy from the client.  Used to reject unsupported
    /// combinations (e.g. `Http2Only` through an HTTP forward proxy)
    /// before any wire I/O is attempted.
    pub(crate) http_version_policy: crate::http_version::HttpVersionPolicy,
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
    let proxy_port = proxy_config.port()?;

    let connect_future = async {
        let addrs = tokio::net::lookup_host(format!("{proxy_host}:{proxy_port}"))
            .await
            .map_err(|e| {
                Error::ProxyConnect(format!(
                    "DNS resolution failed for proxy {proxy_host}:{proxy_port}: {e}"
                ))
            })?;
        let mut last_error: Option<String> = None;
        let mut connected: Option<tokio::net::TcpStream> = None;
        for addr in addrs {
            match tokio::net::TcpStream::connect(addr).await {
                Ok(stream) => {
                    stream
                        .set_nodelay(true)
                        .map_err(|e| Error::ProxyConnect(format!("failed to set nodelay: {e}")))?;
                    connected = Some(stream);
                    break;
                }
                Err(e) => last_error = Some(e.to_string()),
            }
        }
        connected.ok_or_else(|| {
            Error::ProxyConnect(format!(
                "TCP connect to proxy {proxy_host}:{proxy_port} failed: {}",
                last_error.unwrap_or_else(|| "no addresses resolved".into())
            ))
        })
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
            let fallback = rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth();
            // The proxy leg is always HTTP/1.1 (CONNECT/forwarding); never
            // apply the origin H2-only policy here. Advertise http/1.1 only.
            crate::client::configure_tls_alpn(
                fallback,
                crate::http_version::HttpVersionPolicyEnabler::from_policy(
                    crate::http_version::HttpVersionPolicy::Http1Only,
                ),
            )
        };
        let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(rustls_config));
        let domain = proxy_server_name(proxy_host)?;
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

/// Build the TLS server name for a proxy endpoint.
///
/// `ServerName::try_from(&str)` handles DNS names, but IP literals must use
/// rustls's dedicated IP variant so HTTPS proxies addressed by IP can still
/// validate certificates containing an IP subject alternative name.
fn proxy_server_name(proxy_host: &str) -> Result<rustls::pki_types::ServerName<'static>> {
    crate::transport::direct_connector::tls_server_name(proxy_host)
        .map_err(|e| Error::Tls(format!("invalid proxy TLS server name: {e}")))
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
    // HTTP/2-only policy cannot be satisfied through an HTTP forward
    // proxy: the proxy leg is inherently HTTP/1.x (no HTTP/2 framing over
    // an absolute-form request line).  Reject before opening any
    // connection rather than silently downgrading.
    if matches!(
        ctx.http_version_policy,
        crate::http_version::HttpVersionPolicy::Http2Only
    ) {
        return Err(Error::Unsupported(
            "HTTP/2-only policy is not supported through an HTTP forward proxy".into(),
        ));
    }

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
        let value = http::HeaderValue::from_bytes(value)
            .map_err(|e| Error::MalformedProxyResponse(format!("invalid header value: {e}")))?;
        resp_headers_map.append(name, value);
    }

    // Return the body as a streaming response.
    let stream_reader = stream.into_inner();
    let expected_len = response_content_length(&resp_headers);
    let body_stream = ProxyResponseStream::new(initial_buf, stream_reader, expected_len);
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

    let chunked_body = matches!(&body, RequestBody::Stream { length: None, .. });
    if chunked_body && version != http::Version::HTTP_11 {
        return Err(Error::RequestBuild(
            "unknown-length proxy request bodies require HTTP/1.1".into(),
        ));
    }

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
    // separately below from configured auth). When a header exists in both
    // sets, the origin request wins and the proxy-only value is omitted.
    for (name, value) in proxy_headers.iter() {
        if name.as_str().eq_ignore_ascii_case("proxy-authorization") {
            continue;
        }
        if headers.contains(name.as_str()) {
            continue;
        }
        push_header_line(&mut request, name.as_str(), value.as_bytes());
    }

    if chunked_body {
        let transfer_encoding = headers
            .get("transfer-encoding")
            .or_else(|| proxy_headers.get("transfer-encoding"));
        if let Some(value) = transfer_encoding {
            if !value.as_bytes().eq_ignore_ascii_case(b"chunked") {
                return Err(Error::RequestBuild(
                    "unknown-length proxy request bodies require chunked transfer encoding".into(),
                ));
            }
        } else {
            request.extend_from_slice(b"Transfer-Encoding: chunked\r\n");
        }
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

    write_proxy_body(stream, body, chunked_body).await?;

    stream
        .flush()
        .await
        .map_err(|e| Error::ProxyConnect(format!("failed to flush: {e}")))?;

    Ok(())
}

async fn write_proxy_body<S: tokio::io::AsyncWrite + Unpin>(
    stream: &mut S,
    body: RequestBody,
    chunked: bool,
) -> Result<()> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

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
            while let Some(chunk) = body_stream.next().await {
                let chunk = chunk.map_err(|e| Error::Body(e.to_string()))?;
                if chunked {
                    if chunk.is_empty() {
                        continue;
                    }
                    stream
                        .write_all(format!("{:X}\r\n", chunk.len()).as_bytes())
                        .await
                        .map_err(|e| Error::ProxyConnect(format!("failed to write body: {e}")))?;
                    stream
                        .write_all(&chunk)
                        .await
                        .map_err(|e| Error::ProxyConnect(format!("failed to write body: {e}")))?;
                    stream
                        .write_all(b"\r\n")
                        .await
                        .map_err(|e| Error::ProxyConnect(format!("failed to write body: {e}")))?;
                } else {
                    stream
                        .write_all(&chunk)
                        .await
                        .map_err(|e| Error::ProxyConnect(format!("failed to write body: {e}")))?;
                }
            }
            if chunked {
                stream
                    .write_all(b"0\r\n\r\n")
                    .await
                    .map_err(|e| Error::ProxyConnect(format!("failed to write body: {e}")))?;
            }
        }
    }
    Ok(())
}

async fn read_bounded_line<S: tokio::io::AsyncRead + Unpin>(
    stream: &mut tokio::io::BufReader<S>,
    max_len: usize,
) -> Result<Vec<u8>> {
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
    Ok(buf)
}

fn trim_ows(value: &[u8]) -> &[u8] {
    let start = value
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t'))
        .unwrap_or(value.len());
    let end = value
        .iter()
        .rposition(|byte| !matches!(byte, b' ' | b'\t'))
        .map_or(start, |index| index + 1);
    &value[start..end]
}

/// Read an HTTP response from a proxy or destination.
///
/// Returns `(status_code, headers, remaining_initial_bytes)`.
pub(crate) async fn read_proxy_response<S: tokio::io::AsyncRead + Unpin>(
    stream: &mut tokio::io::BufReader<S>,
) -> Result<(u16, Vec<(String, Vec<u8>)>, Vec<u8>, Option<String>)> {
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

    let mut parts = status_line.splitn(3, |byte| *byte == b' ');
    let _version = parts.next();
    let status_code = parts
        .next()
        .and_then(|s| std::str::from_utf8(s).ok())
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| {
            Error::MalformedProxyResponse(format!(
                "invalid status line: {}",
                String::from_utf8_lossy(&status_line)
            ))
        })?;
    // The third part (if present) is the reason phrase.
    let reason_phrase = parts
        .next()
        .map(|phrase| String::from_utf8_lossy(phrase).into_owned());

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

        let colon = line.iter().position(|byte| *byte == b':').ok_or_else(|| {
            Error::MalformedProxyResponse(format!(
                "invalid header line: {}",
                String::from_utf8_lossy(&line)
            ))
        })?;
        let name = std::str::from_utf8(trim_ows(&line[..colon])).map_err(|_| {
            Error::MalformedProxyResponse("proxy response header name is not ASCII".into())
        })?;
        headers.push((name.to_owned(), trim_ows(&line[colon + 1..]).to_vec()));
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
    chunk: BytesMut,
    /// Declared `Content-Length`, if the response carried an explicit
    /// non-chunked length. Used only to tell a truncated body apart from
    /// a complete body followed by an abrupt close.
    expected_len: Option<u64>,
    /// Body bytes yielded so far.
    delivered: u64,
}

impl<S> ProxyResponseStream<S> {
    /// Returns `true` once every declared body byte has been delivered.
    ///
    /// Close-delimited bodies (`expected_len == None`) report complete
    /// only when the peer actually closes: EOF is their framing.
    fn body_is_complete(&self) -> bool {
        self.expected_len.is_some_and(|n| self.delivered >= n)
    }
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
    fn new(initial_buf: Vec<u8>, inner: S, expected_len: Option<u64>) -> Self {
        Self {
            initial_buf: std::io::Cursor::new(initial_buf),
            inner,
            chunk: BytesMut::with_capacity(8192),
            expected_len,
            delivered: 0,
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

        // Read from the proxy stream using poll_read.
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
                        return std::task::Poll::Ready(Some(Err(Error::MalformedProxyResponse(
                            format!("proxy closed connection unexpectedly: {e}"),
                        ))));
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

/// Declared body length for a hand-rolled proxy response.
///
/// Returns the `Content-Length` value when the response has an explicit,
/// non-chunked length so body streams can distinguish a truncated body
/// (EOF before every declared byte arrived) from a complete body followed
/// by an abrupt close. Returns `None` for chunked or close-delimited
/// bodies, where this layer has no declared length to check against.
pub(crate) fn response_content_length(headers: &[(String, Vec<u8>)]) -> Option<u64> {
    let mut content_length: Option<u64> = None;
    let mut chunked = false;
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("transfer-encoding")
            && std::str::from_utf8(value)
                .unwrap_or_default()
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("chunked"))
        {
            chunked = true;
        }
        if name.eq_ignore_ascii_case("content-length") && content_length.is_none() {
            content_length = std::str::from_utf8(value).ok()?.trim().parse().ok();
        }
    }
    if chunked {
        None
    } else {
        content_length
    }
}

#[cfg(test)]
mod tests {
    use super::{
        effective_timeout, proxy_server_name, read_proxy_response, response_content_length,
        write_proxy_request, ProxyResponseStream,
    };
    use crate::error::Error;
    use crate::timeout::TimeoutPhase;
    use std::time::{Duration, Instant};

    /// Simulates a peer that sends `data` then closes abruptly (no TLS
    /// `close_notify`), surfacing `UnexpectedEof` like rustls does.
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

    #[test]
    fn response_content_length_parses_declared_length() {
        let headers = vec![("Content-Length".to_owned(), b"42".to_vec())];
        assert_eq!(response_content_length(&headers), Some(42));
    }

    #[test]
    fn response_content_length_prefers_explicit_over_chunked() {
        // Chunked framing owns the body boundaries; a stale
        // `Content-Length` must not be treated as the declared length.
        let headers = vec![
            ("Content-Length".to_owned(), b"42".to_vec()),
            ("Transfer-Encoding".to_owned(), b"chunked".to_vec()),
        ];
        assert_eq!(response_content_length(&headers), None);
    }

    #[test]
    fn response_content_length_absent_without_length() {
        let headers = vec![("Content-Type".to_owned(), b"text/plain".to_vec())];
        assert_eq!(response_content_length(&headers), None);
    }

    #[tokio::test]
    async fn proxy_body_stream_complete_body_survives_abrupt_close() {
        use futures_util::StreamExt as _;
        // Full declared body arrives, then the peer closes without TLS
        // `close_notify` (normal for real servers): the stream ends
        // cleanly instead of failing the already-complete body.
        let mut stream = ProxyResponseStream::new(b"ok".to_vec(), AbruptReader::new(b""), Some(2));
        let first = stream.next().await.expect("body chunk").expect("no error");
        assert_eq!(first.as_ref(), b"ok");
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn proxy_body_stream_truncated_body_still_errors() {
        use futures_util::StreamExt as _;
        // Only 1 of 2 declared bytes arrives before the abrupt close:
        // this is a genuine truncation and must stay an error.
        let mut stream = ProxyResponseStream::new(Vec::new(), AbruptReader::new(b"o"), Some(2));
        let first = stream.next().await.expect("body chunk").expect("no error");
        assert_eq!(first.as_ref(), b"o");
        let err = stream.next().await.expect("stream item").unwrap_err();
        assert!(
            matches!(err, Error::MalformedProxyResponse(_)),
            "truncation must stay an error, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn proxy_body_stream_close_delimited_ends_at_eof() {
        use futures_util::StreamExt as _;
        // No declared length: EOF is the framing, abrupt or not.
        let mut stream = ProxyResponseStream::new(Vec::new(), AbruptReader::new(b"data"), None);
        let first = stream.next().await.expect("body chunk").expect("no error");
        assert_eq!(first.as_ref(), b"data");
        assert!(stream.next().await.is_none());
    }

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
    fn proxy_tls_server_name_accepts_ip_literals() {
        assert!(proxy_server_name("127.0.0.1").is_ok());
        assert!(proxy_server_name("proxy.example").is_ok());
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
    async fn read_proxy_response_preserves_obs_text_header_values() {
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(
            b"HTTP/1.1 200 OK\r\nX-Note: hi\x80\xff\r\n\r\nbody".to_vec(),
        ));
        let (_, headers, initial, _) = read_proxy_response(&mut reader).await.unwrap();
        assert_eq!(headers, vec![("X-Note".to_owned(), b"hi\x80\xff".to_vec())]);
        assert_eq!(initial, b"body");
    }

    #[tokio::test]
    async fn write_proxy_request_chunks_unknown_length_streams() {
        use crate::body::RequestBody;
        use crate::headers::Headers;
        use bytes::Bytes;
        use futures_util::stream;
        use tokio::io::AsyncReadExt;

        let (mut client_io, mut server_io) = tokio::io::duplex(1024);
        write_proxy_request(
            &mut client_io,
            &http::Method::POST,
            "http://origin.example/upload",
            http::Version::HTTP_11,
            &Headers::new(),
            None,
            &Headers::new(),
            RequestBody::from_stream(
                stream::iter(vec![Ok(Bytes::from_static(b"ab")), Ok(Bytes::new())]),
                None,
            ),
        )
        .await
        .unwrap();
        drop(client_io);

        let mut wire = Vec::new();
        server_io.read_to_end(&mut wire).await.unwrap();
        assert!(wire
            .windows(b"Transfer-Encoding: chunked\r\n".len())
            .any(|w| { w == b"Transfer-Encoding: chunked\r\n" }));
        assert!(wire.ends_with(b"\r\n\r\n2\r\nab\r\n0\r\n\r\n"));
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
