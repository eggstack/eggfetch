//! Compiled protocol capability gate for the CLI.
//!
//! The CLI forwards its `http2`/`http3` crate features to the matching
//! `eggfetch-core` features, so the compiled capability is read with `cfg!`
//! instead of a hand-maintained list of core features.
//!
//! A strict protocol flag (`--http2`, `--http3`) that the built binary cannot
//! honor fails closed before any connection is attempted: the request is
//! rejected with an error that names the missing capability, and the CLI never
//! silently negotiates a different protocol.

/// A protocol capability the CLI can require.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProtocolCapability {
    /// HTTP/2, provided by the CLI's `http2` feature.
    Http2,
    /// HTTP/3, provided by the CLI's `http3` feature.
    Http3,
}

impl ProtocolCapability {
    /// Whether this build has the capability compiled in.
    pub(crate) const fn is_compiled_in(self) -> bool {
        match self {
            Self::Http2 => cfg!(feature = "http2"),
            Self::Http3 => cfg!(feature = "http3"),
        }
    }

    /// The protocol name used in operator-facing messages.
    pub(crate) const fn protocol_name(self) -> &'static str {
        match self {
            Self::Http2 => "HTTP/2",
            Self::Http3 => "HTTP/3",
        }
    }

    /// The CLI crate feature that provides the capability.
    pub(crate) const fn feature_name(self) -> &'static str {
        match self {
            Self::Http2 => "http2",
            Self::Http3 => "http3",
        }
    }

    /// The error message for a build that does not have the capability.
    pub(crate) fn missing_capability_message(self) -> String {
        format!(
            "{} is not supported by this build: the built binary was compiled without the {} feature",
            self.protocol_name(),
            self.feature_name()
        )
    }
}

/// Reject a strict protocol request the built binary cannot honor.
///
/// # Errors
///
/// Returns [`eggfetch_core::Error::Unsupported`] naming the missing capability
/// when the corresponding feature is not compiled into this binary. The
/// rejection happens before the client is built, so no connection is attempted
/// and no request deadline or pool permit is consumed.
pub(crate) fn require(capability: ProtocolCapability) -> Result<(), eggfetch_core::Error> {
    if capability.is_compiled_in() {
        return Ok(());
    }
    Err(eggfetch_core::Error::Unsupported(
        capability.missing_capability_message(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_names_are_stable() {
        assert_eq!(ProtocolCapability::Http2.protocol_name(), "HTTP/2");
        assert_eq!(ProtocolCapability::Http3.protocol_name(), "HTTP/3");
        assert_eq!(ProtocolCapability::Http2.feature_name(), "http2");
        assert_eq!(ProtocolCapability::Http3.feature_name(), "http3");
    }

    #[test]
    fn missing_capability_message_names_the_protocol_and_the_feature() {
        let message = ProtocolCapability::Http2.missing_capability_message();
        assert!(message.contains("HTTP/2"), "{message}");
        assert!(message.contains("http2"), "{message}");
        assert!(!message.contains("HTTP client not available"), "{message}");
    }

    #[test]
    fn require_matches_the_compiled_feature_set() {
        assert_eq!(
            ProtocolCapability::Http2.is_compiled_in(),
            cfg!(feature = "http2")
        );
        assert_eq!(
            ProtocolCapability::Http3.is_compiled_in(),
            cfg!(feature = "http3")
        );
    }

    #[test]
    #[cfg(not(feature = "http2"))]
    fn require_rejects_http2_without_the_feature() {
        let error = require(ProtocolCapability::Http2).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("HTTP/2"), "{message}");
        assert!(!message.contains("HTTP client not available"), "{message}");
    }

    #[test]
    #[cfg(feature = "http2")]
    fn require_accepts_http2_with_the_feature() {
        assert!(require(ProtocolCapability::Http2).is_ok());
    }

    #[test]
    #[cfg(not(feature = "http3"))]
    fn require_rejects_http3_without_the_feature() {
        let error = require(ProtocolCapability::Http3).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("HTTP/3"), "{message}");
        assert!(!message.contains("HTTP client not available"), "{message}");
    }

    #[test]
    #[cfg(feature = "http3")]
    fn require_accepts_http3_with_the_feature() {
        assert!(require(ProtocolCapability::Http3).is_ok());
    }
}
