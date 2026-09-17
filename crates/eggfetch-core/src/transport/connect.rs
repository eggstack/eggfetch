//! HTTPS CONNECT tunnel support for proxy connections.

use bytes::{Bytes, BytesMut};
use std::net::SocketAddr;

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

/// Internal identity for one reusable CONNECT tunnel client.
#[cfg(any(feature = "http1", feature = "http2"))]
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct ConnectRouteKey {
    proxy_identity: Vec<u8>,
    origin: String,
    target: Option<SocketAddr>,
    origin_tls_identity: u64,
    sni_hostname: Option<String>,
    http_version_policy: crate::http_version::HttpVersionPolicy,
    connect_timeout: Option<std::time::Duration>,
    proxy_tls_timeout: Option<std::time::Duration>,
}

#[cfg(any(feature = "http1", feature = "http2"))]
impl ConnectRouteKey {
    #[allow(
        clippy::too_many_arguments,
        reason = "the cache identity must enumerate every connection-affecting CONNECT policy"
    )]
    pub(crate) fn new(
        proxy: &ProxyConfig,
        origin: &url::Url,
        target: Option<SocketAddr>,
        origin_tls_config: Option<&crate::tls::TlsConfig>,
        sni_hostname: Option<&str>,
        http_version_policy: crate::http_version::HttpVersionPolicy,
        connect_timeout: Option<std::time::Duration>,
        proxy_tls_timeout: Option<std::time::Duration>,
    ) -> Self {
        Self {
            proxy_identity: proxy.connection_identity(),
            origin: format!(
                "{}://{}:{}",
                origin.scheme(),
                origin.host_str().unwrap_or_default(),
                origin.port_or_known_default().unwrap_or(443)
            ),
            target,
            origin_tls_identity: origin_tls_config
                .map_or(0, crate::tls::TlsConfig::connection_identity),
            sni_hostname: sni_hostname.map(str::to_owned),
            http_version_policy,
            connect_timeout,
            proxy_tls_timeout,
        }
    }
}

/// Origin-ready TLS stream returned by the Hyper CONNECT connector.
#[cfg(any(feature = "http1", feature = "http2"))]
pub(crate) struct ConnectProxyStream {
    inner: tokio_rustls::client::TlsStream<ProxyTunnel<super::proxy::ProxyIo>>,
}

#[cfg(any(feature = "http1", feature = "http2"))]
impl tokio::io::AsyncRead for ConnectProxyStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

#[cfg(any(feature = "http1", feature = "http2"))]
impl tokio::io::AsyncWrite for ConnectProxyStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bytes: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, bytes)
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

#[cfg(any(feature = "http1", feature = "http2"))]
impl hyper_util::client::legacy::connect::Connection for ConnectProxyStream {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
        let connected = hyper_util::client::legacy::connect::Connected::new();
        if self
            .inner
            .get_ref()
            .1
            .alpn_protocol()
            .is_some_and(|protocol| protocol == b"h2")
        {
            connected.negotiated_h2()
        } else {
            connected
        }
    }
}

/// Narrow CONNECT connector retaining eggfetch's handshake policy while
/// returning an origin-ready stream to Hyper's reusable client.
///
/// The connector owns only connection-scoped policy. In particular it must
/// never retain request-scoped `TransportHints::target`/`trace`,
/// read/write/total budgets, retry/redirect state, bodies, or failure
/// contexts: the SNI hostname is part of the route key and therefore safe to
/// retain, while wire target overrides and trace observers belong to the
/// current request and are supplied separately at dispatch time.
#[cfg(any(feature = "http1", feature = "http2"))]
#[derive(Clone)]
pub(crate) struct ConnectProxyConnector {
    dest_url: url::Url,
    proxy: ProxyConfig,
    sni_hostname: Option<String>,
    proxied_target: Option<SocketAddr>,
    origin_tls_config: Option<crate::tls::TlsConfig>,
    connect_timeout: Option<std::time::Duration>,
    proxy_connect_timeout: Option<std::time::Duration>,
    proxy_tls_timeout: Option<std::time::Duration>,
    http_version_policy: crate::http_version::HttpVersionPolicy,
    metrics: std::sync::Arc<crate::transport::metrics::TransportMetrics>,
}

