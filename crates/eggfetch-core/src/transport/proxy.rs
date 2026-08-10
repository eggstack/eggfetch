//! Proxy request handling for HTTP and HTTPS destinations.

use bytes::Bytes;

use crate::body::{BoxBytesStream, RequestBody, ResponseBody};
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::proxy::{ProxyAuth, ProxyConfig};
use crate::response::Response;
use crate::timeout::TimeoutPhase;

pub(crate) struct ProxyRequestContext<'a> {
    pub(crate) remaining_total: Option<std::time::Duration>,
    pub(crate) tls_config: Option<&'a crate::tls::TlsConfig>,
}

/// Send a request through a proxy.
///
/// Routes to HTTP forwarding, CONNECT tunneling, or SOCKS5 tunneling
/// based on the proxy scheme and destination scheme.
pub(crate) async fn send_proxy_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    proxy_config: &ProxyConfig,
    ctx: &ProxyRequestContext<'_>,
) -> Result<Response> {
    if proxy_config.is_socks() {
        return send_socks_request(dest_url, method, headers, body, version, proxy_config, ctx)
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
                ctx.remaining_total,
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
    remaining_total: Option<std::time::Duration>,
) -> Result<tokio::io::BufReader<tokio::net::TcpStream>> {
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

    let stream = match remaining_total {
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

    Ok(tokio::io::BufReader::new(stream))
}

/// Send an HTTP request through an HTTP forward proxy.
async fn send_http_proxy_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    proxy_config: &ProxyConfig,
    remaining_total: Option<std::time::Duration>,
) -> Result<Response> {
    let mut stream = connect_to_proxy(proxy_config, remaining_total).await?;

    // Write the proxy request with absolute-form URI.
    let absolute_uri = dest_url.as_str();
    write_proxy_request(
        &mut stream,
        method,
        absolute_uri,
        version,
        headers,
        proxy_config.auth(),
        body,
    )
    .await?;

    // Read the response from the proxy.
    let (status, resp_headers, initial_buf) = read_proxy_response(&mut stream).await?;

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
    let (read_half, _write_half) = stream_reader.into_split();
    let body_stream = ProxyResponseStream::new(initial_buf, read_half);
    let body_stream = Box::pin(body_stream) as BoxBytesStream;
    let body = ResponseBody::streaming(body_stream);

    Ok(Response::new(status, version, resp_headers_map, url, body))
}

/// Write an HTTP request to a stream.
pub(crate) async fn write_proxy_request<S: tokio::io::AsyncWrite + Unpin>(
    stream: &mut S,
    method: &http::Method,
    uri: &str,
    version: http::Version,
    headers: &Headers,
    proxy_auth: Option<&ProxyAuth>,
    body: RequestBody,
) -> Result<()> {
    use std::fmt::Write;
    use tokio::io::AsyncWriteExt;

    let version_str = match version {
        http::Version::HTTP_10 => "HTTP/1.0",
        _ => "HTTP/1.1",
    };

    let mut request = format!("{method} {uri} {version_str}\r\n");

    // Add Host header if not present.
    if !headers.contains("host") {
        if let Ok(parsed) = url::Url::parse(uri) {
            let host = if let Some(port) = parsed.port() {
                format!("{}:{port}", parsed.host_str().unwrap_or(""))
            } else {
                parsed.host_str().unwrap_or("").to_string()
            };
            let _ = write!(request, "Host: {host}\r\n");
        }
    }

    // Write regular headers.
    for (name, value) in headers.iter() {
        // Skip proxy-authorization from destination headers.
        if name.as_str().eq_ignore_ascii_case("proxy-authorization") {
            continue;
        }
        if let Ok(value_str) = value.to_str() {
            let _ = write!(request, "{}: {value_str}\r\n", name.as_str());
        }
    }

    // Add Proxy-Authorization if configured.
    if let Some(auth) = proxy_auth {
        let _ = write!(request, "Proxy-Authorization: {}\r\n", auth.header_value());
    }

    request.push_str("\r\n");

    stream
        .write_all(request.as_bytes())
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
            break;
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
) -> Result<(u16, Vec<(String, String)>, Vec<u8>)> {
    use tokio::io::AsyncReadExt;

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

    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| {
            Error::MalformedProxyResponse(format!("invalid status line: {status_line}"))
        })?;

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

        headers.push(
            line.split_once(':')
                .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
                .ok_or_else(|| {
                    Error::MalformedProxyResponse(format!("invalid header line: {line}"))
                })?,
        );

        if headers.len() > MAX_HEADER_COUNT {
            return Err(Error::MalformedProxyResponse(format!(
                "proxy response exceeded maximum header count of {MAX_HEADER_COUNT}"
            )));
        }
    }

    let mut initial_buf = Vec::new();
    let buf_ref = stream.buffer();
    if !buf_ref.is_empty() {
        initial_buf.extend_from_slice(buf_ref);
        let consumed = buf_ref.len();
        let mut discard = vec![0u8; consumed];
        let _ = stream.read(&mut discard).await;
    }

    Ok((status_code, headers, initial_buf))
}

