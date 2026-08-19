//! Async client entry point.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use http::Method;
use hyper_util::rt::TokioExecutor;

use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::http_version::HttpVersionPolicy;
use crate::limits::Limits;
use crate::pool::{Pool, PoolConfig, PoolMetrics};
#[cfg(feature = "proxy")]
use crate::proxy::Proxy;
use crate::redirect::RedirectPolicy;
use crate::request::{Request, RequestBuilder};
use crate::response::Response;
use crate::retry::RetryPolicy;
use crate::timeout::Timeout;
use crate::transport::Connector;

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
    #[cfg(feature = "proxy")]
    pub(crate) environment_proxies: Vec<Proxy>,
    #[cfg(any(feature = "proxy", feature = "http3"))]
    pub(crate) tls_config: Option<crate::tls::TlsConfig>,
    pub(crate) retry: Option<RetryPolicy>,
    #[allow(
        dead_code,
        reason = "stored for inspection and future request-level use"
    )]
    pub(crate) http_version_policy: HttpVersionPolicy,
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
            #[cfg(feature = "proxy")]
            environment_proxies: Vec::new(),
            #[cfg(any(feature = "proxy", feature = "http3"))]
            tls_config: None,
            retry: None,
            http_version_policy: HttpVersionPolicy::default(),
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
    pub(crate) hyper_client: Option<crate::transport::TimeoutHyperClient>,
    /// Direct connector for requests with advanced socket options or local
    /// address binding. Uses a custom connector instead of the standard
    /// hyper-rustls connector path.
    pub(crate) direct_client: Option<crate::transport::TimeoutDirectClient>,
    /// Cached hyper clients keyed by TLS SNI hostname override.
    ///
    /// When a request carries `TransportHints.sni_hostname`, the pipeline
    /// uses a DirectConnector-based client from this cache. The connector
    /// separates DNS/TCP (to the original URL host) from TLS (with the
    /// SNI override hostname), keeping the default path unchanged.
    pub(crate) sni_clients: Mutex<HashMap<String, crate::transport::TimeoutDirectClient>>,
    /// Hyper client for Unix domain socket requests.
    #[cfg(unix)]
    pub(crate) uds_client: Option<crate::transport::TimeoutUdsClient>,
    /// Persistent Hyper clients keyed by effective SOCKS route.
    #[cfg(feature = "proxy")]
    pub(crate) socks_clients: Mutex<
        HashMap<crate::transport::socks::SocksRouteKey, crate::transport::TimeoutSocksClient>,
    >,
    pub(crate) config: ClientConfig,
    pub(crate) pool: Pool,
    #[cfg(feature = "http3")]
    pub(crate) h3_connector: Option<crate::transport::http3::H3Connector>,
}

impl ClientInner {
    /// Get or create a cached hyper client with TLS SNI hostname override.
    ///
    /// The returned client uses a [`DirectConnector`](crate::transport::direct_connector::DirectConnector)
    /// that separates DNS/TCP resolution (to the original URL host) from
    /// TLS negotiation (with the SNI hostname). Clients are cached by
    /// SNI hostname for connection reuse.
    pub(crate) fn sni_client(
        &self,
        sni_hostname: &str,
    ) -> Result<crate::transport::TimeoutDirectClient> {
        let mut clients = self
            .sni_clients
            .lock()
            .map_err(|_| Error::ProxyConnect("SNI client cache is poisoned".into()))?;
        if let Some(client) = clients.get(sni_hostname) {
            return Ok(client.clone());
        }

        let connect_timeout = self.config.timeout.as_ref().and_then(|t| t.connect);

        #[cfg(any(feature = "proxy", feature = "http3"))]
        let tls_connector = match self.config.tls_config.as_ref() {
            Some(tls_config) => match tls_config.build_rustls_config() {
                Ok(rc) => Some(tokio_rustls::TlsConnector::from(Arc::new(rc))),
                Err(_) => None,
            },
            None => None,
        };
        #[cfg(not(any(feature = "proxy", feature = "http3")))]
        let tls_connector = None;

        let base_connector = crate::transport::direct_connector::DirectConnector::new(
            crate::transport::direct_connector::DirectConnectorConfig {
                local_address: None,
                socket_options: Vec::new(),
            },
            tls_connector,
        );
        let sni_connector = base_connector.with_sni(sni_hostname.to_owned());
        let connector =
            crate::transport::connect_timeout::ConnectTimeout::new(sni_connector, connect_timeout);

        let mut builder =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new());
        if let Some(idle_timeout) = self.config.timeout.as_ref().and_then(|t| t.pool).or(self
            .config
            .timeout
            .as_ref()
            .and_then(|t| t.total))
        {
            builder.pool_idle_timeout(idle_timeout);
        }
        let client = builder.build(connector);
        clients.insert(sni_hostname.to_owned(), client.clone());
        Ok(client)
    }
}

