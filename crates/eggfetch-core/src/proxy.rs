//! Proxy subsystem for eggfetch.
//!
//! Provides HTTP proxy forwarding, HTTPS CONNECT tunneling, and
//! SOCKS5 proxy support with secure authentication handling.
//! Proxy logic is feature-gated behind the `proxy` feature.
//!
//! # Supported schemes
//!
//! - `http://` — HTTP proxy (forward HTTP targets, CONNECT for HTTPS)
//! - `socks5://` and `socks5h://` — SOCKS5 proxying. The native API retains
//!   the scheme distinction; the HTTPX compatibility facade normalizes both
//!   schemes to the reference stack's hostname-at-proxy behavior.
//!
//! # Environment policy
//!
//! The native API does not read proxy environment variables. The Python
//! compatibility facade performs HTTPX-compatible environment selection at
//! its boundary before passing explicit routes to this crate.
//!
//! # TLS interception
//!
//! CONNECT tunneling does **not** perform TLS interception. The tunnel
//! is a transparent byte stream between the client and the destination.
//! If a corporate or inspection proxy performs TLS interception (MITM),
//! certificate verification will succeed only if the proxy's CA
//! certificate is trusted by the system or explicitly configured.
//!
//! # Security
//!
//! - Proxy passwords are never exposed in `Debug`, `Display`, logs,
//!   or error messages.
//! - `Proxy-Authorization` is never forwarded to the destination.
//! - Credentials in proxy URLs are rejected with redacted errors.

use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::error::{Error, Result};

mod environment;
mod no_proxy;

/// A single `NO_PROXY` bypass rule.
///
/// Parsed from individual entries in a comma-separated `NO_PROXY` string.
/// Each variant represents a different matching strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoProxyRule {
    /// Matches everything.
    Wildcard,
    /// Host/domain match without a leading dot.
    ///
    /// Uses label-boundary subdomain matching (`host == rule ||
    /// host.ends_with("."+rule)`). Native `NoProxy::parse` creates this
    /// variant for bare hosts, including IP literals like `127.0.0.1`.
    Host(String),
    /// Exact host match used by the HTTPX environment parser.
    ///
    /// `parse_httpx_ip_entry` maps IP literals (including bracketed IPv6)
    /// to this variant for exact equality, while native parsing maps the
    /// same literal to `Host`. The divergence is intentional and
    /// documented: `should_bypass` agrees for exact hosts but `Host`
    /// would also match subdomains (which IP literals never have). A
    /// property test should assert that for IP literals both parsers agree
    /// on bypass for exact matches.
    HostExact(String),
    /// Domain suffix match (leading dot, e.g. `.example.com`).
    DomainSuffix(String),
    /// Exact host + port match.
    HostPort(String, u16),
    /// Exact host + port match used by the HTTPX environment parser.
    HostPortExact(String, u16),
    /// Bare-domain host + explicit port match used by the HTTPX environment parser.
    HostPortHttpx(String, u16),
    /// IP network expressed in CIDR notation.
    IpNetwork(std::net::IpAddr, u8),
    /// Matches the hostname `localhost`.
    Localhost,
    /// Matches only the hostname `localhost` (HTTPX environment semantics).
    LocalhostExact,
    /// Matches one HTTPX URL-pattern exclusion with optional scheme/port.
    SchemeHostPort {
        /// Optional URL scheme constraint.
        scheme: String,
        /// Exact host constraint.
        host: String,
        /// Optional port constraint.
        port: Option<u16>,
    },
}

/// `NO_PROXY` bypass rules for a proxy.
///
/// When a URL matches any bypass rule, the request is sent directly
/// (without going through the proxy). Construct via [`NoProxy::parse`]
/// with a comma-separated list of entries, then attach to a proxy with
/// [`Proxy::no_proxy`].
///
/// Supported entry formats:
/// - `*` — wildcard, matches everything
/// - `localhost` — matches localhost and its loopback IP literals
/// - `.example.com` — domain suffix match (matches subdomains, not the bare
///   domain)
/// - `example.com` — domain match (matches the bare domain and subdomains)
/// - `example.com:8080` — host + port match (uses scheme default port
///   when the URL has no explicit port)
/// - `10.0.0.0/8` — native parser CIDR network; HTTPX compatibility
///   parsing treats CIDR-looking entries as URL-pattern host text
/// - `[::1]` or `[::1]:8080` — IPv6 literal, optionally with port
#[derive(Debug, Clone)]
pub struct NoProxy {
    rules: Vec<NoProxyRule>,
}

impl NoProxy {
    /// Parse a comma-separated list of `NO_PROXY` entries.
    ///
    /// Entries can be:
    /// - `*` — wildcard, matches everything
    /// - `localhost` — matches localhost and its loopback IP literals
    /// - `.example.com` — domain suffix match
    /// - `example.com` — exact host match
    /// - `example.com:8080` — host + port match
    /// - `[::1]` — IPv6 literal
    /// - `[::1]:8080` — IPv6 literal + port
    ///
    /// # Errors
    ///
    /// Returns an error if a port number cannot be parsed.
    pub fn parse(s: &str) -> Result<Self> {
        Self::parse_with_localhost_mode(s, false)
    }

    /// Parse rules using HTTPX 0.28.1 environment matching semantics.
    ///
    /// Unlike the native parser, a `localhost` entry matches only the
    /// hostname `localhost`; it does not implicitly include loopback IPs.
    ///
    /// # Errors
    ///
    /// Returns an error when an entry cannot be represented by HTTPX's
    /// environment URL-pattern conversion.
    pub fn parse_httpx(s: &str) -> Result<Self> {
        Self::parse_with_localhost_mode(s, true)
    }

