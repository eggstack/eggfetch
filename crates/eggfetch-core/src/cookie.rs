//! Cookie subsystem: parsing, matching, and jar management.
//!
//! This module implements an HTTP-client cookie jar following RFC 6265
//! semantics. It handles `Set-Cookie` parsing, domain/path/secure matching,
//! cookie serialization for requests, and thread-safe storage.
//!
//! The [`CookieJar`] is the central type: it stores cookies and answers
//! queries about which cookies should be sent for a given URL.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use url::Url;

/// Global monotonic counter for cookie creation ordering.
static CREATION_COUNTER: AtomicU64 = AtomicU64::new(1);

/// The `SameSite` cookie attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SameSite {
    /// The "Strict" `SameSite` attribute.
    Strict,
    /// The "Lax" `SameSite` attribute.
    Lax,
    /// The "None" `SameSite` attribute.
    None,
}

impl From<cookie::SameSite> for SameSite {
    fn from(s: cookie::SameSite) -> Self {
        match s {
            cookie::SameSite::Strict => Self::Strict,
            cookie::SameSite::Lax => Self::Lax,
            cookie::SameSite::None => Self::None,
        }
    }
}

/// A stored HTTP cookie with all relevant attributes.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cookie {
    /// Cookie name.
    pub(crate) name: String,
    /// Cookie value.
    pub(crate) value: String,
    /// Effective domain (normalized, no leading dot).
    pub(crate) domain: String,
    /// Whether this cookie is restricted to the exact host that set it.
    pub(crate) host_only: bool,
    /// Cookie path.
    pub(crate) path: String,
    /// Whether the cookie requires a secure connection.
    pub(crate) secure: bool,
    /// Whether the cookie is inaccessible to client-side scripts.
    pub(crate) http_only: bool,
    /// The `SameSite` attribute, if present.
    pub(crate) same_site: Option<SameSite>,
    /// Absolute expiry time, if the cookie is persistent.
    pub(crate) expires: Option<SystemTime>,
    /// Whether the cookie has an explicit expiry (persistent cookie).
    pub(crate) persistent: bool,
    /// Monotonic creation index for ordering among same-name cookies.
    pub(crate) creation_index: u64,
}

impl Cookie {
    /// Returns the cookie name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the cookie value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the cookie domain.
    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Returns whether this cookie is host-only.
    #[must_use]
    pub fn is_host_only(&self) -> bool {
        self.host_only
    }

    /// Returns the cookie path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns whether this cookie requires a secure connection.
    #[must_use]
    pub fn is_secure(&self) -> bool {
        self.secure
    }

    /// Returns whether this cookie is HTTP-only.
    #[must_use]
    pub fn is_http_only(&self) -> bool {
        self.http_only
    }

    /// Returns the `SameSite` attribute, if present.
    #[must_use]
    pub fn same_site(&self) -> Option<SameSite> {
        self.same_site
    }

    /// Returns the expiry time, if the cookie is persistent.
    #[must_use]
    pub fn expires(&self) -> Option<SystemTime> {
        self.expires
    }

    /// Returns whether the cookie is persistent (has an explicit expiry).
    #[must_use]
    pub fn is_persistent(&self) -> bool {
        self.persistent
    }

    /// Returns the creation index.
    #[must_use]
    pub fn creation_index(&self) -> u64 {
        self.creation_index
    }

    /// Returns the name-value pair formatted for a `Cookie` header.
    #[must_use]
    pub fn name_value_pair(&self) -> String {
        format!("{}={}", self.name, self.value)
    }
}

/// Replacement key for stored cookies: `name + domain + path`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CookieKey {
    name: String,
    domain: String,
    path: String,
}

impl CookieKey {
    fn new(name: &str, domain: &str, path: &str) -> Self {
        Self {
            name: name.to_lowercase(),
            domain: domain.to_lowercase(),
            path: path.to_owned(),
        }
    }
}

/// Inner state of the cookie jar, protected by a read-write lock.
#[derive(Debug)]
struct JarInner {
    cookies: HashMap<CookieKey, Cookie>,
}

/// A thread-safe cookie jar.
///
/// The jar stores cookies and answers queries about which cookies should
/// be sent for a given URL. It is safe to share across threads via `Clone`
/// (the inner state is `Arc`-wrapped).
#[derive(Debug, Clone)]
pub struct CookieJar {
    inner: Arc<RwLock<JarInner>>,
}

impl Default for CookieJar {
    fn default() -> Self {
        Self::new()
    }
}

