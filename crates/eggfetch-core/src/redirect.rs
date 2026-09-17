//! Redirect handling engine.
//!
//! Implements configurable redirect following with method rewrite rules,
//! cross-origin header stripping, body replayability checks, and loop
//! detection. The redirect engine lives entirely in the Rust core; the
//! Python layer only configures policy and exposes response history.

use http::Method;
use url::Url;

use crate::body::RequestBody;
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::request::Request;

/// Policy for HTTPS-to-HTTP redirect downgrades.
///
/// This is a transport-security boundary, not an updater or
/// application policy. When [`RedirectDowngradePolicy::Deny`] is selected,
/// a redirect whose origin uses `https` and whose resolved target uses
/// `http` is rejected before the next hop is dispatched. No request bytes
/// are sent to the downgraded destination.
///
/// Relative and scheme-relative `Location` values resolve against the
/// originating URL first, so a relative redirect from an `https` origin
/// remains `https` and is allowed under `Deny`. Only an explicit
/// downgrade to the `http` scheme is rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RedirectDowngradePolicy {
    /// Allow HTTPS -> HTTP redirects (historical compatibility default).
    ///
    /// This preserves the pre-existing redirect behavior for callers that
    /// have not opted into strict transport policy.
    #[default]
    Allow,
    /// Reject HTTPS -> HTTP redirects before second-hop I/O.
    Deny,
}

/// Configuration for HTTP redirect behavior.
#[derive(Debug, Clone)]
pub struct RedirectPolicy {
    /// Whether to follow redirects at all.
    pub follow: bool,
    /// Maximum number of redirects to follow before erroring.
    pub max_redirects: usize,
    /// How to handle HTTPS -> HTTP redirect downgrades.
    ///
    /// Defaults to [`RedirectDowngradePolicy::Allow`] for compatibility.
    /// Select [`RedirectDowngradePolicy::Deny`] to reject downgrades
    /// before the downgraded request is dispatched.
    pub downgrade: RedirectDowngradePolicy,
}

impl Default for RedirectPolicy {
    /// Default: do not follow redirects. This matches HTTPX's default.
    fn default() -> Self {
        Self {
            follow: false,
            max_redirects: 20,
            downgrade: RedirectDowngradePolicy::Allow,
        }
    }
}

impl RedirectPolicy {
    /// Create a new redirect policy with explicit settings.
    ///
    /// The downgrade policy defaults to
    /// [`RedirectDowngradePolicy::Allow`] for compatibility. Use
    /// [`RedirectPolicy::strict`] or [`RedirectPolicy::with_downgrade`]
    /// to opt into downgrade rejection.
    #[must_use]
    pub fn new(follow: bool, max_redirects: usize) -> Self {
        Self {
            follow,
            max_redirects,
            downgrade: RedirectDowngradePolicy::Allow,
        }
    }

    /// Create a strict policy that follows redirects but rejects
    /// HTTPS -> HTTP downgrades before second-hop I/O.
    #[must_use]
    pub fn strict(max_redirects: usize) -> Self {
        Self {
            follow: true,
            max_redirects,
            downgrade: RedirectDowngradePolicy::Deny,
        }
    }

    /// Set the downgrade policy, returning the updated policy.
    #[must_use]
    pub fn with_downgrade(mut self, downgrade: RedirectDowngradePolicy) -> Self {
        self.downgrade = downgrade;
        self
    }

    /// Check whether a redirect from `from` to `to` is permitted by this
    /// policy's downgrade rule.
    ///
    /// Returns `Ok(())` when the hop is allowed. Returns
    /// [`Error::InvalidRedirectLocation`] when the hop is an HTTPS -> HTTP
    /// downgrade and the policy is [`RedirectDowngradePolicy::Deny`].
    /// Scheme allow-listing (`http`/`https` only) and userinfo rejection
    /// are enforced separately by [`validate_redirect_url`]; this method
    /// only enforces the downgrade boundary.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidRedirectLocation`] on a denied downgrade.
    pub fn check_downgrade(&self, from: &Url, to: &Url) -> Result<()> {
        check_https_downgrade(from, to, self.downgrade)
    }
}

