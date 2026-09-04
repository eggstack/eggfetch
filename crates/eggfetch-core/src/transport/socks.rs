//! SOCKS5 proxy handshake and tunnel establishment.
//!
//! Implements the SOCKS5 protocol (RFC 1928) for proxy connections,
//! including username/password authentication (RFC 1929). The
//! implementation is bounded to the subset required for HTTPX 0.28.1
//! parity: the pinned HTTPX/httpcore stack sends hostname destinations to the
//! proxy for both `socks5://` and `socks5h://`.
//!
//! # Protocol flow
//!
//! 1. TCP connect to SOCKS5 proxy
//! 2. Method negotiation (no-auth or username/password)
//! 3. Optional username/password subnegotiation
//! 4. CONNECT command with destination address
//! 5. Parse reply — tunnel established

use crate::error::{Error, Result};
use crate::proxy::{ProxyAuth, ProxyConfig};
use crate::timeout::TimeoutPhase;

/// Internal cache identity for one SOCKS route. The type deliberately does
/// not implement `Debug` or `Display`: credentials remain memory-only key
/// material and can never be emitted by route diagnostics.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct SocksRouteKey {
    scheme: String,
    host: String,
    port: u16,
    auth: Option<(String, String)>,
}

impl SocksRouteKey {
    pub(crate) fn from_proxy(proxy: &ProxyConfig) -> Result<Self> {
        let auth = proxy.auth().map(|auth| match auth {
            ProxyAuth::Basic { username, password } => (username.clone(), password.clone()),
        });
        Ok(Self {
            scheme: proxy.scheme().to_owned(),
            host: proxy.host().unwrap_or_default().to_owned(),
            port: proxy.port()?,
            auth,
        })
    }
}

/// SOCKS5 protocol version.
const SOCKS5_VERSION: u8 = 0x05;

/// No authentication method.
const SOCKS5_METHOD_NO_AUTH: u8 = 0x00;

/// Username/password authentication method.
const SOCKS5_METHOD_USERNAME_PASSWORD: u8 = 0x02;

/// No acceptable methods.
const SOCKS5_METHOD_NO_ACCEPTABLE: u8 = 0xFF;

/// Username/password subnegotiation version.
const SOCKS5_SUBNEG_VERSION: u8 = 0x01;

/// CONNECT command.
const SOCKS5_CMD_CONNECT: u8 = 0x01;

/// Address type: IPv4.
const SOCKS5_ATYP_IPV4: u8 = 0x01;

/// Address type: domain name.
const SOCKS5_ATYP_DOMAIN: u8 = 0x03;

/// Address type: IPv6.
const SOCKS5_ATYP_IPV6: u8 = 0x04;

/// Reply code: success.
const SOCKS5_REP_SUCCESS: u8 = 0x00;

/// Maximum domain name length for SOCKS5.
const MAX_SOCKS5_DOMAIN_LEN: usize = 255;

/// Maximum username/password length for SOCKS5 (RFC 1929).
const MAX_SOCKS5_CREDENTIAL_LEN: usize = 255;