#[cfg(feature = "proxy")]
impl ClientInner {
    pub(crate) fn socks_client(
        &self,
        proxy: &crate::proxy::ProxyConfig,
    ) -> Result<crate::transport::TimeoutSocksClient> {
        let key = crate::transport::socks::SocksRouteKey::from_proxy(proxy);
        let mut clients = self
            .socks_clients
            .lock()
            .map_err(|_| Error::ProxyConnect("SOCKS client cache is poisoned".into()))?;
        if let Some(client) = clients.get(&key) {
            return Ok(client.clone());
        }

        let tls_config = if let Some(config) = self.config.tls_config.as_ref() {
            config
                .build_rustls_config()
                .map_err(|e| Error::Tls(format!("failed to build SOCKS TLS config: {e}")))?
        } else {
            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth()
        };
        let connector = crate::transport::socks::SocksConnector::new(
            proxy.clone(),
            Some(tokio_rustls::TlsConnector::from(Arc::new(tls_config))),
            None,
        );
        let connector = crate::transport::connect_timeout::ConnectTimeout::new(
            connector,
            self.config
                .timeout
                .as_ref()
                .and_then(|timeout| timeout.connect),
        );
        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build(connector);
        clients.insert(key, client.clone());
        Ok(client)
    }
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
    /// If a retry policy is configured (on the client or request), the
    /// entire logical request is retried on failure according to the
    /// policy.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails at any stage (connect, TLS,
    /// protocol, body) or if a timeout elapses.
    pub(crate) async fn send(&self, request: Request) -> Result<Response> {
        Box::pin(crate::pipeline::send_with_retry(self, request)).await
    }

