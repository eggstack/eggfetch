//! High-level URL request types and builder.
//!
//! This module requires the `high-level-url` feature. Protocol-neutral
//! transport controls (`ResolvedTarget`, `TransportHints`,
//! `NativeRequestOptions`) live in [`crate::transport_hints`] so the minimal
//! native slice compiles without `url`.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http::Version;

use crate::body::RequestBody;
use crate::client::Client;
use crate::error::Result;
use crate::headers::Headers;
#[cfg(feature = "redirects")]
use crate::redirect::RedirectPolicy;
use crate::response::Response;
#[cfg(feature = "logical-retry")]
use crate::retry::RetryPolicy;
use crate::timeout::Timeout;

use crate::auth::AuthScheme;
#[cfg(feature = "proxy")]
use crate::proxy::ProxyConfig;

// Historical paths: native transport controls moved to
// `crate::transport_hints`. Re-export here so existing
// `crate::request::{ResolvedTarget, TransportHints, NativeRequestOptions}`
// paths keep working for high-level users. New code should import from
// `crate::transport_hints` directly.
pub use crate::transport_hints::{NativeRequestOptions, ResolvedTarget, TransportHints};

/// Proxy override for a specific request.
///
/// Controls whether a request inherits the client-level proxy,
/// bypasses the proxy entirely, or uses a specific proxy configuration.
#[derive(Debug, Clone, Default)]
#[allow(
    clippy::large_enum_variant,
    reason = "ProxyConfig is transient and cloning it is cheap; boxing adds unnecessary allocation"
)]
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
/// This is the complete logical-request state carried across retry and
/// redirect policy transformations. All reconstruction goes through the
/// typed helpers below so a newly added field fails to compile (via
/// exhaustive construction) rather than being silently dropped.
pub(crate) struct RequestParts {
    pub(crate) method: http::Method,
    pub(crate) url: url::Url,
    pub(crate) headers: Headers,
    pub(crate) body: RequestBody,
    pub(crate) version: Version,
    pub(crate) timeout: Option<Timeout>,
    #[cfg(feature = "redirects")]
    pub(crate) redirect: Option<RedirectPolicy>,
    pub(crate) auth: Option<AuthScheme>,
    pub(crate) auth_disabled: bool,
    pub(crate) decompress: Option<bool>,
    pub(crate) max_decoded_body_size: Option<usize>,
    pub(crate) max_decompression_ratio: Option<f64>,
    pub(crate) proxy_override: ProxyOverride,
    #[cfg(feature = "logical-retry")]
    pub(crate) retry: Option<RetryPolicy>,
    pub(crate) transport_hints: TransportHints,
    pub(crate) proxied_target: Option<ResolvedTarget>,
    pub(crate) failure_context: Option<Arc<crate::error::RequestFailureContext>>,
}

