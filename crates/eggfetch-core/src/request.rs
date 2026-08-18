//! Request types and builder.

use std::sync::Arc;

use bytes::Bytes;
use http::Version;

use crate::body::RequestBody;
use crate::client::Client;
use crate::error::Result;
use crate::headers::Headers;
use crate::redirect::RedirectPolicy;
use crate::response::Response;
use crate::retry::RetryPolicy;
use crate::timeout::Timeout;
use crate::trace::TraceObserver;

use crate::auth::AuthScheme;
#[cfg(feature = "proxy")]
use crate::proxy::ProxyConfig;

/// Typed transport-level hints carried on a request.
///
/// These are *not* HTTP headers and do not affect logical URL semantics
/// (routing, cookies, auth-origin comparisons, redirects, proxy selection).
/// They control only the wire-level behavior of the transport layer.
///
/// Timeout is kept in the existing [`Timeout`](crate::Timeout) model and
/// must not be duplicated here.
#[derive(Default, Clone)]
pub struct TransportHints {
    /// Override the wire request target (e.g. `OPTIONS *`, absolute-form).
    ///
    /// When present this replaces only the URI sent on the wire; the
    /// logical URL used for connection routing, Host header defaults,
    /// cookies, auth, redirects, and proxy selection remains unchanged.
    pub target: Option<Bytes>,
    /// Override the TLS Server Name Indication hostname.
    ///
    /// TCP connects to the URL host/IP; TLS uses this name for SNI and
    /// certificate verification.
    pub sni_hostname: Option<String>,
    /// Optional trace observer for transport lifecycle events.
    ///
    /// When present, the transport layer emits typed events at each
    /// lifecycle boundary (TCP connect, TLS handshake, request/response
    /// headers, body chunks, connection close). The observer is invoked
    /// synchronously within the async context and must not block.
    pub trace: Option<Arc<dyn TraceObserver>>,
}

impl std::fmt::Debug for TransportHints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransportHints")
            .field("target", &self.target)
            .field("sni_hostname", &self.sni_hostname)
            .field("trace", &self.trace.as_ref().map(|_| "..."))
            .finish()
    }
}

/// Proxy override for a specific request.
///
/// Controls whether a request inherits the client-level proxy,
/// bypasses the proxy entirely, or uses a specific proxy configuration.
#[derive(Debug, Clone, Default)]
pub enum ProxyOverride {
    /// Inherit the client-level proxy configuration (default).
    #[default]
    Inherit,
    /// Send the request directly, bypassing any client proxy.
    Direct,
    /// Use a specific proxy configuration for this request.
    #[cfg(feature = "proxy")]
    Override(ProxyConfig),
}

/// Parts returned by [`Request::into_parts`].
///
/// `(method, url, headers, body, version, timeout, redirect, auth, auth_disabled, decompress, proxy_override, retry, transport_hints)`
pub(crate) type RequestParts = (
    http::Method,
    url::Url,
    Headers,
    RequestBody,
    Version,
    Option<Timeout>,
    Option<RedirectPolicy>,
    Option<AuthScheme>,
    bool,
    Option<bool>,
    ProxyOverride,
    Option<RetryPolicy>,
    TransportHints,
);

/// An outgoing HTTP request.
#[derive(Debug)]
pub struct Request {
    method: http::Method,
    url: url::Url,
    headers: Headers,
    body: RequestBody,
    version: Version,
    timeout: Option<Timeout>,
    redirect: Option<RedirectPolicy>,
    auth: Option<AuthScheme>,
    auth_disabled: bool,
    decompress: Option<bool>,
    /// Proxy override: `None` = inherit, `Some(None)` = direct, `Some(Some(c))` = explicit.
    proxy_override: ProxyOverride,
    /// Per-request retry policy override.
    retry: Option<RetryPolicy>,
    /// Typed transport-level hints (target override, SNI hostname, etc.).
    transport_hints: TransportHints,
}