    /// Send a single HTTP request and return the streaming response.
    ///
    /// This handles pool acquisition, timeout application, and body
    /// processing for one request/response cycle. It does NOT handle
    /// redirects—that is the responsibility of [`Client::send`].
    pub(crate) async fn send_single_request(
        &self,
        request: Request,
        timeout: &Timeout,
    ) -> Result<Response> {
        crate::pipeline::send_single_request(&self.inner, request, timeout).await
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
    limits: Option<Limits>,
    redirect: RedirectPolicy,
    auth: Option<crate::auth::AuthScheme>,
    #[cfg(feature = "cookies")]
    cookie_jar: Option<CookieJar>,
    automatic_decompression: Option<bool>,
    max_decoded_body_size: Option<usize>,
    max_decompression_ratio: Option<f64>,
    #[cfg(feature = "proxy")]
    proxy: Option<Proxy>,
    #[cfg(feature = "proxy")]
    environment_proxies: Vec<Proxy>,
    tls_config: Option<crate::tls::TlsConfig>,
    retry: Option<RetryPolicy>,
    http_version_policy: HttpVersionPolicy,
    /// Advanced direct-connector config for socket options / local address.
    direct_connector_config: Option<crate::transport::direct_connector::DirectConnectorConfig>,
    /// Unix domain socket path. When set, all requests use UDS transport.
    uds_path: Option<String>,
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
            limits: None,
            redirect: RedirectPolicy::default(),
            auth: None,
            #[cfg(feature = "cookies")]
            cookie_jar: None,
            automatic_decompression: None,
            max_decoded_body_size: None,
            max_decompression_ratio: None,
            #[cfg(feature = "proxy")]
            proxy: None,
            #[cfg(feature = "proxy")]
            environment_proxies: Vec::new(),
            tls_config: None,
            retry: None,
            http_version_policy: HttpVersionPolicy::default(),
            direct_connector_config: None,
            uds_path: None,
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

    /// Set resource limits for the connection pool.
    ///
    /// Limits control logical request concurrency (pool permits) and
    /// physical connection behavior (idle connection caps and expiry).
    /// When set, the limits are applied to the pool configuration.
    #[must_use]
    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = Some(limits);
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

    /// Add a scheme-specific proxy used when no explicit proxy is configured.
    #[cfg(feature = "proxy")]
    #[must_use]
    pub fn environment_proxy(mut self, proxy: Proxy) -> Self {
        self.environment_proxies.push(proxy);
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

    /// Set a custom TLS configuration for this client.
    ///
    /// When set, the client uses the provided [`crate::tls::TlsConfig`] for all
    /// TLS connections instead of the default native-root-with-webpki-fallback
    /// strategy.
    #[must_use]
    pub fn tls_config(mut self, config: crate::tls::TlsConfig) -> Self {
        self.tls_config = Some(config);
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

    /// Set the maximum decoded response body size in bytes.
    /// When set, responses whose decompressed body exceeds this limit
    /// produce an error instead of buffering the full content.
    #[must_use]
    pub fn max_decoded_body_size(mut self, max: usize) -> Self {
        self.max_decoded_body_size = Some(max);
        self
    }

    /// Set the maximum decompression ratio (decoded bytes / compressed bytes).
    /// When set, responses whose expansion ratio exceeds this limit produce
    /// an error. This guards against zip-bomb style attacks.
    #[must_use]
    pub fn max_decompression_ratio(mut self, ratio: f64) -> Self {
        self.max_decompression_ratio = Some(ratio);
        self
    }

    /// Set the retry policy for this client.
    ///
    /// When set, failed requests that match the policy (safe methods,
    /// retryable statuses/errors, replayable bodies) are automatically
    /// retried with exponential backoff.
    #[must_use]
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    /// Set the HTTP version policy for this client.
    ///
    /// Controls which HTTP protocol versions the client may negotiate.
    /// The default is [`HttpVersionPolicy::Auto`], which allows HTTP/2
    /// negotiation via ALPN when the `http2` feature is enabled.
    ///
    /// When the `http2` feature is not compiled in, `Http2Only` and `Auto`
    /// are silently downgraded to `Http1Only`.
    #[must_use]
    pub fn http_version_policy(mut self, policy: HttpVersionPolicy) -> Self {
        self.http_version_policy = policy;
        self
    }

    /// Set a local address to bind outbound sockets to before connecting.
    ///
    /// When set, all direct (non-proxy, non-UDS) connections will bind
    /// to the specified local address before connecting to the remote.
    /// This uses a custom connector path instead of the standard
    /// hyper-rustls connector.
    #[must_use]
    pub fn local_address(mut self, addr: std::net::SocketAddr) -> Self {
        let config = self.direct_connector_config.get_or_insert_with(|| {
            crate::transport::direct_connector::DirectConnectorConfig {
                local_address: None,
                socket_options: Vec::new(),
            }
        });
        config.local_address = Some(addr);
        self
    }

    /// Set socket options to apply to outbound TCP connections.
    ///
    /// Options are applied to the socket before the connect operation.
    /// Recognized options are applied via `tokio::net::TcpSocket` setters;
    /// unrecognized options produce a connection error (never silently
    /// ignored, per the plan's Track 2.4 requirement).
    #[must_use]
    pub fn socket_options(
        mut self,
        options: Vec<crate::transport::direct_connector::SocketOption>,
    ) -> Self {
        let config = self.direct_connector_config.get_or_insert_with(|| {
            crate::transport::direct_connector::DirectConnectorConfig {
                local_address: None,
                socket_options: Vec::new(),
            }
        });
        config.socket_options = options;
        self
    }

    /// Set a Unix domain socket path for all connections.
    ///
    /// When set, all requests are routed through the specified UDS path
    /// instead of making TCP connections. The URL in the request still
    /// provides HTTP scheme/authority/path semantics.
    ///
    /// Only supported on Unix platforms. On non-Unix platforms, this
    /// option is accepted but will produce an error at request time.
    #[must_use]
    pub fn uds_path(mut self, path: String) -> Self {
        self.uds_path = Some(path);
        self
    }

    /// Build the client.
    ///
    /// Native system roots are preferred. If the platform root store is
    /// unavailable, the client falls back to the packaged Mozilla root set
    /// while retaining certificate and hostname verification.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn build(self) -> Client {
        use crate::http_version::HttpVersionPolicyEnabler;
        let enabler = HttpVersionPolicyEnabler::from_policy(self.http_version_policy);

        let mut pool_config = self.pool_config;
        if let Some(limits) = self.limits {
            let limits_config: PoolConfig = limits.into();
            if limits_config.max_connections.is_some() {
                pool_config.max_connections = limits_config.max_connections;
            }
            if limits_config.max_connections_per_host.is_some() {
                pool_config.max_connections_per_host = limits_config.max_connections_per_host;
            }
            if limits_config.max_idle_connections.is_some() {
                pool_config.max_idle_connections = limits_config.max_idle_connections;
            }
            if limits_config.max_idle_connections_per_host.is_some() {
                pool_config.max_idle_connections_per_host =
                    limits_config.max_idle_connections_per_host;
            }
            if limits_config.idle_timeout.is_some() {
                pool_config.idle_timeout = limits_config.idle_timeout;
            }
        }

        #[cfg(feature = "cookies")]
        let cookie_jar = self.cookie_jar.unwrap_or_default();

        let automatic_decompression = self.automatic_decompression.unwrap_or(true);

        // When HTTP/3 is selected, we skip building the hyper client
        let hyper_client = if enabler.use_http3() {
            None
        } else {
            let https = match self.tls_config.as_ref() {
                Some(tls_config) => match tls_config.build_rustls_config() {
                    Ok(mut rc) => {
                        // hyper-rustls requires empty ALPN; it rebuilds based
                        // on enable_http1/enable_http2 calls.
                        rc.alpn_protocols.clear();
                        let builder = hyper_rustls::HttpsConnectorBuilder::new()
                            .with_tls_config(rc)
                            .https_or_http();
                        #[cfg(feature = "http2")]
                        {
                            match (enabler.enable_http1(), enabler.enable_http2()) {
                                (true, true) => builder.enable_http1().enable_http2().build(),
                                (true, false) => builder.enable_http1().build(),
                                (false, true) => builder.enable_http2().build(),
                                (false, false) => {
                                    unreachable!("at least one protocol version must be enabled")
                                }
                            }
                        }
                        #[cfg(not(feature = "http2"))]
                        {
                            let _ = enabler;
                            builder.enable_http1().build()
                        }
                    }
                    Err(_) => build_fallback_connector(enabler),
                },
                None => build_fallback_connector(enabler),
            };

            // Wrap the connector with connect-phase timeout if configured.
            let connect_timeout = self.timeout.as_ref().and_then(|t| t.connect);
            let https =
                crate::transport::connect_timeout::ConnectTimeout::new(https, connect_timeout);

            let mut builder = hyper_util::client::legacy::Client::builder(TokioExecutor::new());
            // When HTTP/1 is disabled and HTTP/2 is enabled, mark the legacy
            // client as HTTP/2-only. This is what enforces the protocol
            // contract: hyper-util will attempt an HTTP/2 handshake on every
            // socket (including cleartext, where it falls through to HTTP/2
            // prior knowledge). When ALPN does not negotiate `h2`, the
            // HTTP/2 handshake fails with a `Connect` error rather than
            // silently downgrading to HTTP/1.1.
            #[cfg(feature = "http2")]
            if !enabler.enable_http1() && enabler.enable_http2() {
                builder.http2_only(true);
            }
            if let Some(timeout) = pool_config.idle_timeout {
                builder.pool_idle_timeout(timeout);
            }
            Some(builder.build(https))
        };

        #[cfg(feature = "http3")]
        let h3_connector = if enabler.use_http3() {
            crate::transport::http3::H3Connector::new(self.tls_config.clone()).ok()
        } else {
            None
        };

        // Build the direct connector client for advanced socket options / local
        // address binding. This uses a custom connector path instead of the
        // standard hyper-rustls connector.
        let direct_client = if let Some(dc_config) = self.direct_connector_config {
            let connect_timeout = self.timeout.as_ref().and_then(|t| t.connect);

            // Build a TLS connector for HTTPS through the direct path.
            let tls_connector = match self.tls_config.as_ref() {
                Some(tls_config) => match tls_config.build_rustls_config() {
                    Ok(rc) => Some(tokio_rustls::TlsConnector::from(Arc::new(rc))),
                    Err(_) => None,
                },
                None => None,
            };

            let direct_connector =
                crate::transport::direct_connector::DirectConnector::new(dc_config, tls_connector);
            let direct_connector = crate::transport::connect_timeout::ConnectTimeout::new(
                direct_connector,
                connect_timeout,
            );

            let mut builder = hyper_util::client::legacy::Client::builder(TokioExecutor::new());
            // Match the standard hyper-rustls path: when H2-only, force
            // http2_only on the legacy client so HTTP/2 handshake runs even
            // when the connector does not advertise ALPN h2. The
            // DirectConnector itself signals ALPN h2 via `Connected::negotiated_h2`
            // when TLS selected it.
            #[cfg(feature = "http2")]
            if !enabler.enable_http1() && enabler.enable_http2() {
                builder.http2_only(true);
            }
            if let Some(timeout) = pool_config.idle_timeout {
                builder.pool_idle_timeout(timeout);
            }
            Some(builder.build(direct_connector))
        } else {
            None
        };

        #[cfg(unix)]
        let uds_client = self.uds_path.map(|path| {
            let tls_connector = self.tls_config.as_ref().and_then(|config| {
                config
                    .build_rustls_config()
                    .ok()
                    .map(|rc| tokio_rustls::TlsConnector::from(Arc::new(rc)))
            });
            let connector = crate::transport::uds::UdsConnector::new(path, tls_connector);
            let connector = crate::transport::connect_timeout::ConnectTimeout::new(
                connector,
                self.timeout.as_ref().and_then(|timeout| timeout.connect),
            );
            let mut builder = hyper_util::client::legacy::Client::builder(TokioExecutor::new());
            // Mirror the standard and direct paths: when H2-only, force
            // http2_only on the legacy client. The UDS connector signals ALPN
            // h2 via `Connected::negotiated_h2` when TLS selected it.
            #[cfg(feature = "http2")]
            if !enabler.enable_http1() && enabler.enable_http2() {
                builder.http2_only(true);
            }
            if let Some(timeout) = pool_config.idle_timeout {
                builder.pool_idle_timeout(timeout);
            }
            builder.build(connector)
        });

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
            #[cfg(feature = "proxy")]
            environment_proxies: self.environment_proxies,
            #[cfg(any(feature = "proxy", feature = "http3"))]
            tls_config: self.tls_config,
            retry: self.retry,
            http_version_policy: self.http_version_policy,
        };

        let pool = Pool::new(pool_config);

        Client {
            inner: Arc::new(ClientInner {
                hyper_client,
                direct_client,
                sni_clients: Mutex::new(HashMap::new()),
                #[cfg(unix)]
                uds_client,
                #[cfg(feature = "proxy")]
                socks_clients: Mutex::new(HashMap::new()),
                config,
                pool,
                #[cfg(feature = "http3")]
                h3_connector,
            }),
        }
    }
}

/// Build a connector with fallback root stores.
///
/// Uses native roots when available, otherwise falls back to the packaged
/// Mozilla root set. Protocol versions are determined by the enabler.
fn build_fallback_connector(enabler: crate::http_version::HttpVersionPolicyEnabler) -> Connector {
    let builder = match hyper_rustls::HttpsConnectorBuilder::new().with_native_roots() {
        Ok(b) => b,
        Err(_) => hyper_rustls::HttpsConnectorBuilder::new().with_webpki_roots(),
    };
    #[cfg(feature = "http2")]
    {
        match (enabler.enable_http1(), enabler.enable_http2()) {
            (true, true) => builder
                .https_or_http()
                .enable_http1()
                .enable_http2()
                .build(),
            (true, false) => builder.https_or_http().enable_http1().build(),
            (false, true) => builder.https_or_http().enable_http2().build(),
            (false, false) => unreachable!("at least one protocol version must be enabled"),
        }
    }
    #[cfg(not(feature = "http2"))]
    {
        let _ = enabler;
        builder.https_or_http().enable_http1().build()
    }
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
        let enabler =
            crate::http_version::HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Auto {
                allow_http3: false,
            });
        let _ = build_fallback_connector(enabler);
    }

    #[test]
    fn client_builder_http1_only() {
        let client = Client::builder()
            .http_version_policy(HttpVersionPolicy::Http1Only)
            .build();
        assert_eq!(
            client.inner.config.http_version_policy,
            HttpVersionPolicy::Http1Only
        );
    }

    #[test]
    fn client_builder_http2_auto() {
        let client = Client::builder()
            .http_version_policy(HttpVersionPolicy::Auto { allow_http3: false })
            .build();
        assert_eq!(
            client.inner.config.http_version_policy,
            HttpVersionPolicy::Auto { allow_http3: false }
        );
    }

    #[test]
    #[cfg(feature = "http2")]
    fn client_builder_http2_only() {
        let client = Client::builder()
            .http_version_policy(HttpVersionPolicy::Http2Only)
            .build();
        assert_eq!(
            client.inner.config.http_version_policy,
            HttpVersionPolicy::Http2Only
        );
    }

    #[test]
    fn connector_builds_for_all_policies() {
        for policy in [
            HttpVersionPolicy::Http1Only,
            HttpVersionPolicy::Auto { allow_http3: false },
            #[cfg(feature = "http2")]
            HttpVersionPolicy::Http2Only,
            #[cfg(feature = "http3")]
            HttpVersionPolicy::Http3Only,
        ] {
            let enabler = crate::http_version::HttpVersionPolicyEnabler::from_policy(policy);
            if !enabler.use_http3() {
                let _ = build_fallback_connector(enabler);
            }
        }
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
