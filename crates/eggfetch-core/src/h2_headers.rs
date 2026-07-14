//! HTTP/2 forbidden header handling.
//!
//! Per RFC 9113, Section 8.2.2, certain HTTP/1.1 connection-specific headers
//! are forbidden in HTTP/2 requests. This module provides utilities to strip
//! those headers before sending.
//!
//! # Forbidden headers
//!
//! The following headers MUST NOT be used in HTTP/2:
//!
//! - `Connection`
//! - `Keep-Alive`
//! - `Proxy-Connection`
//! - `Transfer-Encoding` (HTTP/2 uses DATA frames with content-length instead)
//! - `Upgrade`
//! - `TE` (only `trailers` is permitted as a value)
//!
//! Stripping these headers unconditionally is safe because they are
//! hop-by-hop headers that should never be forwarded end-to-end anyway.
//! Hyper's HTTP/2 transport would reject them if present.

use crate::headers::Headers;

/// Headers that are forbidden in HTTP/2 requests.
///
/// Per RFC 9113, Section 8.2.2, these connection-specific headers must
/// not be used in HTTP/2 messages.
const H2_FORBIDDEN_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-connection",
    "transfer-encoding",
    "upgrade",
];

/// Strip HTTP/2-forbidden headers from the given headers.
///
/// This removes `Connection`, `Keep-Alive`, `Proxy-Connection`,
/// `Transfer-Encoding`, and `Upgrade` unconditionally. The `TE` header
/// is special-cased: only the value `trailers` is permitted in HTTP/2,
/// so if `TE` contains any other value, it is removed.
///
/// This is a no-op if no forbidden headers are present.
pub(crate) fn strip_h2_forbidden_headers(headers: &mut Headers) {
    for name in H2_FORBIDDEN_HEADERS {
        headers.remove(name);
    }

    // TE is special: only "trailers" is allowed in HTTP/2.
    if let Some(te_value) = headers.get("te") {
        let te_str = te_value.to_str().unwrap_or("");
        let allowed = te_str
            .split(',')
            .map(|v| v.trim().to_lowercase())
            .all(|v| v == "trailers");
        if !allowed {
            headers.remove("te");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_connection_header() {
        let mut headers = Headers::new();
        headers.insert("connection", "keep-alive").unwrap();
        strip_h2_forbidden_headers(&mut headers);
        assert!(!headers.contains("connection"));
    }

    #[test]
    fn strips_keep_alive_header() {
        let mut headers = Headers::new();
        headers.insert("keep-alive", "timeout=5").unwrap();
        strip_h2_forbidden_headers(&mut headers);
        assert!(!headers.contains("keep-alive"));
    }

    #[test]
    fn strips_proxy_connection_header() {
        let mut headers = Headers::new();
        headers.insert("proxy-connection", "keep-alive").unwrap();
        strip_h2_forbidden_headers(&mut headers);
        assert!(!headers.contains("proxy-connection"));
    }

    #[test]
    fn strips_transfer_encoding_header() {
        let mut headers = Headers::new();
        headers.insert("transfer-encoding", "chunked").unwrap();
        strip_h2_forbidden_headers(&mut headers);
        assert!(!headers.contains("transfer-encoding"));
    }

    #[test]
    fn strips_upgrade_header() {
        let mut headers = Headers::new();
        headers.insert("upgrade", "h2c").unwrap();
        strip_h2_forbidden_headers(&mut headers);
        assert!(!headers.contains("upgrade"));
    }

    #[test]
    fn strips_te_when_not_trailers() {
        let mut headers = Headers::new();
        headers.insert("te", "gzip").unwrap();
        strip_h2_forbidden_headers(&mut headers);
        assert!(!headers.contains("te"));
    }

    #[test]
    fn preserves_te_trailers() {
        let mut headers = Headers::new();
        headers.insert("te", "trailers").unwrap();
        strip_h2_forbidden_headers(&mut headers);
        assert!(headers.contains("te"));
    }

    #[test]
    fn preserves_normal_headers() {
        let mut headers = Headers::new();
        headers.insert("content-type", "application/json").unwrap();
        headers.insert("accept", "*/*").unwrap();
        strip_h2_forbidden_headers(&mut headers);
        assert!(headers.contains("content-type"));
        assert!(headers.contains("accept"));
    }

    #[test]
    fn no_op_on_empty_headers() {
        let mut headers = Headers::new();
        strip_h2_forbidden_headers(&mut headers);
        assert!(headers.is_empty());
    }
}