/// Perform a SOCKS5 handshake and establish a tunnel to the destination.
///
/// Returns a `BufReader<TcpStream>` connected through the SOCKS5 proxy,
/// ready for HTTP or TLS communication with the origin.
///
/// # Arguments
///
/// * `proxy_config` — SOCKS5 proxy configuration (host, port, auth)
/// * `dest_host` — destination hostname or IP
/// * `dest_port` — destination port
/// * `remote_dns` — whether to send the domain name to the proxy for remote
///   DNS resolution. The compatibility facade passes `true` for both SOCKS
///   schemes because that is the HTTPX 0.28.1 behavior.
/// * `remaining_total` — optional timeout for the entire handshake
///
/// # Errors
///
/// Returns an error if:
/// - TCP connection to the proxy fails
/// - Method negotiation fails
/// - Authentication fails
/// - CONNECT command is rejected
/// - Proxy response is malformed
/// - Timeout expires
pub(crate) async fn socks5_handshake(
    proxy_config: &ProxyConfig,
    dest_host: &str,
    dest_port: u16,
    remote_dns: bool,
    remaining_total: Option<std::time::Duration>,
) -> Result<tokio::net::TcpStream> {
    let deadline = remaining_total.map(|duration| std::time::Instant::now() + duration);

    // Local-DNS path with multiple resolved addresses: try each destination
    // address with a fresh proxy connection; the first successful CONNECT
    // wins. A failed CONNECT may leave the proxy connection unusable, so
    // retries use a new connection rather than reusing the failed stream.
    if !remote_dns && dest_host.parse::<std::net::IpAddr>().is_err() {
        if let Ok(addrs) = resolve_dest_ips(dest_host, deadline).await {
            if addrs.len() > 1 {
                let mut last_err: Option<Error> = None;
                for ip in addrs {
                    match handshake_with_ip(proxy_config, ip, dest_port, deadline).await {
                        Ok(stream) => return Ok(stream),
                        Err(e) => last_err = Some(e),
                    }
                }
                return Err(last_err.unwrap_or_else(|| {
                    Error::Connect(format!(
                        "DNS resolution failed for SOCKS destination {dest_host}"
                    ))
                }));
            }
            // Zero or one address: fall through to the single-attempt path
            // (which re-resolves; cheap and keeps the code path unified).
        }
    }

    let mut stream = establish_socks_proxy_connection(proxy_config, deadline).await?;

    // Phase 4: CONNECT command.
    send_connect(&mut stream, dest_host, dest_port, remote_dns, deadline).await?;

    Ok(stream)
}

/// Establish a SOCKS5 proxy connection through method negotiation and auth.
///
/// Returns a stream ready for the CONNECT command (phases 1-3).
async fn establish_socks_proxy_connection(
    proxy_config: &ProxyConfig,
    deadline: Option<std::time::Instant>,
) -> Result<tokio::net::TcpStream> {
    let proxy_host = proxy_config.host().unwrap_or("127.0.0.1");
    let proxy_port = proxy_config.port()?;

    // Phase 1: TCP connect to proxy.
    let connect_future = async {
        let stream = tokio::net::TcpStream::connect((proxy_host, proxy_port))
            .await
            .map_err(|e| Error::ProxyConnect(format!("failed to connect to SOCKS5 proxy: {e}")))?;
        stream
            .set_nodelay(true)
            .map_err(|e| Error::ProxyConnect(format!("failed to set nodelay: {e}")))?;
        Ok::<_, Error>(stream)
    };

    let mut stream = match deadline {
        Some(deadline) => {
            let dur = deadline.saturating_duration_since(std::time::Instant::now());
            match tokio::time::timeout(dur, connect_future).await {
                Ok(Ok(s)) => s,
                Ok(Err(e)) => return Err(e),
                Err(_) => {
                    return Err(Error::Timeout {
                        phase: TimeoutPhase::ProxyConnect,
                        elapsed: dur,
                    });
                }
            }
        }
        None => connect_future.await?,
    };

    // Phase 2: Method negotiation.
    let selected_method = negotiate_method(&mut stream, proxy_config.auth(), deadline).await?;

    // Phase 3: Username/password authentication (if configured).
    if selected_method == SOCKS5_METHOD_USERNAME_PASSWORD {
        let auth = proxy_config.auth().ok_or_else(|| {
            Error::ProxyConnect(
                "SOCKS5 proxy selected username/password but no credentials were configured".into(),
            )
        })?;
        authenticate(&mut stream, auth, deadline).await?;
    }

    Ok(stream)
}