    fn parse_with_localhost_mode(s: &str, exact_localhost: bool) -> Result<Self> {
        let mut rules = Vec::new();
        for entry in s.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            rules.push(Self::parse_entry(entry, exact_localhost)?);
        }
        Ok(Self { rules })
    }

    fn parse_entry(entry: &str, exact_localhost: bool) -> Result<NoProxyRule> {
        if entry == "*" {
            return Ok(NoProxyRule::Wildcard);
        }
        if entry.eq_ignore_ascii_case("localhost") {
            return Ok(if exact_localhost {
                NoProxyRule::LocalhostExact
            } else {
                NoProxyRule::Localhost
            });
        }

        // HTTPX treats scheme-qualified NO_PROXY values as URL patterns,
        // rather than native CIDR or host rules.
        if exact_localhost && entry.contains("://") {
            let pattern = url::Url::parse(entry).map_err(|_| {
                Error::InvalidProxyUrl(format!("invalid URL in NO_PROXY entry: {entry}"))
            })?;
            let host = pattern.host_str().ok_or_else(|| {
                Error::InvalidProxyUrl(format!("NO_PROXY URL has no host: {entry}"))
            })?;
            return Ok(NoProxyRule::SchemeHostPort {
                scheme: pattern.scheme().to_ascii_lowercase(),
                host: host.to_ascii_lowercase(),
                port: pattern.port(),
            });
        }

        if exact_localhost {
            if let Some(rule) = no_proxy::parse_httpx_ip_entry(entry)? {
                return Ok(rule);
            }
        }

        if let Ok(address) = entry.parse::<std::net::Ipv6Addr>() {
            return Ok(NoProxyRule::Host(address.to_string()));
        }

        if let Some((network, prefix)) = entry.split_once('/') {
            let network = network.parse::<std::net::IpAddr>().map_err(|_| {
                Error::InvalidProxyUrl(format!("invalid IP network in NO_PROXY entry: {entry}"))
            })?;
            let prefix = prefix.parse::<u8>().map_err(|_| {
                Error::InvalidProxyUrl(format!("invalid CIDR prefix in NO_PROXY entry: {entry}"))
            })?;
            let max_prefix = match network {
                std::net::IpAddr::V4(_) => 32,
                std::net::IpAddr::V6(_) => 128,
            };
            if prefix > max_prefix {
                return Err(Error::InvalidProxyUrl(format!(
                    "CIDR prefix exceeds address width in NO_PROXY entry: {entry}"
                )));
            }
            return Ok(NoProxyRule::IpNetwork(network, prefix));
        }

        // IPv6 literal: [::1] or [::1]:8080
        if let Some(rest) = entry.strip_prefix('[') {
            if let Some(close) = rest.find(']') {
                let ipv6 = &rest[..close];
                let remainder = &rest[close + 1..];
                if remainder.is_empty() {
                    // bare IPv6 literal — treat as host
                    return Ok(if exact_localhost {
                        NoProxyRule::HostExact(ipv6.to_ascii_lowercase())
                    } else {
                        NoProxyRule::Host(entry.to_owned())
                    });
                }
                if let Some(port_str) = remainder.strip_prefix(':') {
                    let port = port_str.parse::<u16>().map_err(|_| {
                        Error::InvalidProxyUrl(format!("invalid port in NO_PROXY entry: {entry}"))
                    })?;
                    return Ok(if exact_localhost {
                        NoProxyRule::HostPortExact(format!("[{ipv6}]"), port)
                    } else {
                        NoProxyRule::HostPort(format!("[{ipv6}]"), port)
                    });
                }
            }
        }

        // host:port. HTTPX builds an `all://*host:port` URL pattern for a
        // non-scheme-qualified entry, so the host keeps bare-domain and
        // subdomain matching while the port remains an explicit match. In
        // particular, an entry such as `example.com:80` does not match an
        // HTTP URL whose normalized port is omitted.
        if let Some(colon_pos) = entry.rfind(':') {
            let host = &entry[..colon_pos];
            let port_str = &entry[colon_pos + 1..];
            if host.is_empty() || entry.matches(':').count() > 1 {
                return Err(Error::InvalidProxyUrl(format!(
                    "invalid NO_PROXY host/port entry: {entry}"
                )));
            }
            let port = port_str.parse::<u16>().map_err(|_| {
                Error::InvalidProxyUrl(format!("invalid port in NO_PROXY entry: {entry}"))
            })?;
            return Ok(if exact_localhost {
                NoProxyRule::HostPortHttpx(host.to_owned(), port)
            } else {
                NoProxyRule::HostPort(host.to_owned(), port)
            });
        }

        // Domain suffix: .example.com
        if let Some(suffix) = entry.strip_prefix('.') {
            if suffix.is_empty() {
                return Err(Error::InvalidProxyUrl(
                    "NO_PROXY entry cannot be just a dot".into(),
                ));
            }
            return Ok(NoProxyRule::DomainSuffix(entry.to_owned()));
        }

        // HTTPX builds an `all://*host` pattern for ordinary domains, which
        // matches the bare domain and subdomains at a label boundary. Keep
        // localhost and IP literals on their exact-host paths above.
        Ok(NoProxyRule::Host(entry.to_owned()))
    }

    /// Returns `true` if the given URL should bypass the proxy (go direct).
    #[must_use]
    pub fn should_bypass(&self, url: &url::Url) -> bool {
        self.should_bypass_components(url.scheme(), url.host_str().unwrap_or(""), url.port())
    }

    /// Component-based bypass check for native `http::Uri` origins.
    ///
    /// Shares the exact rule evaluation with [`Self::should_bypass`] so
    /// native transport does not need `url::Url`. `explicit_port` is the
    /// URI's explicit port (`None` when implicit) to preserve the
    /// explicit-vs-default distinction in `HostPortHttpx` and related rules.
    #[must_use]
    pub(crate) fn should_bypass_components(
        &self,
        scheme: &str,
        host: &str,
        port: Option<u16>,
    ) -> bool {
        for rule in &self.rules {
            match rule {
                NoProxyRule::Wildcard => return true,
                NoProxyRule::Localhost => {
                    let ip_host = host
                        .strip_prefix('[')
                        .and_then(|value| value.strip_suffix(']'))
                        .unwrap_or(host);
                    if host.eq_ignore_ascii_case("localhost")
                        || ip_host.parse::<std::net::IpAddr>().is_ok_and(|ip| {
                            ip == std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
                                || ip == std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
                        })
                    {
                        return true;
                    }
                }
                NoProxyRule::LocalhostExact => {
                    if host.eq_ignore_ascii_case("localhost") {
                        return true;
                    }
                }
                NoProxyRule::Host(h) => {
                    if Self::matches_host_rule(host, h) {
                        return true;
                    }
                }
                NoProxyRule::HostExact(h) => {
                    if Self::matches_exact_host(host, h) {
                        return true;
                    }
                }
                NoProxyRule::DomainSuffix(suffix) => {
                    if Self::matches_domain_suffix(host, suffix) {
                        return true;
                    }
                }
                NoProxyRule::HostPort(h, p) => {
                    let port_matches = match port {
                        Some(pu) => pu == *p,
                        None => Self::default_port_for_scheme(scheme) == *p,
                    };
                    let host_matches = if h.starts_with('.') {
                        Self::matches_domain_suffix(host, h)
                    } else {
                        Self::matches_host_rule(host, h)
                    };
                    if port_matches && host_matches {
                        return true;
                    }
                }
                NoProxyRule::HostPortExact(h, p) => {
                    let port_matches = match port {
                        Some(pu) => pu == *p,
                        None => Self::default_port_for_scheme(scheme) == *p,
                    };
                    if port_matches && Self::matches_exact_host(host, h) {
                        return true;
                    }
                }
                NoProxyRule::HostPortHttpx(h, p) => {
                    if port == Some(*p) && Self::matches_host_rule(host, h) {
                        return true;
                    }
                }
                NoProxyRule::IpNetwork(network, prefix) => {
                    if host
                        .trim_start_matches('[')
                        .trim_end_matches(']')
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|candidate| Self::ip_in_network(candidate, *network, *prefix))
                    {
                        return true;
                    }
                }
                NoProxyRule::SchemeHostPort {
                    scheme: rule_scheme,
                    host: rule_host,
                    port: rule_port,
                } => {
                    if scheme.eq_ignore_ascii_case(rule_scheme)
                        && host.eq_ignore_ascii_case(rule_host)
                        && rule_port.is_none_or(|rule_port| {
                            port == Some(rule_port)
                                || (port.is_none()
                                    && Self::default_port_for_scheme(scheme) == rule_port)
                        })
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn matches_domain_suffix(host: &str, suffix: &str) -> bool {
        let host_lower = host.to_ascii_lowercase();
        let suffix_lower = suffix.trim_start_matches('.').to_ascii_lowercase();
        host_lower.len() > suffix_lower.len()
            && host_lower.as_bytes()[host_lower.len() - suffix_lower.len() - 1] == b'.'
            && host_lower.ends_with(&suffix_lower)
    }

    fn matches_host_rule(host: &str, rule: &str) -> bool {
        let host_lower = host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_ascii_lowercase();
        let rule_lower = rule
            .trim_start_matches('.')
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_ascii_lowercase();
        host_lower == rule_lower
            || (host_lower.len() > rule_lower.len()
                && host_lower.as_bytes()[host_lower.len() - rule_lower.len() - 1] == b'.'
                && host_lower.ends_with(&rule_lower))
    }

    fn matches_exact_host(host: &str, rule: &str) -> bool {
        host.trim_start_matches('[')
            .trim_end_matches(']')
            .eq_ignore_ascii_case(rule.trim_start_matches('[').trim_end_matches(']'))
    }

    fn ip_in_network(candidate: std::net::IpAddr, network: std::net::IpAddr, prefix: u8) -> bool {
        match (candidate, network) {
            (std::net::IpAddr::V4(candidate), std::net::IpAddr::V4(network)) => {
                let mask = if prefix == 0 {
                    0
                } else {
                    u32::MAX << (32 - u32::from(prefix))
                };
                u32::from(candidate) & mask == u32::from(network) & mask
            }
            (std::net::IpAddr::V6(candidate), std::net::IpAddr::V6(network)) => {
                let candidate = u128::from(candidate);
                let network = u128::from(network);
                let mask = if prefix == 0 {
                    0
                } else {
                    u128::MAX << (128 - u32::from(prefix))
                };
                candidate & mask == network & mask
            }
            _ => false,
        }
    }

    fn default_port_for_scheme(scheme: &str) -> u16 {
        match scheme {
            "https" => 443,
            _ => 80,
        }
    }
}

/// Authentication credentials for proxy access.
///
/// Currently supports HTTP Basic authentication for proxies.
///
/// # Security
///
/// The `Debug` and `Display` implementations redact both username and
/// password. Credentials are never exposed in error messages or logs.
#[derive(Clone)]
pub enum ProxyAuth {
    /// HTTP Basic proxy authentication.
    Basic {
        /// Proxy username.
        username: String,
        /// Proxy password (redacted in Debug/Display).
        password: String,
    },
}

impl ProxyAuth {
    /// Create a new Basic proxy auth credential.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidProxyUrl`] if:
    /// - The username contains `:` (it delimits the password, mirroring
    ///   [`BasicAuth::new`](crate::auth::BasicAuth::new) and URL userinfo)
    /// - The username or password contains CR (`\r`), LF (`\n`), or NUL (`\0`)
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Result<Self> {
        let username = username.into();
        let password = password.into();

        if username.contains(':') {
            return Err(Error::InvalidProxyUrl(
                "proxy auth username must not contain ':'".into(),
            ));
        }
        if username.contains('\r') || username.contains('\n') {
            return Err(Error::InvalidProxyUrl(
                "proxy auth username must not contain CR/LF".into(),
            ));
        }
        if password.contains('\r') || password.contains('\n') {
            return Err(Error::InvalidProxyUrl(
                "proxy auth password must not contain CR/LF".into(),
            ));
        }
        if username.contains('\0') || password.contains('\0') {
            return Err(Error::InvalidProxyUrl(
                "proxy auth credentials must not contain NUL".into(),
            ));
        }

        Ok(Self::Basic { username, password })
    }

    /// Returns the `Proxy-Authorization` header value.
    pub(crate) fn header_value(&self) -> String {
        match self {
            Self::Basic { username, password } => {
                use base64::Engine;
                let credential = format!("{username}:{password}");
                let encoded =
                    base64::engine::general_purpose::STANDARD.encode(credential.as_bytes());
                format!("Basic {encoded}")
            }
        }
    }
}

impl fmt::Debug for ProxyAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Basic { .. } => f
                .debug_struct("ProxyAuth::Basic")
                .field("username", &"<redacted>")
                .field("password", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ProxyAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Basic { .. } => f.write_str("Basic(<redacted>)"),
        }
    }
}

/// Determines which requests are routed through the proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyRule {
    /// Route all requests through the proxy.
    All,
    /// Route only HTTP requests through the proxy.
    Http,
    /// Route only HTTPS requests through the proxy.
    Https,
}

/// The routing decision for a request.
#[derive(Debug, Clone)]
#[allow(
    clippy::large_enum_variant,
    reason = "ProxyConfig is transient and cloning it is cheap; boxing adds unnecessary allocation"
)]
pub enum ProxyDecision {
    /// Send the request directly to the destination.
    Direct,
    /// Send the request through the specified proxy configuration.
    Proxy(ProxyConfig),
}

/// Resolved proxy configuration for a specific request.
///
/// Contains the proxy URI, optional authentication, and optional
/// proxy-only headers that must be sent to the proxy endpoint but
/// never forwarded to the origin server.
#[derive(Clone)]
pub struct ProxyConfig {
    /// Proxy URI (e.g., `http://proxy.example:8080`).
    pub(crate) uri: url::Url,
    /// Optional proxy authentication.
    pub(crate) auth: Option<ProxyAuth>,
    /// Proxy-only headers applied to proxy-leg requests (CONNECT,
    /// forward-proxy absolute-form).  These are *not* forwarded into
    /// the tunnel or to the origin.
    pub(crate) proxy_headers: crate::headers::Headers,
    /// TLS configuration for the *proxy* endpoint (used when the proxy
    /// endpoint itself is `https://`).  When `None`, the proxy endpoint
    /// uses the proxy/default trust roots.  Origin TLS configuration
    /// (custom CA, client identity, verification toggle) does **not**
    /// fall back into the proxy handshake.
    pub(crate) proxy_tls_config: Option<crate::tls::TlsConfig>,
    /// Caller-supplied physical proxy peers. The proxy URI remains the
    /// logical identity used for TLS and proxy policy.
    pub(crate) resolved_addresses: Option<Arc<[SocketAddr]>>,
}

impl fmt::Debug for ProxyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProxyConfig")
            .field("uri", &self.uri)
            .field("auth", &self.auth)
            .field("proxy_headers", &self.proxy_headers)
            .field(
                "proxy_tls_config",
                &self.proxy_tls_config.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "resolved_addresses",
                &self
                    .resolved_addresses
                    .as_ref()
                    .map(|addresses| addresses.len()),
            )
            .finish()
    }
}

impl ProxyConfig {
    /// Return an opaque connection-affecting identity for internal caches.
    /// It intentionally has no formatting implementation and therefore
    /// cannot disclose credentials through diagnostics.
    ///
    /// The proxy TLS component uses the corrected opaque
    /// `TlsConfig::connection_identity()` token: two proxy configs cloned
    /// from the same base but with different connection-affecting TLS
    /// policy (verification flags, trust roots, provider, mTLS identity,
    /// version bounds, SNI) never alias, while unchanged clones share
    /// identity and remain reusable.
    #[cfg(feature = "proxy")]
    pub(crate) fn connection_identity(&self) -> Vec<u8> {
        let mut identity = Vec::new();
        identity.extend_from_slice(self.uri.as_str().as_bytes());
        identity.push(0);
        if let Some(auth) = self.auth.as_ref() {
            // Hash credential bytes instead of retaining them in the
            // long-lived route key. The hash preserves cache separation
            // without storing key material for the entry lifetime. This is
            // a cache-separation heuristic, not a security boundary: two
            // domain-separated `DefaultHasher` (SipHash, random per-process
            // keys) outputs give a 128-bit separator, so an accidental
            // alias between different credentials is negligible. Keys are
            // little-endian by construction; the identity is in-process
            // only, never persisted or compared across endian targets.
            use std::hash::{Hash, Hasher};
            let header = auth.header_value();
            let mut first = std::collections::hash_map::DefaultHasher::new();
            0u8.hash(&mut first);
            header.hash(&mut first);
            let mut second = std::collections::hash_map::DefaultHasher::new();
            1u8.hash(&mut second);
            header.hash(&mut second);
            identity.extend_from_slice(&first.finish().to_le_bytes());
            identity.extend_from_slice(&second.finish().to_le_bytes());
        }
        identity.push(0);
        for (name, value) in self.proxy_headers.iter() {
            identity.extend_from_slice(name.as_str().as_bytes());
            identity.push(b':');
            identity.extend_from_slice(value.as_bytes());
            identity.push(0);
        }
        identity.extend_from_slice(
            self.proxy_tls_config
                .as_ref()
                .map_or(0u64, crate::tls::TlsConfig::connection_identity)
                .to_ne_bytes()
                .as_slice(),
        );
        identity.push(0);
        if let Some(addresses) = self.resolved_addresses.as_ref() {
            for address in addresses.iter() {
                identity.extend_from_slice(address.to_string().as_bytes());
                identity.push(0);
            }
        }
        identity
    }

