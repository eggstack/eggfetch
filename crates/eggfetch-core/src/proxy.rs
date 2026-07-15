//! Proxy subsystem for eggfetch.
//!
//! Provides HTTP proxy forwarding and HTTPS CONNECT tunneling with
//! secure authentication handling. Proxy logic is feature-gated
//! behind the `proxy` feature.
//!
//! # Supported schemes
//!
//! - `http://` — HTTP proxy (forward HTTP targets, CONNECT for HTTPS)
//!
//! # Environment policy
//!
//! eggfetch does **not** read `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`,
//! or `NO_PROXY` environment variables. Proxy configuration is explicit
//! only: set via [`Proxy::all`], [`Proxy::http`], or [`Proxy::https`]
//! and attach to a client with
//! [`ClientBuilder::proxy`](crate::ClientBuilder::proxy). This avoids
//! surprising behavior when multiple proxy libraries coexist.
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

/// A single `NO_PROXY` bypass rule.
///
/// Parsed from individual entries in a comma-separated `NO_PROXY` string.
/// Each variant represents a different matching strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoProxyRule {
    /// Matches everything.
    Wildcard,
    /// Exact host match.
    Host(String),
    /// Domain suffix match (leading dot, e.g. `.example.com`).
    DomainSuffix(String),
    /// Exact host + port match.
    HostPort(String, u16),
    /// Matches localhost (`127.0.0.1`, `::1`, `localhost`).
    Localhost,
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
/// - `localhost` — matches `localhost`, `127.0.0.1`, `[::1]`
/// - `.example.com` — domain suffix match (matches `example.com` and
///   all subdomains)
/// - `example.com` — exact host match
/// - `example.com:8080` — host + port match (uses scheme default port
///   when the URL has no explicit port)
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
    /// - `localhost` — matches localhost, `127.0.0.1`, `[::1]`
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
        let mut rules = Vec::new();
        for entry in s.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            rules.push(Self::parse_entry(entry)?);
        }
        Ok(Self { rules })
    }

    fn parse_entry(entry: &str) -> Result<NoProxyRule> {
        if entry == "*" {
            return Ok(NoProxyRule::Wildcard);
        }
        if entry.eq_ignore_ascii_case("localhost") {
            return Ok(NoProxyRule::Localhost);
        }

        // IPv6 literal: [::1] or [::1]:8080
        if let Some(rest) = entry.strip_prefix('[') {
            if let Some(close) = rest.find(']') {
                let ipv6 = &rest[..close];
                let remainder = &rest[close + 1..];
                if remainder.is_empty() {
                    // bare IPv6 literal — treat as host
                    return Ok(NoProxyRule::Host(entry.to_owned()));
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
                    if Self::is_localhost(host) {
                        return true;
                    }
                }
                NoProxyRule::Host(h) => {
                    if host.eq_ignore_ascii_case(h.as_str()) {
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
                    if port_matches && host.eq_ignore_ascii_case(h.as_str()) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn is_localhost(host: &str) -> bool {
        host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]"
    }

    fn matches_domain_suffix(host: &str, suffix: &str) -> bool {
        if host.eq_ignore_ascii_case(suffix) {
            return true;
        }
        let host_lower = host.to_ascii_lowercase();
        let suffix_lower = suffix.to_ascii_lowercase();
        host_lower.ends_with(&suffix_lower)
            || (suffix_lower.starts_with('.') && host_lower == suffix_lower[1..])
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
        self.uri.port_or_known_default().unwrap_or(8080)
    }

    /// Returns the proxy scheme.
    #[must_use]
    pub fn scheme(&self) -> &str {
        self.uri.scheme()
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
    #[must_use]
    pub(crate) fn config(&self) -> ProxyConfig {
        ProxyConfig {
            uri: self.uri.clone(),
            auth: self.auth.clone(),
        }
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
/// - scheme must be `http`
/// - host must be present
/// - no userinfo (credentials must be set via `.auth()`)
/// - no fragment
/// - no query string
fn parse_proxy_url(url_str: &str) -> Result<url::Url> {
    let url = url::Url::parse(url_str)
        .map_err(|e| Error::InvalidProxyUrl(format!("invalid proxy URL: {e}")))?;

    match url.scheme() {
        "http" => {}
        other => {
            return Err(Error::InvalidProxyUrl(format!(
                "unsupported proxy scheme '{other}'; only http is supported"
            )));
        }
    }

    if url.host_str().is_none() || url.host_str() == Some("") {
        return Err(Error::InvalidProxyUrl("proxy URL must have a host".into()));
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(Error::InvalidProxyUrl(
            "proxy URL must not contain credentials; use .auth() instead".into(),
        ));
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
    fn parse_proxy_url_rejects_https_scheme() {
        let err = Proxy::all("https://proxy.example:8080").unwrap_err();
        assert_eq!(err.kind(), "invalid_proxy_url");
        assert!(err.to_string().contains("unsupported proxy scheme"));
    }

    #[test]
    fn parse_proxy_url_rejects_no_host() {
        let err = Proxy::all("http://").unwrap_err();
        assert_eq!(err.kind(), "invalid_proxy_url");
    }

    #[test]
    fn parse_proxy_url_rejects_userinfo() {
        let err = Proxy::all("http://user:pass@proxy.example:8080").unwrap_err();
        assert_eq!(err.kind(), "invalid_proxy_url");
        assert!(err.to_string().contains("must not contain credentials"));
        // Ensure credentials are not echoed.
        assert!(!err.to_string().contains("pass"));
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
    fn nobypass_domain_suffix_exact() {
        let np = NoProxy::parse(".example.com").unwrap();
        let url = url::Url::parse("http://example.com/path").unwrap();
        assert!(np.should_bypass(&url));
    }

    #[test]
    fn nobypass_domain_suffix_no_match() {
        let np = NoProxy::parse(".example.com").unwrap();
        let url = url::Url::parse("http://notexample.com/path").unwrap();
        assert!(!np.should_bypass(&url));
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
