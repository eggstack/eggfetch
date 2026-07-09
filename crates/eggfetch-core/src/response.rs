//! Incoming response type.

use bytes::Bytes;
use http::{HeaderMap, StatusCode, Version};
use url::Url;

use crate::body::ResponseBody;
use crate::error::Result;

/// An HTTP response.
#[derive(Debug)]
pub struct Response {
    status: StatusCode,
    version: Version,
    headers: HeaderMap,
    url: Url,
    body: ResponseBody,
}

impl Response {
    /// Create a new response (crate-internal).
    pub(crate) fn new(
        status: StatusCode,
        version: Version,
        headers: HeaderMap,
        url: Url,
        body: ResponseBody,
    ) -> Self {
        Self {
            status,
            version,
            headers,
            url,
            body,
        }
    }

    /// Returns the HTTP status code.
    #[must_use]
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Returns the HTTP version.
    #[must_use]
    pub fn version(&self) -> Version {
        self.version
    }

    /// Returns the response headers.
    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns the final URL of the response (after any redirects).
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Returns `true` if the status code indicates success (2xx).
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    /// Consume the response and return the body bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the body cannot be read.
    pub fn bytes(self) -> Result<Bytes> {
        self.body.bytes()
    }

    /// Consume the response and return the body as a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns an error if the body is not valid UTF-8.
    pub fn text(self) -> Result<String> {
        self.body.text()
    }
}
