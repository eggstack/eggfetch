//! Error type and result alias for the eggfetch engine.

use crate::timeout::TimeoutPhase;

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Result alias using [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Generic transport-failure classification for native embedders.
///
/// This is a diagnostic-only fact surface. It does not imply retryability and
/// does not replace [`Error::kind`], [`Error::is_physical_connection_admission_timeout`],
/// [`Error::is_transport_io_timeout`], [`Error::custom_transport_error`], or
/// [`RequestFailure::network_failure_kind`]. Unknown or ambiguous evidence
/// maps to `None`; classification never parses `Display`/`Debug` text and
/// never exposes URLs, credentials, bodies, or nested error strings.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportFailureKind {
    /// Typed evidence proves connection establishment failed and no more
    /// specific TLS/cancellation/protocol category applies.
    Connect,
    /// TLS configuration, handshake, verification, or nested rustls
    /// processing failed.
    Tls,
    /// Typed HTTP framing/protocol failure, including Hyper
    /// parse/incomplete-message termination where proven.
    Protocol,
    /// Typed evidence says the transport/request was cancelled before
    /// normal completion.
    Cancelled,
}

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
    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
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
            #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
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

    /// Classify this error into a generic native transport-failure category.
    ///
    /// The classifier is diagnostic only and does not imply retryability.
    /// `None` means the available typed evidence does not prove one of the
    /// [`TransportFailureKind`] categories. Timeout, physical-admission,
    /// and transport-I/O-inactivity facts remain authoritative through
    /// their existing accessors and never map here.
    #[must_use]
    pub fn transport_failure_kind(&self) -> Option<TransportFailureKind> {
        classify_transport_failure(self)
    }
}

/// Bound for every transport-classifier source-chain walk.
const TRANSPORT_FAILURE_WALK_BOUND: usize = 32;

/// Direct-variant classification without source inspection.
///
/// Returns `Some` only for explicit Eggfetch variants with unambiguous
/// category evidence. All other variants (including timeouts, admission,
/// proxy, retry, H2 peer-action, and H3 lifecycle variants) return `None`.
fn classify_explicit(error: &Error) -> Option<TransportFailureKind> {
    match error {
        Error::Tls(_)
        | Error::TlsConfig(_)
        | Error::CaBundle(_)
        | Error::ClientCert(_)
        | Error::PrivateKey(_)
        | Error::CertificateVerification(_)
        | Error::HostnameVerification(_) => Some(TransportFailureKind::Tls),
        Error::Protocol(_)
        | Error::Http2Protocol(_)
        | Error::Http2FlowControl(_)
        | Error::H3Protocol(_) => Some(TransportFailureKind::Protocol),
        Error::Connect(_) | Error::H3Connect(_) => Some(TransportFailureKind::Connect),
        Error::CustomTransport(error) => match error.kind() {
            crate::transport::dialer::DialErrorKind::Connection => {
                Some(TransportFailureKind::Connect)
            }
            _ => None,
        },
        _ => None,
    }
}

/// Accumulated typed evidence for [`TransportFailureKind`].
///
/// The public precedence applies when converting into the final category:
/// TLS before generic connect, cancellation next, protocol next, and proven
/// connection I/O last. The four flags are independent by construction (one
/// per category), which is clearer than a bitmask for this fixed set.
// Clippy's struct-bool lint prefers a bitmask past three flags; the four
// named fields are kept because each maps to exactly one public category.
#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
struct TransportEvidence {
    tls: bool,
    cancelled: bool,
    protocol: bool,
    connect: bool,
}