impl CookieJar {
    /// Create a new, empty cookie jar.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(JarInner {
                cookies: HashMap::new(),
            })),
        }
    }

    /// Create a cookie jar pre-loaded with a single cookie.
    #[must_use]
    pub fn with_initial_cookie(cookie: Cookie) -> Self {
        let jar = Self::new();
        jar.set(cookie);
        jar
    }

    /// Parse `Set-Cookie` headers and update the jar.
    ///
    /// `response_url` is the URL of the response that contained the headers.
    /// Each header value is parsed independently; they are never comma-split.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn update_from_response(&self, response_url: &Url, set_cookie_headers: &[String]) {
        let response_host = response_url.host_str().unwrap_or("");
        let is_secure_url = response_url.scheme() == "https";

        for header_value in set_cookie_headers {
            if let Some(cookie) =
                parse_set_cookie(header_value, response_url, response_host, is_secure_url)
            {
                if cookie.persistent {
                    if let Some(expires) = cookie.expires {
                        let now = SystemTime::now();
                        if now >= expires {
                            let key = CookieKey::new(&cookie.name, &cookie.domain, &cookie.path);
                            self.inner.write().unwrap().cookies.remove(&key);
                            continue;
                        }
                    }
                }

                let key = CookieKey::new(&cookie.name, &cookie.domain, &cookie.path);
                self.inner.write().unwrap().cookies.insert(key, cookie);
            }
        }
    }

    /// Get matching cookies for a request URL, serialized as a `Cookie` header.
    ///
    /// Returns `None` if no cookies match.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn cookies_for_url(&self, url: &Url) -> Option<String> {
        self.expire_stale();
        let jar = self.inner.read().unwrap();
        let url_host = url.host_str().unwrap_or("");
        let is_secure = url.scheme() == "https";
        let url_path = url.path();

        // Collect matching cookies.
        let mut matches: Vec<&Cookie> = jar
            .cookies
            .values()
            .filter(|c| cookie_matches(c, url_host, url_path, is_secure))
            .collect();

        if matches.is_empty() {
            return None;
        }

        // For same-name cookies, prefer longer paths, then earlier creation_index.
        matches.sort_by(|a, b| {
            if a.name == b.name {
                let path_cmp = b.path.len().cmp(&a.path.len());
                if path_cmp != std::cmp::Ordering::Equal {
                    return path_cmp;
                }
                a.creation_index.cmp(&b.creation_index)
            } else {
                a.name.cmp(&b.name)
            }
        });

        // Deduplicate by name: keep only the best match per name.
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for c in &matches {
            if seen.insert(c.name.clone()) {
                result.push(format!("{}={}", c.name, c.value));
            }
        }

        if result.is_empty() {
            None
        } else {
            Some(result.join("; "))
        }
    }

    /// Get all cookies in the jar.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn all_cookies(&self) -> Vec<Cookie> {
        self.expire_stale();
        let jar = self.inner.read().unwrap();
        jar.cookies.values().cloned().collect()
    }

    /// Get a specific cookie by name.
    ///
    /// If `domain` and `path` are provided, performs an exact lookup.
    /// If only `name` is provided, returns the first matching cookie
    /// (or `None` if ambiguous).
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn get(&self, name: &str, domain: Option<&str>, path: Option<&str>) -> Option<Cookie> {
        self.expire_stale();
        let jar = self.inner.read().unwrap();

        if let (Some(d), Some(p)) = (domain, path) {
            let key = CookieKey::new(name, d, p);
            return jar.cookies.get(&key).cloned();
        }

        // Without domain+path, find all cookies with this name.
        let matching: Vec<&Cookie> = jar
            .cookies
            .values()
            .filter(|c| c.name.eq_ignore_ascii_case(name))
            .collect();

        matching.into_iter().next().cloned()
    }

    /// Set a cookie explicitly.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn set(&self, cookie: Cookie) {
        let key = CookieKey::new(&cookie.name, &cookie.domain, &cookie.path);
        self.inner.write().unwrap().cookies.insert(key, cookie);
    }

    /// Set a "default" cookie that matches every URL.
    ///
    /// Default cookies have an empty domain and are not host-only, so
    /// they are sent on all requests regardless of the target host.
    /// This is used for pre-populated cookies from a dict (e.g.
    /// `Client(cookies={"name": "value"})`).
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn set_default_cookie(&self, name: String, value: String) {
        let cookie = Cookie {
            name,
            value,
            domain: String::new(),
            host_only: false,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: CREATION_COUNTER.fetch_add(1, Ordering::Relaxed),
        };
        let key = CookieKey::new(&cookie.name, &cookie.domain, &cookie.path);
        self.inner.write().unwrap().cookies.insert(key, cookie);
    }

    /// Delete a cookie by name, domain, and path.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn delete(&self, name: &str, domain: &str, path: &str) {
        let key = CookieKey::new(name, domain, path);
        self.inner.write().unwrap().cookies.remove(&key);
    }

    /// Remove all cookies from the jar.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn clear(&self) {
        self.inner.write().unwrap().cookies.clear();
    }

    /// Returns the number of cookies in the jar.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().unwrap().cookies.len()
    }

    /// Returns `true` if the jar is empty.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.read().unwrap().cookies.is_empty()
    }

    /// Expire cookies that have passed their expiry time.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn expire_stale(&self) {
        let now = SystemTime::now();
        let mut jar = self.inner.write().unwrap();
        jar.cookies
            .retain(|_, c| !c.persistent || c.expires.map_or(true, |exp| now < exp));
    }
}