    /// Returns the proxy URI.
    #[must_use]
    pub fn uri(&self) -> &url::Url {
        &self.uri
    }

    /// Returns a reference to the proxy auth, if configured.
    #[must_use]
    pub fn auth(&self) -> Option<&ProxyAuth> {
        self.auth.as_ref()
    }

    /// Returns the proxy host for pool keying.
    #[must_use]
    pub fn host(&self) -> Option<&str> {
        self.uri.host_str()
    }

    /// Returns the proxy port for pool keying.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidProxyUrl`] when the proxy scheme has no
    /// known default port.
    pub fn port(&self) -> Result<u16> {
        match self.uri.port_or_known_default() {
            Some(port) => Ok(port),
            None => match self.uri.scheme() {
                "socks5" | "socks5h" => Ok(1080),
                _ => Err(Error::InvalidProxyUrl(format!(
                    "proxy scheme {:?} has no known default port",
                    self.uri.scheme()
                ))),
            },
        }
    }

    /// Returns the proxy scheme.
    #[must_use]
    pub fn scheme(&self) -> &str {
        self.uri.scheme()
    }

    /// Returns `true` if this is a SOCKS5 proxy.
    #[must_use]
    pub fn is_socks(&self) -> bool {
        matches!(self.uri.scheme(), "socks5" | "socks5h")
    }

    /// Returns `true` if this is a SOCKS proxy with remote DNS resolution.
    #[must_use]
    pub fn socks_remote_dns(&self) -> bool {
        self.uri.scheme() == "socks5h"
    }

    /// Returns a reference to the proxy-only headers.
    #[must_use]
    pub fn proxy_headers(&self) -> &crate::headers::Headers {
        &self.proxy_headers
    }

    /// Returns a reference to the proxy TLS configuration, if set.
    #[must_use]
    pub fn proxy_tls_config(&self) -> Option<&crate::tls::TlsConfig> {
        self.proxy_tls_config.as_ref()
    }

    /// Returns the caller-supplied physical proxy peers, if configured.
    #[must_use]
    pub fn resolved_addresses(&self) -> Option<&[SocketAddr]> {
        self.resolved_addresses.as_deref()
    }
}

/// An HTTP proxy configuration.
///
/// Defines how requests are routed through a proxy server. Supports
/// HTTP forward proxying and HTTPS CONNECT tunneling.
///
/// # Example
///
/// ```no_run
/// # use eggfetch_core::Proxy;
/// let proxy = Proxy::all("http://proxy:8080")?;
/// # Ok::<(), eggfetch_core::Error>(())
/// ```
#[derive(Clone)]
pub struct Proxy {
    /// Proxy URI.
    uri: url::Url,
    /// Optional proxy authentication.
    auth: Option<ProxyAuth>,
    /// Routing rule.
    rule: ProxyRule,
    /// Optional `NO_PROXY` bypass rules.
    bypass: Option<NoProxy>,
    /// Proxy-only headers sent to the proxy endpoint but never
    /// forwarded to the origin.
    #[allow(
        clippy::struct_field_names,
        reason = "Proxy is already in the proxy context; 'proxy_headers' and 'proxy_tls_config' are unambiguous within this scope"
    )]
    proxy_headers: crate::headers::Headers,
    /// TLS configuration for the *proxy* endpoint (used when the proxy
    /// endpoint itself is `https://`).  When `None`, the proxy endpoint
    /// uses the proxy/default trust roots.  Origin TLS configuration
    /// (custom CA, client identity, verification toggle) does **not**
    /// fall back into the proxy handshake.
    #[allow(
        clippy::struct_field_names,
        reason = "Proxy is already in the proxy context; 'proxy_tls_config' is unambiguous within this scope"
    )]
    proxy_tls_config: Option<crate::tls::TlsConfig>,
    /// Optional caller-supplied physical peers for the proxy endpoint.
    resolved_addresses: Option<Arc<[SocketAddr]>>,
}

impl Proxy {
    /// Create a proxy that routes all requests.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid, has unsupported scheme,
    /// contains userinfo, or has other issues.
    pub fn all(url: &str) -> Result<Self> {
        let uri = parse_proxy_url(url)?;
        Ok(Self {
            uri,
            auth: None,
            rule: ProxyRule::All,
            bypass: None,
            proxy_headers: crate::headers::Headers::new(),
            proxy_tls_config: None,
            resolved_addresses: None,
        })
    }

    /// Create a proxy using HTTPX-compatible URL credentials.
    ///
    /// This compatibility-boundary constructor extracts percent-encoded
    /// userinfo and delegates to the native credential-safe configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when the URL, credentials, or proxy scheme is invalid.
    pub fn all_compat(url: &str) -> Result<Self> {
        // As in `parse_proxy_url`: never echo `url` on parse failure, since
        // the redactor falls back to the raw input when parsing fails.
        let parsed = url::Url::parse(url)
            .map_err(|e| Error::InvalidProxyUrl(format!("invalid proxy URL: {e}")))?;
        let username = percent_encoding::percent_decode_str(parsed.username())
            .decode_utf8_lossy()
            .into_owned();
        let password = percent_encoding::percent_decode_str(parsed.password().unwrap_or(""))
            .decode_utf8_lossy()
            .into_owned();
        let has_auth = !username.is_empty() || parsed.password().is_some();
        let mut without_userinfo = parsed;
        let _ = without_userinfo.set_username("");
        let _ = without_userinfo.set_password(None);
        let mut proxy = Self::all(without_userinfo.as_str())?;
        if has_auth {
            proxy = proxy.auth(ProxyAuth::basic(username, password)?);
        }
        Ok(proxy)
    }

    /// Create a proxy that routes only HTTP requests.
    ///
    /// # Errors
    ///
    /// Same as [`Proxy::all`].
    pub fn http(url: &str) -> Result<Self> {
        let uri = parse_proxy_url(url)?;
        Ok(Self {
            uri,
            auth: None,
            rule: ProxyRule::Http,
            bypass: None,
            proxy_headers: crate::headers::Headers::new(),
            proxy_tls_config: None,
            resolved_addresses: None,
        })
    }

    /// Create an HTTP-only proxy accepting inline URL credentials.
    ///
    /// Mirrors [`Proxy::all_compat`] for the `Http` routing rule: percent-
    /// encoded userinfo is extracted into [`ProxyAuth`] rather than rejected.
    /// Used by environment-proxy resolution, where `HTTP_PROXY` values
    /// commonly embed `user:pass@` credentials.
    ///
    /// # Errors
    ///
    /// Returns an error when the URL, credentials, or proxy scheme is invalid.
    pub fn http_compat(url: &str) -> Result<Self> {
        let mut proxy = Self::all_compat(url)?;
        proxy.rule = ProxyRule::Http;
        Ok(proxy)
    }

    /// Create a proxy that routes only HTTPS requests.
    ///
    /// # Errors
    ///
    /// Same as [`Proxy::all`].
    pub fn https(url: &str) -> Result<Self> {
        let uri = parse_proxy_url(url)?;
        Ok(Self {
            uri,
            auth: None,
            rule: ProxyRule::Https,
            bypass: None,
            proxy_headers: crate::headers::Headers::new(),
            proxy_tls_config: None,
            resolved_addresses: None,
        })
    }

    /// Create an HTTPS-only proxy accepting inline URL credentials.
    ///
    /// Mirrors [`Proxy::all_compat`] for the `Https` routing rule. Used by
    /// environment-proxy resolution, where `HTTPS_PROXY` values commonly
    /// embed `user:pass@` credentials and the proxy endpoint serves as the
    /// CONNECT proxy for HTTPS targets.
    ///
    /// # Errors
    ///
    /// Returns an error when the URL, credentials, or proxy scheme is invalid.
    pub fn https_compat(url: &str) -> Result<Self> {
        let mut proxy = Self::all_compat(url)?;
        proxy.rule = ProxyRule::Https;
        Ok(proxy)
    }