/// Returns `true` when a redirect moves from `https` to `http`.
///
/// Both URLs must already be resolved (relative and scheme-relative
/// `Location` values resolved against the originating URL), so this is a
/// plain scheme comparison.
#[must_use]
pub fn is_https_downgrade(from: &Url, to: &Url) -> bool {
    from.scheme() == "https" && to.scheme() == "http"
}

/// Enforce a downgrade policy for a resolved redirect hop.
///
/// Returns `Ok(())` when the hop is allowed. When `policy` is
/// [`RedirectDowngradePolicy::Deny`] and `to` is an `http` downgrade of an
/// `https` origin, returns [`Error::InvalidRedirectLocation`] before any
/// request is dispatched to `to`.
///
/// # Errors
///
/// Returns [`Error::InvalidRedirectLocation`] on a denied downgrade.
pub fn check_https_downgrade(from: &Url, to: &Url, policy: RedirectDowngradePolicy) -> Result<()> {
    if policy == RedirectDowngradePolicy::Deny && is_https_downgrade(from, to) {
        return Err(Error::InvalidRedirectLocation(
            "redirect downgrade from https to http is not allowed".into(),
        ));
    }
    Ok(())
}

/// Headers that should be removed when the body is dropped (e.g., on
/// POST→GET rewrite).
const BODY_HEADERS: &[&str] = &["content-length", "content-type", "transfer-encoding"];

/// Check if a status code indicates a redirect that should be followed.
#[must_use]
pub fn is_redirect_status(status: http::StatusCode) -> bool {
    matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308)
}

/// Determine the method to use for a redirect request.
///
/// Rules:
/// - 303 (See Other): all non-HEAD methods become GET, body dropped
/// - 301/302: POST becomes GET (body dropped); other methods preserved
/// - 307/308: method and body preserved
#[must_use]
pub fn redirect_method(status: http::StatusCode, current_method: &Method) -> Method {
    match status.as_u16() {
        303 => {
            if current_method == Method::HEAD {
                Method::HEAD
            } else {
                Method::GET
            }
        }
        301 | 302 => {
            if current_method == Method::POST {
                Method::GET
            } else {
                current_method.clone()
            }
        }
        _ => current_method.clone(),
    }
}

/// Returns `true` if the redirect drops the request payload.
///
/// Rules (RFC 9110 §15.4.3/§15.4.4):
/// - 303: the method is rewritten to GET (except HEAD) and the payload
///   plus content-related headers are removed for **all** methods.
/// - 301/302: POST→GET rewrites drop the body; other methods preserved.
#[must_use]
pub fn drops_body_on_redirect(status: http::StatusCode, current_method: &Method) -> bool {
    if status.as_u16() == 303 {
        return current_method != Method::HEAD;
    }
    let new_method = redirect_method(status, current_method);
    current_method == Method::POST && new_method == Method::GET
}

/// Build a follow-up request from a redirect response and the original
/// request.
///
/// Applies method rewrite rules, strips sensitive headers on cross-origin
/// redirects, removes body-specific headers when the body is dropped,
/// and returns the new request with an empty body.
///
/// This compatibility entry point preserves the historical behavior of
/// allowing HTTPS -> HTTP downgrades. Callers that need downgrade
/// rejection should use
/// [`build_redirect_request_with_redirect_policy`] with a
/// [`RedirectPolicy`] whose [`downgrade`](RedirectPolicy::downgrade) is
/// [`RedirectDowngradePolicy::Deny`]; the redirect pipeline enforces the
/// client policy automatically.
///
/// # Errors
///
/// Returns an error if:
/// - The redirect location is missing or invalid
/// - The redirect URL uses an unsupported scheme
/// - The body needs to be resent but is not replayable
pub fn build_redirect_request(
    original: &Request,
    status: http::StatusCode,
    location: &str,
) -> Result<Request> {
    let new_method = redirect_method(status, original.method());
    let drop_body = drops_body_on_redirect(status, original.method());
    build_redirect_request_with_method(
        original,
        location,
        new_method,
        drop_body,
        RedirectDowngradePolicy::Allow,
    )
}

