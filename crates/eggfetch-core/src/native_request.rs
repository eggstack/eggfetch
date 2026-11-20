//! Native request builder for `http::Uri` targets.
//!
//! The high-level request builder is backed by `url::Url`, so it is only
//! available with the `high-level-url` feature and carries the
//! `url`/`idna`/ICU dependency closure with it. This module is the
//! URL-free counterpart: a builder that takes an already-parsed
//! [`http::Uri`] and dispatches through the same native frame-preserving
//! transport as [`crate::Client::execute_http_body`]. It is available in
//! every profile, including the minimal native slice that compiles without
//! `url`.
//!
//! # Semantics
//!
//! The native path dispatches exactly once. It applies no redirects, no
//! logical retries, no cookies, no authentication, no response
//! decompression, and no decoded-body limits. The caller owns IDNA and
//! punycode conversion before constructing the [`http::Uri`], and the
//! built-in proxy and HTTP/3 routes reject native frame bodies.
//!
//! # Relationship to the high-level builder
//!
//! `crate::request::RequestBuilder` and this builder are deliberately
//! separate types rather than one type with two target representations. The
//! high-level builder owns URL semantics (query pairs, relative redirect
//! resolution, cookie and proxy matching by URL) and returns the high-level
//! `crate::response::Response`. This builder owns only the wire-level
//! request and returns a plain [`http::Response`]. Keeping them separate
//! means a native consumer can depend on the transport without pulling in
//! the URL layer, and it keeps the existing high-level signatures intact.

use bytes::Bytes;
use http::Method;

use crate::body::{NativeResponseBody, RequestBody};
use crate::client::Client;
use crate::error::Result;
use crate::headers::Headers;
use crate::redirect::RedirectPolicy;
use crate::timeout::Timeout;
use crate::transport_hints::{NativeRequestOptions, TransportHints};

/// Builder for a native request addressed by an [`http::Uri`.
///
/// Create one with [`crate::Client::get_uri`] or
/// [`crate::Client::request_uri`]. The builder is dispatched through the
/// native transport, so it applies none of the high-level request policy:
/// no redirect following, no logical retries, no cookie jar, no
/// authentication, no response decompression, and no decoded-body limits.
///
/// The request is sent exactly once. [`Self::without_retry`] and
/// [`Self::redirect_policy`] therefore cannot change the wire behavior;
/// they exist so the same request-building code can express a
/// single-attempt contract in every profile.
pub struct NativeRequestBuilder {
    client: Option<Client>,
    method: Method,
    uri: http::Uri,
    headers: Headers,
    body: RequestBody,
    version: http::Version,
    timeout: Option<Timeout>,
    transport_hints: TransportHints,
    /// Whether the caller asserted that this request must not be retried.
    retry_disabled: bool,
    /// The redirect policy the caller attached to this request.
    redirect: Option<RedirectPolicy>,
    error: Option<crate::Error>,
}

impl std::fmt::Debug for NativeRequestBuilder {
    /// Render the builder without the query string.
    ///
    /// A native URI is caller-owned and may carry query material (API keys
    /// routinely travel in query parameters), so only the scheme,
    /// authority, and path are shown.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let scheme = self.uri.scheme_str().unwrap_or("");
        let authority = self
            .uri
            .authority()
            .map_or("", http::uri::Authority::as_str);
        let path = self.uri.path();
        f.debug_struct("NativeRequestBuilder")
            .field("method", &self.method)
            .field("uri", &format!("{scheme}://{authority}{path}"))
            .field("headers", &self.headers)
            .field("body", &self.body)
            .field("version", &self.version)
            .field("timeout", &self.timeout)
            .field("transport_hints", &self.transport_hints)
            .field("retry_disabled", &self.retry_disabled)
            .field("redirect", &self.redirect)
            .finish_non_exhaustive()
    }
}

impl NativeRequestBuilder {
    /// Create a new native request builder associated with a client.
    pub(crate) fn new(client: Client, method: Method, uri: http::Uri) -> Self {
        Self {
            client: Some(client),
            method,
            uri,
            headers: Headers::new(),
            body: RequestBody::default(),
            version: http::Version::HTTP_11,
            timeout: None,
            transport_hints: TransportHints::default(),
            retry_disabled: false,
            redirect: None,
            error: None,
        }
    }

    /// Add a single header to the request.
    ///
    /// An invalid header name or value is recorded and reported by
    /// [`Self::build`] and [`Self::send`].
    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        if let Err(error) = self.headers.insert(name, value) {
            if self.error.is_none() {
                self.error = Some(error);
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

    /// Set the request body from any type that converts into
    /// [`RequestBody`].
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

    /// Set the HTTP version for this request.
    ///
    /// The default is HTTP/1.1. HTTP/2 is selected by protocol negotiation
    /// unless the client is built with an explicit version policy.
    #[must_use]
    pub fn version(mut self, version: http::Version) -> Self {
        self.version = version;
        self
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

    /// Set the transport hints for this request.
    ///
    /// Transport hints control wire-level behavior (target override, SNI
    /// hostname, resolved destination) without affecting logical URL
    /// semantics.
    #[must_use]
    pub fn transport_hints(mut self, hints: TransportHints) -> Self {
        self.transport_hints = hints;
        self
    }

    /// Assert that this request must not be retried.
    ///
    /// The native transport dispatches exactly once, so this call cannot
    /// change the wire behavior: the assertion already holds. It exists so
    /// a request built for the high-level API can express the same
    /// single-attempt contract in a profile that compiles out the retry
    /// engine.
    #[must_use]
    pub fn without_retry(mut self) -> Self {
        self.retry_disabled = true;
        self
    }

    /// Attach a redirect policy to this request.
    ///
    /// The native transport never follows redirects, so this call cannot
    /// change the wire behavior: a 3xx response is returned unchanged. It
    /// exists so a request built for the high-level API can express the
    /// same "do not follow redirects" contract in a profile that compiles
    /// out the redirect loop.
    #[must_use]
    pub fn redirect_policy(mut self, policy: RedirectPolicy) -> Self {
        self.redirect = Some(policy);
        self
    }

    /// Build the request without sending it.
    ///
    /// # Errors
    ///
    /// Returns an error if a previous builder step failed (for example an
    /// invalid header).
    pub fn build(self) -> Result<http::Request<RequestBody>> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let mut request = http::Request::new(self.body);
        *request.uri_mut() = self.uri;
        *request.method_mut() = self.method;
        *request.version_mut() = self.version;
        *request.headers_mut() = self.headers.into_inner();
        Ok(request)
    }

    /// Build and send the request.
    ///
    /// The response is an ordinary [`http::Response`] whose body is a
    /// [`NativeResponseBody`]. The logical pool lease is held by the
    /// response body until it reaches EOF, errors, or is dropped.
    ///
    /// # Errors
    ///
    /// Returns an error if the request could not be built or sent.
    pub async fn send(self) -> Result<http::Response<NativeResponseBody>> {
        let client = self.client.clone().ok_or_else(|| {
            crate::Error::RequestBuild("no client associated with request builder".into())
        })?;
        let timeout = self.timeout;
        let transport_hints = self.transport_hints.clone();
        let request = self.build()?;
        let options = NativeRequestOptions {
            timeout,
            transport_hints,
        };
        client.execute_http_body(request, options).await
    }
}