    /// Set proxy authentication credentials.
    #[must_use]
    pub fn auth(mut self, auth: ProxyAuth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Set `NO_PROXY` bypass rules for this proxy.
    ///
    /// When a URL matches any bypass rule, the request is sent directly
    /// without going through the proxy.
    #[must_use]
    pub fn no_proxy(mut self, no_proxy: NoProxy) -> Self {
        self.bypass = Some(no_proxy);
        self
    }

    /// Set proxy-only headers for this proxy.
    ///
    /// These headers are sent to the proxy endpoint on forward-proxy
    /// and CONNECT requests, but are never forwarded into the tunnel
    /// or to the origin server.
    #[must_use]
    pub fn proxy_headers(mut self, headers: crate::headers::Headers) -> Self {
        self.proxy_headers = headers;
        self
    }

    /// Returns a reference to the proxy-only headers.
    #[must_use]
    pub fn proxy_headers_ref(&self) -> &crate::headers::Headers {
        &self.proxy_headers
    }

    /// Set the TLS configuration for the proxy endpoint itself.
    ///
    /// This governs the TLS handshake to an `https://` proxy endpoint.
    /// When `None`, the proxy endpoint uses the proxy/default trust
    /// roots; the origin TLS configuration is never reused as a
    /// fallback for the proxy handshake.
    #[must_use]
    pub fn with_proxy_tls_config(mut self, config: crate::tls::TlsConfig) -> Self {
        self.proxy_tls_config = Some(config);
        self
    }

    /// Pin proxy TCP connections to caller-supplied physical peers.
    ///
    /// The proxy URL remains the logical identity for proxy TLS SNI and
    /// certificate verification. The addresses are attempted in order and
    /// are never replaced by a system DNS lookup.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::InvalidProxyUrl`] when the set is empty or an
    /// address uses a port different from the proxy URL's effective port.
    pub fn resolved_addresses<I>(mut self, addresses: I) -> Result<Self>
    where
        I: IntoIterator<Item = SocketAddr>,
    {
        let target = crate::transport_hints::ResolvedTarget::new(addresses).map_err(|error| {
            Error::InvalidProxyUrl(format!("invalid resolved proxy peers: {error}"))
        })?;
        let expected_port = self.config().port()?;
        if target
            .addresses()
            .iter()
            .any(|address| address.port() != expected_port)
        {
            return Err(Error::InvalidProxyUrl(format!(
                "all resolved proxy peers must use the proxy URL's effective port {expected_port}"
            )));
        }
        self.resolved_addresses = Some(target.addresses().to_vec().into());
        Ok(self)
    }

    /// Returns a reference to the `NO_PROXY` bypass rules, if configured.
    #[must_use]
    pub fn no_proxy_rules(&self) -> Option<&NoProxy> {
        self.bypass.as_ref()
    }

    /// Determine whether a request with the given scheme should use
    /// this proxy.
    #[must_use]
    pub fn should_use_for_scheme(&self, scheme: &str) -> bool {
        match self.rule {
            ProxyRule::All => true,
            ProxyRule::Http => scheme == "http",
            ProxyRule::Https => scheme == "https",
        }
    }

    /// Build a resolved proxy configuration for a request.
    ///
    /// For HTTP and SOCKS proxies, inline URL credentials are extracted and
    /// used when no explicit `.auth()` was set.
    pub(crate) fn config(&self) -> ProxyConfig {
        let auth = match self.auth {
            Some(ref a) => Some(a.clone()),
            None => {
                // Extract SOCKS credentials from URL userinfo.
                if self.is_socks_url() {
                    let username = percent_encoding::percent_decode_str(self.uri.username())
                        .decode_utf8_lossy()
                        .into_owned();
                    let password =
                        percent_encoding::percent_decode_str(self.uri.password().unwrap_or(""))
                            .decode_utf8_lossy()
                            .into_owned();
                    if !username.is_empty() || !password.is_empty() {
                        Some(ProxyAuth::Basic { username, password })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };

        // Strip userinfo from the stored URI to avoid leaking credentials.
        let mut uri = self.uri.clone();
        if !uri.username().is_empty() || uri.password().is_some() {
            let _ = uri.set_username("");
            let _ = uri.set_password(None);
        }

        ProxyConfig {
            uri,
            auth,
            proxy_headers: self.proxy_headers.clone(),
            proxy_tls_config: self.proxy_tls_config.clone(),
            resolved_addresses: self.resolved_addresses.clone(),
        }
    }

    fn is_socks_url(&self) -> bool {
        matches!(self.uri.scheme(), "socks5" | "socks5h")
    }

    /// Returns the proxy URI.
    #[must_use]
    pub fn uri(&self) -> &url::Url {
        &self.uri
    }

    /// Returns the routing rule.
    #[must_use]
    pub fn rule(&self) -> ProxyRule {
        self.rule
    }
}

impl fmt::Debug for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Redact any userinfo in the debug output; SOCKS URLs intentionally
        // keep inline credentials in `self.uri`, so the raw URL would leak.
        let mut url = self.uri.clone();
        let _ = url.set_username("");
        let _ = url.set_password(None);
        f.debug_struct("Proxy")
            .field("uri", &url)
            .field("auth", &self.auth)
            .field("rule", &self.rule)
            .field("bypass", &self.bypass)
            .field("proxy_headers", &self.proxy_headers)
            .field(
                "proxy_tls_config",
                &self.proxy_tls_config.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "resolved_addresses",
                &self
                    .resolved_addresses
                    .as_ref()
                    .map(|addresses| addresses.len()),
            )
            .finish()
    }
}

impl fmt::Display for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Redact any userinfo in the display.
        let mut url = self.uri.clone();
        let _ = url.set_username("");
        let _ = url.set_password(None);
        write!(f, "Proxy({url})")
    }
}

/// Explicit opt-in snapshot of proxy environment configuration.
///
/// The native client never reads process environment implicitly. Construct
/// this value explicitly — via [`ProxyEnvironment::from_map`] in tests or
/// [`ProxyEnvironment::from_env`] at a call site — and hand it to
/// [`crate::client::ClientBuilder::proxy_environment`] to preserve the
/// command-line client's environment-proxy reachability.
///
/// # Precedence
///
/// For each of `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` / `NO_PROXY`,
/// the lowercase variant wins when both cases are present and non-empty;
/// otherwise the uppercase variant is used. Empty values are treated as
/// absent. This matches the `urllib.request.getproxies()` precedence used
/// by the Python compatibility facade.
///
/// # Routing
///
/// - `https` targets use `HTTPS_PROXY` / `https_proxy`, falling back to
///   `ALL_PROXY` / `all_proxy`.
/// - `http` targets use `HTTP_PROXY` / `http_proxy`, falling back to
///   `ALL_PROXY` / `all_proxy`.
/// - An `http://` proxy URL serves as the CONNECT proxy for HTTPS targets;
///   `socks5://` / `socks5h://` values work wherever the existing proxy
///   parser accepts them (no new SOCKS implementation).
/// - `NO_PROXY` / `no_proxy` is parsed with the native [`NoProxy::parse`]
///   (not the HTTPX-compat parser) and applied before proxy dispatch: a
///   bypassed URL resolves to direct transport (`None`).
///
/// Proxy URLs without a scheme (e.g. `proxy:8080`) are normalized with an
/// `http://` prefix, mirroring the Python facade's environment helper.
/// Invalid proxy or `NO_PROXY` values fail closed with a redacted
/// [`Error::InvalidProxyUrl`] rather than silently falling back to direct
/// transport.
///
/// The snapshot owns its strings: later process-environment changes do not
/// affect an already-constructed value.
#[derive(Clone, Default)]
#[allow(
    clippy::struct_field_names,
    reason = "fields name the four conventional proxy environment variables; the shared `proxy` suffix is the domain vocabulary"
)]
pub struct ProxyEnvironment {
    http_proxy: Option<String>,
    https_proxy: Option<String>,
    all_proxy: Option<String>,
    no_proxy: Option<String>,
}

impl fmt::Debug for ProxyEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never render raw environment values: proxy URLs may embed
        // credentials. Report presence only.
        f.debug_struct("ProxyEnvironment")
            .field("http_proxy", &self.http_proxy.as_ref().map(|_| "<set>"))
            .field("https_proxy", &self.https_proxy.as_ref().map(|_| "<set>"))
            .field("all_proxy", &self.all_proxy.as_ref().map(|_| "<set>"))
            .field("no_proxy", &self.no_proxy.as_ref().map(|_| "<set>"))
            .finish()
    }
}

impl ProxyEnvironment {
    /// Create an empty environment (no proxy routing; direct transport).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a supplied environment snapshot.
    ///
    /// Takes any iterator of `(key, value)` pairs so tests can supply maps
    /// without mutating process environment. Only the eight conventional
    /// keys are read (`HTTP_PROXY` / `http_proxy`, `HTTPS_PROXY` /
    /// `https_proxy`, `ALL_PROXY` / `all_proxy`, `NO_PROXY` / `no_proxy`);
    /// all other entries are ignored and no broader environment capture
    /// occurs.
    #[must_use]
    pub fn from_map<I, K, V>(vars: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut lower_http: Option<String> = None;
        let mut upper_http: Option<String> = None;
        let mut lower_https: Option<String> = None;
        let mut upper_https: Option<String> = None;
        let mut lower_all: Option<String> = None;
        let mut upper_all: Option<String> = None;
        let mut lower_no: Option<String> = None;
        let mut upper_no: Option<String> = None;

        for (key, value) in vars {
            let value = value.as_ref().trim().to_owned();
            if value.is_empty() {
                continue;
            }
            match key.as_ref() {
                "http_proxy" => lower_http = Some(value),
                "HTTP_PROXY" => upper_http = Some(value),
                "https_proxy" => lower_https = Some(value),
                "HTTPS_PROXY" => upper_https = Some(value),
                "all_proxy" => lower_all = Some(value),
                "ALL_PROXY" => upper_all = Some(value),
                "no_proxy" => lower_no = Some(value),
                "NO_PROXY" => upper_no = Some(value),
                _ => {}
            }
        }

        Self {
            http_proxy: lower_http.or(upper_http),
            https_proxy: lower_https.or(upper_https),
            all_proxy: lower_all.or(upper_all),
            no_proxy: lower_no.or(upper_no),
        }
    }

    /// Snapshot the current process environment once.
    ///
    /// This is the only entry point that touches `std::env`, and it copies
    /// the relevant values into an owned snapshot. Prefer
    /// [`ProxyEnvironment::from_map`] in tests to avoid global environment
    /// mutation under parallelism.
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_map(std::env::vars())
    }

    /// Returns `true` when no proxy route is configured.
    ///
    /// A `NO_PROXY`-only snapshot still counts as empty: without an
    /// `HTTP(S)_PROXY` / `ALL_PROXY` value there is nothing to route.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.http_proxy.is_none() && self.https_proxy.is_none() && self.all_proxy.is_none()
    }

    /// Parse the snapshot's `NO_PROXY` value with native semantics.
    fn no_proxy_rules(&self) -> Result<Option<NoProxy>> {
        self.no_proxy.as_deref().map(NoProxy::parse).transpose()
    }

    /// Normalize an environment proxy URL.
    ///
    /// Values without a scheme (e.g. `proxy:8080`) are treated as `http`
    /// proxies, mirroring the Python facade's `normalize_environment_proxy_url`.
    /// Resolve the explicit proxy route for `url`.
    ///
    /// Returns `Ok(None)` for direct transport: no proxy configured for the
    /// URL's scheme, or `NO_PROXY` bypasses it. Returns `Ok(Some(proxy))`
    /// with the proxy's `NO_PROXY` rules attached. Returns a redacted
    /// [`Error::InvalidProxyUrl`] for invalid proxy or `NO_PROXY` values
    /// (fail closed; never silently direct).
    ///
    /// Only `http` and `https` targets are proxied; other schemes resolve
    /// to direct transport.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidProxyUrl`] when a configured proxy URL or
    /// the `NO_PROXY` value cannot be parsed.
    pub fn resolve(&self, url: &url::Url) -> Result<Option<Proxy>> {
        let no_proxy = self.no_proxy_rules()?;
        if let Some(ref rules) = no_proxy {
            if rules.should_bypass(url) {
                return Ok(None);
            }
        }

        let (raw, rule) = match url.scheme() {
            "https" => match self.https_proxy.as_deref().or(self.all_proxy.as_deref()) {
                Some(raw) => (raw, ProxyRule::Https),
                None => return Ok(None),
            },
            "http" => match self.http_proxy.as_deref().or(self.all_proxy.as_deref()) {
                Some(raw) => (raw, ProxyRule::Http),
                None => return Ok(None),
            },
            _ => return Ok(None),
        };
        // A fallback `ALL_PROXY` value routes both schemes; keep the
        // per-URL rule specific except when the value came from ALL_PROXY.
        let from_all = match url.scheme() {
            "https" => self.https_proxy.is_none(),
            "http" => self.http_proxy.is_none(),
            _ => false,
        };

        let normalized = environment::normalize_proxy_url(raw);
        let mut proxy = match (rule, from_all) {
            (_, true) | (ProxyRule::All, false) => Proxy::all_compat(&normalized)?,
            (ProxyRule::Http, false) => Proxy::http_compat(&normalized)?,
            (ProxyRule::Https, false) => Proxy::https_compat(&normalized)?,
        };
        if let Some(rules) = no_proxy {
            proxy = proxy.no_proxy(rules);
        }
        Ok(Some(proxy))
    }

    /// Expand the snapshot into explicit scheme-scoped proxies.
    ///
    /// Returns up to three proxies (`Http`, `Https`, `All` for the
    /// configured values) in specific-before-fallback order, each carrying
    /// the snapshot's `NO_PROXY` rules. Used by
    /// [`crate::client::ClientBuilder::proxy_environment`]. Fails closed on
    /// invalid values with a redacted error.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidProxyUrl`] when any configured proxy URL or
    /// the `NO_PROXY` value cannot be parsed.
    pub fn client_proxies(&self) -> Result<Vec<Proxy>> {
        let no_proxy = self.no_proxy_rules()?;
        let mut proxies = Vec::new();

        if let Some(ref raw) = self.http_proxy {
            let normalized = environment::normalize_proxy_url(raw);
            let mut proxy = Proxy::http_compat(&normalized)?;
            if let Some(ref rules) = no_proxy {
                proxy = proxy.no_proxy(rules.clone());
            }
            proxies.push(proxy);
        }
        if let Some(ref raw) = self.https_proxy {
            let normalized = environment::normalize_proxy_url(raw);
            let mut proxy = Proxy::https_compat(&normalized)?;
            if let Some(ref rules) = no_proxy {
                proxy = proxy.no_proxy(rules.clone());
            }
            proxies.push(proxy);
        }
        if let Some(ref raw) = self.all_proxy {
            let normalized = environment::normalize_proxy_url(raw);
            let mut proxy = Proxy::all_compat(&normalized)?;
            if let Some(ref rules) = no_proxy {
                proxy = proxy.no_proxy(rules.clone());
            }
            proxies.push(proxy);
        }
        Ok(proxies)
    }
}