impl RequestParts {
    /// Reassemble the parts into a [`Request`] without body replay.
    ///
    /// Exhaustively constructs the request so adding a field to
    /// [`RequestParts`] without updating this path is a compile error.
    /// Used by regression tests as the canonical round-trip anchor; the
    /// live retry path uses [`Self::retry_request`] with a shrunk deadline.
    #[allow(
        dead_code,
        reason = "exhaustiveness anchor for RequestParts: guarantees new fields fail to compile even though live code uses retry_request"
    )]
    pub(crate) fn into_request(self) -> Request {
        let RequestParts {
            method,
            url,
            headers,
            body,
            version,
            timeout,
            #[cfg(feature = "redirects")]
            redirect,
            auth,
            auth_disabled,
            decompress,
            max_decoded_body_size,
            max_decompression_ratio,
            proxy_override,
            #[cfg(feature = "logical-retry")]
            retry,
            transport_hints,
            proxied_target,
            failure_context,
        } = self;
        let mut request = Request::new(method, url);
        *request.headers_mut() = headers;
        request.set_body(body);
        request.set_version(version);
        request.set_timeout(timeout);
        #[cfg(feature = "redirects")]
        request.set_redirect(redirect);
        #[cfg(not(feature = "redirects"))]
        let _ = ();
        request.set_auth(auth);
        request.set_auth_disabled(auth_disabled);
        request.set_decompress(decompress);
        request.set_max_decoded_body_size(max_decoded_body_size);
        request.set_max_decompression_ratio(max_decompression_ratio);
        #[cfg(feature = "proxy")]
        request.set_proxy_override(proxy_override);
        #[cfg(not(feature = "proxy"))]
        let _ = proxy_override;
        #[cfg(feature = "logical-retry")]
        request.set_retry(retry);
        #[cfg(not(feature = "logical-retry"))]
        let _ = ();
        request.set_transport_hints(transport_hints);
        request.set_proxied_target(proxied_target);
        request.set_failure_context(failure_context);
        request
    }

    /// Build the retry attempt for the same logical request.
    ///
    /// Only available with the `logical-retry` feature. Preserves every
    /// request-local override that remains valid for
    /// another attempt (including transport hints, proxy/decompression
    /// overrides, auth-disable state, redirect and retry policy) while
    /// applying the shrunk `attempt_timeout` for the remaining total
    /// deadline. Non-replayable stream bodies return the existing
    /// [`crate::error::Error::BodyNotReplayableForRetry`] classification.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::BodyNotReplayableForRetry`] when the
    /// saved body is a one-shot stream.
    #[cfg(feature = "logical-retry")]
    pub(crate) fn retry_request(
        &self,
        attempt_timeout: Option<Timeout>,
    ) -> crate::error::Result<Request> {
        let replayed = self.body.try_clone_for_retry()?;
        let mut request = Request::new(self.method.clone(), self.url.clone());
        *request.headers_mut() = self.headers.clone();
        request.set_body(replayed);
        request.set_version(self.version);
        request.set_timeout(attempt_timeout);
        #[cfg(feature = "redirects")]
        request.set_redirect(self.redirect.clone());
        request.set_auth(self.auth.clone());
        request.set_auth_disabled(self.auth_disabled);
        request.set_decompress(self.decompress);
        request.set_max_decoded_body_size(self.max_decoded_body_size);
        request.set_max_decompression_ratio(self.max_decompression_ratio);
        #[cfg(feature = "proxy")]
        request.set_proxy_override(self.proxy_override.clone());
        request.set_retry(self.retry.clone());
        request.set_transport_hints(self.transport_hints.clone());
        request.set_proxied_target(self.proxied_target.clone());
        request.set_failure_context(self.failure_context.clone());
        Ok(request)
    }

    /// Apply a shrunk total deadline to a timeout value.
    ///
    /// Only available with the `logical-retry` feature. Returns the timeout
    /// with `total` replaced by `total - elapsed` (saturating). The retry
    /// loop uses this so the original total deadline shrinks rather than
    /// restarting on each attempt; the redirect loop inlines the same
    /// shrinking for per-hop budgets.
    ///
    /// Per-phase (`connect`/`read`/`write`/`pool`) values are intentionally
    /// left unclamped: the remaining total stays authoritative as the outer
    /// `send_with_total_timeout` bound around each attempt/hop, so a late
    /// attempt cannot overshoot the total budget even with full per-phase
    /// values.
    #[cfg(feature = "logical-retry")]
    pub(crate) fn shrink_total_deadline(
        timeout: &Timeout,
        elapsed: std::time::Duration,
    ) -> Timeout {
        let mut out = *timeout;
        if let Some(total) = out.total {
            out.total = Some(total.saturating_sub(elapsed));
        }
        out
    }
}

