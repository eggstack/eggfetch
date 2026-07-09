//! Async client entry point.

use bytes::Bytes;
use http::Method;
use hyper_util::rt::TokioExecutor;
use std::sync::Arc;
use std::time::Duration;

use crate::body::ResponseBody;
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::pool::{Pool, PoolConfig, PoolMetrics};
use crate::request::Request;
use crate::request::RequestBuilder;
use crate::response::Response;
use crate::timeout::{Timeout, TimeoutPhase};

type Connector = hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>;
type HyperClient = hyper_util::client::legacy::Client<Connector, http_body_util::Full<Bytes>>;

/// Shared client configuration.
#[derive(Debug, Clone, Default)]
struct ClientConfig {
    default_headers: Headers,
    user_agent: Option<String>,
    timeout: Option<Timeout>,
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
    pool: Pool,
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

    /// Returns a reference to the connection pool metrics.
    #[must_use]
    pub fn pool_metrics(&self) -> &PoolMetrics {
        self.inner.pool.metrics()
    }

    /// Send a request and return the response.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails at any stage (connect, TLS,
    /// protocol, body) or if a timeout elapses.
    pub(crate) async fn send(&self, request: Request) -> Result<Response> {
        let (method, url, headers, body, version, request_timeout) = request.into_parts();

        // Merge client-level and request-level timeouts.
        let timeout = match self.inner.config.timeout {
            Some(client_timeout) => client_timeout.merge(request_timeout),
            None => request_timeout.unwrap_or_default(),
        };

        let uri: http::Uri = url
            .as_str()
            .parse()
            .map_err(|e| Error::InvalidUrl(format!("failed to convert url to URI: {e}")))?;

        // Extract host from URL for pool slot acquisition.
        let host = url.host_str().map(str::to_owned);

        // Acquire a pool slot, respecting pool timeout.
        let _guard = match timeout.pool {
            Some(dur) => {
                match tokio::time::timeout(dur, self.inner.pool.acquire(host.as_deref())).await {
                    Ok(guard) => guard,
                    Err(_) => {
                        return Err(Error::Timeout {
                            phase: TimeoutPhase::Pool,
                            elapsed: dur,
                        });
                    }
                }
            }
            None => self.inner.pool.acquire(host.as_deref()).await,
        };

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

        // Send request and collect response. For buffered requests, we apply
        // the total timeout across the entire send+collect lifecycle.
        //
        // Phase-specific connect/write/read timeouts are not individually
        // isolable through hyper-util's legacy client API. The total timeout
        // provides the wall-clock guarantee; individual phase errors would
        // surface as hyper I/O errors without phase identity. This is a
        // documented limitation of the current implementation.
        //
        // The pool timeout is applied separately above.
        let send_and_collect = async {
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

            Ok::<Response, Error>(Response::new(
                status,
                resp_version,
                resp_headers,
                resp_url,
                response_body,
            ))
        };

        match timeout.total {
            Some(dur) => tokio::time::timeout(dur, send_and_collect)
                .await
                .map_err(|_| Error::Timeout {
                    phase: TimeoutPhase::Total,
                    elapsed: dur,
                })?,
            None => send_and_collect.await,
        }
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
    pool_config: PoolConfig,
    timeout: Option<Timeout>,
}

impl ClientBuilder {
    /// Create a new client builder with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            default_headers: Headers::new(),
            user_agent: None,
            pool_config: PoolConfig::default(),
            timeout: None,
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

    /// Set the maximum number of idle (unused) connections in the pool.
    #[must_use]
    pub fn max_idle_connections(mut self, max: usize) -> Self {
        self.pool_config.max_idle_connections = Some(max);
        self
    }

    /// Set the maximum number of idle connections per individual host.
    #[must_use]
    pub fn max_idle_connections_per_host(mut self, max: usize) -> Self {
        self.pool_config.max_idle_connections_per_host = Some(max);
        self
    }

    /// Set the maximum total number of concurrent connections.
    #[must_use]
    pub fn max_connections(mut self, max: usize) -> Self {
        self.pool_config.max_connections = Some(max);
        self
    }

    /// Set the maximum number of concurrent connections per individual host.
    #[must_use]
    pub fn max_connections_per_host(mut self, max: usize) -> Self {
        self.pool_config.max_connections_per_host = Some(max);
        self
    }

    /// Set the duration after which idle connections are closed.
    #[must_use]
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.pool_config.idle_timeout = Some(timeout);
        self
    }

    /// Set the default timeout for all requests made by this client.
    ///
    /// Request-level timeouts override client-level timeouts on a
    /// per-field basis.
    #[must_use]
    pub fn timeout(mut self, timeout: Timeout) -> Self {
        self.timeout = Some(timeout);
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
            .https_or_http()
            .enable_http1()
            .build();

        let mut builder = hyper_util::client::legacy::Client::builder(TokioExecutor::new());
        if let Some(timeout) = self.pool_config.idle_timeout {
            builder.pool_idle_timeout(timeout);
        }
        let hyper_client: HyperClient = builder.build(https);

        let config = ClientConfig {
            default_headers: self.default_headers,
            user_agent: self.user_agent,
            timeout: self.timeout,
        };

        let pool = Pool::new(self.pool_config);

        Client {
            inner: Arc::new(ClientInner {
                hyper_client,
                config,
                pool,
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