/// Full handshake against one pre-resolved destination IP (fresh proxy
/// connection + CONNECT with that IP).
async fn handshake_with_ip(
    proxy_config: &ProxyConfig,
    dest_ip: std::net::IpAddr,
    dest_port: u16,
    deadline: Option<std::time::Instant>,
) -> Result<tokio::net::TcpStream> {
    let mut stream = establish_socks_proxy_connection(proxy_config, deadline).await?;
    send_connect_ip(&mut stream, dest_ip, dest_port, deadline).await?;
    Ok(stream)
}

/// Stream returned by the SOCKS connector after the tunnel is established.
pub(crate) enum SocksStream {
    /// Plain HTTP tunnel.
    Tcp(tokio::net::TcpStream),
    /// Origin TLS layered over the SOCKS tunnel.
    Tls(Box<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>),
}

impl tokio::io::AsyncRead for SocksStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Tcp(stream) => std::pin::Pin::new(stream).poll_read(cx, buf),
            Self::Tls(stream) => std::pin::Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl tokio::io::AsyncWrite for SocksStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bytes: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match &mut *self {
            Self::Tcp(stream) => std::pin::Pin::new(stream).poll_write(cx, bytes),
            Self::Tls(stream) => std::pin::Pin::new(stream).poll_write(cx, bytes),
        }
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Tcp(stream) => std::pin::Pin::new(stream).poll_flush(cx),
            Self::Tls(stream) => std::pin::Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Tcp(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
            Self::Tls(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
        }
    }
}

impl hyper_util::client::legacy::connect::Connection for SocksStream {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
        let mut connected = hyper_util::client::legacy::connect::Connected::new();
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

/// Hyper connector that establishes one SOCKS5 tunnel per origin connection.
#[derive(Clone)]
pub(crate) struct SocksConnector {
    proxy: ProxyConfig,
    tls: Option<std::sync::Arc<tokio_rustls::TlsConnector>>,
    timeout: Option<std::time::Duration>,
}

impl SocksConnector {
    pub(crate) fn new(
        proxy: ProxyConfig,
        tls: Option<tokio_rustls::TlsConnector>,
        timeout: Option<std::time::Duration>,
    ) -> Self {
        Self {
            proxy,
            tls: tls.map(std::sync::Arc::new),
            timeout,
        }
    }
}

impl tower_service::Service<http::Uri> for SocksConnector {
    type Response = hyper_util::rt::TokioIo<SocksStream>;
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