/// Build a follow-up request while enforcing a [`RedirectPolicy`]'s
/// downgrade rule.
///
/// Behaves like [`build_redirect_request`], except an HTTPS -> HTTP
/// downgrade is rejected before the returned request could be dispatched
/// when `policy.downgrade` is [`RedirectDowngradePolicy::Deny`].
///
/// # Errors
///
/// Same as [`build_redirect_request`], plus
/// [`Error::InvalidRedirectLocation`] on a denied downgrade.
pub fn build_redirect_request_with_redirect_policy(
    original: &Request,
    status: http::StatusCode,
    location: &str,
    policy: &RedirectPolicy,
) -> Result<Request> {
    let new_method = redirect_method(status, original.method());
    let drop_body = drops_body_on_redirect(status, original.method());
    build_redirect_request_with_method(original, location, new_method, drop_body, policy.downgrade)
}

/// Build a follow-up request using a precomputed redirect method and
/// body-dropping decision.
///
/// Internal fast path for the pipeline, which already evaluated
/// [`redirect_method`] and [`drops_body_on_redirect`] for the hop. The
/// `downgrade` policy is enforced after URL resolution and before the
/// caller can dispatch the next hop.
///
/// # Errors
///
/// Same as [`build_redirect_request`], plus
/// [`Error::InvalidRedirectLocation`] on a denied downgrade.
pub(crate) fn build_redirect_request_with_policy(
    original: &Request,
    location: &str,
    new_method: Method,
    drop_body: bool,
    downgrade: RedirectDowngradePolicy,
) -> Result<Request> {
    build_redirect_request_with_method(original, location, new_method, drop_body, downgrade)
}

/// Shared redirect-request constructor.
///
/// Kept private so the public surface stays at the two entry points
/// above plus the pipeline path. `downgrade` is enforced after scheme
/// validation and before headers/body are finalized.
///
/// # Errors
///
/// Same as [`build_redirect_request`], plus
/// [`Error::InvalidRedirectLocation`] on a denied downgrade.
fn build_redirect_request_with_method(
    original: &Request,
    location: &str,
    new_method: Method,
    drop_body: bool,
    downgrade: RedirectDowngradePolicy,
) -> Result<Request> {
    let new_url = resolve_redirect_url(original.url(), location)?;

    validate_redirect_url(&new_url)?;

    check_https_downgrade(original.url(), &new_url, downgrade)?;

    // Build new headers.
    let mut new_headers = original.headers().clone();
    strip_headers_for_redirect(&mut new_headers, original.url(), &new_url, drop_body);

    // Handle body for the redirect.
    let new_body = if drop_body {
        RequestBody::Empty
    } else {
        original.body().try_clone_for_redirect()?
    };

    let mut redirect_request = Request::new(new_method, new_url);
    *redirect_request.headers_mut() = new_headers;
    redirect_request.set_body(new_body);
    // Deliberately do not carry the original request's wire version:
    // the redirect target may be a different host whose negotiated
    // protocol differs (matching httpx, which rebuilds the request).
    // ALPN governs TLS paths and the version policy lives on the
    // client config.

    Ok(redirect_request)
}

/// Resolve a redirect Location against the original URL.
///
/// Supports absolute URLs, relative paths, and scheme-relative URLs.
fn resolve_redirect_url(base: &Url, location: &str) -> Result<Url> {
    base.join(location)
        .map_err(|e| Error::InvalidRedirectLocation(format!("{e}")))
}

/// Validate that a redirect URL uses an allowed scheme.
fn validate_redirect_url(url: &Url) -> Result<()> {
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Error::InvalidRedirectLocation(
            "redirect URL userinfo is not supported".into(),
        ));
    }
    match url.scheme() {
        "http" | "https" => Ok(()),
        other => Err(Error::Unsupported(format!(
            "redirect to unsupported scheme '{other}': only http and https are allowed"
        ))),
    }
}