/// Streaming response body from an HTTP proxy.
///
/// Yields data from an initial buffer first, then reads from the
/// underlying TCP stream.
struct ProxyResponseStream {
    initial_buf: std::io::Cursor<Vec<u8>>,
    inner: tokio::net::tcp::OwnedReadHalf,
}

/// Send a request through a SOCKS5 proxy.
///
/// Performs the SOCKS5 handshake, then either:
/// - Sends HTTP directly over the tunnel (for `http://` destinations)
/// - Performs TLS over the tunnel, then sends HTTP (for `https://` destinations)
async fn send_socks_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    proxy_config: &ProxyConfig,
    ctx: &ProxyRequestContext<'_>,
) -> Result<Response> {
    let dest_host = dest_url
        .host_str()
        .ok_or_else(|| Error::InvalidUrl("destination URL has no host".into()))?;
    let dest_port = dest_url.port_or_known_default().unwrap_or(443);
    let remote_dns = proxy_config.socks_remote_dns();

    // Perform the SOCKS5 handshake.
    let stream = super::socks::socks5_handshake(
        proxy_config,
        dest_host,
        dest_port,
        remote_dns,
        ctx.remaining_total,
    )
    .await?;

    match dest_url.scheme() {
        "http" => send_socks_http_request(dest_url, method, headers, body, version, stream).await,
        "https" => {
            send_socks_https_request(dest_url, method, headers, body, version, stream, ctx).await
        }
        other => Err(Error::Unsupported(format!(
            "unsupported destination scheme '{other}' through SOCKS proxy"
        ))),
    }
}

/// Send an HTTP request over an established SOCKS5 tunnel.
async fn send_socks_http_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    stream: tokio::io::BufReader<tokio::net::TcpStream>,
) -> Result<Response> {
    let mut stream = stream;

    // Write the HTTP request with absolute-form URI.
    let absolute_uri = dest_url.as_str();
    write_proxy_request(
        &mut stream,
        method,
        absolute_uri,
        version,
        headers,
        None, // No proxy auth for the destination request.
        body,
    )
    .await?;

    // Read the response.
    let (status, resp_headers, initial_buf) = read_proxy_response(&mut stream).await?;

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

    let stream_reader = stream.into_inner();
    let (read_half, _write_half) = stream_reader.into_split();
    let body_stream = ProxyResponseStream::new(initial_buf, read_half);
    let body_stream = Box::pin(body_stream) as BoxBytesStream;
    let body = ResponseBody::streaming(body_stream);

    Ok(Response::new(status, version, resp_headers_map, url, body))
}

/// Send an HTTPS request over an established SOCKS5 tunnel.
///
/// Performs TLS handshake through the tunnel, then sends the HTTP
/// request over the encrypted connection.
async fn send_socks_https_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    stream: tokio::io::BufReader<tokio::net::TcpStream>,
    ctx: &ProxyRequestContext<'_>,
) -> Result<Response> {
    let dest_host = dest_url
        .host_str()
        .ok_or_else(|| Error::InvalidUrl("destination URL has no host".into()))?;

    // Get the raw TCP stream from the BufReader.
    let tcp_stream = stream.into_inner();

    // The tunnel is established. Wrap with initial buffer for TLS.
    let tunnel = super::connect::ProxyTunnel::new(Vec::new(), tcp_stream);

    // Perform TLS handshake with the destination through the tunnel.
    let rustls_config = if let Some(tc) = ctx.tls_config {
        tc.build_rustls_config()
            .map_err(|e| Error::Tls(format!("failed to build TLS config for SOCKS tunnel: {e}")))?
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
    let tls_stream = match ctx.remaining_total {
        Some(dur) => match tokio::time::timeout(dur, tls_handshake).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                return Err(Error::Tls(format!(
                    "TLS handshake through SOCKS tunnel failed: {e}"
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
            .map_err(|e| Error::Tls(format!("TLS handshake through SOCKS tunnel failed: {e}")))?,
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
    let body_stream = super::connect::TlsProxyResponseStream::new(initial_buf, stream_reader);
    let body_stream = Box::pin(body_stream) as BoxBytesStream;
    let body = ResponseBody::streaming(body_stream);

    Ok(Response::new(status, version, resp_headers_map, url, body))
}

impl ProxyResponseStream {
    fn new(initial_buf: Vec<u8>, inner: tokio::net::tcp::OwnedReadHalf) -> Self {
        Self {
            initial_buf: std::io::Cursor::new(initial_buf),
            inner,
        }
    }
}

impl futures_core::Stream for ProxyResponseStream {
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

        // Read from the inner TCP stream using poll_read.
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
