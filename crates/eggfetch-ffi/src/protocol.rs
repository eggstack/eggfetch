//! Compiled protocol capability gate for the FFI.
//!
//! The FFI forwards its `http2`/`http3` crate features to the matching
//! `eggfetch-core` features, so the compiled capability is read with `cfg!`
//! instead of a hand-maintained list of core features.
//!
//! A strict HTTP-version policy that the linked library cannot honor is
//! recorded when the policy is set and rejected when a request is sent,
//! before any connection is attempted, with an error that names the missing
//! capability. The native Rust downgrade contract of the engine does not
//! apply here: the FFI is a user-facing protocol control.

use eggfetch_core::HttpVersionPolicy;

/// A protocol capability a strict HTTP-version policy requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MissingProtocolCapability {
    /// HTTP/2, provided by the FFI's `http2` feature.
    Http2,
    /// HTTP/3, provided by the FFI's `http3` feature.
    Http3,
}

impl MissingProtocolCapability {
    /// The protocol name used in operator-facing messages.
    pub(crate) const fn protocol_name(self) -> &'static str {
        match self {
            Self::Http2 => "HTTP/2",
            Self::Http3 => "HTTP/3",
        }
    }

    /// The FFI crate feature that provides the capability.
    pub(crate) const fn feature_name(self) -> &'static str {
        match self {
            Self::Http2 => "http2",
            Self::Http3 => "http3",
        }
    }

    /// The error message for a build that does not have the capability.
    pub(crate) fn message(self) -> String {
        format!(
            "{} is not supported by this build: the linked library was compiled without the {} feature",
            self.protocol_name(),
            self.feature_name()
        )
    }

    /// The core error reported for a request this build cannot honor.
    pub(crate) fn error(self) -> eggfetch_core::Error {
        eggfetch_core::Error::Unsupported(self.message())
    }
}

/// The capability a strict policy requires but this build does not provide.
///
/// `Auto` is not strict: it negotiates down to the protocols this build does
/// have, so it is always accepted. `Http1Only` needs no capability.
pub(crate) fn missing_capability(policy: HttpVersionPolicy) -> Option<MissingProtocolCapability> {
    match policy {
        HttpVersionPolicy::Http2Only if !cfg!(feature = "http2") => {
            Some(MissingProtocolCapability::Http2)
        }
        HttpVersionPolicy::Http3Only if !cfg!(feature = "http3") => {
            Some(MissingProtocolCapability::Http3)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http1_only_never_needs_a_capability() {
        assert_eq!(missing_capability(HttpVersionPolicy::Http1Only), None);
    }

    #[test]
    fn auto_is_never_strict() {
        assert_eq!(
            missing_capability(HttpVersionPolicy::Auto { allow_http3: false }),
            None
        );
        assert_eq!(
            missing_capability(HttpVersionPolicy::Auto { allow_http3: true }),
            None
        );
    }

    #[test]
    #[cfg(not(feature = "http2"))]
    fn http2_only_needs_the_http2_capability() {
        assert_eq!(
            missing_capability(HttpVersionPolicy::Http2Only),
            Some(MissingProtocolCapability::Http2)
        );
    }

    #[test]
    #[cfg(feature = "http2")]
    fn http2_only_is_honored_with_the_capability() {
        assert_eq!(missing_capability(HttpVersionPolicy::Http2Only), None);
    }

    #[test]
    #[cfg(not(feature = "http3"))]
    fn http3_only_needs_the_http3_capability() {
        assert_eq!(
            missing_capability(HttpVersionPolicy::Http3Only),
            Some(MissingProtocolCapability::Http3)
        );
    }

    #[test]
    #[cfg(feature = "http3")]
    fn http3_only_is_honored_with_the_capability() {
        assert_eq!(missing_capability(HttpVersionPolicy::Http3Only), None);
    }

    #[test]
    fn capability_message_names_the_protocol_and_the_feature() {
        let message = MissingProtocolCapability::Http2.message();
        assert!(message.contains("HTTP/2"), "{message}");
        assert!(message.contains("http2"), "{message}");
        assert!(!message.contains("HTTP client not available"), "{message}");
    }
}
