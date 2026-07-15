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
///
/// When the `http3` feature is enabled, [`HttpVersionPolicy::Http3Only`]
/// routes requests over QUIC using the h3 protocol. HTTP/3 uses a separate
/// transport (Quinn) and does not share connections with HTTP/1.1 or HTTP/2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpVersionPolicy {
    /// Only HTTP/1.1 is permitted. The client will not advertise `h2` in
    /// ALPN and will not accept an HTTP/2 response.
    Http1Only,
    /// Only HTTP/2 is permitted. The client advertises `h2` via ALPN. If
    /// the server does not negotiate HTTP/2, the connection fails.
    Http2Only,
    /// Only HTTP/3 is permitted. The client uses QUIC via Quinn and the h3
    /// protocol. Requires the `http3` feature.
    Http3Only,
    /// Allow negotiation. The client advertises both `h2` and `http/1.1`
    /// via ALPN and accepts whichever protocol the server selects.
    /// When `allow_http3` is `true` and the `http3` feature is enabled,
    /// HTTP/3 is also included in auto-negotiation. When `allow_http3`
    /// is `false`, HTTP/3 is excluded; use `Http3Only` to explicitly
    /// opt in.
    Auto {
        /// Whether to allow HTTP/3 in auto-negotiation.
        allow_http3: bool,
    },
}

impl Default for HttpVersionPolicy {
    fn default() -> Self {
        Self::Auto { allow_http3: false }
    }
}

impl HttpVersionPolicy {
    /// Returns `true` if HTTP/3 should be used.
    pub(crate) fn use_http3(self) -> bool {
        matches!(self, Self::Http3Only) || matches!(self, Self::Auto { allow_http3: true })
    }
}

/// Internal helper that resolves which protocol versions to enable on the
/// connector based on the policy and compiled feature flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct HttpVersionPolicyEnabler(HttpVersionPolicy);

impl HttpVersionPolicyEnabler {
    /// Create an enabler from a policy. When the `http2` feature is not
    /// enabled, `Http2Only` and `Auto` are silently downgraded to
    /// `Http1Only`. When the `http3` feature is not enabled, `Http3Only`
    /// is silently downgraded to `Http1Only`.
    #[must_use]
    pub(crate) fn from_policy(policy: HttpVersionPolicy) -> Self {
        match policy {
            #[cfg(not(feature = "http2"))]
            HttpVersionPolicy::Http2Only => Self(HttpVersionPolicy::Http1Only),
            #[cfg(not(feature = "http3"))]
            HttpVersionPolicy::Http3Only => Self(HttpVersionPolicy::Http1Only),
            HttpVersionPolicy::Auto { allow_http3 } => {
                #[cfg(not(feature = "http2"))]
                return Self(HttpVersionPolicy::Http1Only);
                #[cfg(all(feature = "http2", not(feature = "http3")))]
                return Self(HttpVersionPolicy::Auto { allow_http3: false });
                #[cfg(all(feature = "http2", feature = "http3"))]
                return Self(HttpVersionPolicy::Auto { allow_http3 });
            }
            _ => Self(policy),
        }
    }

    /// Returns `true` if HTTP/1.1 should be enabled on the connector.
    #[must_use]
    pub(crate) const fn enable_http1(self) -> bool {
        matches!(
            self.0,
            HttpVersionPolicy::Http1Only | HttpVersionPolicy::Auto { .. }
        )
    }

    /// Returns `true` if HTTP/2 should be enabled on the connector.
    #[must_use]
    pub(crate) const fn enable_http2(self) -> bool {
        matches!(
            self.0,
            HttpVersionPolicy::Http2Only | HttpVersionPolicy::Auto { .. }
        )
    }

    /// Returns `true` if HTTP/3 should be used (QUIC transport).
    #[must_use]
    pub(crate) fn use_http3(self) -> bool {
        match self.0 {
            HttpVersionPolicy::Http3Only => true,
            HttpVersionPolicy::Auto { allow_http3 } => allow_http3,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_auto() {
        assert_eq!(
            HttpVersionPolicy::default(),
            HttpVersionPolicy::Auto { allow_http3: false }
        );
    }

    #[test]
    fn http1_only_enables_http1_only() {
        let enabler = HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Http1Only);
        assert!(enabler.enable_http1());
        assert!(!enabler.enable_http2());
        assert!(!enabler.use_http3());
    }

    #[test]
    #[cfg(feature = "http2")]
    fn auto_enables_both() {
        let enabler =
            HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Auto { allow_http3: false });
        assert!(enabler.enable_http1());
        assert!(enabler.enable_http2());
        assert!(!enabler.use_http3());
    }

    #[test]
    #[cfg(not(feature = "http2"))]
    fn auto_downgrades_without_feature() {
        let enabler =
            HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Auto { allow_http3: false });
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

    #[test]
    #[cfg(feature = "http3")]
    fn http3_only_enables_http3_only() {
        let enabler = HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Http3Only);
        assert!(!enabler.enable_http1());
        assert!(!enabler.enable_http2());
        assert!(enabler.use_http3());
    }

    #[test]
    #[cfg(not(feature = "http3"))]
    fn http3_only_downgrades_without_feature() {
        let enabler = HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Http3Only);
        assert!(enabler.enable_http1());
        assert!(!enabler.enable_http2());
        assert!(!enabler.use_http3());
    }
}
