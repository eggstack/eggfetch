//! Async client entry point.

use std::sync::Arc;
use std::time::Duration;

use http::Method;
use hyper_util::rt::TokioExecutor;

use crate::body::RequestBody;
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::pool::{OriginKey, Pool, PoolConfig, PoolMetrics};
#[cfg(feature = "proxy")]
use crate::proxy::{Proxy, ProxyConfig};
use crate::redirect::RedirectPolicy;
#[cfg(feature = "proxy")]
use crate::request::ProxyOverride;
use crate::request::{Request, RequestBuilder};
use crate::response::Response;
use crate::stream::write_timeout_stream;
use crate::timeout::{Timeout, TimeoutPhase};
#[cfg(feature = "proxy")]
use crate::transport::proxy::send_proxy_request;
use crate::transport::{Connector, HyperClient};

#[cfg(feature = "cookies")]
use crate::cookie::CookieJar;

/// Shared client configuration.
#[derive(Debug, Clone)]
pub(crate) struct ClientConfig {
    pub(crate) default_headers: Headers,
    pub(crate) user_agent: Option<String>,
    pub(crate) timeout: Option<Timeout>,
    pub(crate) redirect: RedirectPolicy,
    pub(crate) auth: Option<crate::auth::AuthScheme>,
    #[cfg(feature = "cookies")]
    pub(crate) cookie_jar: CookieJar,
    pub(crate) automatic_decompression: bool,
    pub(crate) max_decoded_body_size: Option<usize>,
    pub(crate) max_decompression_ratio: Option<f64>,
    #[cfg(feature = "proxy")]
    pub(crate) proxy: Option<Proxy>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            default_headers: Headers::new(),
            user_agent: None,
            timeout: None,
            redirect: RedirectPolicy::default(),
            auth: None,
            #[cfg(feature = "cookies")]
            cookie_jar: CookieJar::new(),
            automatic_decompression: true,
            max_decoded_body_size: None,
            max_decompression_ratio: None,
            #[cfg(feature = "proxy")]
            proxy: None,
        }
    }
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

