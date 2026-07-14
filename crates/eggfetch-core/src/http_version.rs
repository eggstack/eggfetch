//! HTTP version negotiation policy.

/// Policy controlling which HTTP protocol versions the client may negotiate.
///
/// The default is [`HttpVersionPolicy::Auto`], which allows the client to
/// negotiate HTTP/2 via ALPN when available and fall back to HTTP/1.1
/// otherwise.
///
/// When the `http2` feature is not enabled, only [`HttpVersionPolicy::Http1Only`]
/// is valid; attempting to use `Http2Only` or `Auto` without the `http2`
/// feature will result in an HTTP/1.1-only client at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HttpVersionPolicy {
    /// Only HTTP/1.1 is permitted. The client will not advertise `h2` in
    /// ALPN and will not accept an HTTP/2 response.
    Http1Only,
    /// Only HTTP/2 is permitted. The client advertises `h2` via ALPN. If
    /// the server does not negotiate HTTP/2, the connection fails.
    Http2Only,
    /// Allow negotiation. The client advertises both `h2` and `http/1.1`
    /// via ALPN and accepts whichever protocol the server selects.
    #[default]
    Auto,
}

/// Internal helper that resolves which protocol versions to enable on the
/// connector based on the policy and compiled feature flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct HttpVersionPolicyEnabler(HttpVersionPolicy);

impl HttpVersionPolicyEnabler {
    /// Create an enabler from a policy. When the `http2` feature is not
    /// enabled, `Http2Only` and `Auto` are silently downgraded to
    /// `Http1Only`.
    #[must_use]
    pub(crate) fn from_policy(policy: HttpVersionPolicy) -> Self {
        #[cfg(not(feature = "http2"))]
        {
            let _ = policy;
            Self(HttpVersionPolicy::Http1Only)
        }
        #[cfg(feature = "http2")]
        {
            Self(policy)
        }
    }

    /// Returns `true` if HTTP/1.1 should be enabled on the connector.
    #[must_use]
    pub(crate) const fn enable_http1(self) -> bool {
        matches!(
            self.0,
            HttpVersionPolicy::Http1Only | HttpVersionPolicy::Auto
        )
    }

    /// Returns `true` if HTTP/2 should be enabled on the connector.
    #[must_use]
    pub(crate) const fn enable_http2(self) -> bool {
        matches!(
            self.0,
            HttpVersionPolicy::Http2Only | HttpVersionPolicy::Auto
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_auto() {
        assert_eq!(HttpVersionPolicy::default(), HttpVersionPolicy::Auto);
    }

    #[test]
    fn http1_only_enables_http1_only() {
        let enabler = HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Http1Only);
        assert!(enabler.enable_http1());
        assert!(!enabler.enable_http2());
    }

    #[test]
    #[cfg(feature = "http2")]
    fn auto_enables_both() {
        let enabler = HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Auto);
        assert!(enabler.enable_http1());
        assert!(enabler.enable_http2());
    }

    #[test]
    #[cfg(not(feature = "http2"))]
    fn auto_downgrades_without_feature() {
        let enabler = HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Auto);
        assert!(enabler.enable_http1());
        assert!(!enabler.enable_http2());
    }

    #[test]
    #[cfg(feature = "http2")]
    fn http2_only_enables_http2_only() {
        let enabler = HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Http2Only);
        assert!(!enabler.enable_http1());
        assert!(enabler.enable_http2());
    }

    #[test]
    #[cfg(not(feature = "http2"))]
    fn http2_only_downgrades_without_feature() {
        let enabler = HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Http2Only);
        assert!(enabler.enable_http1());
        assert!(!enabler.enable_http2());
    }
}
