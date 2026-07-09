//! Async client entry point.

use bytes::Bytes;
use http::Method;
use hyper_util::rt::TokioExecutor;
use std::sync::Arc;

use crate::body::ResponseBody;
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::request::Request;
use crate::request::RequestBuilder;
use crate::response::Response;

type Connector = hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>;
type HyperClient = hyper_util::client::legacy::Client<Connector, http_body_util::Full<Bytes>>;

/// Shared client configuration.
#[derive(Debug, Clone, Default)]
struct ClientConfig {
    default_headers: Headers,
    user_agent: Option<String>,
}

/// Async HTTP client.
///
/// The client manages connection pooling and shared configuration. Create one
/// with [`Client::new`] or [`Client::builder`].
#[derive(Clone)]
pub struct Client {
    inner: Arc<ClientInner>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").finish_non_exhaustive()
    }
}

struct ClientInner {
    hyper_client: HyperClient,
    config: ClientConfig,
}

impl Client {
    /// Create a new client with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Create a [`ClientBuilder`] for configuring a client.
    #[must_use]
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Create a GET request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid or uses an unsupported scheme.
    pub fn get(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::GET, url)
    }

    /// Create a POST request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid or uses an unsupported scheme.
    pub fn post(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::POST, url)
    }

    /// Create a PUT request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid or uses an unsupported scheme.
    pub fn put(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::PUT, url)
    }

    /// Create a PATCH request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid or uses an unsupported scheme.
    pub fn patch(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::PATCH, url)
    }

    /// Create a DELETE request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid or uses an unsupported scheme.
    pub fn delete(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::DELETE, url)
    }

    /// Create a HEAD request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid or uses an unsupported scheme.
    pub fn head(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::HEAD, url)
    }

    /// Create an OPTIONS request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid or uses an unsupported scheme.
    pub fn options(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::OPTIONS, url)
    }

    /// Create a request builder for the given method and URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid or uses an unsupported scheme.
    pub fn request(&self, method: Method, url: &str) -> Result<RequestBuilder> {
        let parsed = parse_url(url)?;
        Ok(RequestBuilder::new(self.clone(), method, parsed))
    }

    /// Send a request and return the response.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails at any stage (connect, TLS,
    /// protocol, body).
    pub(crate) async fn send(&self, request: Request) -> Result<Response> {
        let (method, url, headers, body, version) = request.into_parts();

        let uri: http::Uri = url
            .as_str()
            .parse()
            .map_err(|e| Error::InvalidUrl(format!("failed to convert url to URI: {e}")))?;

        let mut http_request = http::Request::builder()
            .method(method)
            .uri(uri)
            .version(version);

        // Apply default headers first, then request headers (overrides).
        for (name, value) in self.inner.config.default_headers.iter() {
            http_request = http_request.header(name, value);
        }
        for (name, value) in headers.iter() {
            http_request = http_request.header(name, value);
        }

        // Apply user-agent if set and not already present.
        if let Some(ref ua) = self.inner.config.user_agent {
            if !headers.contains("user-agent") {
                http_request = http_request.header(
                    http::header::USER_AGENT,
                    http::HeaderValue::from_str(ua)
                        .map_err(|e| Error::InvalidHeaderValue(e.to_string()))?,
                );
            }
        }

        let hyper_request = http_request
            .body(body.into_hyper_body())
            .map_err(|e| Error::RequestBuild(e.to_string()))?;

        let hyper_response = self
            .inner
            .hyper_client
            .request(hyper_request)
            .await
            .map_err(Error::HyperClient)?;

        let status = hyper_response.status();
        let resp_version = hyper_response.version();
        let resp_headers = hyper_response.headers().clone();
        let resp_url = url;

        let collected = http_body_util::BodyExt::collect(hyper_response.into_body())
            .await
            .map_err(|e| Error::Body(e.to_string()))?;

        let response_body = ResponseBody::from_bytes(collected.to_bytes());

        Ok(Response::new(
            status,
            resp_version,
            resp_headers,
            resp_url,
            response_body,
        ))
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for configuring a [`Client`].
pub struct ClientBuilder {
    default_headers: Headers,
    user_agent: Option<String>,
}

impl ClientBuilder {
    /// Create a new client builder with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            default_headers: Headers::new(),
            user_agent: None,
        }
    }

    /// Add a default header to all requests made by this client.
    ///
    /// # Errors
    ///
    /// Returns an error if the header name or value is invalid.
    pub fn default_header(mut self, name: &str, value: &str) -> Result<Self> {
        self.default_headers.insert(name, value)?;
        Ok(self)
    }

    /// Set the default user-agent header.
    #[must_use]
    pub fn user_agent(mut self, agent: &str) -> Self {
        self.user_agent = Some(agent.to_owned());
        self
    }

    /// Build the client.
    ///
    /// # Panics
    ///
    /// Panics if the system TLS root certificates cannot be loaded. This
    /// should not happen on any standard operating system.
    #[must_use]
    pub fn build(self) -> Client {
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .expect("failed to load native roots")
            .https_only()
            .enable_http1()
            .build();

        let hyper_client: HyperClient =
            hyper_util::client::legacy::Client::builder(TokioExecutor::new()).build(https);

        let config = ClientConfig {
            default_headers: self.default_headers,
            user_agent: self.user_agent,
        };

        Client {
            inner: Arc::new(ClientInner {
                hyper_client,
                config,
            }),
        }
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_url(url_str: &str) -> Result<url::Url> {
    let url = url::Url::parse(url_str).map_err(|e| Error::InvalidUrl(e.to_string()))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        other => Err(Error::Unsupported(format!(
            "URL scheme '{other}' is not supported; use http or https"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_constructs() {
        let _client = Client::new();
    }

    #[test]
    fn client_default() {
        let _client = Client::default();
    }

    #[test]
    fn client_builder() {
        let _client = Client::builder().user_agent("test-agent").build();
    }

    #[test]
    fn parse_valid_urls() {
        assert!(parse_url("https://example.com").is_ok());
        assert!(parse_url("http://localhost:8080").is_ok());
    }

    #[test]
    fn parse_invalid_schemes() {
        assert!(parse_url("ftp://example.com").is_err());
        assert!(parse_url("file:///tmp/test").is_err());
    }

    #[test]
    fn parse_invalid_urls() {
        assert!(parse_url("not a url").is_err());
    }

    #[test]
    fn get_request_builder() {
        let client = Client::new();
        let builder = client.get("https://example.com").unwrap();
        let req = builder.build().unwrap();
        assert_eq!(*req.method(), Method::GET);
    }
}