impl Request {
    /// Create a new request (crate-internal).
    pub(crate) fn new(method: http::Method, url: url::Url) -> Self {
        Self {
            method,
            url,
            headers: Headers::new(),
            body: RequestBody::default(),
            version: Version::HTTP_11,
            timeout: None,
            redirect: None,
            auth: None,
            auth_disabled: false,
            decompress: None,
            proxy_override: ProxyOverride::Inherit,
            retry: None,
            transport_hints: TransportHints::default(),
        }
    }

    /// Returns the HTTP method.
    #[must_use]
    pub fn method(&self) -> &http::Method {
        &self.method
    }

    /// Returns the request URL.
    #[must_use]
    pub fn url(&self) -> &url::Url {
        &self.url
    }

    /// Returns a reference to the request headers.
    #[must_use]
    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    /// Returns a mutable reference to the request headers.
    pub fn headers_mut(&mut self) -> &mut Headers {
        &mut self.headers
    }

    /// Returns the request body.
    #[must_use]
    pub fn body(&self) -> &RequestBody {
        &self.body
    }

    /// Set the request body.
    pub fn set_body(&mut self, body: RequestBody) {
        self.body = body;
    }

    /// Returns the HTTP version.
    #[must_use]
    pub fn version(&self) -> Version {
        self.version
    }

    /// Set the HTTP version.
    pub fn set_version(&mut self, version: Version) {
        self.version = version;
    }

    /// Returns the request timeout configuration, if set.
    #[must_use]
    pub fn timeout(&self) -> Option<&Timeout> {
        self.timeout.as_ref()
    }

    /// Set the request-level timeout.
    pub fn set_timeout(&mut self, timeout: Option<Timeout>) {
        self.timeout = timeout;
    }

    /// Returns the request-level redirect policy override, if set.
    #[must_use]
    pub fn redirect(&self) -> Option<&RedirectPolicy> {
        self.redirect.as_ref()
    }

    /// Set the request-level redirect policy override.
    pub fn set_redirect(&mut self, redirect: Option<RedirectPolicy>) {
        self.redirect = redirect;
    }

    /// Returns a reference to the request-level auth override, if set.
    #[must_use]
    pub fn auth(&self) -> Option<&AuthScheme> {
        self.auth.as_ref()
    }

    /// Set the request-level auth override.
    pub fn set_auth(&mut self, auth: Option<AuthScheme>) {
        self.auth = auth;
    }

    /// Returns whether auth has been explicitly disabled for this request.
    #[must_use]
    pub fn is_auth_disabled(&self) -> bool {
        self.auth_disabled
    }

    /// Mark this request as having auth explicitly disabled.
    pub fn set_auth_disabled(&mut self, disabled: bool) {
        self.auth_disabled = disabled;
    }

    /// Returns the per-request decompression override, if set.
    ///
    /// - `Some(true)`: force decompression on
    /// - `Some(false)`: force decompression off
    /// - `None`: use client-level setting
    #[must_use]
    pub fn decompress(&self) -> Option<bool> {
        self.decompress
    }

    /// Set the per-request decompression override.
    pub fn set_decompress(&mut self, decompress: Option<bool>) {
        self.decompress = decompress;
    }

    /// Returns the proxy override for this request.
    #[cfg(feature = "proxy")]
    #[must_use]
    pub fn proxy_override(&self) -> &ProxyOverride {
        &self.proxy_override
    }

    /// Set the per-request proxy override.
    #[cfg(feature = "proxy")]
    pub fn set_proxy_override(&mut self, proxy: ProxyOverride) {
        self.proxy_override = proxy;
    }

    /// Returns the per-request retry policy override, if set.
    #[must_use]
    pub fn retry(&self) -> Option<&RetryPolicy> {
        self.retry.as_ref()
    }

    /// Set the per-request retry policy override.
    pub fn set_retry(&mut self, retry: Option<RetryPolicy>) {
        self.retry = retry;
    }