/// Check whether a cookie matches a request.
fn cookie_matches(cookie: &Cookie, url_host: &str, url_path: &str, is_secure: bool) -> bool {
    // Secure cookies only sent over HTTPS.
    if cookie.secure && !is_secure {
        return false;
    }

    // Default cookies (empty domain, not host-only) match every URL.
    if !cookie.host_only && cookie.domain.is_empty() {
        return path_matches(url_path, &cookie.path);
    }

    // Domain matching.
    if cookie.host_only {
        if !url_host.eq_ignore_ascii_case(&cookie.domain) {
            return false;
        }
    } else if !domain_matches(url_host, &cookie.domain) {
        return false;
    }

    // Path matching.
    path_matches(url_path, &cookie.path)
}

/// Check if `request_path` matches the cookie's `cookie_path` per RFC 6265.
///
/// A cookie's path matches if the request path starts with the cookie path.
/// The "/" path matches everything. Both strings must start with "/".
fn path_matches(request_path: &str, cookie_path: &str) -> bool {
    if !request_path.starts_with('/') || !cookie_path.starts_with('/') {
        return false;
    }

    if request_path == cookie_path {
        return true;
    }

    // The cookie path must be a prefix of the request path.
    if !request_path.starts_with(cookie_path) {
        return false;
    }

    // If the cookie path doesn't end with "/", the next char in request_path must be "/".
    if cookie_path.ends_with('/') {
        true
    } else {
        request_path.as_bytes().get(cookie_path.len()) == Some(&b'/')
    }
}

/// Check if `host` domain-matches `domain` per RFC 6265.
///
/// A host domain-matches a domain if:
/// - The host is an exact match, OR
/// - The host is a suffix of the domain with a leading "." (i.e., is a subdomain).
///
/// For our purposes: `host` matches `domain` if `host` equals `domain`
/// or `host` ends with `.{domain}` (case-insensitive).
fn domain_matches(host: &str, domain: &str) -> bool {
    if host.eq_ignore_ascii_case(domain) {
        return true;
    }
    let suffix = format!(".{domain}");
    (host.len() > suffix.len() && host.to_lowercase().ends_with(&suffix.to_lowercase()))
        || (host.len() == suffix.len() && host.eq_ignore_ascii_case(&suffix))
}

/// Determine if a string is an IP address (v4 or v6).
fn is_ip_address(s: &str) -> bool {
    s.parse::<std::net::IpAddr>().is_ok()
}

/// Validate a cookie name per RFC 6265.
///
/// Cookie names must not be empty and must consist of valid token characters.
/// We reject names containing control characters, spaces, tabs, or separators.
fn is_valid_cookie_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    !name
        .bytes()
        .any(|b| b <= 0x20 || b == 0x7f || b == b'"' || b == b',' || b == b';' || b == b'\\')
}

/// Compute the default cookie path from the response URL path per RFC 6265.
///
/// The default path is computed by:
/// 1. If the path is empty or doesn't start with "/", use "/".
/// 2. If the path contains no more than one "/", use "/".
/// 3. Otherwise, output the path from the first character to the character
///    before the last "/".
fn default_cookie_path(url_path: &str) -> String {
    if url_path.is_empty() || !url_path.starts_with('/') {
        return "/".to_owned();
    }
    // Count slashes to check if there's more than one.
    let slash_count = url_path.bytes().filter(|&b| b == b'/').count();
    if slash_count <= 1 {
        return "/".to_owned();
    }
    // Find the last "/" and return everything before it.
    if let Some(last_pos) = url_path.rfind('/') {
        let path = &url_path[..last_pos];
        if path.is_empty() {
            "/".to_owned()
        } else {
            path.to_owned()
        }
    } else {
        "/".to_owned()
    }
}

