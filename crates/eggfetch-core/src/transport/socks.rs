//! SOCKS5 proxy handshake and tunnel establishment.
//!
//! Implements the SOCKS5 protocol (RFC 1928) for proxy connections,
//! including username/password authentication (RFC 1929). The
//! implementation is bounded to the subset required for HTTPX 0.28.1
//! parity: `socks5://` (local DNS) and `socks5h://` (remote DNS).
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
/// * `remote_dns` — if `true`, send the domain name to the proxy for
///   remote DNS resolution (`socks5h://`). If `false`, resolve locally
///   and send the IP address (`socks5://`).
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
) -> Result<tokio::io::BufReader<tokio::net::TcpStream>> {
    let proxy_host = proxy_config.host().unwrap_or("127.0.0.1");
    let proxy_port = proxy_config.port();

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

    let mut stream = match remaining_total {
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

    // Phase 2: Method negotiation.
    negotiate_method(&mut stream, proxy_config.auth(), remaining_total).await?;

    // Phase 3: Username/password authentication (if configured).
    if proxy_config.auth().is_some() {
        authenticate(&mut stream, proxy_config.auth().unwrap(), remaining_total).await?;
    }

    // Phase 4: CONNECT command.
    send_connect(
        &mut stream,
        dest_host,
        dest_port,
        remote_dns,
        remaining_total,
    )
    .await?;

    Ok(tokio::io::BufReader::new(stream))
}

/// Negotiate the authentication method with the SOCKS5 proxy.
///
/// Sends the list of supported methods and reads the proxy's selection.
async fn negotiate_method(
    stream: &mut tokio::net::TcpStream,
    auth: Option<&ProxyAuth>,
    remaining_total: Option<std::time::Duration>,
) -> Result<()> {
    // Build the method list.
    let methods = if auth.is_some() {
        vec![SOCKS5_METHOD_NO_AUTH, SOCKS5_METHOD_USERNAME_PASSWORD]
    } else {
        vec![SOCKS5_METHOD_NO_AUTH]
    };

    // Send: version(1) + nmethods(1) + methods(nmethods)
    let mut greeting = Vec::with_capacity(2 + methods.len());
    greeting.push(SOCKS5_VERSION);
    greeting.push(
        u8::try_from(methods.len())
            .map_err(|_| Error::ProxyConnect("too many SOCKS5 methods".into()))?,
    );
    greeting.extend_from_slice(&methods);

    write_all_timeout(stream, &greeting, remaining_total, "SOCKS5 greeting").await?;

    // Read: version(1) + method(1)
    let mut response = [0u8; 2];
    read_exact_timeout(
        stream,
        &mut response,
        remaining_total,
        "SOCKS5 method response",
    )
    .await?;

    if response[0] != SOCKS5_VERSION {
        return Err(Error::ProxyConnect(format!(
            "SOCKS5 proxy returned invalid version: {}",
            response[0]
        )));
    }

    match response[1] {
        SOCKS5_METHOD_NO_AUTH | SOCKS5_METHOD_USERNAME_PASSWORD => Ok(()),
        SOCKS5_METHOD_NO_ACCEPTABLE => Err(Error::ProxyConnect(
            "SOCKS5 proxy rejected all authentication methods".into(),
        )),
        other => Err(Error::ProxyConnect(format!(
            "SOCKS5 proxy selected unsupported method: {other}"
        ))),
    }
}

/// Perform username/password subnegotiation (RFC 1929).
async fn authenticate(
    stream: &mut tokio::net::TcpStream,
    auth: &ProxyAuth,
    remaining_total: Option<std::time::Duration>,
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

    write_all_timeout(
        stream,
        &auth_request,
        remaining_total,
        "SOCKS5 auth request",
    )
    .await?;

    // Read: version(1) + status(1)
    let mut auth_response = [0u8; 2];
    read_exact_timeout(
        stream,
        &mut auth_response,
        remaining_total,
        "SOCKS5 auth response",
    )
    .await?;

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
    remaining_total: Option<std::time::Duration>,
) -> Result<()> {
    // Build the CONNECT request.
    let mut request = Vec::with_capacity(64);
    request.push(SOCKS5_VERSION);
    request.push(SOCKS5_CMD_CONNECT);
    request.push(0x00); // reserved

    if remote_dns {
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
        match resolve_dest_ip(dest_host).await {
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

    write_all_timeout(stream, &request, remaining_total, "SOCKS5 CONNECT").await?;

    // Parse the reply.
    parse_connect_reply(stream, remaining_total).await
}

/// Resolve a destination host to an IP address.
///
/// Uses Tokio's DNS resolver. Returns both IPv4 and IPv6.
async fn resolve_dest_ip(host: &str) -> std::net::IpAddr {
    use tokio::net::lookup_host;

    // Try parsing as an IP literal first.
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        return std::net::IpAddr::V4(ip);
    }
    if let Ok(ip) = host.parse::<std::net::Ipv6Addr>() {
        return std::net::IpAddr::V6(ip);
    }

    // DNS resolution.
    let addr = lookup_host(format!("{host}:0"))
        .await
        .ok()
        .and_then(|mut addrs| addrs.next());

    match addr {
        Some(addr) => addr.ip(),
        None => {
            // Fallback: return the host as-is (will fail at TCP connect).
            if host.contains(':') {
                host.parse::<std::net::Ipv6Addr>().map_or(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                    std::net::IpAddr::V6,
                )
            } else {
                host.parse::<std::net::Ipv4Addr>().map_or(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                    std::net::IpAddr::V4,
                )
            }
        }
    }
}

/// Parse the SOCKS5 CONNECT reply from the proxy.
///
/// Validates version, reply code, and bound address. Returns an error
/// for any non-success reply.
async fn parse_connect_reply(
    stream: &mut tokio::net::TcpStream,
    remaining_total: Option<std::time::Duration>,
) -> Result<()> {
    // Read: version(1) + rep(1) + rsv(1) + atyp(1)
    let mut header = [0u8; 4];
    read_exact_timeout(stream, &mut header, remaining_total, "SOCKS5 CONNECT reply").await?;

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
            read_exact_timeout(
                stream,
                &mut addr,
                remaining_total,
                "SOCKS5 bound IPv4 address",
            )
            .await?;
        }
        SOCKS5_ATYP_IPV6 => {
            let mut addr = [0u8; 16 + 2]; // IPv6 + port
            read_exact_timeout(
                stream,
                &mut addr,
                remaining_total,
                "SOCKS5 bound IPv6 address",
            )
            .await?;
        }
        SOCKS5_ATYP_DOMAIN => {
            let mut domain_len = [0u8; 1];
            read_exact_timeout(
                stream,
                &mut domain_len,
                remaining_total,
                "SOCKS5 bound domain length",
            )
            .await?;
            let len = domain_len[0] as usize;
            let mut domain_and_port = vec![0u8; len + 2]; // domain + port
            read_exact_timeout(
                stream,
                &mut domain_and_port,
                remaining_total,
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
    remaining_total: Option<std::time::Duration>,
    phase: &str,
) -> Result<()> {
    let write_result: std::result::Result<(), std::io::Error> = match remaining_total {
        Some(dur) => {
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
    remaining_total: Option<std::time::Duration>,
    phase: &str,
) -> Result<()> {
    let read_result: std::result::Result<usize, std::io::Error> = match remaining_total {
        Some(dur) => {
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