    /// Returns a reference to the transport hints for this request.
    #[must_use]
    pub fn transport_hints(&self) -> &TransportHints {
        &self.transport_hints
    }

    /// Set the transport hints for this request.
    pub fn set_transport_hints(&mut self, hints: TransportHints) {
        self.transport_hints = hints;
    }

    /// Decompose a request into its parts.
    ///
    /// Returns `(method, url, headers, body, version, timeout, redirect, auth, auth_disabled, decompress, proxy_override, retry, transport_hints)`.
    pub(crate) fn into_parts(self) -> RequestParts {
        (
            self.method,
            self.url,
            self.headers,
            self.body,
            self.version,
            self.timeout,
            self.redirect,
            self.auth,
            self.auth_disabled,
            self.decompress,
            self.proxy_override,
            self.retry,
            self.transport_hints,
        )
    }
}

/// Fluent builder for constructing requests.
pub struct RequestBuilder {
    client: Option<Client>,
    method: http::Method,
    url: url::Url,
    headers: Headers,
    body: RequestBody,
    timeout: Option<Timeout>,
    redirect: Option<RedirectPolicy>,
    auth: Option<AuthScheme>,
    auth_disabled: bool,
    decompress: Option<bool>,
    proxy_override: ProxyOverride,
    retry: Option<RetryPolicy>,
    transport_hints: TransportHints,
    error: Option<crate::Error>,
}

impl RequestBuilder {
    /// Create a new request builder associated with a client.
    pub(crate) fn new(client: Client, method: http::Method, url: url::Url) -> Self {
        Self {
            client: Some(client),
            method,
            url,
            headers: Headers::new(),
            body: RequestBody::default(),
            timeout: None,
            redirect: None,
            auth: None,
            auth_disabled: false,
            decompress: None,
            proxy_override: ProxyOverride::Inherit,
            retry: None,
            transport_hints: TransportHints::default(),
            error: None,
        }
    }

