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
//! # Security
//!
//! - Proxy passwords are never exposed in `Debug`, `Display`, logs,
//!   or error messages.
//! - `Proxy-Authorization` is never forwarded to the destination.
//! - Credentials in proxy URLs are rejected with redacted errors.

use std::fmt;

use crate::error::{Error, Result};

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
        })
    }

    /// Set proxy authentication credentials.
    #[must_use]
    pub fn auth(mut self, auth: ProxyAuth) -> Self {
        self.auth = Some(auth);
        self
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
}