/// An outgoing HTTP request.
///
/// # Security
///
/// The `Debug` implementation redacts secrets: the URL is rendered without
/// userinfo, query parameters, or fragments (query strings routinely carry
/// API keys), sensitive headers use the redacted form, and the body is
/// summarized by length only. See [`crate::redact`].
pub struct Request {
    method: http::Method,
    url: url::Url,
    headers: Headers,
    body: RequestBody,
    version: Version,
    timeout: Option<Timeout>,
    #[cfg(feature = "redirects")]
    redirect: Option<RedirectPolicy>,
    auth: Option<AuthScheme>,
    auth_disabled: bool,
    decompress: Option<bool>,
    max_decoded_body_size: Option<usize>,
    max_decompression_ratio: Option<f64>,
    /// Proxy override: `None` = inherit, `Some(None)` = direct, `Some(Some(c))` = explicit.
    proxy_override: ProxyOverride,
    /// Per-request retry policy override. Only available with `logical-retry`.
    #[cfg(feature = "logical-retry")]
    retry: Option<RetryPolicy>,
    /// Typed transport-level hints (target override, SNI hostname, etc.).
    transport_hints: TransportHints,
    /// Caller-supplied physical destination for a proxied request.
    proxied_target: Option<ResolvedTarget>,
    /// Optional native detailed-failure context; ordinary requests leave it
    /// absent so they do not allocate diagnostics state.
    failure_context: Option<Arc<crate::error::RequestFailureContext>>,
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("Request");
        debug
            .field("method", &self.method)
            .field("url", &crate::redact::redact_url(&self.url))
            .field("headers", &self.headers)
            .field("body", &self.body)
            .field("version", &self.version)
            .field("timeout", &self.timeout);
        #[cfg(feature = "redirects")]
        debug.field("redirect", &self.redirect);
        debug
            .field("auth", &self.auth)
            .field("auth_disabled", &self.auth_disabled)
            .field("decompress", &self.decompress)
            .field("max_decoded_body_size", &self.max_decoded_body_size)
            .field("max_decompression_ratio", &self.max_decompression_ratio)
            .field("proxy_override", &self.proxy_override);
        #[cfg(feature = "logical-retry")]
        debug.field("retry", &self.retry);
        debug
            .field("transport_hints", &self.transport_hints)
            .field("proxied_target", &self.proxied_target)
            .field(
                "failure_context",
                &self.failure_context.as_ref().map(|_| "configured"),
            )
            .finish()
    }
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
            #[cfg(feature = "redirects")]
            redirect: None,
            auth: None,
            auth_disabled: false,
            decompress: None,
            max_decoded_body_size: None,
            max_decompression_ratio: None,
            proxy_override: ProxyOverride::Inherit,
            #[cfg(feature = "logical-retry")]
            retry: None,
            transport_hints: TransportHints::default(),
            proxied_target: None,
            failure_context: None,
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
    ///
    /// Only available with the `redirects` feature.
    #[cfg(feature = "redirects")]
    #[must_use]
    pub fn redirect(&self) -> Option<&RedirectPolicy> {
        self.redirect.as_ref()
    }

    /// Set the request-level redirect policy override.
    ///
    /// Only available with the `redirects` feature.
    #[cfg(feature = "redirects")]
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

    /// Returns the per-request decoded response body size limit, if set.
    ///
    /// `None` means the client-level setting is used.
    #[must_use]
    pub fn max_decoded_body_size(&self) -> Option<usize> {
        self.max_decoded_body_size
    }

    /// Set the per-request decoded response body size limit.
    pub fn set_max_decoded_body_size(&mut self, max: Option<usize>) {
        self.max_decoded_body_size = max;
    }

    /// Returns the per-request decompression ratio limit, if set.
    ///
    /// `None` means the client-level setting is used.
    #[must_use]
    pub fn max_decompression_ratio(&self) -> Option<f64> {
        self.max_decompression_ratio
    }

    /// Set the per-request decompression ratio limit.
    pub fn set_max_decompression_ratio(&mut self, ratio: Option<f64>) {
        self.max_decompression_ratio = ratio;
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
    ///
    /// Only available with the `logical-retry` feature.
    #[cfg(feature = "logical-retry")]
    #[must_use]
    pub fn retry(&self) -> Option<&RetryPolicy> {
        self.retry.as_ref()
    }

    /// Set the per-request retry policy override.
    ///
    /// Only available with the `logical-retry` feature.
    #[cfg(feature = "logical-retry")]
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

    /// Returns the caller-supplied physical destination for a proxied
    /// request, if configured.
    #[must_use]
    pub fn proxied_target(&self) -> Option<&ResolvedTarget> {
        self.proxied_target.as_ref()
    }

    /// Set the caller-supplied physical destination for a proxied request.
    pub fn set_proxied_target(&mut self, target: Option<ResolvedTarget>) {
        self.proxied_target = target;
    }

    pub(crate) fn set_failure_context(
        &mut self,
        context: Option<Arc<crate::error::RequestFailureContext>>,
    ) {
        self.failure_context = context;
    }

    /// Decompose a request into its parts.
    ///
    /// Returns the request fields as named parts.
    pub(crate) fn into_parts(self) -> RequestParts {
        RequestParts {
            method: self.method,
            url: self.url,
            headers: self.headers,
            body: self.body,
            version: self.version,
            timeout: self.timeout,
            #[cfg(feature = "redirects")]
            redirect: self.redirect,
            auth: self.auth,
            auth_disabled: self.auth_disabled,
            decompress: self.decompress,
            max_decoded_body_size: self.max_decoded_body_size,
            max_decompression_ratio: self.max_decompression_ratio,
            proxy_override: self.proxy_override,
            #[cfg(feature = "logical-retry")]
            retry: self.retry,
            transport_hints: self.transport_hints,
            proxied_target: self.proxied_target,
            failure_context: self.failure_context,
        }
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
    #[cfg(feature = "redirects")]
    redirect: Option<RedirectPolicy>,
    auth: Option<AuthScheme>,
    auth_disabled: bool,
    decompress: Option<bool>,
    max_decoded_body_size: Option<usize>,
    max_decompression_ratio: Option<f64>,
    proxy_override: ProxyOverride,
    #[cfg(feature = "logical-retry")]
    retry: Option<RetryPolicy>,
    transport_hints: TransportHints,
    proxied_target: Option<ResolvedTarget>,
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
            #[cfg(feature = "redirects")]
            redirect: None,
            auth: None,
            auth_disabled: false,
            decompress: None,
            max_decoded_body_size: None,
            max_decompression_ratio: None,
            proxy_override: ProxyOverride::Inherit,
            #[cfg(feature = "logical-retry")]
            retry: None,
            transport_hints: TransportHints::default(),
            proxied_target: None,
            error: None,
        }
    }

    /// Add a single header to the request.
    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        if let Err(e) = self.headers.insert(name, value) {
            if self.error.is_none() {
                self.error = Some(e);
            }
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

    /// Serialize a value as JSON and use it as a replayable request body.
    ///
    /// Sets `Content-Type: application/json` unless the request already has
    /// an explicit content type. The value is serialized before any network
    /// operation; serialization failures are returned immediately. Like the
    /// other body setters, this replaces a body set earlier in the chain. A
    /// later `body()` or `bytes()` call replaces the JSON bytes; it does not
    /// remove a content type that was already set.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::JsonSerialize`] when the value cannot be
    /// serialized, or a header error if the default content type is invalid.
    #[cfg(feature = "json")]
    pub fn json<T: serde::Serialize>(mut self, value: &T) -> Result<Self> {
        let bytes = serde_json::to_vec(value)
            .map_err(|_| crate::Error::JsonSerialize("failed to serialize JSON value".into()))?;
        self.body = RequestBody::Bytes(Bytes::from(bytes));
        if !self.headers.contains("content-type") {
            self.headers.insert("content-type", "application/json")?;
        }
        Ok(self)
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
    /// Only available with the `redirects` feature. When set, this overrides
    /// the client-level redirect policy for this request only.
    #[cfg(feature = "redirects")]
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

    /// Override the client decoded response body size limit for this request.
    ///
    /// The request setting takes precedence over the client setting and is
    /// carried across retries and redirects. It applies to decoded compressed
    /// data and ordinary unencoded/identity response bodies, whether the
    /// response is buffered or streamed. A value of zero rejects any
    /// non-empty decoded response body and yields
    /// [`crate::Error::DecodedBodyTooLarge`].
    #[must_use]
    pub fn max_decoded_body_size(mut self, max: usize) -> Self {
        self.max_decoded_body_size = Some(max);
        self
    }

    /// Override the client decompression ratio limit for this request.
    ///
    /// The request setting takes precedence over the client setting and is
    /// carried across retries and redirects.
    ///
    /// # Panics
    ///
    /// Panics if `ratio` is not finite or not positive.
    #[must_use]
    pub fn max_decompression_ratio(mut self, ratio: f64) -> Self {
        assert!(
            ratio.is_finite() && ratio > 0.0,
            "max_decompression_ratio must be finite and positive, got {ratio}"
        );
        self.max_decompression_ratio = Some(ratio);
        self
    }

    /// Override the retry policy for this specific request.
    ///
    /// Only available with the `logical-retry` feature. When set, this
    /// overrides the client-level retry policy for this request only.
    #[cfg(feature = "logical-retry")]
    #[must_use]
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    /// Disable retries for this specific request, even if the client
    /// has a retry policy configured.
    ///
    /// Only available with the `logical-retry` feature.
    #[cfg(feature = "logical-retry")]
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

    /// Pin this request to caller-supplied remote destinations.
    ///
    /// The logical URL remains unchanged for HTTP authority, TLS identity,
    /// redirects, cookies, and authentication. The set must be non-empty and
    /// each address must use the URL's effective HTTP/HTTPS port. Static
    /// routing is direct-only; proxy and HTTP/3 combinations fail closed.
    #[must_use]
    pub fn resolved_addresses<I>(mut self, addresses: I) -> Self
    where
        I: IntoIterator<Item = SocketAddr>,
    {
        match ResolvedTarget::new(addresses) {
            Ok(target) => self.transport_hints.resolved_target = Some(target),
            Err(error) => {
                if self.error.is_none() {
                    self.error = Some(error);
                }
            }
        }
        self
    }

    /// Pin a proxied request to caller-supplied physical destinations.
    ///
    /// This is distinct from [`Self::resolved_addresses`], which remains a
    /// direct-only route. The logical URL remains authoritative for proxy
    /// selection, HTTP Host, TLS SNI/certificate verification, redirects,
    /// cookies, and authentication. HTTP CONNECT and local-resolution
    /// SOCKS5 use these addresses without an origin DNS lookup. SOCKS5H and
    /// plaintext HTTP forward-proxy requests reject this option.
    #[must_use]
    pub fn proxy_target_addresses<I>(mut self, addresses: I) -> Self
    where
        I: IntoIterator<Item = SocketAddr>,
    {
        match ResolvedTarget::new(addresses) {
            Ok(target) => self.proxied_target = Some(target),
            Err(error) => {
                if self.error.is_none() {
                    self.error = Some(error);
                }
            }
        }
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
        if let Some(target) = &self.transport_hints.resolved_target {
            let expected_port = self.url.port_or_known_default().ok_or_else(|| {
                crate::Error::InvalidResolvedTarget(
                    "resolved destinations require an HTTP or HTTPS URL".into(),
                )
            })?;
            if target
                .addresses()
                .iter()
                .any(|address| address.port() != expected_port)
            {
                return Err(crate::Error::InvalidResolvedTarget(format!(
                    "all resolved destinations must use the URL's effective port {expected_port}"
                )));
            }
        }
        if let Some(target) = &self.proxied_target {
            let expected_port = self.url.port_or_known_default().ok_or_else(|| {
                crate::Error::InvalidResolvedTarget(
                    "proxied destinations require an HTTP or HTTPS URL".into(),
                )
            })?;
            if target
                .addresses()
                .iter()
                .any(|address| address.port() != expected_port)
            {
                return Err(crate::Error::InvalidResolvedTarget(format!(
                    "all proxied destinations must use the URL's effective port {expected_port}"
                )));
            }
        }
        let request_target = self
            .transport_hints
            .target
            .as_deref()
            .unwrap_or_else(|| self.url.as_str().as_bytes());
        self.headers
            .validate_request_size(&self.method, request_target)?;
        let mut req = Request::new(self.method, self.url);
        req.headers = self.headers;
        req.body = self.body;
        req.timeout = self.timeout;
        #[cfg(feature = "redirects")]
        req.set_redirect(self.redirect);
        req.auth = self.auth;
        req.auth_disabled = self.auth_disabled;
        req.decompress = self.decompress;
        req.max_decoded_body_size = self.max_decoded_body_size;
        req.max_decompression_ratio = self.max_decompression_ratio;
        req.proxy_override = self.proxy_override;
        #[cfg(feature = "logical-retry")]
        req.set_retry(self.retry);
        req.transport_hints = self.transport_hints;
        req.proxied_target = self.proxied_target;
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

    /// Build and send the request with optional structured native failure
    /// classification.
    ///
    /// The underlying [`crate::Error`] and all ordinary request behavior are
    /// unchanged. Use [`RequestFailure::into_error`](crate::RequestFailure::into_error)
    /// to recover the legacy error without loss.
    ///
    /// # Errors
    ///
    /// Returns the original error wrapped in [`crate::RequestFailure`] if the
    /// request could not be built or sent.
    pub async fn send_detailed(self) -> std::result::Result<Response, crate::RequestFailure> {
        let Some(client) = self.client.clone() else {
            return Err(crate::RequestFailure::from_error(
                crate::Error::RequestBuild("no client associated with request builder".into()),
            ));
        };
        let request = match self.build() {
            Ok(request) => request,
            Err(error) => return Err(crate::RequestFailure::from_error(error)),
        };
        client.send_detailed(request).await
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{Request, ResolvedTarget};
    use crate::auth::AuthScheme;
    #[cfg(feature = "basic-auth")]
    use crate::auth::BasicAuth;
    use bytes::Bytes;
    use std::net::SocketAddr;

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

    #[test]
    fn request_debug_redacts_secrets() {
        let url = url::Url::parse("https://example.com/submit?api_key=top-secret-key").unwrap();
        let mut req = Request::new(http::Method::POST, url);
        req.headers_mut()
            .insert("authorization", "Bearer bearer-secret-token")
            .unwrap();
        req.headers_mut()
            .insert("cookie", "session=cookie-secret")
            .unwrap();
        req.set_body(crate::body::RequestBody::Bytes(Bytes::from(
            "password=form-secret",
        )));
        #[cfg(feature = "basic-auth")]
        req.set_auth(Some(AuthScheme::Basic(
            BasicAuth::new("user", "auth-secret").unwrap(),
        )));
        #[cfg(not(feature = "basic-auth"))]
        req.set_auth(Some(AuthScheme::bearer("auth-secret").unwrap()));
        let debug = format!("{req:?}");
        for leaked in [
            "top-secret-key",
            "bearer-secret-token",
            "cookie-secret",
            "form-secret",
            "auth-secret",
        ] {
            assert!(
                !debug.contains(leaked),
                "Request Debug must not leak {leaked:?}: {debug}"
            );
        }
        // Non-secret structure is still visible for diagnostics.
        assert!(debug.contains("example.com"), "keeps the host: {debug}");
        assert!(debug.contains("POST"), "keeps the method: {debug}");
    }

    #[test]
    fn resolved_target_rejects_empty_and_hides_addresses_in_debug() {
        assert!(ResolvedTarget::new(Vec::<SocketAddr>::new()).is_err());
        let target = ResolvedTarget::new(["127.0.0.1:443".parse().unwrap()]).unwrap();
        let debug = format!("{target:?}");
        assert!(debug.contains("address_count: 1"));
        assert!(!debug.contains("127.0.0.1"));
    }

    #[test]
    fn ordinary_request_has_no_failure_context() {
        let request = Request::new(
            http::Method::GET,
            url::Url::parse("http://example.com/").expect("valid URL"),
        );
        assert!(request.failure_context.is_none());
    }

    #[test]
    fn request_builder_validates_resolved_target_port() {
        let client = crate::client::Client::new();
        let request = client
            .get("http://example.invalid/")
            .unwrap()
            .resolved_addresses(["127.0.0.1:80".parse().unwrap()])
            .build()
            .unwrap();
        assert_eq!(
            request
                .transport_hints()
                .resolved_target
                .as_ref()
                .unwrap()
                .addresses()
                .len(),
            1
        );

        let error = client
            .get("http://example.invalid/")
            .unwrap()
            .resolved_addresses(["127.0.0.1:443".parse().unwrap()])
            .build()
            .unwrap_err();
        assert_eq!(error.kind(), "invalid_resolved_target");
    }

    #[cfg(feature = "json")]
    #[test]
    fn request_json_sets_default_content_type_and_replayable_body() {
        let client = crate::client::Client::new();
        let request = client
            .post("http://example.invalid/")
            .unwrap()
            .json(&serde_json::json!({"key": "value"}))
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(
            request.headers().get("content-type").unwrap(),
            "application/json"
        );
        assert!(request.body().is_replayable());

        let request = client
            .post("http://example.invalid/")
            .unwrap()
            .header("content-type", "application/vnd.api+json")
            .json(&serde_json::json!([1, 2, 3]))
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(
            request.headers().get("content-type").unwrap(),
            "application/vnd.api+json"
        );
    }
}
