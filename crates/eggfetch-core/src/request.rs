//! Request types and builder.

use bytes::Bytes;
use http::Version;

use crate::body::RequestBody;
use crate::client::Client;
use crate::error::Result;
use crate::headers::Headers;
use crate::response::Response;
use crate::timeout::Timeout;

/// An outgoing HTTP request.
#[derive(Debug)]
pub struct Request {
    method: http::Method,
    url: url::Url,
    headers: Headers,
    body: RequestBody,
    version: Version,
    timeout: Option<Timeout>,
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

    pub(crate) fn into_parts(
        self,
    ) -> (
        http::Method,
        url::Url,
        Headers,
        RequestBody,
        Version,
        Option<Timeout>,
    ) {
        (
            self.method,
            self.url,
            self.headers,
            self.body,
            self.version,
            self.timeout,
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
        client.send(request).await
    }
}