/// Strip sensitive and body-specific headers for a redirect request.
fn strip_headers_for_redirect(
    headers: &mut Headers,
    original_url: &Url,
    new_url: &Url,
    drop_body: bool,
) {
    // Always strip `authorization` and `proxy-authorization`. The pipeline
    // re-applies configured auth via `resolve_request_auth`, and leaving
    // the prior hop's header in place would trigger `ConflictingAuth` on
    // same-origin redirects that inherit the header. Stripping here is
    // safe because the pipeline decides whether to re-attach credentials
    // based on the `credentials_allowed` flag for the hop.
    for name in ["authorization", "proxy-authorization"] {
        headers.remove(name);
    }

    // Strip remaining sensitive headers on cross-origin redirects.
    // `authorization` and `proxy-authorization` were already stripped above,
    // so only the cross-origin-specific entries remain here.
    let cross_origin = original_url.origin() != new_url.origin();
    if cross_origin {
        for name in ["cookie", "host"] {
            headers.remove(name);
        }
    }

    // Remove body-specific headers when body is dropped.
    if drop_body {
        for name in BODY_HEADERS {
            headers.remove(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use http::StatusCode;
    use proptest::prelude::*;

    #[test]
    fn redirect_method_303_get() {
        assert_eq!(
            redirect_method(StatusCode::SEE_OTHER, &Method::GET),
            Method::GET
        );
    }

    #[test]
    fn redirect_method_303_post() {
        assert_eq!(
            redirect_method(StatusCode::SEE_OTHER, &Method::POST),
            Method::GET
        );
    }

    #[test]
    fn redirect_method_303_head() {
        assert_eq!(
            redirect_method(StatusCode::SEE_OTHER, &Method::HEAD),
            Method::HEAD
        );
    }

    #[test]
    fn redirect_method_301_post() {
        assert_eq!(
            redirect_method(StatusCode::MOVED_PERMANENTLY, &Method::POST),
            Method::GET
        );
    }

    #[test]
    fn redirect_method_302_post() {
        assert_eq!(
            redirect_method(StatusCode::FOUND, &Method::POST),
            Method::GET
        );
    }

    #[test]
    fn redirect_method_301_get() {
        assert_eq!(
            redirect_method(StatusCode::MOVED_PERMANENTLY, &Method::GET),
            Method::GET
        );
    }

    #[test]
    fn redirect_method_307_post() {
        assert_eq!(
            redirect_method(StatusCode::TEMPORARY_REDIRECT, &Method::POST),
            Method::POST
        );
    }

    #[test]
    fn redirect_method_308_post() {
        assert_eq!(
            redirect_method(StatusCode::PERMANENT_REDIRECT, &Method::POST),
            Method::POST
        );
    }

    #[test]
    fn redirect_method_307_preserves_put() {
        assert_eq!(
            redirect_method(StatusCode::TEMPORARY_REDIRECT, &Method::PUT),
            Method::PUT
        );
    }

    #[test]
    fn is_redirect_status_codes() {
        assert!(is_redirect_status(StatusCode::MOVED_PERMANENTLY));
        assert!(is_redirect_status(StatusCode::FOUND));
        assert!(is_redirect_status(StatusCode::SEE_OTHER));
        assert!(is_redirect_status(StatusCode::TEMPORARY_REDIRECT));
        assert!(is_redirect_status(StatusCode::PERMANENT_REDIRECT));
        assert!(!is_redirect_status(StatusCode::OK));
        assert!(!is_redirect_status(StatusCode::NOT_FOUND));
    }

    #[test]
    fn drops_body_on_301_post() {
        assert!(drops_body_on_redirect(
            StatusCode::MOVED_PERMANENTLY,
            &Method::POST
        ));
    }

    #[test]
    fn drops_body_on_303_post() {
        assert!(drops_body_on_redirect(StatusCode::SEE_OTHER, &Method::POST));
    }

    #[test]
    fn preserves_body_on_307_post() {
        assert!(!drops_body_on_redirect(
            StatusCode::TEMPORARY_REDIRECT,
            &Method::POST
        ));
    }

    #[test]
    fn drops_body_on_303_put() {
        // RFC 9110 §15.4.4: 303 removes the payload for all methods.
        assert!(drops_body_on_redirect(StatusCode::SEE_OTHER, &Method::PUT));
        assert!(drops_body_on_redirect(
            StatusCode::SEE_OTHER,
            &Method::DELETE
        ));
    }

    #[test]
    fn build_redirect_303_put_drops_payload_and_content_headers() {
        let mut req = Request::new(Method::PUT, Url::parse("https://example.com/a").unwrap());
        req.set_body(RequestBody::from(Bytes::from("payload")));
        req.headers_mut().insert("content-length", "7").unwrap();
        req.headers_mut()
            .insert("content-type", "text/plain")
            .unwrap();

        let redirect =
            build_redirect_request(&req, StatusCode::SEE_OTHER, "https://example.com/b").unwrap();

        assert_eq!(*redirect.method(), Method::GET);
        assert!(redirect.body().is_empty());
        assert!(redirect.headers().get("content-length").is_none());
        assert!(redirect.headers().get("content-type").is_none());
    }

    #[test]
    fn build_redirect_cross_origin_strips_host() {
        let mut req = Request::new(Method::GET, Url::parse("https://example.com/a").unwrap());
        req.headers_mut().insert("host", "internal.corp").unwrap();

        let same_origin =
            build_redirect_request(&req, StatusCode::FOUND, "https://example.com/b").unwrap();
        assert!(same_origin.headers().get("host").is_some());

        let cross_origin =
            build_redirect_request(&req, StatusCode::FOUND, "https://other.com/b").unwrap();
        assert!(cross_origin.headers().get("host").is_none());
    }

    #[test]
    fn preserves_body_on_302_get() {
        assert!(!drops_body_on_redirect(StatusCode::FOUND, &Method::GET));
    }

    // --- build_redirect_request tests ---

    #[test]
    fn build_redirect_same_origin_strips_authorization() {
        // The pipeline re-applies configured auth via `resolve_request_auth`;
        // leaving the prior hop's `Authorization` in place would trigger
        // `ConflictingAuth` on a same-origin redirect that inherits the
        // header. `cookie` and `host` survive same-origin redirects so the
        // server sees the same session state.
        let mut req = Request::new(Method::GET, Url::parse("https://example.com/a").unwrap());
        req.headers_mut()
            .insert("authorization", "Bearer tok")
            .unwrap();
        req.headers_mut().insert("cookie", "session=abc").unwrap();

        let redirect =
            build_redirect_request(&req, StatusCode::FOUND, "https://example.com/b").unwrap();

        assert_eq!(*redirect.method(), Method::GET);
        assert_eq!(redirect.url().as_str(), "https://example.com/b");
        assert!(redirect.headers().get("authorization").is_none());
        assert!(redirect.headers().get("cookie").is_some());
    }

    #[test]
    fn build_redirect_cross_origin_strips_auth() {
        let mut req = Request::new(Method::GET, Url::parse("https://example.com/a").unwrap());
        req.headers_mut()
            .insert("authorization", "Bearer tok")
            .unwrap();
        req.headers_mut().insert("cookie", "session=abc").unwrap();
        req.headers_mut()
            .insert("proxy-authorization", "Basic foo")
            .unwrap();
        req.headers_mut().insert("x-custom", "keep").unwrap();

        let redirect =
            build_redirect_request(&req, StatusCode::FOUND, "https://other.com/b").unwrap();

        assert!(redirect.headers().get("authorization").is_none());
        assert!(redirect.headers().get("cookie").is_none());
        assert!(redirect.headers().get("proxy-authorization").is_none());
        assert!(redirect.headers().get("x-custom").is_some());
    }

    #[test]
    fn build_redirect_301_post_drops_body_headers() {
        let mut req = Request::new(Method::POST, Url::parse("https://example.com/a").unwrap());
        req.set_body(RequestBody::from(Bytes::from("payload")));
        req.headers_mut().insert("content-length", "7").unwrap();
        req.headers_mut()
            .insert("content-type", "text/plain")
            .unwrap();
        req.headers_mut()
            .insert("transfer-encoding", "chunked")
            .unwrap();

        let redirect =
            build_redirect_request(&req, StatusCode::MOVED_PERMANENTLY, "https://example.com/b")
                .unwrap();

        assert_eq!(*redirect.method(), Method::GET);
        assert!(redirect.body().is_empty());
        assert!(redirect.headers().get("content-length").is_none());
        assert!(redirect.headers().get("content-type").is_none());
        assert!(redirect.headers().get("transfer-encoding").is_none());
    }

    #[test]
    fn build_redirect_307_post_preserves_body_headers() {
        let mut req = Request::new(Method::POST, Url::parse("https://example.com/a").unwrap());
        req.set_body(RequestBody::from(Bytes::from("payload")));
        req.headers_mut().insert("content-length", "7").unwrap();
        req.headers_mut()
            .insert("content-type", "text/plain")
            .unwrap();

        let redirect = build_redirect_request(
            &req,
            StatusCode::TEMPORARY_REDIRECT,
            "https://example.com/b",
        )
        .unwrap();

        assert_eq!(*redirect.method(), Method::POST);
        match redirect.body() {
            RequestBody::Bytes(b) => assert_eq!(b, "payload"),
            _ => panic!("expected bytes body"),
        }
        assert!(redirect.headers().get("content-length").is_some());
        assert!(redirect.headers().get("content-type").is_some());
    }

    #[test]
    fn build_redirect_relative_url() {
        let req = Request::new(Method::GET, Url::parse("https://example.com/a/b").unwrap());

        let redirect = build_redirect_request(&req, StatusCode::FOUND, "/c/d").unwrap();

        assert_eq!(redirect.url().as_str(), "https://example.com/c/d");
    }

    #[test]
    fn build_redirect_scheme_relative_url() {
        let req = Request::new(Method::GET, Url::parse("https://example.com/a").unwrap());

        let redirect = build_redirect_request(&req, StatusCode::FOUND, "//other.com/b").unwrap();

        assert_eq!(redirect.url().as_str(), "https://other.com/b");
    }

    #[test]
    fn build_redirect_rejects_userinfo() {
        let req = Request::new(Method::GET, Url::parse("https://example.com/a").unwrap());
        let error =
            build_redirect_request(&req, StatusCode::FOUND, "https://user:pass@example.com/b")
                .unwrap_err();
        assert!(matches!(error, Error::InvalidRedirectLocation(_)));
    }

    #[test]
    fn build_redirect_unsupported_scheme_errors() {
        let req = Request::new(Method::GET, Url::parse("https://example.com/a").unwrap());

        let err = build_redirect_request(&req, StatusCode::FOUND, "ftp://example.com/b");
        assert!(err.is_err());
    }

    #[test]
    fn build_redirect_does_not_preserve_original_version() {
        // The redirect target may negotiate a different protocol than the
        // original host; the rebuilt request must not inherit the old
        // wire version.
        let mut req = Request::new(Method::GET, Url::parse("https://example.com/a").unwrap());
        req.set_version(http::Version::HTTP_2);

        let redirect =
            build_redirect_request(&req, StatusCode::FOUND, "https://example.com/b").unwrap();

        assert_ne!(redirect.version(), http::Version::HTTP_2);
        assert_eq!(redirect.version(), http::Version::HTTP_11);
    }

    #[test]
    fn build_redirect_empty_body_preserved() {
        let req = Request::new(Method::POST, Url::parse("https://example.com/a").unwrap());

        let redirect = build_redirect_request(
            &req,
            StatusCode::TEMPORARY_REDIRECT,
            "https://example.com/b",
        )
        .unwrap();

        assert_eq!(*redirect.method(), Method::POST);
        assert!(redirect.body().is_empty());
    }

    // --- RedirectPolicy tests ---

    #[test]
    fn redirect_policy_default() {
        let policy = RedirectPolicy::default();
        assert!(!policy.follow);
        assert_eq!(policy.max_redirects, 20);
        assert_eq!(policy.downgrade, RedirectDowngradePolicy::Allow);
    }

    #[test]
    fn redirect_policy_new() {
        let policy = RedirectPolicy::new(true, 5);
        assert!(policy.follow);
        assert_eq!(policy.max_redirects, 5);
        // Compatibility: historical behavior allows downgrades unless the
        // caller opts into strict mode.
        assert_eq!(policy.downgrade, RedirectDowngradePolicy::Allow);
    }

    #[test]
    fn redirect_policy_strict_denies_downgrade() {
        let policy = RedirectPolicy::strict(10);
        assert!(policy.follow);
        assert_eq!(policy.max_redirects, 10);
        assert_eq!(policy.downgrade, RedirectDowngradePolicy::Deny);
    }

    #[test]
    fn redirect_policy_with_downgrade() {
        let policy = RedirectPolicy::new(true, 5).with_downgrade(RedirectDowngradePolicy::Deny);
        assert_eq!(policy.downgrade, RedirectDowngradePolicy::Deny);
    }

    #[test]
    fn is_https_downgrade_detects_scheme_change() {
        let https = Url::parse("https://example.com/a").unwrap();
        let https_other = Url::parse("https://other.com/b").unwrap();
        let http = Url::parse("http://example.com/b").unwrap();
        let http_origin = Url::parse("http://example.com/a").unwrap();

        assert!(is_https_downgrade(&https, &http));
        assert!(!is_https_downgrade(&https, &https_other));
        assert!(!is_https_downgrade(&http_origin, &http));
        // Upgrades are never downgrades.
        assert!(!is_https_downgrade(&http_origin, &https));
    }

    #[test]
    fn strict_policy_allows_https_to_https() {
        let policy = RedirectPolicy::strict(5);
        let from = Url::parse("https://example.com/a").unwrap();
        let to = Url::parse("https://other.com/b").unwrap();
        assert!(policy.check_downgrade(&from, &to).is_ok());
    }

    #[test]
    fn strict_policy_allows_relative_https_equivalent() {
        // A relative Location resolves against the https origin, so the
        // resolved target stays https and must succeed under Deny.
        let policy = RedirectPolicy::strict(5);
        let req = Request::new(Method::GET, Url::parse("https://example.com/a").unwrap());
        let redirect =
            build_redirect_request_with_redirect_policy(&req, StatusCode::FOUND, "/b/c", &policy)
                .expect("relative redirect from https must succeed under Deny");
        assert_eq!(redirect.url().as_str(), "https://example.com/b/c");
    }

    #[test]
    fn strict_policy_allows_scheme_relative_https() {
        let policy = RedirectPolicy::strict(5);
        let req = Request::new(Method::GET, Url::parse("https://example.com/a").unwrap());
        let redirect = build_redirect_request_with_redirect_policy(
            &req,
            StatusCode::FOUND,
            "//other.com/b",
            &policy,
        )
        .expect("scheme-relative redirect from https resolves to https");
        assert_eq!(redirect.url().as_str(), "https://other.com/b");
    }

    #[test]
    fn strict_policy_rejects_https_to_http() {
        let policy = RedirectPolicy::strict(5);
        let from = Url::parse("https://example.com/a").unwrap();
        let to = Url::parse("http://example.com/b").unwrap();
        let err = policy.check_downgrade(&from, &to).unwrap_err();
        assert!(matches!(err, Error::InvalidRedirectLocation(_)));
        assert_eq!(err.kind(), "invalid_redirect_location");
    }

    #[test]
    fn compat_policy_allows_https_to_http() {
        // Default/compat behavior is unchanged when strict mode is not
        // selected: downgrades still build.
        let policy = RedirectPolicy::new(true, 5);
        let req = Request::new(Method::GET, Url::parse("https://example.com/a").unwrap());
        let redirect = build_redirect_request_with_redirect_policy(
            &req,
            StatusCode::FOUND,
            "http://example.com/b",
            &policy,
        )
        .expect("compat policy must allow downgrade");
        assert_eq!(redirect.url().scheme(), "http");
    }

    #[test]
    fn compat_build_redirect_request_allows_downgrade() {
        // The historical entry point stays compatibility-preserving.
        let req = Request::new(Method::GET, Url::parse("https://example.com/a").unwrap());
        let redirect = build_redirect_request(&req, StatusCode::FOUND, "http://example.com/b")
            .expect("compat entry point allows downgrade");
        assert_eq!(redirect.url().scheme(), "http");
    }

    #[test]
    fn strict_build_redirect_rejects_downgrade_before_dispatch() {
        // The builder returns Err without producing a dispatchable
        // request: there is no next-hop request object to send, so no
        // second-hop I/O can occur.
        let policy = RedirectPolicy::strict(5);
        let req = Request::new(Method::GET, Url::parse("https://example.com/a").unwrap());
        let err = build_redirect_request_with_redirect_policy(
            &req,
            StatusCode::FOUND,
            "http://example.com/b",
            &policy,
        )
        .unwrap_err();
        assert!(matches!(err, Error::InvalidRedirectLocation(_)));
    }

    #[test]
    fn strict_multi_hop_chain_rejects_on_downgrade_hop() {
        // Simulate a multi-hop HTTPS chain followed by a downgrade: the
        // first hops succeed, the downgrade hop fails.
        let policy = RedirectPolicy::strict(5);
        let first = Request::new(Method::GET, Url::parse("https://a.example/1").unwrap());
        let second = build_redirect_request_with_redirect_policy(
            &first,
            StatusCode::FOUND,
            "https://b.example/2",
            &policy,
        )
        .expect("https -> https hop succeeds");
        assert_eq!(second.url().as_str(), "https://b.example/2");

        let err = build_redirect_request_with_redirect_policy(
            &second,
            StatusCode::FOUND,
            "http://c.example/3",
            &policy,
        )
        .unwrap_err();
        assert!(matches!(err, Error::InvalidRedirectLocation(_)));
    }

    #[test]
    fn strict_cross_origin_https_preserves_header_stripping() {
        // Sensitive-header stripping is unchanged for allowed
        // cross-origin redirects under strict mode.
        let policy = RedirectPolicy::strict(5);
        let mut req = Request::new(Method::GET, Url::parse("https://example.com/a").unwrap());
        req.headers_mut()
            .insert("authorization", "Bearer tok")
            .unwrap();
        req.headers_mut().insert("cookie", "session=abc").unwrap();
        req.headers_mut().insert("x-custom", "keep").unwrap();

        let redirect = build_redirect_request_with_redirect_policy(
            &req,
            StatusCode::FOUND,
            "https://other.com/b",
            &policy,
        )
        .expect("allowed cross-origin https redirect succeeds");

        assert!(redirect.headers().get("authorization").is_none());
        assert!(redirect.headers().get("cookie").is_none());
        assert!(redirect.headers().get("x-custom").is_some());
    }

    proptest::proptest! {
        #[test]
        fn redirect_method_preserves_non_post_on_301_302(method in "[A-Z]{3,7}") {
            use http::Method;
            let m = Method::from_bytes(method.as_bytes());
            if let Ok(m) = m {
                if m != Method::POST {
                    prop_assert_eq!(redirect_method(StatusCode::MOVED_PERMANENTLY, &m), m.clone());
                    prop_assert_eq!(redirect_method(StatusCode::FOUND, &m), m);
                }
            }
        }

        #[test]
        #[allow(clippy::if_not_else)]
        fn redirect_method_303_always_get(method in "[A-Z]{3,7}") {
            use http::Method;
            let m = Method::from_bytes(method.as_bytes());
            if let Ok(m) = m {
                if m != Method::HEAD {
                    prop_assert_eq!(redirect_method(StatusCode::SEE_OTHER, &m), Method::GET);
                } else {
                    prop_assert_eq!(redirect_method(StatusCode::SEE_OTHER, &m), Method::HEAD);
                }
            }
        }

        #[test]
        fn drops_body_consistent_with_redirect_method(status in "[3][012378][1234567890]", method in "[A-Z]{3,7}") {
            use http::Method;
            let s = StatusCode::from_bytes(status.as_bytes());
            let m = Method::from_bytes(method.as_bytes());
            if let (Ok(s), Ok(m)) = (s, m) {
                let new_method = redirect_method(s, &m);
                let drops = drops_body_on_redirect(s, &m);
                let expected = if s.as_u16() == 303 {
                    m != Method::HEAD
                } else {
                    m == Method::POST && new_method == Method::GET
                };
                prop_assert_eq!(drops, expected);
            }
        }

        #[test]
        fn is_redirect_status_only_matches_3xx(status in 100u16..600) {
            let s = StatusCode::from_u16(status);
            if let Ok(s) = s {
                let is_redirect = is_redirect_status(s);
                let expected = matches!(status, 301 | 302 | 303 | 307 | 308);
                prop_assert_eq!(is_redirect, expected);
            }
        }
    }
}
