//! Bounded CONNECT response-head parser.
//!
//! The parser operates on a caller-owned [`tokio::io::BufReader`] so bytes
//! already read past `\r\n\r\n` stay buffered for the tunnel. It returns
//! the status without imposing any success policy and never consumes the
//! response body.

use tokio::io::{AsyncRead, AsyncReadExt, BufReader};

use crate::error::ConnectError;

/// Caller-provided bounds for a CONNECT response head.
///
/// `max_header_count` counts actual header fields only; the status line
/// and the terminal blank line are not counted. Aggregate accounting sums
/// the lengths of header lines without their trailing `CRLF`, excluding
/// the status line and terminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    clippy::struct_field_names,
    reason = "bounds share the `max_` prefix by domain convention; the four limits are always configured together"
)]
pub struct ConnectResponseLimits {
    /// Maximum status-line length in bytes (without `CRLF`).
    pub max_status_line: usize,
    /// Maximum single header-line length in bytes (without `CRLF`).
    pub max_header_line: usize,
    /// Maximum aggregate header bytes (sum of header-line lengths).
    pub max_headers_bytes: usize,
    /// Maximum number of header fields.
    pub max_header_count: usize,
}

impl Default for ConnectResponseLimits {
    /// Eggfetch-oriented defaults matching the historical proxy parser.
    fn default() -> Self {
        Self {
            max_status_line: 4096,
            max_header_line: 8192,
            max_headers_bytes: 65_536,
            max_header_count: 100,
        }
    }
}

impl ConnectResponseLimits {
    /// Create explicit limits.
    #[must_use]
    pub fn new(
        max_status_line: usize,
        max_header_line: usize,
        max_headers_bytes: usize,
        max_header_count: usize,
    ) -> Self {
        Self {
            max_status_line,
            max_header_line,
            max_headers_bytes,
            max_header_count,
        }
    }
}

/// Parsed CONNECT response head.
///
/// Header values are raw bytes (`obs-text` preserved); no whole-head or
/// header-value UTF-8 is required. Duplicate response headers are
/// preserved in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectResponseHead {
    /// Numeric status code (returned intact; no success policy applied).
    pub status: u16,
    /// Response headers as `(name, raw value)` pairs.
    pub headers: Vec<(String, Vec<u8>)>,
    /// Raw reason phrase bytes when present.
    pub reason_phrase: Option<Vec<u8>>,
}

/// Read one CONNECT response head from a buffered stream.
///
/// The stream must already be connected; this function performs no dialing,
/// TLS, timeout, or retry. Bytes buffered past the head terminator remain
/// in `stream`'s buffer for the caller to drain. The response body is never
/// consumed.
///
/// # Errors
///
/// Returns [`ConnectError`] on I/O failure, truncation, malformed status or
/// header lines, or when any caller-provided bound is exceeded. Error
/// messages never echo unbounded proxy-controlled bytes.
pub async fn read_connect_response_head<S>(
    stream: &mut BufReader<S>,
    limits: &ConnectResponseLimits,
) -> Result<ConnectResponseHead, ConnectError>
where
    S: AsyncRead + Unpin,
{
    let status_line = read_bounded_line(stream, limits.max_status_line)
        .await
        .map_err(|error| match error {
            ConnectError::HeaderLineTooLarge { .. } => ConnectError::StatusLineTooLarge {
                max: limits.max_status_line,
            },
            other => other,
        })?;

    if status_line.is_empty() {
        return Err(ConnectError::MalformedStatus);
    }

    let mut parts = status_line.splitn(3, |byte| *byte == b' ');
    let _version = parts.next();
    let status = parts
        .next()
        .and_then(|token| std::str::from_utf8(token).ok())
        .and_then(|text| text.parse::<u16>().ok())
        .ok_or(ConnectError::MalformedStatus)?;
    let reason_phrase = parts.next().map(<[u8]>::to_vec);
    // Reject empty reason marker? `splitn` yields an empty third part for a
    // trailing space (e.g. "HTTP/1.1 200 "); normalize it to `None` so
    // callers see absence rather than an empty phrase.
    let reason_phrase = reason_phrase.filter(|phrase| !phrase.is_empty());

    let mut headers = Vec::new();
    let mut total_header_bytes: usize = 0;
    loop {
        let line = read_bounded_line(stream, limits.max_header_line).await?;
        if line.is_empty() {
            break;
        }
        total_header_bytes = total_header_bytes.checked_add(line.len()).ok_or(
            ConnectError::ResponseHeadTooLarge {
                max: limits.max_headers_bytes,
            },
        )?;
        if total_header_bytes > limits.max_headers_bytes {
            return Err(ConnectError::ResponseHeadTooLarge {
                max: limits.max_headers_bytes,
            });
        }
        if headers.len() >= limits.max_header_count {
            return Err(ConnectError::TooManyHeaders {
                max: limits.max_header_count,
            });
        }
        let colon = line
            .iter()
            .position(|byte| *byte == b':')
            .ok_or(ConnectError::MalformedHeader)?;
        let name_bytes = trim_ows(&line[..colon]);
        let value_bytes = trim_ows(&line[colon + 1..]);
        let name = std::str::from_utf8(name_bytes).map_err(|_| ConnectError::MalformedHeader)?;
        validate_response_header_name(name)?;
        headers.push((name.to_owned(), value_bytes.to_vec()));
    }

    Ok(ConnectResponseHead {
        status,
        headers,
        reason_phrase,
    })
}

/// Read one CRLF-terminated line without the terminator.
///
/// Returns [`ConnectError::UnexpectedEof`] on EOF before `\n`, and
/// [`ConnectError::HeaderLineTooLarge`] when `max_len` is exceeded. The
/// caller maps the status-line case to `StatusLineTooLarge`.
async fn read_bounded_line<S>(
    stream: &mut BufReader<S>,
    max_len: usize,
) -> Result<Vec<u8>, ConnectError>
where
    S: AsyncRead + Unpin,
{
    let mut buf = Vec::with_capacity(256.min(max_len.saturating_add(1)));
    let mut byte = [0_u8; 1];
    loop {
        let n = stream.read(&mut byte).await?;
        if n == 0 {
            return Err(ConnectError::UnexpectedEof);
        }
        if byte[0] == b'\n' {
            break;
        }
        buf.push(byte[0]);
        if buf.len() > max_len {
            return Err(ConnectError::HeaderLineTooLarge { max: max_len });
        }
    }
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    Ok(buf)
}

/// Strip optional whitespace (`SP`/`HTAB`) around a header field.
fn trim_ows(value: &[u8]) -> &[u8] {
    let start = value
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t'))
        .unwrap_or(value.len());
    let end = value
        .iter()
        .rposition(|byte| !matches!(byte, b' ' | b'\t'))
        .map_or(start, |index| index + 1);
    &value[start..end]
}

/// Validate a response header name as an HTTP token.
fn validate_response_header_name(name: &str) -> Result<(), ConnectError> {
    if name.is_empty() || name.len() > 256 {
        return Err(ConnectError::MalformedHeader);
    }
    for byte in name.bytes() {
        if !is_token_byte(byte) {
            return Err(ConnectError::MalformedHeader);
        }
    }
    Ok(())
}

/// HTTP `tchar` per RFC 9110 §5.1.
fn is_token_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'!' | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'\''
            | b'*'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~'
            | b'0'..=b'9'
            | b'A'..=b'Z'
            | b'a'..=b'z'
    )
}