/// Parse and validate a proxy URL.
///
/// Requirements:
/// - scheme must be `http`, `https`, `socks5`, or `socks5h`
/// - host must be present
/// - no fragment
/// - no query string
/// - for `http`/`https`: userinfo is rejected here (use the `*_compat`
///   constructors, which extract it into [`ProxyAuth`])
/// - for `socks5`/`socks5h`: inline userinfo is accepted and validated
fn parse_proxy_url(url_str: &str) -> Result<url::Url> {
    // Deliberately do not echo `url_str` into the error: parsing has
    // failed, so `redact_url_string` would fall back to the raw input
    // (potentially containing credentials). `url::ParseError` renders
    // only a static description and never embeds the input. This mirrors
    // the client-side URL parser's handling of the same hazard.
    let url = url::Url::parse(url_str)
        .map_err(|e| Error::InvalidProxyUrl(format!("invalid proxy URL: {e}")))?;

    match url.scheme() {
        "http" | "https" => {
            if !url.username().is_empty() || url.password().is_some() {
                return Err(Error::InvalidProxyUrl(
                    "proxy URL must not contain credentials; use .auth() instead".into(),
                ));
            }
        }
        "socks5" | "socks5h" => {
            // SOCKS5 proxies accept inline credentials in the URL.
            // Validate them at construction so CR/LF never reaches the
            // SOCKS5 username/password subnegotiation payload (the
            // `config()` extractor cannot fail).
            let username = percent_encoding::percent_decode_str(url.username()).decode_utf8_lossy();
            let password = percent_encoding::percent_decode_str(url.password().unwrap_or(""))
                .decode_utf8_lossy();
            if username.contains('\r')
                || username.contains('\n')
                || password.contains('\r')
                || password.contains('\n')
                || username.contains('\0')
                || password.contains('\0')
            {
                return Err(Error::InvalidProxyUrl(
                    "proxy auth credentials must not contain CR/LF or NUL".into(),
                ));
            }
        }
        other => {
            return Err(Error::InvalidProxyUrl(format!(
                "unsupported proxy scheme '{other}'; supported: http, https, socks5, socks5h"
            )));
        }
    }

    if url.host_str().is_none() || url.host_str() == Some("") {
        return Err(Error::InvalidProxyUrl("proxy URL must have a host".into()));
    }

    if url.fragment().is_some() {
        return Err(Error::InvalidProxyUrl(
            "proxy URL must not contain a fragment".into(),
        ));
    }

    if url.query().is_some() {
        return Err(Error::InvalidProxyUrl(
            "proxy URL must not contain a query string".into(),
        ));
    }

    Ok(url)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_proxy_url() {
        let proxy = Proxy::all("http://proxy.example:8080").unwrap();
        assert_eq!(proxy.uri().host_str(), Some("proxy.example"));
        assert_eq!(proxy.uri().port(), Some(8080));
    }

    #[test]
    fn unparseable_proxy_url_error_redacts_credentials() {
        // The input fails `Url::parse` (unterminated IPv6 literal), so the
        // redactor would fall back to the raw input: the error must not echo
        // it. See the client-side URL parser for the same hazard.
        let err = Proxy::all("http://user:proxy-secret-1@[::1").unwrap_err();
        let msg = err.to_string();
        assert!(
            !msg.contains("proxy-secret-1"),
            "proxy parse error must not leak credentials: {msg}"
        );
    }

    #[test]
    fn unparseable_compat_proxy_url_error_redacts_credentials() {
        let err = Proxy::all_compat("http://user:proxy-secret-2@[::1").unwrap_err();
        let msg = err.to_string();
        assert!(
            !msg.contains("proxy-secret-2"),
            "compat proxy parse error must not leak credentials: {msg}"
        );
    }

    #[test]
    fn compat_constructors_extract_inline_credentials() {
        let proxy = Proxy::https_compat("http://user:pass@proxy.example:8080").unwrap();
        assert_eq!(proxy.rule(), ProxyRule::Https);
        assert!(proxy.config().auth().is_some());

        let proxy = Proxy::http_compat("http://user:pass@proxy.example:8080").unwrap();
        assert_eq!(proxy.rule(), ProxyRule::Http);
        assert!(proxy.config().auth().is_some());
    }

    // --- ProxyEnvironment tests (supplied snapshots; never mutate process env) ---

    fn https_url() -> url::Url {
        url::Url::parse("https://example.com/resource").unwrap()
    }

    fn http_url() -> url::Url {
        url::Url::parse("http://example.com/resource").unwrap()
    }

    #[test]
    fn proxy_environment_https_proxy_selected() {
        let env = ProxyEnvironment::from_map([("HTTPS_PROXY", "http://proxy.example:8080")]);
        let proxy = env
            .resolve(&https_url())
            .expect("resolve succeeds")
            .expect("https proxy selected");
        assert_eq!(proxy.uri().host_str(), Some("proxy.example"));
        assert_eq!(proxy.rule(), ProxyRule::Https);
    }

    #[test]
    fn proxy_environment_lowercase_precedence() {
        // Lowercase wins when both cases are present (matches
        // urllib.request.getproxies() precedence used by the Python facade).
        let env = ProxyEnvironment::from_map([
            ("HTTPS_PROXY", "http://upper.example:8080"),
            ("https_proxy", "http://lower.example:8080"),
        ]);
        let proxy = env
            .resolve(&https_url())
            .expect("resolve succeeds")
            .expect("proxy selected");
        assert_eq!(proxy.uri().host_str(), Some("lower.example"));
    }

    #[test]
    fn proxy_environment_all_proxy_fallback() {
        let env = ProxyEnvironment::from_map([("ALL_PROXY", "http://fallback.example:8080")]);
        let https = env
            .resolve(&https_url())
            .expect("resolve succeeds")
            .expect("all_proxy fallback for https");
        assert_eq!(https.uri().host_str(), Some("fallback.example"));

        let http = env
            .resolve(&http_url())
            .expect("resolve succeeds")
            .expect("all_proxy fallback for http");
        assert_eq!(http.uri().host_str(), Some("fallback.example"));
    }

    #[test]
    fn proxy_environment_scheme_specific_beats_fallback() {
        let env = ProxyEnvironment::from_map([
            ("HTTPS_PROXY", "http://specific.example:8080"),
            ("ALL_PROXY", "http://fallback.example:8080"),
        ]);
        let proxy = env
            .resolve(&https_url())
            .expect("resolve succeeds")
            .expect("proxy selected");
        assert_eq!(proxy.uri().host_str(), Some("specific.example"));
    }

    #[test]
    fn proxy_environment_no_proxy_bypass_exact_and_domain() {
        let env = ProxyEnvironment::from_map([
            ("HTTPS_PROXY", "http://proxy.example:8080"),
            ("NO_PROXY", "example.com, .internal.example"),
        ]);
        // Exact host bypass.
        assert!(env
            .resolve(&https_url())
            .expect("resolve succeeds")
            .is_none());

        // Domain-suffix bypass.
        let sub = url::Url::parse("https://api.internal.example/data").unwrap();
        assert!(env.resolve(&sub).expect("resolve succeeds").is_none());

        // Unrelated host still routes via the proxy.
        let other = url::Url::parse("https://other.example/data").unwrap();
        assert!(env.resolve(&other).expect("resolve succeeds").is_some());
    }

    #[test]
    fn proxy_environment_http_proxy_connect_for_https() {
        // An http:// proxy URL serves as the CONNECT proxy for HTTPS targets.
        let env =
            ProxyEnvironment::from_map([("HTTPS_PROXY", "http://connect-proxy.example:3128")]);
        let proxy = env
            .resolve(&https_url())
            .expect("resolve succeeds")
            .expect("CONNECT proxy selected");
        assert_eq!(proxy.uri().scheme(), "http");
        assert_eq!(proxy.uri().host_str(), Some("connect-proxy.example"));
    }

    #[test]
    fn proxy_environment_socks_falls_out_of_parser() {
        let env = ProxyEnvironment::from_map([("HTTPS_PROXY", "socks5h://socks.example:1080")]);
        let proxy = env
            .resolve(&https_url())
            .expect("resolve succeeds")
            .expect("SOCKS proxy selected");
        assert!(proxy.config().is_socks());
    }

    #[test]
    fn proxy_environment_bare_host_gets_http_scheme() {
        let env = ProxyEnvironment::from_map([("HTTPS_PROXY", "proxy.example:8080")]);
        let proxy = env
            .resolve(&https_url())
            .expect("resolve succeeds")
            .expect("bare host normalized");
        assert_eq!(proxy.uri().scheme(), "http");
        assert_eq!(proxy.uri().host_str(), Some("proxy.example"));
    }

    #[test]
    fn proxy_environment_invalid_proxy_fails_closed_redacted() {
        let env = ProxyEnvironment::from_map([("HTTPS_PROXY", "http://user:env-secret-9@[::1")]);
        let err = env.resolve(&https_url()).unwrap_err();
        assert!(matches!(err, Error::InvalidProxyUrl(_)));
        let msg = err.to_string();
        assert!(
            !msg.contains("env-secret-9"),
            "environment proxy error must not leak credentials: {msg}"
        );
        assert_eq!(err.kind(), "invalid_proxy_url");
    }

    #[test]
    fn proxy_environment_no_resolver_means_direct() {
        let env = ProxyEnvironment::new();
        assert!(env.is_empty());
        assert!(env
            .resolve(&https_url())
            .expect("resolve succeeds")
            .is_none());
        assert!(env
            .resolve(&http_url())
            .expect("resolve succeeds")
            .is_none());
        assert!(env.client_proxies().expect("proxies succeed").is_empty());
    }

    #[test]
    fn proxy_environment_snapshot_stable_after_source_changes() {
        let mut source = vec![(
            "HTTPS_PROXY".to_owned(),
            "http://first.example:8080".to_owned(),
        )];
        let env = ProxyEnvironment::from_map(source.clone());
        // Mutating the source after the snapshot must not affect resolution.
        source[0].1 = "http://second.example:8080".to_owned();
        let proxy = env
            .resolve(&https_url())
            .expect("resolve succeeds")
            .expect("proxy selected");
        assert_eq!(proxy.uri().host_str(), Some("first.example"));
    }

    #[test]
    fn proxy_environment_debug_redacts_values() {
        let env = ProxyEnvironment::from_map([
            (
                "HTTPS_PROXY",
                "http://user:debug-secret-7@proxy.example:8080",
            ),
            ("NO_PROXY", "example.com"),
        ]);
        let rendered = format!("{env:?}");
        assert!(
            !rendered.contains("debug-secret-7"),
            "Debug must not leak proxy credentials: {rendered}"
        );
        assert!(
            !rendered.contains("proxy.example"),
            "Debug must not echo proxy host: {rendered}"
        );
    }

    #[test]
    fn proxy_environment_client_proxies_specific_before_fallback() {
        let env = ProxyEnvironment::from_map([
            ("HTTP_PROXY", "http://http-proxy.example:8080"),
            ("HTTPS_PROXY", "http://https-proxy.example:8080"),
            ("ALL_PROXY", "http://fallback.example:8080"),
        ]);
        let proxies = env.client_proxies().expect("proxies succeed");
        assert_eq!(proxies.len(), 3);
        assert_eq!(proxies[0].rule(), ProxyRule::Http);
        assert_eq!(proxies[1].rule(), ProxyRule::Https);
        assert_eq!(proxies[2].rule(), ProxyRule::All);
    }

    #[test]
    fn parse_proxy_url_default_port() {
        let proxy = Proxy::all("http://proxy.example").unwrap();
        assert_eq!(proxy.uri().port_or_known_default(), Some(80));
    }

    #[test]
    fn parse_proxy_url_accepts_https_scheme() {
        let proxy = Proxy::all("https://proxy.example:8443").unwrap();
        assert_eq!(proxy.uri().scheme(), "https");
        assert_eq!(proxy.config().port().unwrap(), 8443);
    }

    #[test]
    fn parse_proxy_url_https_default_port() {
        let proxy = Proxy::all("https://proxy.example").unwrap();
        assert_eq!(proxy.config().port().unwrap(), 443);
    }

    #[test]
    fn parse_proxy_url_accepts_socks5() {
        let proxy = Proxy::all("socks5://proxy.example:1080").unwrap();
        assert_eq!(proxy.uri().host_str(), Some("proxy.example"));
        assert_eq!(proxy.uri().port(), Some(1080));
        assert_eq!(proxy.uri().scheme(), "socks5");
    }

    #[test]
    fn parse_proxy_url_accepts_socks5h() {
        let proxy = Proxy::all("socks5h://proxy.example:1080").unwrap();
        assert_eq!(proxy.uri().scheme(), "socks5h");
    }

    #[test]
    fn parse_proxy_url_socks5_default_port() {
        let proxy = Proxy::all("socks5://proxy.example").unwrap();
        // The `url` crate doesn't know the default port for socks5,
        // but ProxyConfig::port() handles it correctly.
        let config = proxy.config();
        assert_eq!(config.port().unwrap(), 1080);
    }

    #[test]
    fn parse_proxy_url_socks5_with_credentials() {
        // SOCKS5 URLs with inline credentials are accepted.
        let proxy = Proxy::all("socks5://user:pass@proxy.example:1080").unwrap();
        let config = proxy.config();
        assert!(config.auth().is_some());
        if let Some(auth) = config.auth() {
            let debug = format!("{auth:?}");
            assert!(debug.contains("user"));
            // The actual password value must not appear in debug output.
            assert!(!debug.contains("pass@"), "password should be redacted");
            assert!(debug.contains("<redacted>"));
        }
    }

    #[test]
    fn parse_proxy_url_socks5_decodes_credentials() {
        let proxy = Proxy::all("socks5://user%40name:p%40ss@proxy.example:1080").unwrap();
        let config = proxy.config();
        match config.auth().unwrap() {
            ProxyAuth::Basic { username, password } => {
                assert_eq!(username, "user@name");
                assert_eq!(password, "p@ss");
            }
        }
    }

    #[test]
    fn parse_proxy_url_socks5_rejects_crlf_credentials() {
        // Percent-encoded CR/LF in userinfo must fail construction instead
        // of flowing into the SOCKS5 subnegotiation payload.
        assert!(Proxy::all("socks5://user%0D%0Ax:pass@proxy.example:1080").is_err());
        assert!(Proxy::all("socks5://user:p%0Aass@proxy.example:1080").is_err());
        assert!(Proxy::all("socks5h://user%0dname:pass@proxy.example:1080").is_err());
    }

    #[test]
    fn parse_proxy_url_socks5_no_userinfo() {
        let proxy = Proxy::all("socks5://proxy.example:1080").unwrap();
        let config = proxy.config();
        assert!(config.auth().is_none());
    }

    #[test]
    fn proxy_config_is_socks() {
        let http_proxy = Proxy::all("http://proxy:8080").unwrap();
        assert!(!http_proxy.config().is_socks());

        let socks_proxy = Proxy::all("socks5://proxy:1080").unwrap();
        assert!(socks_proxy.config().is_socks());

        let socks5h_proxy = Proxy::all("socks5h://proxy:1080").unwrap();
        assert!(socks5h_proxy.config().is_socks());
    }

    #[test]
    fn proxy_config_socks_remote_dns() {
        let socks5 = Proxy::all("socks5://proxy:1080").unwrap();
        assert!(!socks5.config().socks_remote_dns());

        let socks5h = Proxy::all("socks5h://proxy:1080").unwrap();
        assert!(socks5h.config().socks_remote_dns());
    }

    #[test]
    fn parse_proxy_url_rejects_no_host() {
        let err = Proxy::all("http://").unwrap_err();
        assert_eq!(err.kind(), "invalid_proxy_url");
    }

    #[test]
    fn compat_proxy_url_extracts_http_userinfo_and_redacts_it() {
        let proxy = Proxy::all_compat("http://user:pass@proxy.example:8080").unwrap();
        assert!(proxy.config().auth().is_some());
        assert!(!proxy.to_string().contains("pass"));
    }

    #[test]
    fn parse_proxy_url_rejects_http_userinfo() {
        let err = Proxy::all("http://user:pass@proxy.example:8080").unwrap_err();
        assert_eq!(err.kind(), "invalid_proxy_url");
    }

    #[test]
    fn parse_proxy_url_rejects_fragment() {
        let err = Proxy::all("http://proxy.example:8080#frag").unwrap_err();
        assert_eq!(err.kind(), "invalid_proxy_url");
    }

    #[test]
    fn parse_proxy_url_rejects_query() {
        let err = Proxy::all("http://proxy.example:8080?key=val").unwrap_err();
        assert_eq!(err.kind(), "invalid_proxy_url");
    }

    #[test]
    fn proxy_rule_all_matches_any_scheme() {
        let proxy = Proxy::all("http://proxy:8080").unwrap();
        assert!(proxy.should_use_for_scheme("http"));
        assert!(proxy.should_use_for_scheme("https"));
    }

    #[test]
    fn proxy_rule_http_only_matches_http() {
        let proxy = Proxy::http("http://proxy:8080").unwrap();
        assert!(proxy.should_use_for_scheme("http"));
        assert!(!proxy.should_use_for_scheme("https"));
    }

    #[test]
    fn proxy_rule_https_only_matches_https() {
        let proxy = Proxy::https("http://proxy:8080").unwrap();
        assert!(!proxy.should_use_for_scheme("http"));
        assert!(proxy.should_use_for_scheme("https"));
    }

    #[test]
    fn proxy_auth_redacted_debug() {
        let auth = ProxyAuth::basic("user", "secret123").unwrap();
        let debug = format!("{auth:?}");
        assert!(!debug.contains("\"user\""));
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("secret123"));
    }

    #[test]
    fn proxy_auth_redacted_display() {
        let auth = ProxyAuth::basic("user", "secret123").unwrap();
        let display = format!("{auth}");
        assert!(!display.contains("user"));
        assert!(!display.contains("secret123"));
        assert!(display.contains("<redacted>"));
    }

    #[test]
    fn proxy_auth_rejects_cr() {
        let err = ProxyAuth::basic("user\r", "pass").unwrap_err();
        assert_eq!(err.kind(), "invalid_proxy_url");
    }

    #[test]
    fn proxy_auth_rejects_lf() {
        let err = ProxyAuth::basic("user", "pass\n").unwrap_err();
        assert_eq!(err.kind(), "invalid_proxy_url");
    }

    #[test]
    fn proxy_auth_rejects_colon_in_username() {
        // Mirrors `BasicAuth::new`: ':' delimits the password, so a
        // username containing it would produce ambiguous credentials.
        let err = ProxyAuth::basic("user:name", "pass").unwrap_err();
        assert_eq!(err.kind(), "invalid_proxy_url");
    }

    #[test]
    fn proxy_auth_header_value() {
        use base64::Engine;
        let auth = ProxyAuth::basic("user", "pass").unwrap();
        let val = auth.header_value();
        assert!(val.starts_with("Basic "));
        let encoded = val.strip_prefix("Basic ").unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "user:pass");
    }

    #[test]
    fn proxy_auth_shared_subset_matches_connect_wire_helper() {
        for (username, password) in [("user", "pass"), ("", ""), ("u", "p:ass"), ("é", "päss")] {
            let proxy = ProxyAuth::basic(username, password).unwrap();
            let wire = eggfetch_http_connect::basic_auth_value(username, password).unwrap();
            assert_eq!(proxy.header_value(), wire);
        }
    }

    #[test]
    fn proxy_auth_control_domain_remains_intentionally_narrower() {
        // ProxyAuth predates the wire helper and its accepted domain is part
        // of the existing core contract.  Keep the difference explicit:
        // the wire owner rejects all controls, while ProxyAuth only rejects
        // CR/LF/NUL before its value reaches the CONNECT serializer.
        assert!(ProxyAuth::basic("u\t", "p").is_ok());
        assert!(eggfetch_http_connect::basic_auth_value("u\t", "p").is_err());
        assert!(ProxyAuth::basic("u\u{7f}", "p").is_ok());
        assert!(eggfetch_http_connect::basic_auth_value("u\u{7f}", "p").is_err());
    }

    #[test]
    fn proxy_display_redacts_url() {
        let proxy = Proxy::all("http://proxy.example:8080").unwrap();
        let display = format!("{proxy}");
        assert!(display.contains("proxy.example"));
        assert!(display.contains("Proxy("));
    }

    #[test]
    fn proxy_debug_redacts_socks_url_credentials() {
        // SOCKS URLs intentionally retain inline credentials in `uri`;
        // the Debug output must still strip them.
        let proxy = Proxy::all("socks5://user:secret123@proxy.example:1080").unwrap();
        let debug = format!("{proxy:?}");
        assert!(debug.contains("proxy.example"));
        assert!(!debug.contains("secret123"));
        assert!(!debug.contains("user:"));
    }

    #[test]
    fn proxy_config_host_port() {
        let proxy = Proxy::all("http://proxy.example:3128").unwrap();
        let config = proxy.config();
        assert_eq!(config.host(), Some("proxy.example"));
        assert_eq!(config.port().unwrap(), 3128);
        assert_eq!(config.scheme(), "http");
    }

    #[test]
    fn proxy_config_default_port() {
        let proxy = Proxy::all("http://proxy.example").unwrap();
        let config = proxy.config();
        assert_eq!(config.port().unwrap(), 80);
    }

    #[test]
    fn proxy_config_socks_default_port() {
        let proxy = Proxy::all("socks5://proxy.example").unwrap();
        let config = proxy.config();
        assert_eq!(config.port().unwrap(), 1080);
    }

    #[test]
    fn proxy_resolved_addresses_validate_and_preserve_order() {
        let proxy = Proxy::all("http://proxy.example:3128")
            .unwrap()
            .resolved_addresses([
                "198.51.100.21:3128".parse().unwrap(),
                "198.51.100.20:3128".parse().unwrap(),
            ])
            .unwrap();
        assert_eq!(
            proxy.config().resolved_addresses().unwrap(),
            &[
                "198.51.100.21:3128".parse().unwrap(),
                "198.51.100.20:3128".parse().unwrap(),
            ]
        );
    }

    #[test]
    fn proxy_resolved_addresses_reject_empty_and_wrong_port() {
        let proxy = Proxy::all("http://proxy.example:3128").unwrap();
        assert!(matches!(
            proxy.clone().resolved_addresses(Vec::<SocketAddr>::new()),
            Err(Error::InvalidProxyUrl(_))
        ));
        assert!(matches!(
            proxy.resolved_addresses(["198.51.100.20:8080".parse().unwrap()]),
            Err(Error::InvalidProxyUrl(_))
        ));
    }

    #[test]
    fn proxy_debug_reports_peer_count_without_topology() {
        let proxy = Proxy::all("http://proxy.example:3128")
            .unwrap()
            .resolved_addresses(["198.51.100.20:3128".parse().unwrap()])
            .unwrap();
        let debug = format!("{proxy:?}");
        assert!(debug.contains("resolved_addresses: Some(1)"));
        assert!(!debug.contains("198.51.100.20"));
    }

    #[test]
    fn proxy_config_unknown_scheme_has_no_panic_port() {
        let config = ProxyConfig {
            uri: url::Url::parse("unknown://proxy.example").unwrap(),
            auth: None,
            proxy_headers: crate::headers::Headers::new(),
            proxy_tls_config: None,
            resolved_addresses: None,
        };
        assert!(matches!(config.port(), Err(Error::InvalidProxyUrl(_))));
    }

    #[test]
    fn proxy_with_auth() {
        let auth = ProxyAuth::basic("user", "pass").unwrap();
        let proxy = Proxy::all("http://proxy:8080").unwrap().auth(auth);
        let config = proxy.config();
        assert!(config.auth().is_some());
    }

    #[test]
    fn proxy_with_no_proxy() {
        let np = NoProxy::parse("localhost").unwrap();
        let proxy = Proxy::all("http://proxy:8080").unwrap().no_proxy(np);
        assert!(proxy.no_proxy_rules().is_some());
    }

    #[test]
    fn noparse_empty_string() {
        let np = NoProxy::parse("").unwrap();
        assert!(np.rules.is_empty());
    }

    #[test]
    fn noparse_whitespace_only() {
        let np = NoProxy::parse(" , , ").unwrap();
        assert!(np.rules.is_empty());
    }

    #[test]
    fn noparse_single_host() {
        let np = NoProxy::parse("example.com").unwrap();
        assert_eq!(np.rules.len(), 1);
        assert_eq!(np.rules[0], NoProxyRule::Host("example.com".into()));
    }

    #[test]
    fn noparse_domain_suffix() {
        let np = NoProxy::parse(".example.com").unwrap();
        assert_eq!(np.rules.len(), 1);
        assert_eq!(
            np.rules[0],
            NoProxyRule::DomainSuffix(".example.com".into())
        );
    }

    #[test]
    fn noparse_host_port() {
        let np = NoProxy::parse("example.com:8080").unwrap();
        assert_eq!(np.rules.len(), 1);
        assert_eq!(
            np.rules[0],
            NoProxyRule::HostPort("example.com".into(), 8080)
        );
    }

    #[test]
    fn noparse_localhost() {
        let np = NoProxy::parse("localhost").unwrap();
        assert_eq!(np.rules.len(), 1);
        assert_eq!(np.rules[0], NoProxyRule::Localhost);
    }

    #[test]
    fn noparse_wildcard() {
        let np = NoProxy::parse("*").unwrap();
        assert_eq!(np.rules.len(), 1);
        assert_eq!(np.rules[0], NoProxyRule::Wildcard);
    }

    #[test]
    fn noparse_mixed() {
        let np = NoProxy::parse("localhost, .example.com, 10.0.0.1:8080").unwrap();
        assert_eq!(np.rules.len(), 3);
        assert_eq!(np.rules[0], NoProxyRule::Localhost);
        assert_eq!(
            np.rules[1],
            NoProxyRule::DomainSuffix(".example.com".into())
        );
        assert_eq!(np.rules[2], NoProxyRule::HostPort("10.0.0.1".into(), 8080));
    }

    #[test]
    fn noparse_ipv6_literal() {
        let np = NoProxy::parse("[::1]").unwrap();
        assert_eq!(np.rules.len(), 1);
        assert_eq!(np.rules[0], NoProxyRule::Host("[::1]".into()));
    }

    #[test]
    fn noparse_ipv6_with_port() {
        let np = NoProxy::parse("[::1]:8080").unwrap();
        assert_eq!(np.rules.len(), 1);
        assert_eq!(np.rules[0], NoProxyRule::HostPort("[::1]".into(), 8080));
    }

    #[test]
    fn noparse_invalid_port_rejected() {
        let err = NoProxy::parse("example.com:notaport").unwrap_err();
        assert!(matches!(err, Error::InvalidProxyUrl(_)));
    }

    #[test]
    fn noparse_dot_only() {
        let err = NoProxy::parse(".").unwrap_err();
        assert_eq!(err.kind(), "invalid_proxy_url");
    }

    #[test]
    fn nobypass_empty_rules() {
        let np = NoProxy::parse("").unwrap();
        let url = url::Url::parse("http://example.com").unwrap();
        assert!(!np.should_bypass(&url));
    }

    #[test]
    fn nobypass_exact_host() {
        let np = NoProxy::parse("example.com").unwrap();
        let url = url::Url::parse("http://example.com/path").unwrap();
        assert!(np.should_bypass(&url));
    }

    #[test]
    fn nobypass_exact_host_no_match() {
        let np = NoProxy::parse("example.com").unwrap();
        let url = url::Url::parse("http://other.com/path").unwrap();
        assert!(!np.should_bypass(&url));
    }

    #[test]
    fn nobypass_exact_host_case_insensitive() {
        let np = NoProxy::parse("Example.Com").unwrap();
        let url = url::Url::parse("http://example.com/path").unwrap();
        assert!(np.should_bypass(&url));
    }

    #[test]
    fn nobypass_domain_suffix_match() {
        let np = NoProxy::parse(".example.com").unwrap();
        let url = url::Url::parse("http://foo.example.com/path").unwrap();
        assert!(np.should_bypass(&url));
    }

    #[test]
    fn nobypass_domain_suffix_excludes_bare_domain() {
        let np = NoProxy::parse(".example.com").unwrap();
        let url = url::Url::parse("http://example.com/path").unwrap();
        assert!(!np.should_bypass(&url));
    }

    #[test]
    fn nobypass_domain_suffix_no_match() {
        let np = NoProxy::parse(".example.com").unwrap();
        let url = url::Url::parse("http://notexample.com/path").unwrap();
        assert!(!np.should_bypass(&url));
    }

    #[test]
    fn nobypass_bare_domain_includes_subdomains() {
        let np = NoProxy::parse("example.com").unwrap();
        for url in ["http://example.com/path", "http://api.example.com/path"] {
            assert!(np.should_bypass(&url::Url::parse(url).unwrap()));
        }
        assert!(!np.should_bypass(&url::Url::parse("http://badexample.com").unwrap()));
    }

    #[test]
    fn parse_httpx_bare_domain_includes_subdomains_at_label_boundary() {
        let np = NoProxy::parse_httpx("example.test").unwrap();
        for url in [
            "http://example.test/",
            "http://www.example.test/",
            "http://deep.www.example.test/",
        ] {
            assert!(np.should_bypass(&url::Url::parse(url).unwrap()), "{url}");
        }
        for url in [
            "http://wwwexample.test/",
            "http://notexample.test/",
            "http://example.test.evil/",
        ] {
            assert!(!np.should_bypass(&url::Url::parse(url).unwrap()), "{url}");
        }
    }

    #[test]
    fn parse_httpx_leading_dot_excludes_bare_domain() {
        let np = NoProxy::parse_httpx(".example.test").unwrap();
        assert!(!np.should_bypass(&url::Url::parse("http://example.test/").unwrap()));
        assert!(np.should_bypass(&url::Url::parse("http://www.example.test/").unwrap()));
    }

    #[test]
    fn parse_httpx_host_port_requires_explicit_normalized_port() {
        let np = NoProxy::parse_httpx("example.test:80").unwrap();
        assert!(!np.should_bypass(&url::Url::parse("http://example.test/").unwrap()));
        assert!(!np.should_bypass(&url::Url::parse("http://example.test:80/").unwrap()));
        assert!(!np.should_bypass(&url::Url::parse("http://example.test:8080/").unwrap()));
        assert!(np.should_bypass(&url::Url::parse("https://example.test:80/").unwrap()));

        let non_default = NoProxy::parse_httpx("example.test:8080").unwrap();
        assert!(non_default.should_bypass(&url::Url::parse("http://example.test:8080/").unwrap()));
        assert!(
            non_default.should_bypass(&url::Url::parse("http://www.example.test:8080/").unwrap())
        );
        assert!(!non_default.should_bypass(&url::Url::parse("http://example.test:8081/").unwrap()));
    }

    #[test]
    fn nobypass_cidr_matches_ip_address() {
        let np = NoProxy::parse("10.0.0.0/8").unwrap();
        assert!(np.should_bypass(&url::Url::parse("http://10.42.1.9").unwrap()));
        assert!(!np.should_bypass(&url::Url::parse("http://11.42.1.9").unwrap()));
    }

    #[test]
    fn nobypass_host_port_match() {
        let np = NoProxy::parse("example.com:8080").unwrap();
        let url = url::Url::parse("http://example.com:8080/path").unwrap();
        assert!(np.should_bypass(&url));
    }

    #[test]
    fn nobypass_host_port_no_match_port() {
        let np = NoProxy::parse("example.com:8080").unwrap();
        let url = url::Url::parse("http://example.com:9090/path").unwrap();
        assert!(!np.should_bypass(&url));
    }

    #[test]
    fn nobypass_host_port_no_match_host() {
        let np = NoProxy::parse("example.com:8080").unwrap();
        let url = url::Url::parse("http://other.com:8080/path").unwrap();
        assert!(!np.should_bypass(&url));
    }

    #[test]
    fn nobypass_host_port_default_port_https() {
        let np = NoProxy::parse("example.com:443").unwrap();
        let url = url::Url::parse("https://example.com/path").unwrap();
        assert!(np.should_bypass(&url));
    }

    #[test]
    fn nobypass_host_port_default_port_http() {
        let np = NoProxy::parse("example.com:80").unwrap();
        let url = url::Url::parse("http://example.com/path").unwrap();
        assert!(np.should_bypass(&url));
    }

    #[test]
    fn nobypass_localhost_name() {
        let np = NoProxy::parse("localhost").unwrap();
        let url = url::Url::parse("http://localhost/path").unwrap();
        assert!(np.should_bypass(&url));
    }

    #[test]
    fn nobypass_localhost_127() {
        let np = NoProxy::parse("localhost").unwrap();
        let url = url::Url::parse("http://127.0.0.1/path").unwrap();
        assert!(np.should_bypass(&url));
    }

    #[test]
    fn nobypass_localhost_ipv6() {
        let np = NoProxy::parse("localhost").unwrap();
        let url = url::Url::parse("http://[::1]/path").unwrap();
        assert!(np.should_bypass(&url));
    }

    #[test]
    fn nobypass_localhost_not_matching() {
        let np = NoProxy::parse("localhost").unwrap();
        let url = url::Url::parse("http://example.com/path").unwrap();
        assert!(!np.should_bypass(&url));
    }

    #[test]
    fn httpx_localhost_does_not_match_loopback_literals() {
        let np = NoProxy::parse_httpx("localhost").unwrap();
        assert!(np.should_bypass(&url::Url::parse("http://localhost/path").unwrap()));
        assert!(!np.should_bypass(&url::Url::parse("http://127.0.0.1/path").unwrap()));
        assert!(!np.should_bypass(&url::Url::parse("http://[::1]/path").unwrap()));
    }

    #[test]
    fn nobypass_wildcard_matches_everything() {
        let np = NoProxy::parse("*").unwrap();
        let urls = [
            "http://example.com",
            "https://other.org",
            "http://10.0.0.1:8080",
            "http://localhost",
        ];
        for url_str in &urls {
            let url = url::Url::parse(url_str).unwrap();
            assert!(np.should_bypass(&url), "should bypass {url_str}");
        }
    }

    #[test]
    fn nobypass_ipv6_literal() {
        let np = NoProxy::parse("[::1]").unwrap();
        let url = url::Url::parse("http://[::1]/path").unwrap();
        assert!(np.should_bypass(&url));
    }

    #[test]
    fn nobypass_ipv4_literal() {
        let np = NoProxy::parse("10.0.0.1").unwrap();
        let url = url::Url::parse("http://10.0.0.1/path").unwrap();
        assert!(np.should_bypass(&url));
    }

    #[test]
    fn httpx_scheme_qualified_no_proxy_is_scheme_specific() {
        let np = NoProxy::parse_httpx("http://example.test:8080").unwrap();
        assert!(np.should_bypass(&url::Url::parse("http://example.test:8080/").unwrap()));
        assert!(!np.should_bypass(&url::Url::parse("https://example.test:8080/").unwrap()));
        assert!(!np.should_bypass(&url::Url::parse("http://example.test:8081/").unwrap()));
    }

    #[test]
    fn httpx_bare_ipv6_no_proxy_is_not_host_port() {
        let np = NoProxy::parse_httpx("::1").unwrap();
        assert!(np.should_bypass(&url::Url::parse("http://[::1]/").unwrap()));
    }

    #[test]
    fn httpx_rejects_bracketed_and_prefix_looking_ipv6_entries() {
        for entry in ["[::1]", "[::1]:8080", "::1/128", "[::1]/128"] {
            assert!(NoProxy::parse_httpx(entry).is_err(), "{entry}");
        }
    }

    #[test]
    fn httpx_invalid_ipv6_entry_error_is_bounded() {
        let entry = format!("[{}]", ":".repeat(1000));
        let error = NoProxy::parse_httpx(&entry).unwrap_err().to_string();
        assert!(error.len() < 320);
        assert!(!error.contains(&entry));
    }

    #[test]
    fn native_ipv6_cidr_behavior_remains_available() {
        let np = NoProxy::parse("2001:db8::/32").unwrap();
        assert!(np.should_bypass(&url::Url::parse("http://[2001:db8::1]/").unwrap()));
        assert!(!np.should_bypass(&url::Url::parse("http://[2001:db9::1]/").unwrap()));
    }

    #[test]
    fn bare_ipv6_no_proxy_matches() {
        let np = NoProxy::parse("::1").unwrap();
        assert!(np.should_bypass(&url::Url::parse("http://[::1]/").unwrap()));
    }

    #[test]
    fn httpx_cidr_looking_no_proxy_is_exact_host_text() {
        let np = NoProxy::parse_httpx("10.0.0.0/8").unwrap();
        assert!(np.should_bypass(&url::Url::parse("http://10.0.0.0/").unwrap()));
        assert!(!np.should_bypass(&url::Url::parse("http://10.42.1.9/").unwrap()));
    }

    #[test]
    fn nobypass_multiple_rules_first_match() {
        let np = NoProxy::parse("localhost, .example.com").unwrap();
        let url1 = url::Url::parse("http://localhost/path").unwrap();
        let url2 = url::Url::parse("http://foo.example.com/path").unwrap();
        let url3 = url::Url::parse("http://other.com/path").unwrap();
        assert!(np.should_bypass(&url1));
        assert!(np.should_bypass(&url2));
        assert!(!np.should_bypass(&url3));
    }

    #[cfg(feature = "proxy")]
    #[test]
    fn proxy_tls_weak_and_strict_identities_do_not_alias() {
        use crate::tls::TlsConfig;
        let base = TlsConfig::builder().build();
        let strict_tls = base.clone();
        let weak_tls = base.clone().danger_accept_invalid_certs(true);
        let strict = Proxy::all("https://proxy.example:8443")
            .unwrap()
            .with_proxy_tls_config(strict_tls)
            .config()
            .clone();
        let weak = Proxy::all("https://proxy.example:8443")
            .unwrap()
            .with_proxy_tls_config(weak_tls)
            .config()
            .clone();
        assert_ne!(
            strict.connection_identity(),
            weak.connection_identity(),
            "proxy TLS verification change must fragment route identity"
        );
        let strict_again = strict.clone();
        assert_eq!(
            strict.connection_identity(),
            strict_again.connection_identity(),
            "unchanged proxy clones must remain reusable"
        );
    }

    #[cfg(feature = "proxy")]
    #[test]
    fn proxy_identity_is_opaque_and_non_renderable() {
        let proxy = Proxy::all("http://proxy.example:8080")
            .unwrap()
            .auth(ProxyAuth::basic("user", "secret-pass").unwrap());
        let config = proxy.config();
        let identity = config.connection_identity();
        assert!(!identity.is_empty());
        let debug = format!("{config:?}");
        assert!(
            !debug.contains("secret-pass"),
            "proxy identity diagnostics must not leak credentials: {debug}"
        );
    }
}

