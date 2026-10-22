//! Cookie policy parity fixture (ADR-0009 rule 3; milestone 005 §6.1/§6.2).
//!
//! This is the engine side of the cookie measurement. It feeds one identical
//! `Set-Cookie` header stream through the engine's jar and records, per case,
//! whether the cookie is accepted, the stored domain, the host-only flag, the
//! path, the secure flag, the expiry, and the exact `Cookie` header the
//! engine contributes for a follow-up request.
//!
//! The facade side of the same matrix is measured in
//! `crates/eggfetch-python/tests/compat/test_cookie_policy_parity.py` against
//! a real local HTTP server. Both files use the same case ids, hosts, paths
//! and `Set-Cookie` headers, so the two matrices can be compared row by row.
//!
//! The expected verdicts below are the *agreed* verdicts: the engine and the
//! facade must reach the same accept/reject outcome and contribute the same
//! `Cookie` header for every case, except the one case that is recorded as a
//! residual difference in `docs/residual-differences.md`
//! (`domain_ipv6_literal`).

#![cfg(feature = "cookies")]

use eggfetch_core::cookie::{parse_set_cookie_headers, CookieJar};
use url::Url;

/// One row of the shared cookie policy matrix.
#[derive(Debug, Clone, Copy)]
struct Case {
    /// Stable case id, shared with the facade-side fixture.
    id: &'static str,
    /// Host that served the response (the jar's request host).
    host: &'static str,
    /// Path the `Set-Cookie` response was served from.
    set_path: &'static str,
    /// Path of the follow-up request whose `Cookie` header is measured.
    request_path: &'static str,
    /// The `Set-Cookie` header value under test.
    set_cookie: &'static str,
}

/// The agreed expectation for a case.
///
/// Every attribute is `None` when the cookie is rejected (never stored).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Expectation {
    /// RFC 6265 §5.3 domain-value. The engine normalizes away the leading
    /// dot of a `Domain` attribute, so it never carries one.
    domain: Option<&'static str>,
    /// The host-only flag (`true` when no `Domain` attribute was present).
    host_only: Option<bool>,
    /// The stored path.
    path: Option<&'static str>,
    /// The secure flag.
    secure: Option<bool>,
    /// Whether the cookie carries an explicit expiry.
    persistent: Option<bool>,
    /// The exact `Cookie` header the engine contributes for `request_path`.
    cookie_header: Option<&'static str>,
}

/// The measured verdict for a case.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Measured {
    /// Stored domain, if the cookie was stored.
    domain: Option<String>,
    /// Host-only flag, if the cookie was stored.
    host_only: Option<bool>,
    /// Stored path, if the cookie was stored.
    path: Option<String>,
    /// Secure flag, if the cookie was stored.
    secure: Option<bool>,
    /// Explicit-expiry flag, if the cookie was stored.
    persistent: Option<bool>,
    /// The exact `Cookie` header the engine contributes for `request_path`.
    cookie_header: Option<String>,
}

