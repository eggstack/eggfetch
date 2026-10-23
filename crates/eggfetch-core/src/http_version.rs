//! HTTP version negotiation policy.

use crate::error::Error;

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

/// Typed evidence about the HTTP/3 capability of the running build.
///
/// The value is resolved from the compiled feature set so that failure
/// classification never has to inspect a rendered message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Http3Capability {
    /// The `http3` feature is compiled in.
    Compiled,
    /// The `http3` feature is not compiled in.
    NotCompiled,
}

impl Http3Capability {
    /// The HTTP/3 capability of the running build.
    pub(crate) const CURRENT: Self = if cfg!(feature = "http3") {
        Self::Compiled
    } else {
        Self::NotCompiled
    };

    /// Returns `true` when this build can route HTTP/3 requests.
    pub(crate) const fn is_available(self) -> bool {
        matches!(self, Self::Compiled)
    }
}

/// Classify a standard-route dispatch that has no usable Hyper client.
///
/// Two different operator problems share one failure point:
///
/// - the build has no HTTP/3 support while the caller asked for HTTP/3
///   (a build capability gap), and
/// - the request had no usable client for any other reason (a transport
///   failure).
///
/// Reporting the second for the first misreports a build capability gap as a
/// transport failure, so the two are separated here. The decision is derived
/// from the typed policy value and the compiled capability, never from parsing
/// a `Display` string.
///
/// Both outcomes stay inside the existing [`Error::Unsupported`] domain: no
/// public error variant is added and the transport-failure taxonomy is
/// unchanged. Only the message differs.
pub(crate) fn missing_client_error(policy: HttpVersionPolicy) -> Error {
    if matches!(policy, HttpVersionPolicy::Http3Only) && !Http3Capability::CURRENT.is_available() {
        return Error::Unsupported(
            "HTTP/3 is not supported by this build: the http3 feature was not compiled in".into(),
        );
    }
    Error::Unsupported("HTTP client not available for this protocol".into())
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
            #[cfg(not(feature = "transport-http2"))]
            HttpVersionPolicy::Http2Only => Self(HttpVersionPolicy::Http1Only),
            #[cfg(not(feature = "http3"))]
            HttpVersionPolicy::Http3Only => Self(HttpVersionPolicy::Http1Only),
            #[allow(
                unused_variables,
                reason = "the `Auto` arm ignores the caller's HTTP/3 flag when the http2 or http3 feature is compiled out, so the binding is unused in those builds"
            )]
            HttpVersionPolicy::Auto { allow_http3 } => {
                #[cfg(not(feature = "transport-http2"))]
                return Self(HttpVersionPolicy::Http1Only);
                #[cfg(all(feature = "transport-http2", not(feature = "http3")))]
                return Self(HttpVersionPolicy::Auto { allow_http3: false });
                #[cfg(all(feature = "transport-http2", feature = "http3"))]
                return Self(HttpVersionPolicy::Auto { allow_http3 });
            }
            _ => Self(policy),
        }
    }

    /// Returns `true` if HTTP/1.1 should be enabled on the connector.
    #[must_use]
    #[allow(
        dead_code,
        reason = "the HTTP/1.1 switch is only read by the ALPN and connector builders, which are compiled out when neither tls-rustls nor transport-http2 is enabled"
    )]
    pub(crate) const fn enable_http1(self) -> bool {
        matches!(
            self.0,
            HttpVersionPolicy::Http1Only | HttpVersionPolicy::Auto { .. }
        )
    }

    /// Returns `true` if HTTP/2 should be enabled on the connector.
    #[must_use]
    #[allow(
        dead_code,
        reason = "the HTTP/2 switch is only read by the ALPN and connector builders, which are compiled out when neither tls-rustls nor transport-http2 is enabled"
    )]
    pub(crate) const fn enable_http2(self) -> bool {
        matches!(
            self.0,
            HttpVersionPolicy::Http2Only | HttpVersionPolicy::Auto { .. }
        )
    }

    /// Returns `true` if HTTP/3 should be used (QUIC transport).
    #[must_use]
    #[allow(
        dead_code,
        reason = "the HTTP/3 switch is only read by the HTTP/3 connector setup, which is compiled out when the http3 feature is disabled"
    )]
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
    #[cfg(feature = "transport-http2")]
    fn auto_enables_both() {
        let enabler =
            HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Auto { allow_http3: false });
        assert!(enabler.enable_http1());
        assert!(enabler.enable_http2());
        assert!(!enabler.use_http3());
    }

    #[test]
    #[cfg(not(feature = "transport-http2"))]
    fn auto_downgrades_without_feature() {
        let enabler =
            HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Auto { allow_http3: false });
        assert!(enabler.enable_http1());
        assert!(!enabler.enable_http2());
    }

    #[test]
    #[cfg(feature = "transport-http2")]
    fn http2_only_enables_http2_only() {
        let enabler = HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Http2Only);
        assert!(!enabler.enable_http1());
        assert!(enabler.enable_http2());
    }

    #[test]
    #[cfg(not(feature = "transport-http2"))]
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

    #[test]
    #[cfg(not(feature = "http3"))]
    fn missing_client_error_names_the_http3_capability_gap() {
        let error = missing_client_error(HttpVersionPolicy::Http3Only);
        let message = error.to_string();
        assert!(
            message.contains("HTTP/3"),
            "the failure must name HTTP/3: {message}"
        );
        assert!(
            !message.contains("HTTP client not available"),
            "the failure must not reuse the generic missing-client wording: {message}"
        );
    }

    #[test]
    #[cfg(feature = "http3")]
    fn missing_client_error_keeps_the_generic_wording_when_http3_is_compiled() {
        let error = missing_client_error(HttpVersionPolicy::Http3Only);
        let message = error.to_string();
        assert!(
            message.contains("HTTP client not available"),
            "a compiled-in HTTP/3 build has no capability gap to report: {message}"
        );
    }

    #[test]
    fn missing_client_error_keeps_the_generic_wording_for_other_policies() {
        for policy in [
            HttpVersionPolicy::Http1Only,
            HttpVersionPolicy::Http2Only,
            HttpVersionPolicy::Auto { allow_http3: false },
            HttpVersionPolicy::Auto { allow_http3: true },
        ] {
            let message = missing_client_error(policy).to_string();
            assert!(
                message.contains("HTTP client not available"),
                "unexpected classification for {policy:?}: {message}"
            );
        }
    }

    #[test]
    fn classification_is_typed_and_does_not_render_the_policy() {
        // The classification is driven by the policy variant and the compiled
        // capability, never by a rendered policy string: the produced message
        // must not embed the policy's own rendering.
        let message = missing_client_error(HttpVersionPolicy::Http3Only).to_string();
        assert!(
            !message.contains("Http3Only"),
            "the classification must not embed a rendered policy: {message}"
        );
        assert!(
            !message.contains("Auto"),
            "the classification must not embed a rendered policy: {message}"
        );
    }

    #[test]
    fn http3_capability_matches_the_compiled_feature_set() {
        assert_eq!(
            Http3Capability::CURRENT.is_available(),
            cfg!(feature = "http3"),
            "the typed capability must track the compiled feature set"
        );
    }
}