pub(crate) struct ClientInner {
    pub(crate) hyper_client: HyperClient,
    pub(crate) config: ClientConfig,
    pub(crate) pool: Pool,
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
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn get(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::GET, url)
    }

    /// Create a POST request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn post(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::POST, url)
    }

    /// Create a PUT request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn put(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::PUT, url)
    }

    /// Create a PATCH request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn patch(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::PATCH, url)
    }

    /// Create a DELETE request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn delete(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::DELETE, url)
    }

    /// Create a HEAD request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn head(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::HEAD, url)
    }

    /// Create an OPTIONS request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn options(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::OPTIONS, url)
    }

    /// Create a request builder for the given method and URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn request(&self, method: Method, url: &str) -> Result<RequestBuilder> {
        let parsed = parse_url(url)?;
        Ok(RequestBuilder::new(self.clone(), method, parsed))
    }

    /// Returns a reference to the connection pool metrics.
    #[must_use]
    pub fn pool_metrics(&self) -> &PoolMetrics {
        self.inner.pool.metrics()
    }

    /// Returns a reference to the client's cookie jar.
    ///
    /// Only available when the `cookies` feature is enabled.
    #[cfg(feature = "cookies")]
    #[must_use]
    pub fn cookies(&self) -> &CookieJar {
        &self.inner.config.cookie_jar
    }

    /// Returns a reference to the shared client configuration.
    #[must_use]
    pub(crate) fn config(&self) -> &ClientConfig {
        &self.inner.config
    }

    /// Send a request and return the response, following redirects if
    /// the client's redirect policy allows.
    ///
    /// The redirect loop enforces `max_redirects`, performs method
    /// rewrites per HTTP semantics, strips sensitive headers on
    /// cross-origin hops, and records redirect history.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails at any stage (connect, TLS,
    /// protocol, body) or if a timeout elapses.
    pub(crate) async fn send(&self, request: Request) -> Result<Response> {
        crate::pipeline::send_with_redirects(self, request).await
    }

    /// Send a single HTTP request and return the streaming response.
    ///
    /// This handles pool acquisition, timeout application, and body
    /// processing for one request/response cycle. It does NOT handle
    /// redirects—that is the responsibility of [`Client::send`].
    #[allow(clippy::too_many_lines)]
    pub(crate) async fn send_single_request(
        &self,
        request: Request,
        timeout: &Timeout,
    ) -> Result<Response> {
        let (
            method,
            url,
            headers,
            body,
            version,
            _request_timeout,
            _request_redirect,
            _,
            _,
            request_decompress,
            proxy_override,
        ) = request.into_parts();

        // Determine effective decompression setting.
        let decompression_enabled =
            request_decompress.unwrap_or(self.inner.config.automatic_decompression);

        // Inject Accept-Encoding if automatic decompression is enabled
        // and the user has not supplied their own.
        let mut headers = headers;
        if decompression_enabled && !headers.contains("accept-encoding") {
            if let Some(value) = crate::compression::accept_encoding_value() {
                headers.insert("accept-encoding", value)?;
            }
        }

        // Determine effective proxy configuration.
        #[cfg(feature = "proxy")]
        let effective_proxy = self.resolve_proxy(&url, &proxy_override);
        #[cfg(not(feature = "proxy"))]
        {
            let _ = proxy_override;
        }
        #[cfg(not(feature = "proxy"))]
        let effective_proxy: Option<()> = None;

        #[cfg(feature = "proxy")]
        let origin = match effective_proxy {
            Some(ref proxy_config) => {
                let is_tunnel = url.scheme() == "https";
                OriginKey::from_url_with_proxy(
                    url.scheme(),
                    &url,
                    proxy_config.host(),
                    Some(proxy_config.port()),
                    is_tunnel,
                )
            }
            None => OriginKey::from_url(url.scheme(), &url),
        };
        #[cfg(not(feature = "proxy"))]
        let origin = OriginKey::from_url(url.scheme(), &url);

        let started = std::time::Instant::now();
        let pool_deadline = match (timeout.pool, timeout.total) {
            (Some(pool), Some(total)) if total < pool => Some((total, TimeoutPhase::Total)),
            (Some(pool), _) => Some((pool, TimeoutPhase::Pool)),
            (None, Some(total)) => Some((total, TimeoutPhase::Total)),
            (None, None) => None,
        };
        let guard = match pool_deadline {
            Some((duration, phase)) => {
                match tokio::time::timeout(duration, self.inner.pool.acquire(origin.as_ref())).await
                {
                    Ok(guard) => guard,
                    Err(_) => {
                        return Err(Error::Timeout {
                            phase,
                            elapsed: duration,
                        })
                    }
                }
            }
            None => self.inner.pool.acquire(origin.as_ref()).await,
        };

        let body = match (body, timeout.write) {
            (RequestBody::Stream { stream, length }, Some(write_dur)) => {
                let wrapped = write_timeout_stream(stream, write_dur);
                RequestBody::Stream {
                    stream: wrapped,
                    length,
                }
            }
            (b, _) => b,
        };

        let headers = crate::pipeline::apply_content_length(headers, &body)?;

        // Add user-agent if not already set.
        let mut headers = headers;
        if let Some(ref ua) = self.inner.config.user_agent {
            if !headers.contains("user-agent") {
                headers.insert(http::header::USER_AGENT.as_str(), ua.as_str())?;
            }
        }

        // Send through proxy or directly.
        let remaining_total = timeout
            .total
            .map(|total| total.saturating_sub(started.elapsed()));

        let response = match effective_proxy {
            #[cfg(feature = "proxy")]
            Some(ref proxy_config) => {
                if headers.contains("proxy-authorization") && proxy_config.auth().is_some() {
                    return Err(Error::ConflictingAuth(
                        "conflict: both request Proxy-Authorization header and proxy auth are configured; remove one".into(),
                    ));
                }
                send_proxy_request(
                    &url,
                    &method,
                    &headers,
                    body,
                    version,
                    proxy_config,
                    remaining_total,
                )
                .await?
            }
            _ => {
                let uri: http::Uri = url
                    .as_str()
                    .parse()
                    .map_err(|e| Error::InvalidUrl(format!("failed to convert url to URI: {e}")))?;

                let mut http_request = http::Request::builder()
                    .method(method)
                    .uri(uri)
                    .version(version);

                for (name, value) in headers.iter() {
                    http_request = http_request.header(name, value);
                }

                let hyper_request = http_request
                    .body(body.into_http_body())
                    .map_err(|e| Error::RequestBuild(e.to_string()))?;

                let send_future = crate::transport::direct::send_request(
                    &self.inner.hyper_client,
                    hyper_request,
                    url.clone(),
                );

                match remaining_total {
                    Some(dur) => match tokio::time::timeout(dur, send_future).await {
                        Ok(Ok(resp)) => resp,
                        Ok(Err(e)) => return Err(e),
                        Err(_) => {
                            return Err(Error::Timeout {
                                phase: TimeoutPhase::Total,
                                elapsed: dur,
                            });
                        }
                    },
                    None => send_future.await?,
                }
            }
        };

        let mut response = response;

        // Apply decompression if enabled.
        if decompression_enabled {
            let content_encoding = response
                .headers()
                .get("content-encoding")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);

            let limit = crate::compression::DecompressionLimit {
                max_decoded_body_size: self.inner.config.max_decoded_body_size,
                max_decompression_ratio: self.inner.config.max_decompression_ratio,
            };

            response = crate::response_decode::apply_decompression(
                response,
                content_encoding.as_deref(),
                limit,
            )?;
        }

        crate::pipeline::apply_read_timeout_and_lease(&mut response, guard, timeout.read);

        Ok(response)
    }

    /// Resolve the effective proxy configuration for a request.
    ///
    /// Applies the tri-state override model:
    /// - `Inherit`: use client-level proxy
    /// - `Direct`: direct, no proxy
    /// - `Override(config)`: use request-level proxy
    #[cfg(feature = "proxy")]
    fn resolve_proxy(&self, url: &url::Url, proxy_override: &ProxyOverride) -> Option<ProxyConfig> {
        match proxy_override {
            ProxyOverride::Override(config) => Some(config.clone()),
            ProxyOverride::Direct => None,
            ProxyOverride::Inherit => self
                .inner
                .config
                .proxy
                .as_ref()
                .filter(|p| p.should_use_for_scheme(url.scheme()))
                .filter(|p| p.no_proxy_rules().map_or(true, |np| !np.should_bypass(url)))
                .map(Proxy::config),
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
    redirect: RedirectPolicy,
    auth: Option<crate::auth::AuthScheme>,
    #[cfg(feature = "cookies")]
    cookie_jar: Option<CookieJar>,
    automatic_decompression: Option<bool>,
    max_decoded_body_size: Option<usize>,
    max_decompression_ratio: Option<f64>,
    #[cfg(feature = "proxy")]
    proxy: Option<Proxy>,
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
            redirect: RedirectPolicy::default(),
            auth: None,
            #[cfg(feature = "cookies")]
            cookie_jar: None,
            automatic_decompression: None,
            max_decoded_body_size: None,
            max_decompression_ratio: None,
            #[cfg(feature = "proxy")]
            proxy: None,
        }
    }

    /// Add a default header to all requests made by this client.
    ///
    /// # Errors
    ///
    /// Returns an error if `name` or `value` is not a valid header field.
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
    #[must_use]
    pub fn timeout(mut self, timeout: Timeout) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the redirect policy for this client.
    #[must_use]
    pub fn follow_redirects(mut self, follow: bool) -> Self {
        self.redirect.follow = follow;
        self
    }

    /// Set the maximum number of redirects to follow.
    #[must_use]
    pub fn max_redirects(mut self, max: usize) -> Self {
        self.redirect.max_redirects = max;
        self
    }

    /// Set the full redirect policy.
    #[must_use]
    pub fn redirect_policy(mut self, policy: RedirectPolicy) -> Self {
        self.redirect = policy;
        self
    }

    /// Set a shared cookie jar for this client.
    ///
    /// When set, the client will automatically inject matching cookies
    /// into requests and update the jar from `Set-Cookie` response headers.
    ///
    /// Only available when the `cookies` feature is enabled.
    #[cfg(feature = "cookies")]
    #[must_use]
    pub fn cookie_jar(mut self, jar: CookieJar) -> Self {
        self.cookie_jar = Some(jar);
        self
    }

    /// Set default authentication for all requests made by this client.
    ///
    /// The configured auth is applied to every request unless overridden
    /// or disabled at the request level. Auth is recomputed per redirect
    /// hop; cross-origin redirects never carry client auth.
    #[must_use]
    pub fn auth(mut self, auth: impl Into<crate::auth::AuthScheme>) -> Self {
        self.auth = Some(auth.into());
        self
    }

    /// Set the default proxy for all requests made by this client.
    ///
    /// When set, all matching requests are routed through the specified
    /// proxy. Can be overridden or disabled per-request.
    #[cfg(feature = "proxy")]
    #[must_use]
    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.proxy = Some(proxy);
        self
    }

    /// Set `NO_PROXY` bypass rules for the default proxy.
    ///
    /// When set, URLs matching any bypass rule are sent directly
    /// without going through the proxy.
    #[cfg(feature = "proxy")]
    #[must_use]
    pub fn no_proxy(mut self, no_proxy: crate::proxy::NoProxy) -> Self {
        if let Some(proxy) = self.proxy.take() {
            self.proxy = Some(proxy.no_proxy(no_proxy));
        }
        self
    }

    /// Enable or disable automatic response decompression.
    ///
    /// When enabled (the default), the client sends an
    /// `Accept-Encoding` header and transparently decompresses
    /// response bodies. Decoded `Content-Encoding` and
    /// `Content-Length` headers are removed from the response.
    ///
    /// Can be overridden per-request via
    /// [`RequestBuilder::decompress`].
    #[must_use]
    pub fn automatic_decompression(mut self, enabled: bool) -> Self {
        self.automatic_decompression = Some(enabled);
        self
    }

    /// Build the client.
    ///
    /// Native system roots are preferred. If the platform root store is
    /// unavailable, the client falls back to the packaged Mozilla root set
    /// while retaining certificate and hostname verification.
    #[must_use]
    pub fn build(self) -> Client {
        let https = match hyper_rustls::HttpsConnectorBuilder::new().with_native_roots() {
            Ok(builder) => builder.https_or_http().enable_http1().build(),
            Err(_) => build_webpki_connector(),
        };

        let mut builder = hyper_util::client::legacy::Client::builder(TokioExecutor::new());
        if let Some(timeout) = self.pool_config.idle_timeout {
            builder.pool_idle_timeout(timeout);
        }
        let hyper_client: HyperClient = builder.build(https);

        #[cfg(feature = "cookies")]
        let cookie_jar = self.cookie_jar.unwrap_or_default();

        let automatic_decompression = self.automatic_decompression.unwrap_or(true);

        let config = ClientConfig {
            default_headers: self.default_headers,
            user_agent: self.user_agent,
            timeout: self.timeout,
            redirect: self.redirect,
            auth: self.auth,
            #[cfg(feature = "cookies")]
            cookie_jar,
            automatic_decompression,
            max_decoded_body_size: self.max_decoded_body_size,
            max_decompression_ratio: self.max_decompression_ratio,
            #[cfg(feature = "proxy")]
            proxy: self.proxy,
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

/// Build a connector with the packaged Mozilla root set.
///
/// Kept as a separate function so the fallback construction path can be
/// exercised without depending on the host's native trust store.
fn build_webpki_connector() -> Connector {
    hyper_rustls::HttpsConnectorBuilder::new()
        .with_webpki_roots()
        .https_or_http()
        .enable_http1()
        .build()
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_url(url_str: &str) -> Result<url::Url> {
    let url = url::Url::parse(url_str).map_err(|e| Error::InvalidUrl(e.to_string()))?;
    if !url.username().is_empty() || url.password().is_some() {
        // URL userinfo is both easy to leak through diagnostics and
        // surprising for an HTTP client with explicit auth APIs.
        return Err(Error::InvalidUrl(
            "URL userinfo is not supported; configure authentication explicitly".into(),
        ));
    }
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
    #[cfg(feature = "proxy")]
    use crate::proxy::ProxyAuth;
    use bytes::Bytes;

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
    fn tls_root_store_paths_construct() {
        // Native roots are environment-dependent, but when available the
        // production path must build the same verified connector shape.
        if let Ok(builder) = hyper_rustls::HttpsConnectorBuilder::new().with_native_roots() {
            let _ = builder.https_or_http().enable_http1().build();
        }
        let _ = build_webpki_connector();
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

    #[test]
    fn apply_content_length_empty_body() {
        let headers = Headers::new();
        let body = crate::body::RequestBody::Empty;
        let out = crate::pipeline::apply_content_length(headers, &body).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "0");
    }

    #[test]
    fn apply_content_length_bytes_body() {
        let headers = Headers::new();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let out = crate::pipeline::apply_content_length(headers, &body).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "5");
    }

    #[test]
    fn apply_content_length_stream_known() {
        let headers = Headers::new();
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = crate::body::RequestBody::from_stream(stream, Some(7));
        let out = crate::pipeline::apply_content_length(headers, &body).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "7");
    }

    #[test]
    fn apply_content_length_stream_unknown() {
        let headers = Headers::new();
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = crate::body::RequestBody::from_stream(stream, None);
        let out = crate::pipeline::apply_content_length(headers, &body).unwrap();
        assert!(out.get("content-length").is_none());
    }

    #[test]
    fn apply_content_length_user_matches() {
        let mut headers = Headers::new();
        headers.insert("content-length", "5").unwrap();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let out = crate::pipeline::apply_content_length(headers, &body).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "5");
    }

    #[test]
    fn apply_content_length_user_mismatch_errors() {
        let mut headers = Headers::new();
        headers.insert("content-length", "10").unwrap();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let err = crate::pipeline::apply_content_length(headers, &body).unwrap_err();
        assert_eq!(err.kind(), "request_build");
    }

    #[test]
    fn apply_content_length_rejects_invalid_value() {
        let mut headers = Headers::new();
        headers.insert("content-length", "not-a-number").unwrap();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let err = crate::pipeline::apply_content_length(headers, &body).unwrap_err();
        assert_eq!(err.kind(), "invalid_header_value");
    }

    #[test]
    fn apply_content_length_rejects_unknown_stream_override() {
        let mut headers = Headers::new();
        headers.insert("content-length", "5").unwrap();
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = crate::body::RequestBody::from_stream(stream, None);
        let err = crate::pipeline::apply_content_length(headers, &body).unwrap_err();
        assert_eq!(err.kind(), "request_build");
    }

    #[test]
    fn parse_url_rejects_userinfo_without_echoing_credentials() {
        let err = parse_url("https://user:secret@example.com").unwrap_err();
        assert_eq!(err.kind(), "invalid_url");
        assert!(!err.to_string().contains("secret"));
    }

    #[cfg(feature = "proxy")]
    #[tokio::test]
    async fn proxy_auth_conflict_with_header() {
        let proxy = Proxy::all("http://proxy.example:8080")
            .unwrap()
            .auth(ProxyAuth::basic("user", "pass").unwrap());
        let client = Client::builder().proxy(proxy).build();
        let request = client
            .get("http://destination.example")
            .unwrap()
            .header("proxy-authorization", "Basic dXNlcjpwYXNz")
            .build()
            .unwrap();
        let err = client.send(request).await.unwrap_err();
        assert_eq!(err.kind(), "conflicting_auth");
        assert!(err.to_string().contains("Proxy-Authorization"));
    }

    #[cfg(feature = "proxy")]
    #[tokio::test]
    async fn proxy_auth_no_conflict_without_header() {
        let proxy = Proxy::all("http://proxy.example:8080")
            .unwrap()
            .auth(ProxyAuth::basic("user", "pass").unwrap());
        let client = Client::builder().proxy(proxy).build();
        let request = client
            .get("http://destination.example")
            .unwrap()
            .build()
            .unwrap();
        let err = client.send(request).await.unwrap_err();
        assert_ne!(err.kind(), "conflicting_auth");
    }

    #[cfg(feature = "proxy")]
    #[tokio::test]
    async fn proxy_auth_no_conflict_with_header_only() {
        let proxy = Proxy::all("http://proxy.example:8080").unwrap();
        let client = Client::builder().proxy(proxy).build();
        let request = client
            .get("http://destination.example")
            .unwrap()
            .header("proxy-authorization", "Basic dXNlcjpwYXNz")
            .build()
            .unwrap();
        let err = client.send(request).await.unwrap_err();
        assert_ne!(err.kind(), "conflicting_auth");
    }
}