    fn call(&mut self, dst: http::Uri) -> Self::Future {
        let proxy = self.proxy.clone();
        let tls = self.tls.clone();
        let timeout = self.timeout;
        Box::pin(async move {
            let host = dst.host().ok_or_else(|| -> Self::Error {
                Error::InvalidUrl("SOCKS destination has no host".into()).into()
            })?;
            let port = dst
                .port_u16()
                .or_else(|| (dst.scheme_str() == Some("https")).then_some(443))
                .unwrap_or(80);
            let stream = socks5_handshake(&proxy, host, port, proxy.socks_remote_dns(), timeout)
                .await
                .map_err(|e| -> Self::Error { e.into() })?;
            if dst.scheme_str() != Some("https") {
                return Ok(hyper_util::rt::TokioIo::new(SocksStream::Tcp(stream)));
            }
            let connector = tls.ok_or_else(|| -> Self::Error {
                Error::Tls("SOCKS HTTPS requires a TLS connector".into()).into()
            })?;
            let name = crate::transport::direct_connector::tls_server_name(host).map_err(
                |e| -> Self::Error {
                    Error::Tls(format!("invalid SOCKS TLS server name '{host}': {e}")).into()
                },
            )?;
            let stream = connector
                .connect(name, stream)
                .await
                .map_err(|e| -> Self::Error {
                    Error::Tls(format!("TLS handshake through SOCKS tunnel failed: {e}")).into()
                })?;
            Ok(hyper_util::rt::TokioIo::new(SocksStream::Tls(Box::new(
                stream,
            ))))
        })
    }
}

/// Negotiate the authentication method with the SOCKS5 proxy.
///
/// Sends the list of supported methods and reads the proxy's selection.
async fn negotiate_method(
    stream: &mut tokio::net::TcpStream,
    auth: Option<&ProxyAuth>,
    deadline: Option<std::time::Instant>,
) -> Result<u8> {
    // httpcore offers exactly one method: credentials select RFC 1929,
    // otherwise the client offers no authentication. This is intentionally
    // narrower than the set of methods the protocol could support.
    let expected_method = if auth.is_some() {
        SOCKS5_METHOD_USERNAME_PASSWORD
    } else {
        SOCKS5_METHOD_NO_AUTH
    };
    let methods = vec![expected_method];

    // Send: version(1) + nmethods(1) + methods(nmethods)
    let mut greeting = Vec::with_capacity(2 + methods.len());
    greeting.push(SOCKS5_VERSION);
    greeting.push(
        u8::try_from(methods.len())
            .map_err(|_| Error::ProxyConnect("too many SOCKS5 methods".into()))?,
    );
    greeting.extend_from_slice(&methods);

    write_all_timeout(stream, &greeting, deadline, "SOCKS5 greeting").await?;

    // Read: version(1) + method(1)
    let mut response = [0u8; 2];
    read_exact_timeout(stream, &mut response, deadline, "SOCKS5 method response").await?;

    if response[0] != SOCKS5_VERSION {
        return Err(Error::ProxyConnect(format!(
            "SOCKS5 proxy returned invalid version: {}",
            response[0]
        )));
    }

    match response[1] {
        SOCKS5_METHOD_NO_ACCEPTABLE => Err(Error::ProxyConnect(
            "SOCKS5 proxy rejected all authentication methods".into(),
        )),
        selected if selected == expected_method => Ok(selected),
        other => Err(Error::ProxyConnect(format!(
            "SOCKS5 proxy selected method {other}, but the client offered {expected_method}"
        ))),
    }
}

/// Perform username/password subnegotiation (RFC 1929).
async fn authenticate(
    stream: &mut tokio::net::TcpStream,
    auth: &ProxyAuth,
    deadline: Option<std::time::Instant>,
) -> Result<()> {
    let (username, password) = match auth {
        ProxyAuth::Basic { username, password } => (username.as_bytes(), password.as_bytes()),
    };

    if username.len() > MAX_SOCKS5_CREDENTIAL_LEN {
        return Err(Error::ProxyConnect(format!(
            "SOCKS5 username exceeds maximum length of {MAX_SOCKS5_CREDENTIAL_LEN}"
        )));
    }
    if password.len() > MAX_SOCKS5_CREDENTIAL_LEN {
        return Err(Error::ProxyConnect(format!(
            "SOCKS5 password exceeds maximum length of {MAX_SOCKS5_CREDENTIAL_LEN}"
        )));
    }

    // Send: version(1) + ulen(1) + username(ulen) + plen(1) + password(plen)
    let mut auth_request = Vec::with_capacity(3 + username.len() + password.len());
    auth_request.push(SOCKS5_SUBNEG_VERSION);
    auth_request.push(
        u8::try_from(username.len())
            .map_err(|_| Error::ProxyConnect("SOCKS5 username too long".into()))?,
    );
    auth_request.extend_from_slice(username);
    auth_request.push(
        u8::try_from(password.len())
            .map_err(|_| Error::ProxyConnect("SOCKS5 password too long".into()))?,
    );
    auth_request.extend_from_slice(password);

    write_all_timeout(stream, &auth_request, deadline, "SOCKS5 auth request").await?;

    // Read: version(1) + status(1)
    let mut auth_response = [0u8; 2];
    read_exact_timeout(stream, &mut auth_response, deadline, "SOCKS5 auth response").await?;

    if auth_response[0] != SOCKS5_SUBNEG_VERSION {
        return Err(Error::ProxyConnect(format!(
            "SOCKS5 auth returned invalid subnegotiation version: {}",
            auth_response[0]
        )));
    }

    if auth_response[1] != 0x00 {
        return Err(Error::ProxyConnect(
            "SOCKS5 proxy rejected authentication credentials".into(),
        ));
    }

    Ok(())
}

/// Send a CONNECT command and parse the reply.
///
/// Establishes a tunnel to the specified destination through the SOCKS5
/// proxy. Uses the address type appropriate for the DNS mode:
/// - `remote_dns=true`: sends domain name (`ATYP_DOMAIN`)
/// - `remote_dns=false`: resolves locally and sends IP (`ATYP_IPV4`/`ATYP_IPV6`)
async fn send_connect(
    stream: &mut tokio::net::TcpStream,
    dest_host: &str,
    dest_port: u16,
    remote_dns: bool,
    deadline: Option<std::time::Instant>,
) -> Result<()> {
    // Build the CONNECT request.
    let mut request = Vec::with_capacity(64);
    request.push(SOCKS5_VERSION);
    request.push(SOCKS5_CMD_CONNECT);
    request.push(0x00); // reserved

    if let Ok(ip) = dest_host.parse::<std::net::IpAddr>() {
        match ip {
            std::net::IpAddr::V4(ip) => {
                request.push(SOCKS5_ATYP_IPV4);
                request.extend_from_slice(&ip.octets());
            }
            std::net::IpAddr::V6(ip) => {
                request.push(SOCKS5_ATYP_IPV6);
                request.extend_from_slice(&ip.octets());
            }
        }
    } else if remote_dns {
        // Send domain name to proxy for remote resolution.
        let host_bytes = dest_host.as_bytes();
        if host_bytes.len() > MAX_SOCKS5_DOMAIN_LEN {
            return Err(Error::ProxyConnect(format!(
                "SOCKS5 destination domain exceeds maximum length of {MAX_SOCKS5_DOMAIN_LEN}"
            )));
        }
        request.push(SOCKS5_ATYP_DOMAIN);
        request.push(
            u8::try_from(host_bytes.len())
                .map_err(|_| Error::ProxyConnect("SOCKS5 domain name too long".into()))?,
        );
        request.extend_from_slice(host_bytes);
    } else {
        // Resolve locally and send IP address.
        match resolve_dest_ip(dest_host, deadline).await? {
            std::net::IpAddr::V4(ip) => {
                request.push(SOCKS5_ATYP_IPV4);
                request.extend_from_slice(&ip.octets());
            }
            std::net::IpAddr::V6(ip) => {
                request.push(SOCKS5_ATYP_IPV6);
                request.extend_from_slice(&ip.octets());
            }
        }
    }

    request.extend_from_slice(&dest_port.to_be_bytes());

    write_all_timeout(stream, &request, deadline, "SOCKS5 CONNECT").await?;

    // Parse the reply.
    parse_connect_reply(stream, deadline).await
}

/// Send a CONNECT command for a pre-resolved destination IP.
async fn send_connect_ip(
    stream: &mut tokio::net::TcpStream,
    dest_ip: std::net::IpAddr,
    dest_port: u16,
    deadline: Option<std::time::Instant>,
) -> Result<()> {
    let mut request = Vec::with_capacity(64);
    request.push(SOCKS5_VERSION);
    request.push(SOCKS5_CMD_CONNECT);
    request.push(0x00); // reserved
    match dest_ip {
        std::net::IpAddr::V4(ip) => {
            request.push(SOCKS5_ATYP_IPV4);
            request.extend_from_slice(&ip.octets());
        }
        std::net::IpAddr::V6(ip) => {
            request.push(SOCKS5_ATYP_IPV6);
            request.extend_from_slice(&ip.octets());
        }
    }
    request.extend_from_slice(&dest_port.to_be_bytes());

    write_all_timeout(stream, &request, deadline, "SOCKS5 CONNECT").await?;

    // Parse the reply.
    parse_connect_reply(stream, deadline).await
}

/// Resolve a destination host to all IP addresses.
///
/// Uses Tokio's DNS resolver. Returns both IPv4 and IPv6 in resolver order.
async fn resolve_dest_ips(
    host: &str,
    deadline: Option<std::time::Instant>,
) -> Result<Vec<std::net::IpAddr>> {
    use tokio::net::lookup_host;

    // Try parsing as an IP literal first.
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        return Ok(vec![std::net::IpAddr::V4(ip)]);
    }
    if let Ok(ip) = host.parse::<std::net::Ipv6Addr>() {
        return Ok(vec![std::net::IpAddr::V6(ip)]);
    }

