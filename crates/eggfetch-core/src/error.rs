//! Error type and result alias for the eggfetch engine.

use crate::timeout::TimeoutPhase;

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Result alias using [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Evidence-backed detail for a request that failed while establishing a
/// network connection.
///
/// This is an additive, opt-in classification. It deliberately does not
/// replace [`Error::kind`] and is only reported when the transport exposes
/// typed evidence for the category.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetworkFailureKind {
    /// Name resolution failed before a destination address was attempted.
    Dns,
    /// Every attempted destination failed with connection refusal.
    ConnectionRefused,
    /// A connection-establishment failure that is not more specifically
    /// classified.
    Connect,
}

/// A request error with optional structured connection-failure detail.
///
/// The ordinary [`Error`] remains the compatibility surface. Use
/// [`Client::send_detailed`](crate::Client::send_detailed) or
/// [`RequestBuilder::send_detailed`](crate::RequestBuilder::send_detailed)
/// when native Rust code needs the additional, evidence-backed network
/// classification.
pub struct RequestFailure {
    error: Error,
    network_failure: Option<NetworkFailureKind>,
}

impl RequestFailure {
    /// Return the original public error by reference.
    #[must_use]
    pub fn error(&self) -> &Error {
        &self.error
    }

    /// Consume the detailed wrapper and return the original public error.
    #[must_use]
    pub fn into_error(self) -> Error {
        self.error
    }

    /// Return evidence-backed connection-failure detail, when available.
    #[must_use]
    pub fn network_failure_kind(&self) -> Option<NetworkFailureKind> {
        self.network_failure
    }

    /// Return whether the error represents a timeout.
    ///
    /// This includes both ordinary request-phase [`Error::Timeout`] values
    /// and established-transport inactivity [`Error::TransportIoTimeout`]
    /// values. Caller-dialer timeout categories remain available through
    /// [`Error::custom_transport_error`] and are not reclassified here.
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(
            self.error,
            Error::Timeout { .. } | Error::TransportIoTimeout { .. }
        )
    }

    /// Return the request timeout phase, when the error carries one.
    ///
    /// Established-transport inactivity timeouts intentionally return
    /// `None`; they are not ordinary request-phase timeouts.
    #[must_use]
    pub fn timeout_phase(&self) -> Option<TimeoutPhase> {
        match self.error {
            Error::Timeout { phase, .. } => Some(phase),
            _ => None,
        }
    }

    pub(crate) fn from_error(error: Error) -> Self {
        Self {
            error,
            network_failure: None,
        }
    }

    pub(crate) fn from_context(error: Error, context: &RequestFailureContext) -> Self {
        Self {
            error,
            network_failure: context.network_failure_kind(),
        }
    }
}

impl std::fmt::Debug for RequestFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestFailure")
            .field("error", &self.error)
            .field("network_failure", &self.network_failure)
            .finish()
    }
}

impl std::fmt::Display for RequestFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for RequestFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.error.source()
    }
}

/// Request-local storage for the one terminal network classification.
///
/// This is only allocated by detailed sends. An atomic is sufficient because
/// the context stores one small value and later terminal attempts replace
/// earlier transient classifications.
pub(crate) struct RequestFailureContext {
    network_failure: AtomicU8,
}