#[cfg(any(feature = "http1", feature = "http2"))]
impl ConnectProxyConnector {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        dest_url: url::Url,
        proxy: ProxyConfig,
        transport_hints: &crate::request::TransportHints,
        proxied_target: Option<SocketAddr>,
        origin_tls_config: Option<crate::tls::TlsConfig>,
        connect_timeout: Option<std::time::Duration>,
        proxy_connect_timeout: Option<std::time::Duration>,
        proxy_tls_timeout: Option<std::time::Duration>,
        http_version_policy: crate::http_version::HttpVersionPolicy,
        metrics: std::sync::Arc<crate::transport::metrics::TransportMetrics>,
    ) -> Self {
        Self {
            dest_url,
            proxy,
            // Retain only the connection-affecting SNI hint (already part of
            // `ConnectRouteKey`). Wire target overrides, resolved targets,
            // and trace observers stay request-scoped and are never cached.
            sni_hostname: transport_hints.sni_hostname.clone(),
            proxied_target,
            origin_tls_config,
            connect_timeout,
            proxy_connect_timeout,
            proxy_tls_timeout,
            http_version_policy,
            metrics,
        }
    }
}

#[cfg(any(feature = "http1", feature = "http2"))]
impl tower_service::Service<http::Uri> for ConnectProxyConnector {
    type Response = hyper_util::rt::TokioIo<ConnectProxyStream>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = std::pin::Pin<
        Box<
            dyn std::future::Future<Output = std::result::Result<Self::Response, Self::Error>>
                + Send,
        >,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _dst: http::Uri) -> Self::Future {
        let dest_url = self.dest_url.clone();
        let proxy = self.proxy.clone();
        let sni_hostname = self.sni_hostname.clone();
        let proxied_target = self.proxied_target;
        let origin_tls_config = self.origin_tls_config.clone();
        let connect_timeout = self.connect_timeout;
        let proxy_connect_timeout = self.proxy_connect_timeout;
        let proxy_tls_timeout = self.proxy_tls_timeout;
        let http_version_policy = self.http_version_policy;
        let metrics = self.metrics.clone();
        Box::pin(async move {
            // The cached connector owns only connection-scoped policy. The
            // current logical request's total budget is enforced around the
            // Hyper dispatch future, not captured by this reusable client.
            // Only the keyed SNI hint is retained; wire target overrides,
            // resolved targets, and trace observers stay request-scoped.
            let transport_hints = crate::request::TransportHints {
                sni_hostname,
                ..Default::default()
            };
            let ctx = ProxyRequestContext {
                remaining_total: None,
                deadline: None,
                connect_timeout,
                proxy_connect_timeout,
                proxy_tls_timeout,
                write_timeout: None,
                read_timeout: None,
                http_version_policy,
                origin_tls_config: origin_tls_config.as_ref(),
                proxy_tls_config: proxy.proxy_tls_config(),
                proxied_target: None,
                socks_client: None,
                #[cfg(any(feature = "http1", feature = "http2"))]
                forward_client: None,
                #[cfg(any(feature = "http1", feature = "http2"))]
                connect_client: None,
                failure_context: None,
                transport_metrics: Some(metrics),
            };
            let stream =
                establish_https_tunnel(&dest_url, &proxy, &transport_hints, &ctx, proxied_target)
                    .await
                    .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)?;
            Ok(hyper_util::rt::TokioIo::new(ConnectProxyStream {
                inner: stream,
            }))
        })
    }
}

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
    #[cfg(any(feature = "http1", feature = "http2"))]
    if let Some(client) = ctx.connect_client.as_ref() {
        return send_https_connect_request_hyper(
            dest_url,
            method,
            headers,
            body,
            version,
            transport_hints,
            client,
            ctx.failure_context,
        )
        .await;
    }
    let targets: Vec<Option<SocketAddr>> = match ctx.proxied_target {
        Some(target) => target.addresses().iter().copied().map(Some).collect(),
        None => vec![None],
    };
    let last = targets.len().saturating_sub(1);
    let mut body = Some(body);
    let mut last_error = None;

    for (index, target) in targets.into_iter().enumerate() {
        let attempt_body = if index == last {
            body.take().ok_or_else(|| {
                crate::error::Error::Connect("missing request body for CONNECT attempt".into())
            })?
        } else {
            body.as_ref()
                .ok_or_else(|| {
                    crate::error::Error::Connect("missing request body for CONNECT retry".into())
                })?
                .try_clone_for_retry()?
        };
        match send_https_connect_request_once(
            dest_url,
            method,
            headers,
            attempt_body,
            version,
            proxy_config,
            transport_hints,
            ctx,
            target,
        )
        .await
        {
            Ok(response) => return Ok(response),
            Err(error) if index < last && target_failure_may_retry(&error) => {
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error
        .unwrap_or_else(|| crate::error::Error::Connect("no CONNECT targets attempted".into())))
}

/// Send an origin-form request over a CONNECT tunnel whose transport and
/// lifecycle are owned by Hyper's reusable client.
#[cfg(any(feature = "http1", feature = "http2"))]
#[allow(clippy::too_many_arguments)]
async fn send_https_connect_request_hyper(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    transport_hints: &crate::request::TransportHints,
    client: &crate::transport::TimeoutConnectClient,
    failure_context: Option<&crate::error::RequestFailureContext>,
) -> Result<Response> {
    let target = if let Some(target) = transport_hints.target.as_deref() {
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
    // Hyper needs an absolute URI to select the connector. Because the
    // connector reports an origin connection, Hyper emits only the
    // path-and-query on the wire. Preserve ordinary origin-form overrides by
    // attaching them to the logical authority; unusual targets fall back to
    // the legacy path below.
    let request_uri = if target.starts_with('/') {
        format!(
            "{}://{}{}",
            dest_url.scheme(),
            dest_url.host_str().unwrap_or_default(),
            target
        )
    } else {
        dest_url.as_str().to_owned()
    };
    let uri = request_uri
        .parse::<http::Uri>()
        .map_err(|error| Error::RequestBuild(format!("invalid CONNECT target: {error}")))?;
    let mut request = http::Request::builder()
        .method(method)
        .uri(uri)
        .version(version);
    for (name, value) in headers.iter() {
        request = request.header(name, value);
    }
    let request = request
        .body(body.into_http_body())
        .map_err(|error| Error::RequestBuild(error.to_string()))?;
    crate::transport::direct::send_request(
        client,
        request,
        dest_url.clone(),
        transport_hints.trace.as_deref(),
        failure_context,
    )
    .await
}

fn target_failure_may_retry(error: &Error) -> bool {
    matches!(
        error,
        Error::ProxyConnectRejected {
            status: 502 | 504,
            ..
        }
    )
}

/// Establish and authenticate an HTTPS CONNECT tunnel, returning the
/// origin-ready TLS stream. Hyper owns HTTP framing after this boundary.
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn establish_https_tunnel(
    dest_url: &url::Url,
    proxy_config: &ProxyConfig,
    transport_hints: &crate::request::TransportHints,
    ctx: &ProxyRequestContext<'_>,
    proxied_target: Option<SocketAddr>,
) -> Result<tokio_rustls::client::TlsStream<ProxyTunnel<super::proxy::ProxyIo>>> {
    use tokio::io::AsyncWriteExt;
    let mut stream = connect_to_proxy(
        proxy_config,
        ctx.proxy_connect_timeout,
        ctx.proxy_tls_timeout,
        ctx.deadline,
        ctx.proxy_tls_config,
        ctx.transport_metrics.as_ref(),
    )
    .await?;
    let dest_host = dest_url
        .host_str()
        .ok_or_else(|| Error::InvalidUrl("destination URL has no host".into()))?;
    let dest_port = dest_url.port_or_known_default().unwrap_or(443);
    let connect_target = proxied_target.map_or_else(
        || authority_form_target(dest_host, dest_port),
        |address| authority_form_target(&address.ip().to_string(), address.port()),
    );
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
    for (name, value) in proxy_config.proxy_headers().iter() {
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
    let tunnel = ProxyTunnel::new(initial_buf, stream.into_inner());
    let default_tls_config = crate::tls::TlsConfig::default();
    let tls_config = ctx.origin_tls_config.unwrap_or(&default_tls_config);
    let rustls_config = tls_config
        .build_rustls_config()
        .map_err(|e| Error::Tls(format!("failed to build TLS config for tunnel: {e}")))?;
    let rustls_config = crate::client::configure_tls_alpn(
        rustls_config,
        crate::http_version::HttpVersionPolicyEnabler::from_policy(ctx.http_version_policy),
    );
    let tls_connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(rustls_config));
    let sni_name = transport_hints.sni_hostname.as_deref().unwrap_or(dest_host);
    let domain = crate::transport::direct_connector::tls_server_name(sni_name)
        .map_err(|e| Error::Tls(format!("invalid TLS server name: {e}")))?;
    let tls_handshake = tls_connector.connect(domain, tunnel);
    match effective_timeout(ctx.deadline, ctx.connect_timeout)? {
        Some(dur) => match tokio::time::timeout(dur, tls_handshake).await {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(e)) => Err(Error::Tls(format!(
                "TLS handshake through tunnel failed: {e}"
            ))),
            Err(_) => Err(Error::Timeout {
                phase: TimeoutPhase::Connect,
                elapsed: dur,
            }),
        },
        None => tls_handshake
            .await
            .map_err(|e| Error::Tls(format!("TLS handshake through tunnel failed: {e}"))),
    }
}

#[allow(clippy::too_many_lines)] // CONNECT owns the ordered proxy/tunnel/origin phases.
#[allow(clippy::too_many_arguments)] // Target pinning is an internal transport parameter.
async fn send_https_connect_request_once(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    proxy_config: &ProxyConfig,
    transport_hints: &crate::request::TransportHints,
    ctx: &ProxyRequestContext<'_>,
    proxied_target: Option<SocketAddr>,
) -> Result<Response> {
    let tls_stream =
        establish_https_tunnel(dest_url, proxy_config, transport_hints, ctx, proxied_target)
            .await?;

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
                this.delivered = this.delivered.saturating_add(n as u64);
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
                    this.delivered = this.delivered.saturating_add(n as u64);
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

    #[cfg(any(feature = "http1", feature = "http2"))]
    #[allow(
        clippy::too_many_lines,
        reason = "route-key matrix enumerates every connection dimension"
    )]
    #[test]
    fn connect_route_key_isolates_tls_policy_and_reuses_compatible() {
        use super::ConnectRouteKey;
        use crate::http_version::HttpVersionPolicy;
        use crate::proxy::Proxy;
        use crate::tls::TlsConfig;
        use std::time::Duration;

        // Connection-vs-request matrix (CONNECT route): connection-scoped
        // = proxy URI/auth/headers/TLS/pinned peer, origin, pinned target,
        // origin TLS token, SNI, HTTP version policy, connect-phase
        // timeouts. Request-scoped (never in key) = total/read/write/pool
        // deadlines, retry/redirect, body, cookies/auth headers,
        // decompression limits, trace/failure context, wire target
        // overrides. Keys intentionally lack `Debug`/`Display` (credential
        // material), so compare with `==`/`!=`.
        let proxy_base = TlsConfig::builder().build();
        let strict_proxy = Proxy::all("http://proxy.example:8080")
            .unwrap()
            .with_proxy_tls_config(proxy_base.clone())
            .config()
            .clone();
        let weak_proxy = Proxy::all("http://proxy.example:8080")
            .unwrap()
            .with_proxy_tls_config(proxy_base.clone().danger_accept_invalid_certs(true))
            .config()
            .clone();
        let origin = url::Url::parse("https://origin.example/").unwrap();
        let origin_tls = TlsConfig::builder().build();
        let policy = HttpVersionPolicy::Auto { allow_http3: false };

        let strict_key = ConnectRouteKey::new(
            &strict_proxy,
            &origin,
            None,
            Some(&origin_tls),
            None,
            policy,
            None,
            None,
        );
        let strict_again = ConnectRouteKey::new(
            &strict_proxy,
            &origin,
            None,
            Some(&origin_tls),
            None,
            policy,
            None,
            None,
        );
        assert!(strict_key == strict_again, "compatible CONNECT must reuse");

        let weak_key = ConnectRouteKey::new(
            &weak_proxy,
            &origin,
            None,
            Some(&origin_tls),
            None,
            policy,
            None,
            None,
        );
        assert!(
            strict_key != weak_key,
            "proxy TLS change must fragment CONNECT identity"
        );

        let weak_origin = origin_tls.clone().danger_accept_invalid_certs(true);
        let weak_origin_key = ConnectRouteKey::new(
            &strict_proxy,
            &origin,
            None,
            Some(&weak_origin),
            None,
            policy,
            None,
            None,
        );
        assert!(
            strict_key != weak_origin_key,
            "origin TLS change must fragment CONNECT identity"
        );

        // Table-driven remaining connection-affecting mutations.
        let other_origin = url::Url::parse("https://other.example/").unwrap();
        assert!(
            strict_key
                != ConnectRouteKey::new(
                    &strict_proxy,
                    &other_origin,
                    None,
                    Some(&origin_tls),
                    None,
                    policy,
                    None,
                    None,
                ),
            "origin change must fragment CONNECT identity"
        );
        let target: std::net::SocketAddr = "127.0.0.1:443".parse().unwrap();
        assert!(
            strict_key
                != ConnectRouteKey::new(
                    &strict_proxy,
                    &origin,
                    Some(target),
                    Some(&origin_tls),
                    None,
                    policy,
                    None,
                    None,
                ),
            "pinned target change must fragment CONNECT identity"
        );
        assert!(
            strict_key
                != ConnectRouteKey::new(
                    &strict_proxy,
                    &origin,
                    None,
                    Some(&origin_tls),
                    Some("other.example"),
                    policy,
                    None,
                    None,
                ),
            "SNI change must fragment CONNECT identity"
        );
        assert!(
            strict_key
                != ConnectRouteKey::new(
                    &strict_proxy,
                    &origin,
                    None,
                    Some(&origin_tls),
                    None,
                    HttpVersionPolicy::Http1Only,
                    None,
                    None,
                ),
            "HTTP version change must fragment CONNECT identity"
        );
        assert!(
            strict_key
                != ConnectRouteKey::new(
                    &strict_proxy,
                    &origin,
                    None,
                    Some(&origin_tls),
                    None,
                    policy,
                    Some(Duration::from_millis(100)),
                    None,
                ),
            "connect timeout change must fragment CONNECT identity"
        );
        let authed = Proxy::all("http://proxy.example:8080")
            .unwrap()
            .auth(crate::proxy::ProxyAuth::basic("user", "pass").unwrap())
            .config()
            .clone();
        assert!(
            strict_key
                != ConnectRouteKey::new(
                    &authed,
                    &origin,
                    None,
                    Some(&origin_tls),
                    None,
                    policy,
                    None,
                    None,
                ),
            "proxy auth change must fragment CONNECT identity"
        );

        // Request-only state is not represented: totals and other logical
        // request policy are not constructor inputs by design. Differing
        // totals reuse the same key; the behavior tests in `proxy_tests.rs`
        // prove sharing across 500 ms vs 5 s totals with forced reconnect.
        let key_a = ConnectRouteKey::new(
            &strict_proxy,
            &origin,
            None,
            Some(&origin_tls),
            None,
            policy,
            None,
            None,
        );
        let key_b = ConnectRouteKey::new(
            &strict_proxy,
            &origin,
            None,
            Some(&origin_tls),
            None,
            policy,
            None,
            None,
        );
        assert!(key_a == key_b, "request-only mutations must not fragment");
    }
}