    // DNS resolution.
    let lookup = lookup_host(format!("{host}:0"));
    let result = match deadline {
        Some(deadline) => {
            let dur = deadline.saturating_duration_since(std::time::Instant::now());
            tokio::time::timeout(dur, lookup)
                .await
                .map_err(|_| Error::Timeout {
                    phase: TimeoutPhase::ProxyConnect,
                    elapsed: dur,
                })?
        }
        None => lookup.await,
    };
    let addrs: Vec<std::net::IpAddr> = result
        .map(|addrs| addrs.map(|a| a.ip()).collect())
        .unwrap_or_default();

    if addrs.is_empty() {
        // Never redirect an unresolved destination to loopback.
        return Err(Error::Connect(format!(
            "DNS resolution failed for SOCKS destination {host}"
        )));
    }
    Ok(addrs)
}

/// Resolve a destination host to an IP address.
///
/// Uses Tokio's DNS resolver. Returns the first resolved address; callers
/// that need multi-homed fallback should use [`resolve_dest_ips`].
async fn resolve_dest_ip(
    host: &str,
    deadline: Option<std::time::Instant>,
) -> Result<std::net::IpAddr> {
    let addrs = resolve_dest_ips(host, deadline).await?;
    addrs.into_iter().next().ok_or_else(|| {
        Error::Connect(format!(
            "DNS resolution failed for SOCKS destination {host}"
        ))
    })
}

