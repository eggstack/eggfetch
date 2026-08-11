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

use crate::error::{Error, Result};
use crate::redact::redact_url_string;

/// A single `NO_PROXY` bypass rule.
///
/// Parsed from individual entries in a comma-separated `NO_PROXY` string.
/// Each variant represents a different matching strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoProxyRule {
    /// Matches everything.
    Wildcard,
    /// Host/domain match without a leading dot.
    Host(String),
    /// Domain suffix match (leading dot, e.g. `.example.com`).
    DomainSuffix(String),
    /// Exact host + port match.
    HostPort(String, u16),
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
    /// Returns an error when an entry contains an invalid port or CIDR value.
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

        // HTTPX's is_ipv4_hostname/is_ipv6_hostname check happens before
        // URL-pattern construction. A CIDR-looking value therefore becomes
        // an exact host pattern for the address portion; it is not subnet
        // matching in the compatibility facade.
        if exact_localhost {
            if let Some((address, _prefix)) = entry.split_once('/') {
                if address.parse::<std::net::IpAddr>().is_ok() {
                    return Ok(NoProxyRule::Host(address.to_ascii_lowercase()));
                }
            }
            if let Ok(address) = entry.parse::<std::net::IpAddr>() {
                return Ok(NoProxyRule::Host(address.to_string()));
            }
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
                    return Ok(NoProxyRule::Host(if exact_localhost {
                        ipv6.to_ascii_lowercase()
                    } else {
                        entry.to_owned()
                    }));
                }
                if let Some(port_str) = remainder.strip_prefix(':') {
                    let port = port_str.parse::<u16>().map_err(|_| {
                        Error::InvalidProxyUrl(format!("invalid port in NO_PROXY entry: {entry}"))
                    })?;
                    return Ok(NoProxyRule::HostPort(format!("[{ipv6}]"), port));
                }
            }
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

        // host:port
        if let Some(colon_pos) = entry.rfind(':') {
            let host = &entry[..colon_pos];
            let port_str = &entry[colon_pos + 1..];
            if let Ok(port) = port_str.parse::<u16>() {
                return Ok(NoProxyRule::HostPort(host.to_owned(), port));
            }
        }

        // plain host
        Ok(NoProxyRule::Host(entry.to_owned()))
    }

    /// Returns `true` if the given URL should bypass the proxy (go direct).
    #[must_use]
    pub fn should_bypass(&self, url: &url::Url) -> bool {
        let host = url.host_str().unwrap_or("");
        let port = url.port();

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
                NoProxyRule::DomainSuffix(suffix) => {
                    if Self::matches_domain_suffix(host, suffix) {
                        return true;
                    }
                }
                NoProxyRule::HostPort(h, p) => {
                    let port_matches = match port {
                        Some(pu) => pu == *p,
                        None => Self::default_port_for_scheme(url.scheme()) == *p,
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
                NoProxyRule::IpNetwork(network, prefix) => {
                    if host
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|candidate| Self::ip_in_network(candidate, *network, *prefix))
                    {
                        return true;
                    }
                }
                NoProxyRule::SchemeHostPort {
                    scheme,
                    host: rule_host,
                    port: rule_port,
                } => {
                    if url.scheme().eq_ignore_ascii_case(scheme)
                        && host.eq_ignore_ascii_case(rule_host)
                        && rule_port.map_or(true, |rule_port| {
                            port == Some(rule_port)
                                || (port.is_none()
                                    && Self::default_port_for_scheme(url.scheme()) == rule_port)
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
        host_lower.ends_with(&format!(".{suffix_lower}"))
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
        host_lower == rule_lower || host_lower.ends_with(&format!(".{rule_lower}"))
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
/// The `Debug` and `Display` implementations redact the password.
/// The raw password is never exposed in error messages or logs.
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
    /// Returns [`Error::InvalidProxyUrl`] if the username or password
    /// contains invalid characters.
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Result<Self> {
        let username = username.into();
        let password = password.into();

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
            Self::Basic { username, .. } => f
                .debug_struct("ProxyAuth::Basic")
                .field("username", username)
                .field("password", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ProxyAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Basic { username, .. } => {
                write!(f, "Basic(username={username})")
            }
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
pub enum ProxyDecision {
    /// Send the request directly to the destination.
    Direct,
    /// Send the request through the specified proxy configuration.
    Proxy(ProxyConfig),
}

/// Resolved proxy configuration for a specific request.
///
/// Contains the proxy URI and optional authentication, without the
/// rule (which has already been evaluated).
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// Proxy URI (e.g., `http://proxy.example:8080`).
    pub(crate) uri: url::Url,
    /// Optional proxy authentication.
    pub(crate) auth: Option<ProxyAuth>,
}

impl ProxyConfig {
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
    #[must_use]
    pub fn port(&self) -> u16 {
        self.uri
            .port_or_known_default()
            .unwrap_or_else(|| match self.uri.scheme() {
                "socks5" | "socks5h" => 1080,
                _ => 80,
            })
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
        let parsed = url::Url::parse(url).map_err(|e| {
            Error::InvalidProxyUrl(format!(
                "invalid proxy URL: {e} ({})",
                redact_url_string(url)
            ))
        })?;
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
        })
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
        })
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
        if uri.username().is_empty() && uri.password().is_none() {
            // No credentials to strip.
        } else {
            let _ = uri.set_username("");
            let _ = uri.set_password(None);
        }

        ProxyConfig { uri, auth }
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
        f.debug_struct("Proxy")
            .field("uri", &self.uri)
            .field("auth", &self.auth)
            .field("rule", &self.rule)
            .field("bypass", &self.bypass)
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

/// Parse and validate a proxy URL.
///
/// Requirements:
/// - scheme must be `http`, `https`, `socks5`, or `socks5h`
/// - host must be present
/// - no fragment
/// - no query string
/// - for `http`/`https`: userinfo is extracted as Basic credentials
/// - for `socks5`/`socks5h`: userinfo is extracted as SOCKS credentials
fn parse_proxy_url(url_str: &str) -> Result<url::Url> {
    let url = url::Url::parse(url_str).map_err(|e| {
        Error::InvalidProxyUrl(format!(
            "invalid proxy URL: {e} ({})",
            redact_url_string(url_str)
        ))
    })?;

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
    fn parse_proxy_url_default_port() {
        let proxy = Proxy::all("http://proxy.example").unwrap();
        assert_eq!(proxy.uri().port_or_known_default(), Some(80));
    }

    #[test]
    fn parse_proxy_url_accepts_https_scheme() {
        let proxy = Proxy::all("https://proxy.example:8443").unwrap();
        assert_eq!(proxy.uri().scheme(), "https");
        assert_eq!(proxy.config().port(), 8443);
    }

    #[test]
    fn parse_proxy_url_https_default_port() {
        let proxy = Proxy::all("https://proxy.example").unwrap();
        assert_eq!(proxy.config().port(), 443);
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
        assert_eq!(config.port(), 1080);
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
        assert!(debug.contains("user"));
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("secret123"));
    }

    #[test]
    fn proxy_auth_redacted_display() {
        let auth = ProxyAuth::basic("user", "secret123").unwrap();
        let display = format!("{auth}");
        assert!(display.contains("user"));
        assert!(!display.contains("secret123"));
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
    fn proxy_display_redacts_url() {
        let proxy = Proxy::all("http://proxy.example:8080").unwrap();
        let display = format!("{proxy}");
        assert!(display.contains("proxy.example"));
        assert!(display.contains("Proxy("));
    }

    #[test]
    fn proxy_config_host_port() {
        let proxy = Proxy::all("http://proxy.example:3128").unwrap();
        let config = proxy.config();
        assert_eq!(config.host(), Some("proxy.example"));
        assert_eq!(config.port(), 3128);
        assert_eq!(config.scheme(), "http");
    }

    #[test]
    fn proxy_config_default_port() {
        let proxy = Proxy::all("http://proxy.example").unwrap();
        let config = proxy.config();
        assert_eq!(config.port(), 80);
    }

    #[test]
    fn proxy_config_socks_default_port() {
        let proxy = Proxy::all("socks5://proxy.example").unwrap();
        let config = proxy.config();
        assert_eq!(config.port(), 1080);
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
    fn noparse_invalid_port_treated_as_host() {
        let np = NoProxy::parse("example.com:notaport").unwrap();
        assert_eq!(np.rules.len(), 1);
        assert_eq!(
            np.rules[0],
            NoProxyRule::Host("example.com:notaport".into())
        );
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
    use tokio::io::BufReader;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime for proxy response parsing");

    rt.block_on(async {
        let cursor = std::io::Cursor::new(data.to_vec());
        let mut reader = BufReader::new(cursor);
        let (status, headers, _remaining) =
            crate::transport::proxy::read_proxy_response(&mut reader).await?;
        Ok((status, headers))
    })
}