/// Parse a raw proxy CONNECT response from bytes.
///
/// Feed arbitrary bytes through the proxy response parser. Returns
/// `(status_code, headers)` on success, or an error for malformed input.
///
/// # Internal use
///
/// This is a testing/fuzzing entry point. It is not part of the stable
/// public API.
#[doc(hidden)]
pub fn parse_proxy_response_bytes(
    data: &[u8],
) -> crate::error::Result<(u16, Vec<(String, String)>)> {
    fn read_line(data: &[u8], max_len: usize) -> crate::error::Result<(&[u8], &[u8])> {
        let newline = data.iter().position(|&byte| byte == b'\n').ok_or_else(|| {
            Error::MalformedProxyResponse("proxy closed connection before end of line".into())
        })?;
        let (line, remaining) = data.split_at(newline + 1);
        let mut line = &line[..newline];
        if line.last() == Some(&b'\r') {
            line = &line[..line.len() - 1];
        }
        if line.len() > max_len {
            return Err(Error::MalformedProxyResponse(format!(
                "proxy response line exceeded maximum length of {max_len} bytes"
            )));
        }
        Ok((line, remaining))
    }

    // This adapter intentionally retains its core error taxonomy and UTF-8
    // result shape for fuzz/tests, but its bounds are owned by the shared
    // wire crate so they cannot silently drift from production CONNECT.
    let limits = eggfetch_http_connect::ConnectResponseLimits::default();

    let (status_line, mut remaining) = read_line(data, limits.max_status_line)?;
    let status_line = std::str::from_utf8(status_line).map_err(|_| {
        Error::MalformedProxyResponse("proxy response contains invalid UTF-8".into())
    })?;
    if status_line.is_empty() {
        return Err(Error::MalformedProxyResponse(
            "proxy closed connection before response".into(),
        ));
    }
    let mut parts = status_line.splitn(3, ' ');
    let _version = parts.next();
    let status_code = parts
        .next()
        .and_then(|part| part.parse::<u16>().ok())
        .ok_or_else(|| {
            Error::MalformedProxyResponse(format!("invalid status line: {status_line}"))
        })?;

    let mut headers = Vec::new();
    let mut total_header_bytes = 0usize;
    loop {
        let (line, rest) = read_line(remaining, limits.max_header_line)?;
        remaining = rest;
        if line.is_empty() {
            break;
        }
        total_header_bytes += line.len();
        if total_header_bytes > limits.max_headers_bytes {
            return Err(Error::MalformedProxyResponse(format!(
                "proxy response headers exceeded maximum total size of {} bytes",
                limits.max_headers_bytes
            )));
        }
        if headers.len() >= limits.max_header_count {
            return Err(Error::MalformedProxyResponse(format!(
                "proxy response exceeded maximum header count of {}",
                limits.max_header_count
            )));
        }
        let line = std::str::from_utf8(line).map_err(|_| {
            Error::MalformedProxyResponse("proxy response contains invalid UTF-8".into())
        })?;
        headers.push(
            line.split_once(':')
                .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
                .ok_or_else(|| {
                    Error::MalformedProxyResponse(format!("invalid header line: {line}"))
                })?,
        );
    }

    Ok((status_code, headers))
}