/// Parse a single `Set-Cookie` header value into a `Cookie`.
///
/// Returns `None` if the header is invalid or should be rejected.
fn parse_set_cookie(
    header_value: &str,
    response_url: &Url,
    response_host: &str,
    is_secure_url: bool,
) -> Option<Cookie> {
    let parsed = cookie::Cookie::parse_encoded(header_value).ok()?;

    let name = parsed.name().to_owned();
    let value = parsed.value().to_owned();

    if !is_valid_cookie_name(&name) {
        return None;
    }

    // Determine domain and host-only.
    let (domain, host_only) = match parsed.domain() {
        Some(raw_domain) => {
            // Strip leading dot(s).
            let normalized = raw_domain.trim_start_matches('.').to_owned();

            if normalized.is_empty() {
                return None;
            }

            // Reject IP addresses with explicit Domain attribute.
            if is_ip_address(&normalized) {
                return None;
            }

            // Validate: the setting host must domain-match the Domain attribute.
            if !domain_matches(response_host, &normalized) {
                return None;
            }

            (normalized, false)
        }
        None => {
            // No Domain attribute: host-only cookie for the response host.
            (response_host.to_owned(), true)
        }
    };

    // Determine path.
    let path = match parsed.path() {
        Some(p) if p.starts_with('/') => p.to_owned(),
        _ => default_cookie_path(response_url.path()),
    };

    let secure = parsed.secure().unwrap_or(false);
    let http_only = parsed.http_only().unwrap_or(false);
    let same_site = parsed.same_site().map(SameSite::from);

    // Max-Age and Expires handling.
    let (persistent, expires) = resolve_expiration(&parsed, is_secure_url);

    Some(Cookie {
        name,
        value,
        domain,
        host_only,
        path,
        secure,
        http_only,
        same_site,
        expires,
        persistent,
        creation_index: CREATION_COUNTER.fetch_add(1, Ordering::Relaxed),
    })
}

/// Resolve cookie expiration from Max-Age and Expires attributes.
///
/// Max-Age takes precedence over Expires.
/// Max-Age <= 0 means the cookie should be deleted (returns expiry in the past).
fn resolve_expiration(
    parsed: &cookie::Cookie<'_>,
    _is_secure_url: bool,
) -> (bool, Option<SystemTime>) {
    if let Some(max_age) = parsed.max_age() {
        let duration_secs = max_age.whole_seconds();

        if max_age.is_negative() || duration_secs == 0 {
            // Max-Age <= 0: mark for deletion by setting expiry to UNIX epoch.
            return (true, Some(SystemTime::UNIX_EPOCH));
        }

        // Positive Max-Age: compute expiry from now.
        // Safety: duration_secs is positive at this point (checked above).
        #[allow(clippy::cast_sign_loss)]
        let secs = duration_secs as u64;
        let now = SystemTime::now();
        let expiry = now + std::time::Duration::from_secs(secs);
        return (true, Some(expiry));
    }

    // No Max-Age: try Expires.
    if let Some(cookie::Expiration::DateTime(dt)) = parsed.expires() {
        let system_time = SystemTime::from(dt);
        return (true, Some(system_time));
    }

    // Session cookie.
    (false, None)
}