    /// Add a single header to the request.
    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        if let Err(e) = self.headers.insert(name, value) {
            self.error = Some(e);
        }
        self
    }

    /// Replace all headers with the provided set.
    #[must_use]
    pub fn headers(mut self, headers: Headers) -> Self {
        self.headers = headers;
        self
    }

    /// Append a query parameter to the URL.
    #[must_use]
    pub fn query(mut self, key: &str, value: &str) -> Self {
        self.url.query_pairs_mut().append_pair(key, value);
        self
    }

    /// Set the request body from any type that converts into `RequestBody`.
    #[must_use]
    pub fn body(mut self, body: impl Into<RequestBody>) -> Self {
        self.body = body.into();
        self
    }

    /// Set the request body from bytes.
    #[must_use]
    pub fn bytes(self, data: impl Into<Bytes>) -> Self {
        self.body(RequestBody::Bytes(data.into()))
    }

    /// Set the timeout for this specific request.
    ///
    /// When set, this overrides the client-level timeout on a per-field
    /// basis: only fields present here replace the corresponding
    /// client-level fields.
    #[must_use]
    pub fn timeout(mut self, timeout: Timeout) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Override the redirect policy for this specific request.
    ///
    /// When set, this overrides the client-level redirect policy for
    /// this request only.
    #[must_use]
    pub fn redirect_policy(mut self, policy: RedirectPolicy) -> Self {
        self.redirect = Some(policy);
        self
    }

    /// Set authentication for this specific request.
    ///
    /// Overrides client-level auth.
    #[must_use]
    pub fn auth(mut self, auth: impl Into<AuthScheme>) -> Self {
        self.auth = Some(auth.into());
        self.auth_disabled = false;
        self
    }

    /// Disable authentication for this specific request.
    ///
    /// When set, no auth is applied to this request even if the client
    /// has a default auth configured.
    #[must_use]
    pub fn without_auth(mut self) -> Self {
        self.auth_disabled = true;
        self.auth = None;
        self
    }

    /// Override the proxy for this specific request.
    ///
    /// When set, this proxy is used instead of the client-level proxy.
    #[cfg(feature = "proxy")]
    #[must_use]
    pub fn proxy(mut self, proxy: &crate::proxy::Proxy) -> Self {
        self.proxy_override = ProxyOverride::Override(proxy.config());
        self
    }

    /// Disable proxy for this specific request.
    ///
    /// When set, the request is sent directly without going through
    /// any proxy, even if the client has a default proxy configured.
    #[cfg(feature = "proxy")]
    #[must_use]
    pub fn without_proxy(mut self) -> Self {
        self.proxy_override = ProxyOverride::Direct;
        self
    }

    /// Override decompression for this specific request.
    ///
    /// - `true`: enable decompression regardless of client setting
    /// - `false`: disable decompression regardless of client setting
    #[must_use]
    pub fn decompress(mut self, decompress: bool) -> Self {
        self.decompress = Some(decompress);
        self
    }

    /// Override the retry policy for this specific request.
    ///
    /// When set, this overrides the client-level retry policy for
    /// this request only.
    #[must_use]
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    /// Disable retries for this specific request, even if the client
    /// has a retry policy configured.
    #[must_use]
    pub fn without_retry(mut self) -> Self {
        self.retry = Some(RetryPolicy::default());
        self
    }

    /// Set the transport hints for this request.
    ///
    /// Transport hints control wire-level behavior (target override,
    /// SNI hostname) without affecting logical URL semantics.
    #[must_use]
    pub fn transport_hints(mut self, hints: TransportHints) -> Self {
        self.transport_hints = hints;
        self
    }

    /// Build the request without sending it.
    ///
    /// # Errors
    ///
    /// Returns an error if a previous builder step failed (e.g., invalid
    /// header).
    pub fn build(mut self) -> Result<Request> {
        if let Some(e) = self.error.take() {
            return Err(e);
        }
        let mut req = Request::new(self.method, self.url);
        req.headers = self.headers;
        req.body = self.body;
        req.timeout = self.timeout;
        req.redirect = self.redirect;
        req.auth = self.auth;
        req.auth_disabled = self.auth_disabled;
        req.decompress = self.decompress;
        req.proxy_override = self.proxy_override;
        req.retry = self.retry;
        req.transport_hints = self.transport_hints;
        Ok(req)
    }

    /// Build and send the request.
    ///
    /// # Errors
    ///
    /// Returns an error if the request could not be built or sent.
    pub async fn send(self) -> Result<Response> {
        let client = self.client.clone().ok_or_else(|| {
            crate::Error::RequestBuild("no client associated with request builder".into())
        })?;
        let request = self.build()?;
        Box::pin(client.send(request)).await
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    proptest::proptest! {
        #[test]
        fn url_parse_round_trip(scheme in "https?", host in "[a-z]{2,10}\\.[a-z]{2,5}", path in "/[a-z]{0,20}") {
            let url_str = format!("{scheme}://{host}{path}");
            let parsed = url::Url::parse(&url_str).unwrap();
            let serialized = parsed.to_string();
            prop_assert_eq!(url_str, serialized);
        }

        #[test]
        fn query_pairs_round_trip(key in "[a-z]{1,10}", value in "[a-z0-9]{1,20}") {
            let mut url = url::Url::parse("http://example.com/").unwrap();
            url.query_pairs_mut().append_pair(&key, &value);
            let found = url.query_pairs().find(|(k, _)| k == &key).map(|(_, v)| v.into_owned());
            prop_assert_eq!(found, Some(value));
        }

        #[test]
        fn url_with_arbitrary_path(path in "/[a-zA-Z0-9._/-]{0,50}") {
            let url_str = format!("http://example.com{path}");
            // Must not panic
            let _ = url::Url::parse(&url_str);
        }
    }
}
