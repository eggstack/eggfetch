//! Neutral protocol errors for HTTP/1 CONNECT wire framing.
//!
//! The errors describe only what went wrong on the wire. They carry no
//! Eggfetch client policy (`TimeoutPhase`, retryability, route identity),
//! no TLS state, and no downstream-consumer variants. Diagnostics are
//! bounded: attacker-controlled response bytes and credential material are
//! never echoed.

use std::io;

/// Neutral CONNECT wire error.
///
/// All variants are fail-closed protocol failures. Messages are fixed
/// strings or bounded sizes; they never include raw credentials or
/// unbounded proxy-controlled bytes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ConnectError {
    /// CONNECT target host/port is invalid (empty, control bytes, bad brackets).
    #[error("invalid CONNECT target")]
    InvalidTarget,
    /// A caller-supplied header name/value is invalid or unsafe.
    #[error("invalid CONNECT header")]
    InvalidHeader,
    /// Proxy credentials are invalid (control characters, bad shape).
    #[error("invalid proxy credentials")]
    InvalidCredentials,
    /// Duplicate `Proxy-Authorization` was supplied.
    #[error("duplicate proxy-authorization header")]
    DuplicateAuthorization,
    /// A reserved header would override generated framing.
    #[error("reserved CONNECT header")]
    ReservedHeader,
    /// Serialized request head exceeds the caller-provided bound.
    #[error("CONNECT request head too large: {len} exceeds maximum {max}")]
    RequestHeadTooLarge {
        /// Observed serialized length in bytes.
        len: usize,
        /// Caller-provided maximum in bytes.
        max: usize,
    },
    /// Underlying transport I/O failed.
    #[error("CONNECT transport I/O failed: {0}")]
    Io(String),
    /// Status line is syntactically invalid.
    #[error("malformed CONNECT status line")]
    MalformedStatus,
    /// A header line is syntactically invalid.
    #[error("malformed CONNECT header line")]
    MalformedHeader,
    /// Status line exceeds the caller-provided bound.
    #[error("CONNECT status line exceeds maximum of {max} bytes")]
    StatusLineTooLarge {
        /// Caller-provided maximum in bytes.
        max: usize,
    },
    /// A header line exceeds the caller-provided bound.
    #[error("CONNECT header line exceeds maximum of {max} bytes")]
    HeaderLineTooLarge {
        /// Caller-provided maximum in bytes.
        max: usize,
    },
    /// Aggregate response head exceeds the caller-provided bound.
    #[error("CONNECT response head exceeds maximum of {max} bytes")]
    ResponseHeadTooLarge {
        /// Caller-provided maximum in bytes.
        max: usize,
    },
    /// Response carries more header fields than allowed.
    #[error("too many CONNECT response headers: maximum {max}")]
    TooManyHeaders {
        /// Caller-provided maximum header count.
        max: usize,
    },
    /// Stream ended before the response head completed.
    #[error("unexpected EOF while reading CONNECT response head")]
    UnexpectedEof,
}

impl From<io::Error> for ConnectError {
    fn from(error: io::Error) -> Self {
        // `io::Error` display strings are transport diagnostics, not
        // proxy-controlled response bytes or credentials.
        Self::Io(error.to_string())
    }
}