/// Parse the SOCKS5 CONNECT reply from the proxy.
///
/// Validates version, reply code, and bound address. Returns an error
/// for any non-success reply.
async fn parse_connect_reply(
    stream: &mut tokio::net::TcpStream,
    deadline: Option<std::time::Instant>,
) -> Result<()> {
    // Read: version(1) + rep(1) + rsv(1) + atyp(1)
    let mut header = [0u8; 4];
    read_exact_timeout(stream, &mut header, deadline, "SOCKS5 CONNECT reply").await?;

    if header[0] != SOCKS5_VERSION {
        return Err(Error::ProxyConnect(format!(
            "SOCKS5 reply has invalid version: {}",
            header[0]
        )));
    }

    if header[1] != SOCKS5_REP_SUCCESS {
        let msg = match header[1] {
            0x01 => "general SOCKS server failure",
            0x02 => "connection not allowed by ruleset",
            0x03 => "network unreachable",
            0x04 => "host unreachable",
            0x05 => "connection refused",
            0x06 => "TTL expired",
            0x07 => "command not supported",
            0x08 => "address type not supported",
            code => {
                return Err(Error::ProxyConnect(format!(
                    "SOCKS5 CONNECT failed with unknown reply code: {code}"
                )))
            }
        };
        return Err(Error::ProxyConnect(format!("SOCKS5 CONNECT failed: {msg}")));
    }

    // Read the bound address (variable length based on address type).
    match header[3] {
        SOCKS5_ATYP_IPV4 => {
            let mut addr = [0u8; 4 + 2]; // IPv4 + port
            read_exact_timeout(stream, &mut addr, deadline, "SOCKS5 bound IPv4 address").await?;
        }
        SOCKS5_ATYP_IPV6 => {
            let mut addr = [0u8; 16 + 2]; // IPv6 + port
            read_exact_timeout(stream, &mut addr, deadline, "SOCKS5 bound IPv6 address").await?;
        }
        SOCKS5_ATYP_DOMAIN => {
            let mut domain_len = [0u8; 1];
            read_exact_timeout(
                stream,
                &mut domain_len,
                deadline,
                "SOCKS5 bound domain length",
            )
            .await?;
            let len = domain_len[0] as usize;
            let mut domain_and_port = vec![0u8; len + 2]; // domain + port
            read_exact_timeout(
                stream,
                &mut domain_and_port,
                deadline,
                "SOCKS5 bound domain address",
            )
            .await?;
        }
        other => {
            return Err(Error::MalformedProxyResponse(format!(
                "SOCKS5 reply has unknown address type: {other}"
            )));
        }
    }

    Ok(())
}

