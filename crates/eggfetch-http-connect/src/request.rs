//! CONNECT request serialization and Basic auth helper.
//!
//! Serialization is byte-oriented and side-effect free: it has no timeout,
//! socket, TLS, or retry behavior. Callers write the returned bytes under
//! their own write-deadline policy.

use std::fmt;

use crate::error::ConnectError;
use crate::target::ConnectTarget;

/// A CONNECT request to serialize.
///
/// `proxy_authorization` holds a pre-encoded header value such as
/// `Basic <base64>` (see [`basic_auth_value`]). `extra_headers` holds
/// proxy-only headers as `(name, raw value)` pairs; values are preserved
/// byte-for-byte where HTTP permits (including `obs-text` 0x80..=0xFF).
/// `max_head_bytes` bounds the serialized head before any write.
pub struct ConnectRequest<'a> {
    /// CONNECT destination; also sources the generated `Host` value.
    pub target: &'a ConnectTarget,
    /// Optional pre-encoded `Proxy-Authorization` value (without the name).
    pub proxy_authorization: Option<&'a str>,
    /// Proxy-only headers as `(name, raw value)` pairs.
    pub extra_headers: &'a [(String, Vec<u8>)],
    /// Maximum serialized head size in bytes.
    pub max_head_bytes: usize,
}

impl fmt::Debug for ConnectRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never render credential material: the explicit auth value and
        // any `proxy-authorization` entry smuggled via `extra_headers`
        // are presence-only.
        let redacted_extra: Vec<(&str, String)> = self
            .extra_headers
            .iter()
            .map(|(name, value)| {
                if name.eq_ignore_ascii_case("proxy-authorization") {
                    (name.as_str(), String::from("<redacted>"))
                } else {
                    (name.as_str(), String::from_utf8_lossy(value).into_owned())
                }
            })
            .collect();
        f.debug_struct("ConnectRequest")
            .field("target", &self.target)
            .field(
                "proxy_authorization",
                &self.proxy_authorization.map(|_| "<redacted>"),
            )
            .field("extra_headers", &redacted_extra)
            .field("max_head_bytes", &self.max_head_bytes)
            .finish()
    }
}

/// Build a `Basic <base64(user:pass)>` proxy-auth value.
///
/// # Errors
///
/// Returns [`ConnectError::InvalidCredentials`] when the username contains
/// `:`, or when either input contains `CR`, `LF`, `NUL`, or other control
/// bytes. Inputs are never echoed in errors.
pub fn basic_auth_value(username: &str, password: &str) -> Result<String, ConnectError> {
    if username.contains(':') {
        return Err(ConnectError::InvalidCredentials);
    }
    for value in [username, password] {
        for c in value.chars() {
            if c == '\r' || c == '\n' || c == '\0' || c.is_control() || c == '\u{7f}' {
                return Err(ConnectError::InvalidCredentials);
            }
        }
    }
    {
        use base64::Engine as _;
        let credential = format!("{username}:{password}");
        let encoded = base64::engine::general_purpose::STANDARD.encode(credential.as_bytes());
        Ok(format!("Basic {encoded}"))
    }
}

/// Serialize a CONNECT request head.
///
/// Wire shape:
///
/// ```text
/// CONNECT host:port HTTP/1.1\r\n
/// Host: host:port\r\n
/// [Proxy-Authorization: ...\r\n]
/// [proxy-only headers...]
/// \r\n
/// ```
///
/// # Errors
///
/// Returns [`ConnectError`] when a header name/value is unsafe, when a
/// reserved header (`Host`) would override the generated authority, when
/// two `Proxy-Authorization` values would be emitted, or when the
/// serialized head exceeds `max_head_bytes`. No credentials or header
/// bytes are echoed in errors.
pub fn encode_connect_request(request: &ConnectRequest<'_>) -> Result<Vec<u8>, ConnectError> {
    let authority = request.target.authority();

    if let Some(auth) = request.proxy_authorization {
        validate_header_value_bytes(auth.as_bytes())?;
    }

    // Count proxy-authorization occurrences deterministically: an explicit
    // value plus any extra entry is a duplicate; two extra entries are a
    // duplicate even without an explicit value.
    let mut extra_auth_count = 0_usize;
    for (name, value) in request.extra_headers {
        validate_header_name(name)?;
        validate_header_value_bytes(value)?;
        if name.eq_ignore_ascii_case("proxy-authorization") {
            extra_auth_count += 1;
        }
        if name.eq_ignore_ascii_case("host") {
            return Err(ConnectError::ReservedHeader);
        }
    }
    if request.proxy_authorization.is_some() && extra_auth_count > 0 {
        return Err(ConnectError::DuplicateAuthorization);
    }
    if extra_auth_count > 1 {
        return Err(ConnectError::DuplicateAuthorization);
    }

    let max = request.max_head_bytes;
    let mut out = Vec::new();

    let request_line = format!("CONNECT {authority} HTTP/1.1\r\n");
    let host_line = format!("Host: {authority}\r\n");
    // Bound incrementally so a hostile extra-header set cannot force an
    // unbounded allocation before the limit check.
    let mut len = 0_usize;
    for chunk_len in [
        request_line.len(),
        host_line.len(),
        request
            .proxy_authorization
            .map_or(0, |v| "Proxy-Authorization: ".len() + v.len() + 2),
    ] {
        len = len
            .checked_add(chunk_len)
            .ok_or(ConnectError::RequestHeadTooLarge {
                len: usize::MAX,
                max,
            })?;
        if len > max {
            return Err(ConnectError::RequestHeadTooLarge { len, max });
        }
    }
    for (name, value) in request.extra_headers {
        let header_len = name.len().checked_add(2 + value.len() + 2).ok_or(
            ConnectError::RequestHeadTooLarge {
                len: usize::MAX,
                max,
            },
        )?;
        len = len
            .checked_add(header_len)
            .ok_or(ConnectError::RequestHeadTooLarge {
                len: usize::MAX,
                max,
            })?;
        if len > max {
            return Err(ConnectError::RequestHeadTooLarge { len, max });
        }
    }
    len = len
        .checked_add(2)
        .ok_or(ConnectError::RequestHeadTooLarge {
            len: usize::MAX,
            max,
        })?;
    if len > max {
        return Err(ConnectError::RequestHeadTooLarge { len, max });
    }

    out.extend_from_slice(request_line.as_bytes());
    out.extend_from_slice(host_line.as_bytes());
    if let Some(auth) = request.proxy_authorization {
        out.extend_from_slice(b"Proxy-Authorization: ");
        out.extend_from_slice(auth.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    for (name, value) in request.extra_headers {
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(value);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    debug_assert_eq!(out.len(), len);
    Ok(out)
}

/// Validate a header name as an HTTP token without echoing it.
fn validate_header_name(name: &str) -> Result<(), ConnectError> {
    if name.is_empty() || name.len() > 256 {
        return Err(ConnectError::InvalidHeader);
    }
    for byte in name.bytes() {
        if !is_token_byte(byte) {
            return Err(ConnectError::InvalidHeader);
        }
    }
    Ok(())
}

/// Validate raw header value bytes (allows `obs-text`, rejects framing controls).
fn validate_header_value_bytes(value: &[u8]) -> Result<(), ConnectError> {
    for byte in value {
        // HT (0x09) is allowed; CR/LF, other C0 controls, and DEL would
        // allow response splitting and are rejected.
        if matches!(byte, 0x00..=0x08 | 0x0A..=0x1F | 0x7F) {
            return Err(ConnectError::InvalidHeader);
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
