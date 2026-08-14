//! Centralized secret redaction for eggfetch.
//!
//! All Debug, Display, log, and error output must go through redaction
//! helpers to prevent credential leakage. This module provides the
//! canonical set of sensitive header names and URL sanitization.

use http::{HeaderMap, HeaderValue};
use url::Url;

/// Header names that may contain secrets.
pub const SENSITIVE_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
];

/// Returns true if the given header name is considered sensitive.
pub fn is_sensitive_header(name: &str) -> bool {
    SENSITIVE_HEADERS
        .iter()
        .any(|sensitive| sensitive.eq_ignore_ascii_case(name))
}

/// Clone a [`HeaderMap`], replacing all sensitive header values with
/// `<redacted>`.
pub fn redact_headers(headers: &HeaderMap) -> HeaderMap {
    let mut redacted = headers.clone();
    for name in SENSITIVE_HEADERS {
        if redacted.contains_key(*name) {
            redacted.remove(*name);
            redacted.insert(*name, HeaderValue::from_static("<redacted>"));
        }
    }
    redacted
}

/// Sanitize a URL by stripping userinfo, query parameters, and fragments.
pub fn redact_url(url: &Url) -> String {
    let mut safe = url.clone();
    let _ = safe.set_username("");
    let _ = safe.set_password(None);
    safe.set_query(None);
    safe.set_fragment(None);
    safe.to_string()
}

/// Redact a header value for safe display.
pub fn redact_header_value(value: &str) -> &str {
    if value.is_empty() {
        value
    } else {
        "<redacted>"
    }
}

/// Redact credentials from a URL string for safe display in error messages.
///
/// If the string parses as a valid URL, credentials are stripped. If parsing
/// fails, the original string is returned unchanged.
pub fn redact_url_string(url_str: &str) -> String {
    match Url::parse(url_str) {
        Ok(url) => redact_url(&url),
        Err(_) => url_str.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_headers_list() {
        assert!(is_sensitive_header("authorization"));
        assert!(is_sensitive_header("Authorization"));
        assert!(is_sensitive_header("proxy-authorization"));
        assert!(is_sensitive_header("cookie"));
        assert!(is_sensitive_header("set-cookie"));
        assert!(!is_sensitive_header("content-type"));
        assert!(!is_sensitive_header("user-agent"));
    }

    #[test]
    fn redact_headers_replaces_sensitive() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str("Bearer secret123").unwrap(),
        );
        headers.insert("cookie", HeaderValue::from_str("session=abc").unwrap());
        headers.insert("content-type", HeaderValue::from_str("text/html").unwrap());

        let redacted = redact_headers(&headers);
        assert_eq!(redacted.get("authorization").unwrap(), "<redacted>");
        assert_eq!(redacted.get("cookie").unwrap(), "<redacted>");
        assert_eq!(redacted.get("content-type").unwrap(), "text/html");
    }

    #[test]
    fn redact_url_strips_userinfo_and_query() {
        let url = Url::parse("https://user:pass@example.com/path?q=secret#frag").unwrap();
        let redacted = redact_url(&url);
        assert_eq!(redacted, "https://example.com/path");
    }

    #[test]
    fn redact_url_preserves_path() {
        let url = Url::parse("https://example.com/some/path").unwrap();
        let redacted = redact_url(&url);
        assert_eq!(redacted, "https://example.com/some/path");
    }

    #[test]
    fn redact_proxy_authorization() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "proxy-authorization",
            HeaderValue::from_str("Basic dXNlcjpwYXNz").unwrap(),
        );
        let redacted = redact_headers(&headers);
        assert_eq!(redacted.get("proxy-authorization").unwrap(), "<redacted>");
    }

    #[test]
    fn redact_set_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "set-cookie",
            HeaderValue::from_str("session=abc123; Path=/").unwrap(),
        );
        let redacted = redact_headers(&headers);
        assert_eq!(redacted.get("set-cookie").unwrap(), "<redacted>");
    }

    #[test]
    fn redact_url_with_no_secrets() {
        let url = Url::parse("https://example.com/api").unwrap();
        let redacted = redact_url(&url);
        assert_eq!(redacted, "https://example.com/api");
    }

    #[test]
    fn redact_url_with_query_params() {
        let url = Url::parse("https://example.com/api?key=secret&other=value").unwrap();
        let redacted = redact_url(&url);
        assert!(!redacted.contains("secret"));
        assert!(!redacted.contains("key="));
    }

    #[test]
    fn redact_empty_headers() {
        let headers = HeaderMap::new();
        let redacted = redact_headers(&headers);
        assert!(redacted.is_empty());
    }

    #[test]
    fn redact_url_with_fragment() {
        let url = Url::parse("https://example.com/path#section").unwrap();
        let redacted = redact_url(&url);
        assert!(!redacted.contains('#'));
    }

    #[test]
    fn redact_url_with_password_only() {
        let url = Url::parse("https://user@example.com/path").unwrap();
        let redacted = redact_url(&url);
        assert!(!redacted.contains("user"));
    }

    #[test]
    fn redact_url_string_strips_credentials() {
        let redacted = redact_url_string("https://user:pass@example.com/path?q=secret");
        assert_eq!(redacted, "https://example.com/path");
        assert!(!redacted.contains("user"));
        assert!(!redacted.contains("pass"));
    }

    #[test]
    fn redact_url_string_no_credentials() {
        let redacted = redact_url_string("https://example.com/api");
        assert_eq!(redacted, "https://example.com/api");
    }

    #[test]
    fn redact_url_string_invalid_returns_original() {
        let input = "not a url at all";
        let redacted = redact_url_string(input);
        assert_eq!(redacted, input);
    }
}
