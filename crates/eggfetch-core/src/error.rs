//! Error type and result alias for the eggfetch engine.

use crate::timeout::TimeoutPhase;

/// Result alias using [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Error taxonomy for the eggfetch engine.
///
/// This is the single error entry point for callers. Source errors are
/// preserved via [`std::error::Error::source`].
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    /// The provided URL could not be parsed.
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    /// The provided HTTP method could not be parsed.
    #[error("invalid method: {0}")]
    InvalidMethod(String),

    /// The provided header name is not valid.
    #[error("invalid header name: {0}")]
    InvalidHeaderName(String),

    /// The provided header value is not valid.
    #[error("invalid header value: {0}")]
    InvalidHeaderValue(String),

    /// The request could not be built.
    #[error("request build error: {0}")]
    RequestBuild(String),

    /// A connection could not be established.
    #[error("connect error: {0}")]
    Connect(String),

    /// A TLS handshake or configuration error occurred.
    #[error("TLS error: {0}")]
    Tls(String),

    /// An HTTP protocol error occurred.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// An error occurred while processing a request or response body.
    #[error("body error: {0}")]
    Body(String),

    /// An error from the underlying hyper HTTP engine.
    #[error("hyper error: {0}")]
    Hyper(#[from] std::sync::Arc<hyper::Error>),

    /// An error from the hyper-util legacy client.
    #[error("hyper client error: {0}")]
    HyperClient(#[source] std::sync::Arc<hyper_util::client::legacy::Error>),

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::sync::Arc<std::io::Error>),

    /// The requested feature is not yet supported.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// Connection pool acquisition failed or was cancelled.
    #[error("pool error: {0}")]
    Pool(String),

    /// The redirect location is missing or invalid.
    #[error("invalid redirect location: {0}")]
    InvalidRedirectLocation(String),

    /// An authentication header value is invalid.
    #[error("invalid auth header: {0}")]
    InvalidAuthHeader(String),

    /// Conflicting authentication sources (e.g., explicit header + auth config).
    #[error("conflicting auth: {0}")]
    ConflictingAuth(String),

    /// A streaming body cannot be replayed for a redirect.
    #[error("body not replayable for redirect: streaming request bodies cannot be resent")]
    BodyNotReplayableForRedirect,

    /// Too many redirects were followed.
    #[error("too many redirects ({followed} followed, max {max})")]
    TooManyRedirects {
        /// Number of redirects actually followed.
        followed: usize,
        /// Maximum allowed.
        max: usize,
    },

    /// A timeout elapsed during the specified phase.
    #[error("{phase} timeout after {elapsed:?}")]
    Timeout {
        /// Which phase of the request timed out.
        phase: TimeoutPhase,
        /// The duration that was exceeded.
        elapsed: std::time::Duration,
    },
}

impl Error {
    /// Returns the category of this error as a static string.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::InvalidUrl(_) => "invalid_url",
            Self::InvalidMethod(_) => "invalid_method",
            Self::InvalidHeaderName(_) => "invalid_header_name",
            Self::InvalidHeaderValue(_) => "invalid_header_value",
            Self::RequestBuild(_) => "request_build",
            Self::Connect(_) => "connect",
            Self::Tls(_) => "tls",
            Self::Protocol(_) => "protocol",
            Self::Body(_) => "body",
            Self::Hyper(_) => "hyper",
            Self::HyperClient(_) => "hyper_client",
            Self::Io(_) => "io",
            Self::Unsupported(_) => "unsupported",
            Self::InvalidRedirectLocation(_) => "invalid_redirect_location",
            Self::InvalidAuthHeader(_) => "invalid_auth_header",
            Self::ConflictingAuth(_) => "conflicting_auth",
            Self::BodyNotReplayableForRedirect => "body_not_replayable_for_redirect",
            Self::TooManyRedirects { .. } => "too_many_redirects",
            Self::Pool(_) => "pool",
            Self::Timeout { phase, .. } => match phase {
                crate::timeout::TimeoutPhase::Pool => "timeout_pool",
                crate::timeout::TimeoutPhase::Connect => "timeout_connect",
                crate::timeout::TimeoutPhase::Write => "timeout_write",
                crate::timeout::TimeoutPhase::Read => "timeout_read",
                crate::timeout::TimeoutPhase::Total => "timeout_total",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = Error::InvalidUrl("not a url".into());
        assert_eq!(err.to_string(), "invalid URL: not a url");
    }

    #[test]
    fn error_kind() {
        let err = Error::Connect("refused".into());
        assert_eq!(err.kind(), "connect");
    }

    #[test]
    fn error_from_io() {
        let io_err = std::sync::Arc::new(std::io::Error::other("test"));
        let err: Error = io_err.into();
        assert_eq!(err.kind(), "io");
    }
}