/// The shared cookie policy matrix and the agreed verdict for each row.
const MATRIX: &[(Case, Expectation)] = &[
    // ── Domain attribute ────────────────────────────────────────────
    (
        Case {
            id: "no_domain",
            host: "example.test",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1",
        },
        Expectation {
            domain: Some("example.test"),
            host_only: Some(true),
            path: Some("/"),
            secure: Some(false),
            persistent: Some(false),
            cookie_header: Some("a=1"),
        },
    ),
    (
        Case {
            id: "domain_exact",
            host: "example.test",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Domain=example.test",
        },
        Expectation {
            domain: Some("example.test"),
            host_only: Some(false),
            path: Some("/"),
            secure: Some(false),
            persistent: Some(false),
            cookie_header: Some("a=1"),
        },
    ),
    (
        Case {
            id: "domain_leading_dot",
            host: "example.test",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Domain=.example.test",
        },
        Expectation {
            domain: Some("example.test"),
            host_only: Some(false),
            path: Some("/"),
            secure: Some(false),
            persistent: Some(false),
            cookie_header: Some("a=1"),
        },
    ),
    (
        Case {
            id: "domain_parent",
            host: "sub.example.test",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Domain=example.test",
        },
        Expectation {
            domain: Some("example.test"),
            host_only: Some(false),
            path: Some("/"),
            secure: Some(false),
            persistent: Some(false),
            cookie_header: Some("a=1"),
        },
    ),
    (
        Case {
            id: "domain_mismatch",
            host: "example.test",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Domain=other.test",
        },
        Expectation {
            domain: None,
            host_only: None,
            path: None,
            secure: None,
            persistent: None,
            cookie_header: None,
        },
    ),
    (
        Case {
            id: "domain_unrelated_suffix",
            host: "example.test",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Domain=evil.test",
        },
        Expectation {
            domain: None,
            host_only: None,
            path: None,
            secure: None,
            persistent: None,
            cookie_header: None,
        },
    ),
    // ── IP literals ──────────────────────────────────────────────────
    // ADR-0009 rule 3: an explicit `Domain` naming the response host's own
    // IP literal is accepted, matching the facade's `http.cookiejar` verdict
    // (HTTPX 0.28.1). The IP literal only ever matches by exact equality
    // (RFC 6265 §5.2.3), so no other host can receive the cookie.
    (
        Case {
            id: "domain_ipv4_literal",
            host: "127.0.0.1",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Domain=127.0.0.1",
        },
        Expectation {
            domain: Some("127.0.0.1"),
            host_only: Some(false),
            path: Some("/"),
            secure: Some(false),
            persistent: Some(false),
            cookie_header: Some("a=1"),
        },
    ),
    (
        Case {
            id: "domain_ipv4_leading_dot",
            host: "127.0.0.1",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Domain=.127.0.0.1",
        },
        Expectation {
            domain: Some("127.0.0.1"),
            host_only: Some(false),
            path: Some("/"),
            secure: Some(false),
            persistent: Some(false),
            cookie_header: Some("a=1"),
        },
    ),
    (
        Case {
            id: "domain_ipv4_mismatch",
            host: "127.0.0.1",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Domain=127.0.0.2",
        },
        Expectation {
            domain: None,
            host_only: None,
            path: None,
            secure: None,
            persistent: None,
            cookie_header: None,
        },
    ),
    // Recorded residual difference: the facade stores this cookie but does
    // not return it (see `docs/residual-differences.md`). The engine keeps
    // RFC 6265 §5.2.3 behavior and returns it.
    (
        Case {
            id: "domain_ipv6_literal",
            host: "[::1]",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Domain=[::1]",
        },
        Expectation {
            domain: Some("[::1]"),
            host_only: Some(false),
            path: Some("/"),
            secure: Some(false),
            persistent: Some(false),
            cookie_header: Some("a=1"),
        },
    ),
    // A host-only cookie for the very same IPv6 host is returned by both
    // jars: the residual difference above is specific to an explicit
    // IPv6-literal `Domain` attribute.
    (
        Case {
            id: "ipv6_host_only",
            host: "[::1]",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1",
        },
        Expectation {
            domain: Some("[::1]"),
            host_only: Some(true),
            path: Some("/"),
            secure: Some(false),
            persistent: Some(false),
            cookie_header: Some("a=1"),
        },
    ),
    (
        Case {
            id: "domain_ipv6_unbracketed",
            host: "[::1]",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Domain=::1",
        },
        Expectation {
            domain: None,
            host_only: None,
            path: None,
            secure: None,
            persistent: None,
            cookie_header: None,
        },
    ),
    // ── Path scoping ────────────────────────────────────────────────
    (
        Case {
            id: "path_api",
            host: "example.test",
            set_path: "/api/set",
            request_path: "/api/v1",
            set_cookie: "a=1; Path=/api",
        },
        Expectation {
            domain: Some("example.test"),
            host_only: Some(true),
            path: Some("/api"),
            secure: Some(false),
            persistent: Some(false),
            cookie_header: Some("a=1"),
        },
    ),
    (
        Case {
            id: "path_api_no_match",
            host: "example.test",
            set_path: "/api/set",
            request_path: "/other",
            set_cookie: "a=1; Path=/api",
        },
        Expectation {
            domain: Some("example.test"),
            host_only: Some(true),
            path: Some("/api"),
            secure: Some(false),
            persistent: Some(false),
            cookie_header: None,
        },
    ),
    (
        Case {
            id: "path_root",
            host: "example.test",
            set_path: "/s",
            request_path: "/api/v1",
            set_cookie: "a=1; Path=/",
        },
        Expectation {
            domain: Some("example.test"),
            host_only: Some(true),
            path: Some("/"),
            secure: Some(false),
            persistent: Some(false),
            cookie_header: Some("a=1"),
        },
    ),
    // ── Secure / expiry ─────────────────────────────────────────────
    (
        Case {
            id: "secure_set",
            host: "example.test",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Secure",
        },
        Expectation {
            domain: Some("example.test"),
            host_only: Some(true),
            path: Some("/"),
            secure: Some(true),
            persistent: Some(false),
            // Cleartext request: a `Secure` cookie is never sent.
            cookie_header: None,
        },
    ),
    (
        Case {
            id: "max_age",
            host: "example.test",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Max-Age=3600",
        },
        Expectation {
            domain: Some("example.test"),
            host_only: Some(true),
            path: Some("/"),
            secure: Some(false),
            persistent: Some(true),
            cookie_header: Some("a=1"),
        },
    ),
    (
        Case {
            id: "expires_future",
            host: "example.test",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Expires=Wed, 21 Oct 2035 07:28:00 GMT",
        },
        Expectation {
            domain: Some("example.test"),
            host_only: Some(true),
            path: Some("/"),
            secure: Some(false),
            persistent: Some(true),
            cookie_header: Some("a=1"),
        },
    ),
    (
        Case {
            id: "expires_past",
            host: "example.test",
            set_path: "/s",
            request_path: "/",
            set_cookie: "a=1; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
        },
        Expectation {
            domain: None,
            host_only: None,
            path: None,
            secure: None,
            persistent: None,
            cookie_header: None,
        },
    ),
];