/// Write all bytes with an optional timeout.
async fn write_all_timeout(
    stream: &mut tokio::net::TcpStream,
    data: &[u8],
    deadline: Option<std::time::Instant>,
    phase: &str,
) -> Result<()> {
    let write_result: std::result::Result<(), std::io::Error> = match deadline {
        Some(deadline) => {
            let dur = deadline.saturating_duration_since(std::time::Instant::now());
            let fut = tokio::io::AsyncWriteExt::write_all(stream, data);
            match tokio::time::timeout(dur, fut).await {
                Ok(r) => r,
                Err(_) => {
                    return Err(Error::Timeout {
                        phase: TimeoutPhase::ProxyConnect,
                        elapsed: dur,
                    });
                }
            }
        }
        None => tokio::io::AsyncWriteExt::write_all(stream, data).await,
    };

    write_result.map_err(|e| Error::ProxyConnect(format!("{phase} write failed: {e}")))
}

/// Read exact bytes with an optional timeout.
async fn read_exact_timeout(
    stream: &mut tokio::net::TcpStream,
    buf: &mut [u8],
    deadline: Option<std::time::Instant>,
    phase: &str,
) -> Result<()> {
    let read_result: std::result::Result<usize, std::io::Error> = match deadline {
        Some(deadline) => {
            let dur = deadline.saturating_duration_since(std::time::Instant::now());
            let fut = tokio::io::AsyncReadExt::read_exact(stream, buf);
            match tokio::time::timeout(dur, fut).await {
                Ok(r) => r,
                Err(_) => {
                    return Err(Error::Timeout {
                        phase: TimeoutPhase::ProxyConnect,
                        elapsed: dur,
                    });
                }
            }
        }
        None => tokio::io::AsyncReadExt::read_exact(stream, buf).await,
    };

    read_result.map(|_| ()).map_err(|e| match e.kind() {
        std::io::ErrorKind::UnexpectedEof => {
            Error::MalformedProxyResponse(format!("{phase}: connection closed prematurely"))
        }
        _ => Error::ProxyConnect(format!("{phase} read failed: {e}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socks5_method_constants() {
        assert_eq!(SOCKS5_VERSION, 0x05);
        assert_eq!(SOCKS5_METHOD_NO_AUTH, 0x00);
        assert_eq!(SOCKS5_METHOD_USERNAME_PASSWORD, 0x02);
        assert_eq!(SOCKS5_METHOD_NO_ACCEPTABLE, 0xFF);
    }

    #[test]
    fn socks5_reply_constants() {
        assert_eq!(SOCKS5_REP_SUCCESS, 0x00);
        assert_eq!(SOCKS5_ATYP_IPV4, 0x01);
        assert_eq!(SOCKS5_ATYP_DOMAIN, 0x03);
        assert_eq!(SOCKS5_ATYP_IPV6, 0x04);
    }

    #[test]
    fn max_domain_len() {
        assert_eq!(MAX_SOCKS5_DOMAIN_LEN, 255);
    }
}