impl TransportEvidence {
    /// Record the typed evidence carried by one wrapped cause.
    ///
    /// `body_boundary` proves the failure surfaced while polling a Hyper
    /// response body (the `Error::Hyper` variant is only constructed there),
    /// so premature-termination/malformed-framing I/O inside it is proven
    /// HTTP framing evidence, while the same I/O kinds elsewhere stay
    /// unknown. No `Display`/`Debug` text is inspected.
    fn inspect(&mut self, node: &(dyn std::error::Error + 'static), body_boundary: bool) {
        use std::sync::Arc;
        if let Some(inner) = node
            .downcast_ref::<Error>()
            .or_else(|| node.downcast_ref::<Arc<Error>>().map(Arc::as_ref))
        {
            match classify_explicit(inner) {
                Some(TransportFailureKind::Tls) => self.tls = true,
                Some(TransportFailureKind::Protocol) => self.protocol = true,
                Some(TransportFailureKind::Connect) => self.connect = true,
                // Explicit classification never returns Cancelled today;
                // retain the arm so a future explicit cancellation fact
                // cannot silently fall through.
                Some(TransportFailureKind::Cancelled) => self.cancelled = true,
                None => {}
            }
        }
        let hyper_error = node
            .downcast_ref::<hyper::Error>()
            .or_else(|| node.downcast_ref::<Arc<hyper::Error>>().map(Arc::as_ref));
        if let Some(hyper_error) = hyper_error {
            if hyper_error.is_canceled() {
                self.cancelled = true;
            }
            if hyper_error.is_parse() || hyper_error.is_incomplete_message() {
                self.protocol = true;
            }
        }
        #[cfg(feature = "tls-rustls")]
        if node.downcast_ref::<rustls::Error>().is_some()
            || node.downcast_ref::<Arc<rustls::Error>>().is_some()
        {
            self.tls = true;
        }
        // The legacy client `Error` type only exists when Hyper's HTTP/1 or
        // HTTP/2 protocol support is compiled in (via `transport-http1/2`).
        #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
        {
            let legacy = node
                .downcast_ref::<hyper_util::client::legacy::Error>()
                .or_else(|| {
                    node.downcast_ref::<Arc<hyper_util::client::legacy::Error>>()
                        .map(Arc::as_ref)
                });
            if legacy.is_some_and(hyper_util::client::legacy::Error::is_connect) {
                self.connect = true;
            }
        }
        let io_error = node
            .downcast_ref::<std::io::Error>()
            .or_else(|| node.downcast_ref::<Arc<std::io::Error>>().map(Arc::as_ref));
        if let Some(io_error) = io_error {
            // Only connection refusal is proven establishment failure
            // anywhere.
            if io_error.kind() == std::io::ErrorKind::ConnectionRefused {
                self.connect = true;
            } else if body_boundary
                && matches!(
                    io_error.kind(),
                    std::io::ErrorKind::UnexpectedEof
                        | std::io::ErrorKind::InvalidData
                        | std::io::ErrorKind::InvalidInput
                )
            {
                self.protocol = true;
            }
        }
    }

    /// Resolve the accumulated evidence into the final category.
    fn category(self) -> Option<TransportFailureKind> {
        if self.tls {
            return Some(TransportFailureKind::Tls);
        }
        if self.cancelled {
            return Some(TransportFailureKind::Cancelled);
        }
        if self.protocol {
            return Some(TransportFailureKind::Protocol);
        }
        if self.connect {
            return Some(TransportFailureKind::Connect);
        }
        None
    }
}

/// Central private classifier authority for [`TransportFailureKind`].
///
/// Precedence is typed and explicit: explicit Eggfetch variants first, then
/// bounded wrapped causes with TLS before generic connect, cancellation
/// next, protocol next, and proven connection I/O last. Caller-owned dialer
/// sources are never inspected beyond their
/// [`crate::transport::dialer::DialErrorKind`]; unknown evidence stays
/// unknown and no `Display`/`Debug` text is parsed.
///
/// Two transport-layer facts shape the walk. First, `std::io::Error` hides
/// its custom payload from `source()` (it is exposed only through
/// `get_ref()`), so the walk descends through `get_ref()` edges as well as
/// `source()` edges; this is how nested `rustls::Error` evidence behind
/// hyper-rustls `io::Error` wrappers is found without string matching.
/// Second, the `Error::Hyper` variant is only constructed at the
/// `IncomingErrorBody` polling boundary (after response headers), so I/O
/// premature-termination/malformed-framing evidence inside it proves an HTTP
/// body framing failure, while the same I/O kinds in a bare `Error::Io`
/// stay unknown.
fn classify_transport_failure(error: &Error) -> Option<TransportFailureKind> {
    use std::error::Error as StdError;
    // Caller-owned dialer failures preserve DialErrorKind authority. Only
    // Connection maps broadly; richer custom_transport_error() is unchanged
    // and nested caller sources are never inspected.
    if let Error::CustomTransport(_) = error {
        return classify_explicit(error);
    }
    if let Some(kind) = classify_explicit(error) {
        return Some(kind);
    }
    // The body-boundary context: `Error::Hyper` proves the failure surfaced
    // while polling a Hyper response body, so truncated/malformed framing I/O
    // inside it is proven HTTP framing evidence.
    let body_boundary = matches!(error, Error::Hyper(_));
    let mut evidence = TransportEvidence::default();
    // Bounded worklist DFS over `source()`/`get_ref()` edges. `Error` wraps
    // transport errors in `Arc`, so each expected type is checked both
    // directly and behind `Arc`.
    let mut stack: Vec<&(dyn StdError + 'static)> = vec![error];
    let mut visited: Vec<*const (dyn StdError + 'static)> = Vec::new();
    let mut visits = 0usize;
    // The top-level error itself was already checked explicitly; only wrapped
    // typed causes are inspected below.
    let mut skipped_top = false;
    while let Some(node) = stack.pop() {
        if visits >= TRANSPORT_FAILURE_WALK_BOUND {
            break;
        }
        visits += 1;
        let ptr = std::ptr::from_ref(node);
        if visited.contains(&ptr) {
            continue;
        }
        visited.push(ptr);
        if skipped_top {
            evidence.inspect(node, body_boundary);
        } else {
            skipped_top = true;
        }
        if let Some(source) = node.source() {
            stack.push(source);
        }
        // `io::Error` custom payloads are invisible to `source()`; descend
        // through `get_ref()` so nested typed evidence (e.g. `rustls::Error`
        // behind hyper-rustls wrappers) is still found.
        let payload: Option<&(dyn StdError + 'static)> = node
            .downcast_ref::<std::io::Error>()
            .and_then(|io_error| io_error.get_ref())
            .map(|payload| payload as &(dyn StdError + 'static))
            .or_else(|| {
                node.downcast_ref::<std::sync::Arc<std::io::Error>>()
                    .and_then(|io_error| io_error.get_ref())
                    .map(|payload| payload as &(dyn StdError + 'static))
            });
        if let Some(payload) = payload {
            stack.push(payload);
        }
    }
    evidence.category()
}

/// Returns `true` when a hyper-util legacy client error wraps a canceled
/// hyper connection, using the same centralized classifier evidence as the
/// public [`Error::transport_failure_kind`].
///
/// This preserves the existing retry policy semantics exactly: only the
/// typed cancellation fact is consulted, never the broader
/// [`TransportFailureKind`] category.
#[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
pub(crate) fn hyper_legacy_is_canceled(inner: &hyper_util::client::legacy::Error) -> bool {
    use std::error::Error as _;
    use std::sync::Arc;
    let mut source = inner.source();
    for _ in 0..TRANSPORT_FAILURE_WALK_BOUND {
        let Some(err) = source else { break };
        let canceled = err
            .downcast_ref::<hyper::Error>()
            .or_else(|| err.downcast_ref::<Arc<hyper::Error>>().map(Arc::as_ref))
            .is_some_and(hyper::Error::is_canceled);
        if canceled {
            return true;
        }
        source = err.source();
    }
    false
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

    #[cfg(feature = "logical-retry")]
    #[test]
    fn http2_refused_stream_is_retryable() {
        let err = Error::Http2StreamReset {
            reason: "REFUSED_STREAM: stream refused before processing".into(),
        };
        assert!(crate::retry::RetryPolicy::is_error_retryable(&err));
    }

    #[cfg(feature = "logical-retry")]
    #[test]
    fn http2_cancel_is_not_retryable() {
        let err = Error::Http2StreamReset {
            reason: "CANCEL: stream no longer needed".into(),
        };
        assert!(!crate::retry::RetryPolicy::is_error_retryable(&err));
    }

    #[cfg(feature = "logical-retry")]
    #[test]
    fn http2_go_away_is_not_retryable() {
        let err = Error::Http2GoAway {
            last_stream_id: 0,
            debug_data: " shutting down".into(),
        };
        assert!(!crate::retry::RetryPolicy::is_error_retryable(&err));
    }

    #[test]
    fn transport_failure_explicit_tls_family() {
        for error in [
            Error::Tls("handshake failed".into()),
            Error::TlsConfig("bad config".into()),
            Error::CaBundle("bad bundle".into()),
            Error::ClientCert("bad cert".into()),
            Error::PrivateKey("bad key".into()),
            Error::CertificateVerification("expired".into()),
            Error::HostnameVerification("mismatch".into()),
        ] {
            assert_eq!(
                error.transport_failure_kind(),
                Some(TransportFailureKind::Tls),
                "kind={}",
                error.kind()
            );
        }
    }

    #[test]
    fn transport_failure_explicit_protocol_family() {
        for error in [
            Error::Protocol("framing".into()),
            Error::Http2Protocol("h2 framing".into()),
            Error::Http2FlowControl("window exhausted".into()),
            Error::H3Protocol("h3 framing".into()),
        ] {
            assert_eq!(
                error.transport_failure_kind(),
                Some(TransportFailureKind::Protocol),
                "kind={}",
                error.kind()
            );
        }
    }

    #[test]
    fn transport_failure_explicit_connect_family() {
        assert_eq!(
            Error::Connect("refused".into()).transport_failure_kind(),
            Some(TransportFailureKind::Connect)
        );
        assert_eq!(
            Error::H3Connect("quic refused".into()).transport_failure_kind(),
            Some(TransportFailureKind::Connect)
        );
        let connection = Error::CustomTransport(std::sync::Arc::new(
            crate::transport::dialer::DialError::new(
                crate::transport::dialer::DialErrorKind::Connection,
                "route failed",
            ),
        ));
        assert_eq!(
            connection.transport_failure_kind(),
            Some(TransportFailureKind::Connect)
        );
    }

    #[test]
    fn transport_failure_custom_transport_preserves_dial_kind() {
        for kind in [
            crate::transport::dialer::DialErrorKind::Timeout,
            crate::transport::dialer::DialErrorKind::Authentication,
            crate::transport::dialer::DialErrorKind::Rejected,
            crate::transport::dialer::DialErrorKind::Other,
        ] {
            let error = Error::CustomTransport(std::sync::Arc::new(
                crate::transport::dialer::DialError::new(kind, "route failed"),
            ));
            assert_eq!(error.transport_failure_kind(), None);
            assert!(error.custom_transport_error().is_some());
        }
    }

    #[test]
    fn transport_failure_timeouts_admission_and_io_timeouts_stay_unknown() {
        let timeout = Error::Timeout {
            phase: TimeoutPhase::Connect,
            elapsed: std::time::Duration::from_millis(5),
        };
        assert_eq!(timeout.transport_failure_kind(), None);
        let io_timeout = Error::TransportIoTimeout {
            direction: crate::transport::lifecycle::TransportIoDirection::Read,
            elapsed: std::time::Duration::from_secs(1),
        };
        assert_eq!(io_timeout.transport_failure_kind(), None);
        assert!(io_timeout.is_transport_io_timeout());
        let admission =
            Error::Pool(crate::transport::lifecycle::PHYSICAL_ADMISSION_TIMEOUT.to_owned());
        assert!(admission.is_physical_connection_admission_timeout());
        assert_eq!(admission.transport_failure_kind(), None);
    }

    #[test]
    fn transport_failure_unknown_stays_unknown() {
        for error in [
            Error::InvalidUrl("bad".into()),
            Error::Body("body".into()),
            Error::Pool("busy".into()),
            Error::ProxyConnect("proxy down".into()),
            Error::Http2GoAway {
                last_stream_id: 0,
                debug_data: "shutdown".into(),
            },
            Error::Http2StreamReset {
                reason: "CANCEL".into(),
            },
            Error::H3ConnectionClosed("closed".into()),
            Error::H3Stream("reset".into()),
            Error::TooManyRedirects {
                followed: 5,
                max: 5,
            },
        ] {
            assert_eq!(
                error.transport_failure_kind(),
                None,
                "kind={}",
                error.kind()
            );
        }
    }

    #[test]
    fn transport_failure_does_not_change_kind_or_display() {
        let error = Error::Connect("refused".into());
        assert_eq!(error.kind(), "connect");
        assert_eq!(error.to_string(), "connect error: refused");
        assert_eq!(
            error.transport_failure_kind(),
            Some(TransportFailureKind::Connect)
        );
        let tls = Error::Tls("boom".into());
        assert_eq!(tls.kind(), "tls");
        assert_eq!(tls.to_string(), "TLS error: boom");
    }

    #[test]
    fn transport_failure_wrapped_io_refused_maps_connect_unexpected_eof_stays_unknown() {
        let refused = Error::Io(std::sync::Arc::new(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "refused",
        )));
        assert_eq!(
            refused.transport_failure_kind(),
            Some(TransportFailureKind::Connect)
        );
        let eof = Error::Io(std::sync::Arc::new(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "eof",
        )));
        assert_eq!(eof.transport_failure_kind(), None);
    }

    #[test]
    fn transport_failure_output_is_enum_only_and_redacted() {
        use std::fmt::Write as _;
        #[derive(Debug)]
        struct SecretSource;
        impl std::fmt::Display for SecretSource {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("authorization: Bearer hunter2 cookie=secret")
            }
        }
        impl std::error::Error for SecretSource {}
        let error = Error::CustomTransport(std::sync::Arc::new(
            crate::transport::dialer::DialError::with_source(
                crate::transport::dialer::DialErrorKind::Connection,
                "safe route description",
                SecretSource,
            ),
        ));
        let kind = error.transport_failure_kind();
        assert_eq!(kind, Some(TransportFailureKind::Connect));
        let mut rendered = String::new();
        write!(rendered, "{kind:?}").unwrap();
        assert!(!rendered.contains("hunter2"));
        assert!(!rendered.contains("authorization"));
        let debug = format!("{error:?}");
        assert!(!debug.contains("hunter2"));
    }
}