/// Measure one case through the engine jar.
fn measure(case: &Case) -> Measured {
    let response_url =
        Url::parse(&format!("http://{}{}", case.host, case.set_path)).expect("response url");
    let request_url =
        Url::parse(&format!("http://{}{}", case.host, case.request_path)).expect("request url");
    let headers = vec![case.set_cookie.to_owned()];

    let jar = CookieJar::new();
    jar.update_from_response(&response_url, &headers);
    let stored = jar.all_cookies();
    let cookie_header = jar.cookies_for_url(&request_url);

    let (domain, host_only, path, secure, persistent) = match stored.as_slice() {
        [] => (None, None, None, None, None),
        [cookie] => (
            Some(cookie.domain().to_owned()),
            Some(cookie.is_host_only()),
            Some(cookie.path().to_owned()),
            Some(cookie.is_secure()),
            Some(cookie.is_persistent()),
        ),
        other => {
            panic!(
                "case {}: expected at most one stored cookie, got {}",
                case.id,
                other.len()
            )
        }
    };

    Measured {
        domain,
        host_only,
        path,
        secure,
        persistent,
        cookie_header,
    }
}

#[test]
fn engine_cookie_policy_matches_agreed_matrix() {
    for (case, expected) in MATRIX {
        let measured = measure(case);
        assert_eq!(
            measured.domain.as_deref(),
            expected.domain,
            "case {}: stored domain",
            case.id
        );
        assert_eq!(
            measured.host_only, expected.host_only,
            "case {}: host-only flag",
            case.id
        );
        assert_eq!(
            measured.path.as_deref(),
            expected.path,
            "case {}: path",
            case.id
        );
        assert_eq!(measured.secure, expected.secure, "case {}: secure", case.id);
        assert_eq!(
            measured.persistent, expected.persistent,
            "case {}: persistent",
            case.id
        );
        assert_eq!(
            measured.cookie_header.as_deref(),
            expected.cookie_header,
            "case {}: contributed Cookie header",
            case.id
        );
    }
}

#[test]
fn ip_literal_domain_only_matches_the_exact_host() {
    // The relaxation of the IP-literal `Domain` rule must not widen
    // cross-host matching: an IP-literal `Domain` matches only the host it
    // names, exactly (RFC 6265 §5.2.3).
    let jar = CookieJar::new();
    let response_url = Url::parse("http://127.0.0.1/s").expect("response url");
    jar.update_from_response(&response_url, &["a=1; Domain=127.0.0.1".to_owned()]);

    let same_host = Url::parse("http://127.0.0.1/other").expect("url");
    assert_eq!(
        jar.cookies_for_url(&same_host).as_deref(),
        Some("a=1"),
        "the named IP literal receives the cookie"
    );

    let other_host = Url::parse("http://127.0.0.2/").expect("url");
    assert_eq!(
        jar.cookies_for_url(&other_host),
        None,
        "a different IP literal must not receive the cookie"
    );

    let subdomain = Url::parse("http://127.0.0.1.evil.test/").expect("url");
    assert_eq!(
        jar.cookies_for_url(&subdomain),
        None,
        "a host that merely starts with the named IP literal must not receive the cookie"
    );
}

#[test]
fn expired_cookie_is_parsed_but_not_stored() {
    // The engine parses an already-expired cookie and evicts it instead of
    // storing it, so it is never observable.
    let url = Url::parse("http://example.test/s").expect("url");
    let header = "a=1; Expires=Thu, 01 Jan 1970 00:00:00 GMT".to_owned();
    let parsed = parse_set_cookie_headers(&url, std::slice::from_ref(&header));
    assert_eq!(parsed.len(), 1);
    assert!(parsed[0].is_persistent());

    let jar = CookieJar::new();
    jar.update_from_response(&url, &[header]);
    assert!(jar.is_empty());
    assert_eq!(jar.cookies_for_url(&url), None);
}
