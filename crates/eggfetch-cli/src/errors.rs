//! Private CLI error classification and exit-code policy.

/// Exit codes.
pub(crate) const EXIT_SUCCESS: u8 = 0;
pub(crate) const EXIT_USAGE: u8 = 2;
pub(crate) const EXIT_CONNECT: u8 = 3;
pub(crate) const EXIT_TIMEOUT: u8 = 4;
pub(crate) const EXIT_PROTOCOL: u8 = 5;
pub(crate) const EXIT_STATUS: u8 = 6;
pub(crate) const EXIT_IO: u8 = 7;

#[derive(Debug)]
pub(crate) struct StatusError(pub(crate) u16);

impl std::fmt::Display for StatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP status {}", self.0)
    }
}

impl std::error::Error for StatusError {}

pub(crate) fn map_error_to_exit_code(err: &eggfetch_core::Error) -> u8 {
    use eggfetch_core::Error;
    match err {
        // Usage / configuration errors
        Error::InvalidUrl(_)
        | Error::InvalidMethod(_)
        | Error::InvalidHeaderName(_)
        | Error::InvalidHeaderValue(_)
        | Error::RequestBuild(_)
        | Error::InvalidResolvedTarget(_)
        | Error::ConflictingAuth(_)
        | Error::InvalidProxyUrl(_)
        | Error::TlsConfig(_)
        | Error::CaBundle(_)
        | Error::ClientCert(_)
        | Error::PrivateKey(_)
        | Error::CertificateVerification(_)
        | Error::HostnameVerification(_) => EXIT_USAGE,

        // Timeout errors
        Error::Timeout { .. } | Error::TransportIoTimeout { .. } => EXIT_TIMEOUT,

        // Protocol / decompression errors
        Error::Protocol(_)
        | Error::Decompression(_)
        | Error::UnsupportedContentEncoding(_)
        | Error::DecodedBodyTooLarge
        | Error::DecompressionRatioExceeded
        | Error::Body(_)
        | Error::Unsupported(_)
        | Error::InvalidRedirectLocation(_)
        | Error::InvalidAuthHeader(_)
        | Error::BodyNotReplayableForRedirect
        | Error::TooManyRedirects { .. }
        | Error::BodyNotReplayableForRetry
        | Error::RetryBudgetExhausted { .. }
        | Error::RetryNotConfigured
        | Error::ResolvedTargetRedirect
        | Error::JsonSerialize(_)
        | Error::JsonDeserialize(_)
        | Error::Http2GoAway { .. }
        | Error::Http2StreamReset { .. }
        | Error::Http2FlowControl(_)
        | Error::Http2Protocol(_)
        | Error::H3Connect(_)
        | Error::H3ConnectionClosed(_)
        | Error::H3Stream(_)
        | Error::H3Protocol(_)
        | Error::TraceCallbackAborted => EXIT_PROTOCOL,

        // Connect / TLS / proxy errors
        Error::Connect(_)
        | Error::CustomTransport(_)
        | Error::Tls(_)
        | Error::Pool(_)
        | Error::ProxyConnect(_)
        | Error::ProxyAuthRequired
        | Error::ProxyConnectRejected { .. }
        | Error::MalformedProxyResponse(_) => EXIT_CONNECT,

        // I/O errors
        Error::Hyper(_) | Error::HyperClient(_) | Error::Io(_) => EXIT_IO,
    }
}

/// Exit code for errors that are neither [`eggfetch_core::Error`] nor
/// [`std::io::Error`] (e.g. [`StatusError`], `anyhow` usage errors, file
/// output failures wrapped with context).
///
/// Uses typed downcasts only — never message-substring heuristics — so an
/// unrelated error mentioning "status" or "parse" cannot mis-map. (The
/// SIGINT fallback in `main` uses the same typed chain scan for
/// `ErrorKind::Interrupted`.)
pub(crate) fn map_unknown_error_to_exit_code(err: &anyhow::Error) -> u8 {
    if err.downcast_ref::<StatusError>().is_some() {
        return EXIT_STATUS;
    }
    if err
        .chain()
        .any(|cause| cause.downcast_ref::<std::io::Error>().is_some())
    {
        return EXIT_IO;
    }
    EXIT_USAGE
}
