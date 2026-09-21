//! Private client configuration and its redacted defaults.

use std::sync::Arc;

use crate::headers::Headers;
use crate::http_version::HttpVersionPolicy;
use crate::timeout::Timeout;

#[cfg(feature = "cookies")]
use crate::cookie::CookieJar;
#[cfg(feature = "proxy")]
use crate::proxy::Proxy;
#[cfg(feature = "redirects")]
use crate::redirect::RedirectPolicy;
#[cfg(feature = "logical-retry")]
use crate::retry::RetryPolicy;
#[cfg(feature = "advanced-routing")]
use crate::transport::dialer::Dialer;

/// Shared client configuration. This module is private; public builder
/// methods remain declared in the parent `client` module.
#[derive(Clone)]
pub(crate) struct ClientConfig {
    pub(crate) default_headers: Headers,
    pub(crate) user_agent: Option<String>,
    pub(crate) timeout: Option<Timeout>,
    #[cfg(feature = "redirects")]
    pub(crate) redirect: RedirectPolicy,
    pub(crate) auth: Option<crate::auth::AuthScheme>,
    #[cfg(feature = "cookies")]
    pub(crate) cookie_jar: CookieJar,
    pub(crate) automatic_decompression: bool,
    pub(crate) max_decoded_body_size: Option<usize>,
    pub(crate) max_decompression_ratio: Option<f64>,
    #[cfg(feature = "proxy")]
    pub(crate) proxy: Option<Proxy>,
    #[cfg(feature = "proxy")]
    pub(crate) environment_proxies: Vec<Proxy>,
    #[cfg(feature = "tls-rustls")]
    pub(crate) tls_config: Option<crate::tls::TlsConfig>,
    #[cfg(feature = "logical-retry")]
    pub(crate) retry: Option<RetryPolicy>,
    pub(crate) retry_canceled_requests: bool,
    #[cfg(feature = "advanced-routing")]
    pub(crate) dialer: Option<Arc<dyn Dialer>>,
    #[cfg(feature = "advanced-routing")]
    pub(crate) uds_configured: bool,
    #[allow(
        dead_code,
        reason = "stored for inspection and future request-level use"
    )]
    pub(crate) http_version_policy: HttpVersionPolicy,
}

impl std::fmt::Debug for ClientConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("ClientConfig");
        debug
            .field("default_headers", &self.default_headers)
            .field("user_agent", &self.user_agent)
            .field("timeout", &self.timeout);
        #[cfg(feature = "redirects")]
        debug.field("redirect", &self.redirect);
        debug.field("auth", &self.auth);
        #[cfg(feature = "cookies")]
        debug.field("cookie_jar", &self.cookie_jar);
        debug
            .field("automatic_decompression", &self.automatic_decompression)
            .field("max_decoded_body_size", &self.max_decoded_body_size)
            .field("max_decompression_ratio", &self.max_decompression_ratio);
        #[cfg(feature = "proxy")]
        debug
            .field("proxy", &self.proxy)
            .field("environment_proxies", &self.environment_proxies);
        #[cfg(feature = "tls-rustls")]
        debug.field("tls_config", &self.tls_config);
        #[cfg(feature = "logical-retry")]
        debug.field("retry", &self.retry);
        debug.field("retry_canceled_requests", &self.retry_canceled_requests);
        #[cfg(feature = "advanced-routing")]
        debug
            .field("dialer", &self.dialer.as_ref().map(|_| "configured"))
            .field("uds_configured", &self.uds_configured);
        debug
            .field("http_version_policy", &self.http_version_policy)
            .finish_non_exhaustive()
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            default_headers: Headers::new(),
            user_agent: None,
            timeout: None,
            #[cfg(feature = "redirects")]
            redirect: RedirectPolicy::default(),
            auth: None,
            #[cfg(feature = "cookies")]
            cookie_jar: CookieJar::new(),
            automatic_decompression: true,
            max_decoded_body_size: None,
            max_decompression_ratio: None,
            #[cfg(feature = "proxy")]
            proxy: None,
            #[cfg(feature = "proxy")]
            environment_proxies: Vec::new(),
            #[cfg(feature = "tls-rustls")]
            tls_config: None,
            #[cfg(feature = "logical-retry")]
            retry: None,
            retry_canceled_requests: true,
            #[cfg(feature = "advanced-routing")]
            dialer: None,
            #[cfg(feature = "advanced-routing")]
            uds_configured: false,
            http_version_policy: HttpVersionPolicy::default(),
        }
    }
}