/// Parse multiple `Set-Cookie` header values and return valid cookies.
///
/// Each header value is parsed independently. Invalid headers are silently
/// skipped.
///
/// `response_url` is the URL of the response that contained the headers.
pub fn parse_set_cookie_headers(response_url: &Url, set_cookie_headers: &[String]) -> Vec<Cookie> {
    let response_host = response_url.host_str().unwrap_or("");
    let is_secure_url = response_url.scheme() == "https";

    set_cookie_headers
        .iter()
        .filter_map(|h| parse_set_cookie(h, response_url, response_host, is_secure_url))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn basic_cookie_parsing() {
        let url = make_url("http://example.com/path");
        let headers = vec!["session=abc123".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert_eq!(cookies.len(), 1);
        let c = &cookies[0];
        assert_eq!(c.name(), "session");
        assert_eq!(c.value(), "abc123");
        assert_eq!(c.domain(), "example.com");
        assert!(c.is_host_only());
        assert_eq!(c.path(), "/"); // default from "/path"
        assert!(!c.is_secure());
        assert!(!c.is_http_only());
        assert!(c.same_site().is_none());
        assert!(!c.is_persistent());
    }

    #[test]
    fn cookie_with_domain_attribute() {
        let url = make_url("http://sub.example.com/path");
        let headers = vec!["key=value; Domain=example.com".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert_eq!(cookies.len(), 1);
        let c = &cookies[0];
        assert_eq!(c.domain(), "example.com");
        assert!(!c.is_host_only());
    }

    #[test]
    fn cookie_with_leading_dot_domain() {
        let url = make_url("http://sub.example.com/path");
        let headers = vec!["key=value; Domain=.example.com".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].domain(), "example.com");
    }

    #[test]
    fn domain_mismatch_rejected() {
        let url = make_url("http://evil.com/path");
        let headers = vec!["key=value; Domain=example.com".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert!(cookies.is_empty());
    }

    #[test]
    fn ip_address_domain_rejected() {
        let url = make_url("http://192.168.1.1/path");
        let headers = vec!["key=value; Domain=192.168.1.1".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert!(cookies.is_empty());
    }

    #[test]
    fn host_only_cookie() {
        let url = make_url("http://example.com/path");
        let headers = vec!["key=value".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert_eq!(cookies.len(), 1);
        assert!(cookies[0].is_host_only());
        assert_eq!(cookies[0].domain(), "example.com");
    }

    #[test]
    fn path_matching_basic() {
        assert!(path_matches("/a/b/c", "/a/b"));
        assert!(path_matches("/a/b/c", "/a"));
        assert!(path_matches("/a/b/c", "/"));
        assert!(path_matches("/a/b/c", "/a/b/c"));
        assert!(!path_matches("/a/b", "/a/b/c"));
        assert!(!path_matches("/xyz", "/abc"));
    }

    #[test]
    fn path_matching_slash_prefix() {
        // "/a/b" should NOT match cookie path "/a/bc"
        assert!(!path_matches("/a/bc", "/a/b"));
    }

    #[test]
    fn path_matching_root() {
        assert!(path_matches("/", "/"));
        assert!(path_matches("/anything", "/"));
    }

    #[test]
    fn domain_matching() {
        assert!(domain_matches("example.com", "example.com"));
        assert!(domain_matches("sub.example.com", "example.com"));
        assert!(domain_matches("a.b.example.com", "example.com"));
        assert!(!domain_matches("notexample.com", "example.com"));
        assert!(!domain_matches("example.com", "sub.example.com"));
        assert!(!domain_matches("evil-example.com", "example.com"));
    }

    #[test]
    fn secure_only_cookie() {
        let url = make_url("https://example.com/path");
        let headers = vec!["key=secret; Secure".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert_eq!(cookies.len(), 1);
        assert!(cookies[0].is_secure());

        // Should match HTTPS.
        let jar = CookieJar::with_initial_cookie(cookies.into_iter().next().unwrap());
        assert!(jar
            .cookies_for_url(&make_url("https://example.com/"))
            .is_some());

        // Should NOT match HTTP.
        assert!(jar
            .cookies_for_url(&make_url("http://example.com/"))
            .is_none());
    }

    #[test]
    fn http_only_cookie() {
        let url = make_url("http://example.com/path");
        let headers = vec!["key=value; HttpOnly".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert_eq!(cookies.len(), 1);
        assert!(cookies[0].is_http_only());
    }

    #[test]
    fn same_site_strict() {
        let url = make_url("http://example.com/path");
        let headers = vec!["key=value; SameSite=Strict".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].same_site(), Some(SameSite::Strict));
    }

    #[test]
    fn same_site_lax() {
        let url = make_url("http://example.com/path");
        let headers = vec!["key=value; SameSite=Lax".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].same_site(), Some(SameSite::Lax));
    }

    #[test]
    fn same_site_none() {
        let url = make_url("https://example.com/path");
        let headers = vec!["key=value; SameSite=None; Secure".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].same_site(), Some(SameSite::None));
    }

    #[test]
    fn max_age_precedence_over_expires() {
        let url = make_url("http://example.com/path");
        // Max-Age=3600 should override Expires (which is in the past).
        let headers =
            vec!["key=value; Max-Age=3600; Expires=Thu, 01 Jan 1970 00:00:00 GMT".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert_eq!(cookies.len(), 1);
        let c = &cookies[0];
        assert!(c.is_persistent());
        // Should not be expired (Max-Age=3600 from now).
        if let Some(exp) = c.expires() {
            assert!(exp > SystemTime::now());
        } else {
            panic!("expected expiry");
        }
    }

    #[test]
    fn max_age_zero_deletes() {
        let url = make_url("http://example.com/path");
        // First set a cookie.
        let headers = vec!["key=value".to_owned()];
        let jar = CookieJar::new();
        jar.update_from_response(&url, &headers);
        assert_eq!(jar.len(), 1);

        // Now delete it with Max-Age=0.
        let headers = vec!["key=anything; Max-Age=0".to_owned()];
        jar.update_from_response(&url, &headers);
        assert_eq!(jar.len(), 0);
    }

    #[test]
    fn max_age_negative_deletes() {
        let url = make_url("http://example.com/path");
        let jar = CookieJar::new();
        let headers = vec!["key=value".to_owned()];
        jar.update_from_response(&url, &headers);
        assert_eq!(jar.len(), 1);

        let headers = vec!["key=anything; Max-Age=-1".to_owned()];
        jar.update_from_response(&url, &headers);
        assert_eq!(jar.len(), 0);
    }

    #[test]
    fn expired_cookie_not_stored() {
        let url = make_url("http://example.com/path");
        // Expires in the past.
        let headers = vec!["key=value; Expires=Thu, 01 Jan 1970 00:00:00 GMT".to_owned()];
        let jar = CookieJar::new();
        jar.update_from_response(&url, &headers);
        assert_eq!(jar.len(), 0);
    }

    #[test]
    fn default_path_derivation() {
        assert_eq!(default_cookie_path("/a/b/c"), "/a/b");
        assert_eq!(default_cookie_path("/a/b"), "/a");
        assert_eq!(default_cookie_path("/a"), "/");
        assert_eq!(default_cookie_path("/"), "/");
        assert_eq!(default_cookie_path(""), "/");
        assert_eq!(default_cookie_path("/a/"), "/a");
    }

    #[test]
    fn default_path_from_url() {
        let url = make_url("http://example.com/a/b/c");
        let headers = vec!["key=value".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].path(), "/a/b");
    }

    #[test]
    fn same_name_different_paths_ordering() {
        let jar = CookieJar::new();
        let url = make_url("http://example.com/a/b/c");

        // Set cookies with same name but different paths.
        let c1 = Cookie {
            name: "key".to_owned(),
            value: "short".to_owned(),
            domain: "example.com".to_owned(),
            host_only: true,
            path: "/a".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        };
        let c2 = Cookie {
            name: "key".to_owned(),
            value: "long".to_owned(),
            domain: "example.com".to_owned(),
            host_only: true,
            path: "/a/b".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 2,
        };
        jar.set(c1);
        jar.set(c2);

        let header = jar.cookies_for_url(&url).unwrap();
        // Longer path wins.
        assert_eq!(header, "key=long");
    }

    #[test]
    fn cookie_serialization_format() {
        let jar = CookieJar::new();
        let url = make_url("http://example.com/");

        jar.set(Cookie {
            name: "a".to_owned(),
            value: "1".to_owned(),
            domain: "example.com".to_owned(),
            host_only: true,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        });
        jar.set(Cookie {
            name: "b".to_owned(),
            value: "2".to_owned(),
            domain: "example.com".to_owned(),
            host_only: true,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 2,
        });

        let header = jar.cookies_for_url(&url).unwrap();
        // Sorted by name since paths are equal length.
        assert_eq!(header, "a=1; b=2");
    }

    #[test]
    fn invalid_cookie_name_rejected() {
        let url = make_url("http://example.com/path");
        // Cookie name with a semicolon is invalid.
        let headers = vec!["bad;name=value".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert!(cookies.is_empty());
    }

    #[test]
    fn invalid_empty_cookie_name_rejected() {
        let url = make_url("http://example.com/path");
        // Empty name should be rejected.
        let headers = vec!["=value".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert!(cookies.is_empty());
    }

    #[test]
    fn jar_get_with_domain_and_path() {
        let jar = CookieJar::new();
        jar.set(Cookie {
            name: "key".to_owned(),
            value: "val".to_owned(),
            domain: "example.com".to_owned(),
            host_only: false,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        });

        let c = jar.get("key", Some("example.com"), Some("/"));
        assert!(c.is_some());
        assert_eq!(c.unwrap().value(), "val");

        // Wrong domain.
        assert!(jar.get("key", Some("other.com"), Some("/")).is_none());
        // Wrong path.
        assert!(jar
            .get("key", Some("example.com"), Some("/other"))
            .is_none());
    }

    #[test]
    fn jar_delete() {
        let jar = CookieJar::new();
        jar.set(Cookie {
            name: "key".to_owned(),
            value: "val".to_owned(),
            domain: "example.com".to_owned(),
            host_only: false,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        });
        assert_eq!(jar.len(), 1);
        jar.delete("key", "example.com", "/");
        assert_eq!(jar.len(), 0);
    }

    #[test]
    fn jar_clear() {
        let jar = CookieJar::new();
        jar.set(Cookie {
            name: "a".to_owned(),
            value: "1".to_owned(),
            domain: "example.com".to_owned(),
            host_only: false,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        });
        jar.set(Cookie {
            name: "b".to_owned(),
            value: "2".to_owned(),
            domain: "example.com".to_owned(),
            host_only: false,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 2,
        });
        assert_eq!(jar.len(), 2);
        jar.clear();
        assert!(jar.is_empty());
    }

    #[test]
    fn jar_clone_shares_state() {
        let jar1 = CookieJar::new();
        jar1.set(Cookie {
            name: "key".to_owned(),
            value: "val".to_owned(),
            domain: "example.com".to_owned(),
            host_only: true,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        });
        let jar2 = jar1.clone();
        assert_eq!(jar2.len(), 1);
        jar2.clear();
        assert!(jar1.is_empty());
    }

    #[test]
    fn host_only_cookie_not_sent_cross_host() {
        let jar = CookieJar::new();
        jar.set(Cookie {
            name: "key".to_owned(),
            value: "val".to_owned(),
            domain: "a.example.com".to_owned(),
            host_only: true,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        });

        // Exact host matches.
        assert!(jar
            .cookies_for_url(&make_url("http://a.example.com/"))
            .is_some());
        // Different host does not match.
        assert!(jar
            .cookies_for_url(&make_url("http://b.example.com/"))
            .is_none());
    }

    #[test]
    fn domain_cookie_sent_to_subdomains() {
        let jar = CookieJar::new();
        jar.set(Cookie {
            name: "key".to_owned(),
            value: "val".to_owned(),
            domain: "example.com".to_owned(),
            host_only: false,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        });

        assert!(jar
            .cookies_for_url(&make_url("http://example.com/"))
            .is_some());
        assert!(jar
            .cookies_for_url(&make_url("http://sub.example.com/"))
            .is_some());
        assert!(jar
            .cookies_for_url(&make_url("http://a.b.example.com/"))
            .is_some());
        // Sibling domain does not match.
        assert!(jar
            .cookies_for_url(&make_url("http://notexample.com/"))
            .is_none());
    }

    #[test]
    fn cookie_replacement_same_identity() {
        let url = make_url("http://example.com/path");
        let jar = CookieJar::new();

        let headers1 = vec!["key=old".to_owned()];
        jar.update_from_response(&url, &headers1);
        assert_eq!(jar.len(), 1);

        let headers2 = vec!["key=new".to_owned()];
        jar.update_from_response(&url, &headers2);
        // Should replace, not add.
        assert_eq!(jar.len(), 1);
        let c = jar.get("key", Some("example.com"), Some("/")).unwrap();
        assert_eq!(c.value(), "new");
    }

    #[test]
    fn name_value_pair() {
        let c = Cookie {
            name: "foo".to_owned(),
            value: "bar".to_owned(),
            domain: "example.com".to_owned(),
            host_only: true,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        };
        assert_eq!(c.name_value_pair(), "foo=bar");
    }

    #[test]
    fn multiple_set_cookie_headers() {
        let url = make_url("http://example.com/path");
        let headers = vec![
            "a=1".to_owned(),
            "b=2; Path=/".to_owned(),
            "c=3; Domain=example.com; Secure".to_owned(),
        ];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert_eq!(cookies.len(), 3);
    }

    #[test]
    fn all_cookies_returns_vec() {
        let jar = CookieJar::new();
        jar.set(Cookie {
            name: "x".to_owned(),
            value: "1".to_owned(),
            domain: "a.com".to_owned(),
            host_only: true,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        });
        jar.set(Cookie {
            name: "y".to_owned(),
            value: "2".to_owned(),
            domain: "b.com".to_owned(),
            host_only: true,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 2,
        });
        let all = jar.all_cookies();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn cookies_for_url_none_when_empty() {
        let jar = CookieJar::new();
        assert!(jar
            .cookies_for_url(&make_url("http://example.com/"))
            .is_none());
    }

    #[test]
    fn cookie_with_explicit_path() {
        let url = make_url("http://example.com/a/b/c");
        let headers = vec!["key=value; Path=/a".to_owned()];
        let cookies = parse_set_cookie_headers(&url, &headers);
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].path(), "/a");
    }

    #[test]
    fn domain_case_insensitive() {
        let jar = CookieJar::new();
        jar.set(Cookie {
            name: "key".to_owned(),
            value: "val".to_owned(),
            domain: "Example.COM".to_owned(),
            host_only: false,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        });

        // Should match case-insensitively.
        assert!(jar
            .cookies_for_url(&make_url("http://example.com/"))
            .is_some());
    }

    #[test]
    fn is_ip_address_check() {
        assert!(is_ip_address("192.168.1.1"));
        assert!(is_ip_address("127.0.0.1"));
        assert!(is_ip_address("::1"));
        assert!(is_ip_address("2001:db8::1"));
        assert!(!is_ip_address("example.com"));
        assert!(!is_ip_address("not-an-ip"));
    }

    #[test]
    fn cookie_header_not_sent_when_no_match() {
        let jar = CookieJar::new();
        jar.set(Cookie {
            name: "key".to_owned(),
            value: "val".to_owned(),
            domain: "example.com".to_owned(),
            host_only: true,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        });

        // Different domain.
        assert!(jar
            .cookies_for_url(&make_url("http://other.com/"))
            .is_none());
    }

    #[test]
    fn persistent_cookie_expiry() {
        let url = make_url("http://example.com/path");
        let headers = vec!["key=value; Max-Age=-1".to_owned()];
        let jar = CookieJar::new();
        jar.update_from_response(&url, &headers);
        // Max-Age negative => sets expiry to UNIX epoch => expired.
        assert_eq!(jar.len(), 0);
    }

    #[test]
    fn with_initial_cookie() {
        let c = Cookie {
            name: "init".to_owned(),
            value: "val".to_owned(),
            domain: "example.com".to_owned(),
            host_only: true,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        };
        let jar = CookieJar::with_initial_cookie(c);
        assert_eq!(jar.len(), 1);
    }

    #[test]
    fn creation_index_monotonically_increases() {
        let url = make_url("http://example.com/path");
        let headers1 = vec!["a=1".to_owned()];
        let headers2 = vec!["b=2".to_owned()];
        let cookies1 = parse_set_cookie_headers(&url, &headers1);
        let cookies2 = parse_set_cookie_headers(&url, &headers2);
        assert!(cookies2[0].creation_index() > cookies1[0].creation_index());
    }

    #[test]
    fn same_name_different_creation_index_ordering() {
        let jar = CookieJar::new();
        let url = make_url("http://example.com/");

        jar.set(Cookie {
            name: "key".to_owned(),
            value: "second".to_owned(),
            domain: "example.com".to_owned(),
            host_only: true,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 2,
        });
        jar.set(Cookie {
            name: "key".to_owned(),
            value: "first".to_owned(),
            domain: "example.com".to_owned(),
            host_only: true,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        });

        // Both have same path length; earlier creation_index wins.
        let header = jar.cookies_for_url(&url).unwrap();
        assert_eq!(header, "key=first");
    }

    #[test]
    fn path_match_no_prefix_without_slash() {
        // "/abc" should NOT match cookie path "/ab"
        assert!(!path_matches("/abc", "/ab"));
        // But "/abc" should match "/a" if next char is "/"
        assert!(path_matches("/ab/c", "/ab"));
    }

    #[test]
    fn jar_len_and_is_empty() {
        let jar = CookieJar::new();
        assert!(jar.is_empty());
        assert_eq!(jar.len(), 0);

        jar.set(Cookie {
            name: "x".to_owned(),
            value: "1".to_owned(),
            domain: "example.com".to_owned(),
            host_only: true,
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires: None,
            persistent: false,
            creation_index: 1,
        });
        assert!(!jar.is_empty());
        assert_eq!(jar.len(), 1);
    }

    #[test]
    fn cookie_display_via_name_value_pair() {
        let c = Cookie {
            name: "session".to_owned(),
            value: "abc123".to_owned(),
            domain: "example.com".to_owned(),
            host_only: true,
            path: "/".to_owned(),
            secure: true,
            http_only: true,
            same_site: Some(SameSite::Lax),
            expires: None,
            persistent: false,
            creation_index: 1,
        };
        assert_eq!(c.name_value_pair(), "session=abc123");
        assert!(c.is_secure());
        assert!(c.is_http_only());
        assert_eq!(c.same_site(), Some(SameSite::Lax));
    }
}