impl RequestFailureContext {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            network_failure: AtomicU8::new(0),
        })
    }

    pub(crate) fn clear(&self) {
        self.network_failure.store(0, Ordering::Relaxed);
    }

    pub(crate) fn record(&self, kind: NetworkFailureKind) {
        self.network_failure
            .store(kind as u8 + 1, Ordering::Relaxed);
    }

    fn network_failure_kind(&self) -> Option<NetworkFailureKind> {
        match self.network_failure.load(Ordering::Relaxed) {
            1 => Some(NetworkFailureKind::Dns),
            2 => Some(NetworkFailureKind::ConnectionRefused),
            3 => Some(NetworkFailureKind::Connect),
            _ => None,
        }
    }
}

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

    /// A caller-supplied resolved destination is invalid for the request.
    #[error("invalid resolved destination: {0}")]
    InvalidResolvedTarget(String),

    /// A pinned request attempted to follow a redirect that changes origin.
    #[error("resolved destination cannot be reused across a cross-origin redirect")]
    ResolvedTargetRedirect,

    /// Request JSON serialization failed.
    #[error("JSON serialization error: {0}")]
    JsonSerialize(String),

    /// Response JSON deserialization failed.
    #[error("JSON deserialization error: {0}")]
    JsonDeserialize(String),

    /// A connection could not be established.
    #[error("connect error: {0}")]
    Connect(String),

    /// A caller-supplied native dialer failed.
    #[error("custom transport error: {0}")]
    CustomTransport(#[source] std::sync::Arc<crate::transport::dialer::DialError>),

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
    #[cfg(any(feature = "http1", feature = "http2"))]
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

    /// A response decompression error occurred.
    #[error("decompression error: {0}")]
    Decompression(String),

    /// The server used a content encoding that is not supported.
    #[error("unsupported content encoding: {0}")]
    UnsupportedContentEncoding(String),

    /// A timeout elapsed during the specified phase.
    #[error("{phase} timeout after {elapsed:?}")]
    Timeout {
        /// Which phase of the request timed out.
        phase: TimeoutPhase,
        /// The duration that was exceeded.
        elapsed: std::time::Duration,
    },

    /// An established transport made no read or write progress before its
    /// dedicated inactivity deadline.
    #[error("transport {direction:?} inactivity timeout after {elapsed:?}")]
    TransportIoTimeout {
        /// Which established I/O direction stalled.
        direction: crate::transport::lifecycle::TransportIoDirection,
        /// Duration for which no progress was observed.
        elapsed: std::time::Duration,
    },

    /// The proxy URL is invalid or malformed.
    #[error("invalid proxy URL: {0}")]
    InvalidProxyUrl(String),

    /// The proxy server rejected the connection or tunnel.
    #[error("proxy error: {0}")]
    ProxyConnect(String),

    /// The proxy server requires authentication.
    #[error("proxy authentication required")]
    ProxyAuthRequired,

    /// The CONNECT tunnel was rejected by the proxy.
    #[error("CONNECT rejected: {status} {body}")]
    ProxyConnectRejected {
        /// HTTP status code from the proxy.
        status: u16,
        /// Response body or description.
        body: String,
    },

    /// The proxy response could not be parsed.
    #[error("malformed proxy response: {0}")]
    MalformedProxyResponse(String),

    /// The decoded body exceeded the configured size limit.
    #[error("decoded body exceeded max decoded body size")]
    DecodedBodyTooLarge,

    /// The decompression ratio exceeded the configured limit.
    #[error("decompression ratio exceeded max ratio")]
    DecompressionRatioExceeded,

    /// A TLS configuration error occurred.
    #[error("TLS configuration error: {0}")]
    TlsConfig(String),

    /// A CA certificate bundle could not be parsed.
    #[error("CA bundle error: {0}")]
    CaBundle(String),

    /// A client certificate or private key could not be loaded.
    #[error("client certificate error: {0}")]
    ClientCert(String),

    /// A private key could not be parsed or decrypted.
    #[error("private key error: {0}")]
    PrivateKey(String),

    /// Certificate verification failed.
    #[error("certificate verification failed: {0}")]
    CertificateVerification(String),

    /// Hostname verification failed.
    #[error("hostname verification failed: {0}")]
    HostnameVerification(String),

    /// The request body is not replayable and cannot be retried.
    #[error("body not replayable for retry")]
    BodyNotReplayableForRetry,

    /// The retry budget was exhausted.
    #[error("retry budget exhausted after {attempts} attempts")]
    RetryBudgetExhausted {
        /// Number of attempts made.
        attempts: usize,
    },

    /// Retry is not enabled for this request or client.
    #[error("retry not configured")]
    RetryNotConfigured,

    /// The HTTP/2 connection received a GOAWAY frame from the server.
    ///
    /// `last_stream_id` is currently always `0`: `h2` 0.4 does not expose
    /// the GOAWAY last-stream-id through its public error API, so the raw
    /// error text is preserved in `debug_data` instead.
    #[error("HTTP/2 GOAWAY: last_stream_id={last_stream_id}, debug={debug_data}")]
    Http2GoAway {
        /// The last stream ID the server will process (currently always `0`, see above).
        last_stream_id: u32,
        /// Debug data from the GOAWAY frame.
        debug_data: String,
    },

    /// An HTTP/2 stream was reset by the server with the given reason code.
    #[error("HTTP/2 stream reset: {reason}")]
    Http2StreamReset {
        /// The HTTP/2 reason code (e.g., `REFUSED_STREAM`, `CANCEL`).
        reason: String,
    },

    /// An HTTP/2 flow-control error occurred.
    #[error("HTTP/2 flow control error: {0}")]
    Http2FlowControl(String),

    /// An HTTP/2 protocol error occurred.
    #[error("HTTP/2 protocol error: {0}")]
    Http2Protocol(String),

    /// An HTTP/3 connection error occurred.
    #[error("HTTP/3 connect error: {0}")]
    H3Connect(String),

    /// An HTTP/3 connection was closed by the peer.
    #[error("HTTP/3 connection closed: {0}")]
    H3ConnectionClosed(String),

    /// An HTTP/3 stream error occurred.
    #[error("HTTP/3 stream error: {0}")]
    H3Stream(String),

    /// An HTTP/3 protocol error occurred.
    #[error("HTTP/3 protocol error: {0}")]
    H3Protocol(String),

    /// A trace callback signaled the transport to abort.
    #[error("trace callback aborted the request")]
    TraceCallbackAborted,
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
            Self::InvalidResolvedTarget(_) => "invalid_resolved_target",
            Self::ResolvedTargetRedirect => "resolved_target_redirect",
            Self::JsonSerialize(_) => "json_serialize",
            Self::JsonDeserialize(_) => "json_deserialize",
            Self::Connect(_) => "connect",
            Self::CustomTransport(_) => "custom_transport",
            Self::Tls(_) => "tls",
            Self::Protocol(_) => "protocol",
            Self::Body(_) => "body",
            Self::Hyper(_) => "hyper",
            #[cfg(any(feature = "http1", feature = "http2"))]
            Self::HyperClient(_) => "hyper_client",
            Self::Io(_) => "io",
            Self::Unsupported(_) => "unsupported",
            Self::InvalidRedirectLocation(_) => "invalid_redirect_location",
            Self::InvalidAuthHeader(_) => "invalid_auth_header",
            Self::ConflictingAuth(_) => "conflicting_auth",
            Self::BodyNotReplayableForRedirect => "body_not_replayable_for_redirect",
            Self::TooManyRedirects { .. } => "too_many_redirects",
            Self::Pool(_) => "pool",
            Self::Decompression(_) => "decompression",
            Self::UnsupportedContentEncoding(_) => "unsupported_content_encoding",
            Self::Timeout { phase, .. } => match phase {
                crate::timeout::TimeoutPhase::Pool => "timeout_pool",
                crate::timeout::TimeoutPhase::Connect => "timeout_connect",
                crate::timeout::TimeoutPhase::ProxyConnect => "timeout_proxy_connect",
                crate::timeout::TimeoutPhase::ProxyTls => "timeout_proxy_tls",
                crate::timeout::TimeoutPhase::Write => "timeout_write",
                crate::timeout::TimeoutPhase::Read => "timeout_read",
                crate::timeout::TimeoutPhase::Total => "timeout_total",
            },
            Self::TransportIoTimeout { direction, .. } => match direction {
                crate::transport::lifecycle::TransportIoDirection::Read => "transport_read_timeout",
                crate::transport::lifecycle::TransportIoDirection::Write => {
                    "transport_write_timeout"
                }
            },
            Self::InvalidProxyUrl(_) => "invalid_proxy_url",
            Self::ProxyConnect(_) => "proxy_connect",
            Self::ProxyAuthRequired => "proxy_auth_required",
            Self::ProxyConnectRejected { .. } => "proxy_connect_rejected",
            Self::MalformedProxyResponse(_) => "malformed_proxy_response",
            Self::DecodedBodyTooLarge => "decoded_body_too_large",
            Self::DecompressionRatioExceeded => "decompression_ratio_exceeded",
            Self::TlsConfig(_) => "tls_config",
            Self::CaBundle(_) => "ca_bundle",
            Self::ClientCert(_) => "client_cert",
            Self::PrivateKey(_) => "private_key",
            Self::CertificateVerification(_) => "certificate_verification",
            Self::HostnameVerification(_) => "hostname_verification",
            Self::BodyNotReplayableForRetry => "body_not_replayable_for_retry",
            Self::RetryBudgetExhausted { .. } => "retry_budget_exhausted",
            Self::RetryNotConfigured => "retry_not_configured",
            Self::Http2GoAway { .. } => "http2_go_away",
            Self::Http2StreamReset { .. } => "http2_stream_reset",
            Self::Http2FlowControl(_) => "http2_flow_control",
            Self::Http2Protocol(_) => "http2_protocol",
            Self::H3Connect(_) => "h3_connect",
            Self::H3ConnectionClosed(_) => "h3_connection_closed",
            Self::H3Stream(_) => "h3_stream",
            Self::H3Protocol(_) => "h3_protocol",
            Self::TraceCallbackAborted => "trace_callback_aborted",
        }
    }

    /// Returns the nested caller error when this is a custom-dialer failure.
    #[must_use]
    pub fn custom_transport_error(&self) -> Option<&crate::transport::dialer::DialError> {
        match self {
            Self::CustomTransport(error) => Some(error.as_ref()),
            _ => None,
        }
    }

    /// Returns whether this is a physical-connection admission timeout.
    #[must_use]
    pub fn is_physical_connection_admission_timeout(&self) -> bool {
        matches!(self, Self::Pool(message) if message == crate::transport::lifecycle::PHYSICAL_ADMISSION_TIMEOUT)
    }

    /// Returns whether this is an established-transport inactivity timeout.
    #[must_use]
    pub fn is_transport_io_timeout(&self) -> bool {
        matches!(self, Self::TransportIoTimeout { .. })
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

    #[test]
    fn request_failure_exposes_timeout_without_changing_error() {
        let error = Error::Timeout {
            phase: TimeoutPhase::Connect,
            elapsed: std::time::Duration::from_millis(5),
        };
        let failure = RequestFailure::from_error(error);
        assert!(failure.is_timeout());
        assert_eq!(failure.timeout_phase(), Some(TimeoutPhase::Connect));
        assert_eq!(failure.error().kind(), "timeout_connect");
        assert_eq!(failure.into_error().kind(), "timeout_connect");
    }

    #[test]
    fn request_failure_keeps_transport_timeout_distinct() {
        let failure = RequestFailure::from_error(Error::TransportIoTimeout {
            direction: crate::transport::lifecycle::TransportIoDirection::Read,
            elapsed: std::time::Duration::from_secs(1),
        });
        assert!(failure.is_timeout());
        assert_eq!(failure.timeout_phase(), None);
    }

    #[test]
    fn request_failure_context_replaces_transient_classification() {
        let context = RequestFailureContext::new();
        context.record(NetworkFailureKind::Dns);
        context.record(NetworkFailureKind::ConnectionRefused);
        let failure = RequestFailure::from_context(Error::Connect("refused".into()), &context);
        assert_eq!(
            failure.network_failure_kind(),
            Some(NetworkFailureKind::ConnectionRefused)
        );
        context.clear();
        assert_eq!(context.network_failure_kind(), None);
    }

    #[test]
    fn error_http2_go_away_display() {
        let err = Error::Http2GoAway {
            last_stream_id: 1,
            debug_data: "no error: connection closed".into(),
        };
        assert_eq!(err.kind(), "http2_go_away");
        let msg = err.to_string();
        assert!(msg.contains("GOAWAY"));
        assert!(msg.contains("no error: connection closed"));
    }

    #[test]
    fn error_http2_stream_reset_display() {
        let err = Error::Http2StreamReset {
            reason: "REFUSED_STREAM: stream refused".into(),
        };
        assert_eq!(err.kind(), "http2_stream_reset");
        assert!(err.to_string().contains("REFUSED_STREAM"));
    }

    #[test]
    fn error_http2_flow_control_display() {
        let err = Error::Http2FlowControl("flow-control violated".into());
        assert_eq!(err.kind(), "http2_flow_control");
    }

    #[test]
    fn error_http2_protocol_display() {
        let err = Error::Http2Protocol("stream closed after headers".into());
        assert_eq!(err.kind(), "http2_protocol");
    }

    #[test]
    fn http2_refused_stream_is_retryable() {
        let err = Error::Http2StreamReset {
            reason: "REFUSED_STREAM: stream refused before processing".into(),
        };
        assert!(crate::retry::RetryPolicy::is_error_retryable(&err));
    }

    #[test]
    fn http2_cancel_is_not_retryable() {
        let err = Error::Http2StreamReset {
            reason: "CANCEL: stream no longer needed".into(),
        };
        assert!(!crate::retry::RetryPolicy::is_error_retryable(&err));
    }

    #[test]
    fn http2_go_away_is_not_retryable() {
        let err = Error::Http2GoAway {
            last_stream_id: 0,
            debug_data: " shutting down".into(),
        };
        assert!(!crate::retry::RetryPolicy::is_error_retryable(&err));
    }
}
