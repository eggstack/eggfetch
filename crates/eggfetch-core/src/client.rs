//! Async client entry point.

use std::sync::Arc;
use std::time::Duration;

#[cfg(any(
    feature = "transport-http1",
    feature = "transport-http2",
    feature = "advanced-routing"
))]
use crate::transport::hyper_client::{build_hyper_client, HyperClientPolicy};
#[cfg(feature = "advanced-routing")]
use crate::transport::hyper_client::{
    BoundedClientCache, RESOLVED_CLIENT_CACHE_MAX_ENTRIES, SNI_CLIENT_CACHE_MAX_ENTRIES,
};
#[cfg(feature = "proxy")]
use crate::transport::hyper_client::{
    CONNECT_CLIENT_CACHE_MAX_ENTRIES, FORWARD_CLIENT_CACHE_MAX_ENTRIES,
    SOCKS_CLIENT_CACHE_MAX_ENTRIES,
};
#[cfg(feature = "high-level-url")]
use http::Method;
#[cfg(any(feature = "advanced-routing", feature = "proxy"))]
use tokio::sync::Mutex;

use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::http_version::HttpVersionPolicy;
use crate::limits::Limits;
use crate::pool::{Pool, PoolConfig, PoolMetrics};
#[cfg(feature = "proxy")]
use crate::proxy::Proxy;
#[cfg(feature = "redirects")]
use crate::redirect::RedirectPolicy;
#[cfg(feature = "high-level-url")]
use crate::request::{Request, RequestBuilder};
#[cfg(feature = "high-level-url")]
use crate::response::Response;
#[cfg(feature = "logical-retry")]
use crate::retry::RetryPolicy;
use crate::timeout::Timeout;
#[cfg(feature = "advanced-routing")]
use crate::transport::dialer::Dialer;
#[cfg(any(
    feature = "transport-http1",
    feature = "transport-http2",
    feature = "advanced-routing"
))]
use crate::transport::lifecycle::LifecycleConfig;
use crate::transport::lifecycle::{PhysicalConnectionPolicy, TransportIoTimeout};
use crate::transport_hints::NativeRequestOptions;

mod config;
use config::ClientConfig;
mod connectors;
mod routes;
#[cfg(feature = "advanced-routing")]
use connectors::build_custom_connector;
#[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
use connectors::build_standard_connector;
#[cfg(all(test, any(feature = "transport-http1", feature = "transport-http2")))]
use connectors::build_standard_connector_with_resolver;
#[cfg(feature = "tls-rustls")]
pub(crate) use connectors::configure_tls_alpn;
#[cfg(feature = "advanced-routing")]
use routes::ResolvedRouteKey;

#[cfg(feature = "cookies")]
use crate::cookie::CookieJar;

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
    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
    pub(crate) hyper_client: Option<crate::transport::TimeoutHyperClient>,
    /// Hyper client backed by the caller-supplied native dialer.
    #[cfg(feature = "advanced-routing")]
    pub(crate) custom_client: Option<crate::transport::TimeoutCustomClient>,
    /// Error captured while building the configured TLS policy, if any.
    ///
    /// `ClientBuilder::build` is intentionally infallible, so HTTPS requests
    /// surface this error at dispatch time instead of falling back to a
    /// different trust policy.
    #[cfg(feature = "tls-rustls")]
    pub(crate) tls_config_error: Option<String>,
    /// Direct connector for requests with advanced socket options or local
    /// address binding. Uses a custom connector instead of the standard
    /// hyper-rustls connector path.
    #[cfg(feature = "advanced-routing")]
    pub(crate) direct_client: Option<crate::transport::TimeoutDirectClient>,
    /// Base direct-connector configuration (local address / socket options)
    /// used to construct SNI-override clients so they preserve source
    /// binding and socket tuning.
    #[cfg(feature = "advanced-routing")]
    pub(crate) direct_connector_config:
        Option<crate::transport::direct_connector::DirectConnectorConfig>,
    /// Cached hyper clients keyed by TLS SNI hostname override.
    ///
    /// When a request carries `TransportHints.sni_hostname`, the pipeline
    /// uses a DirectConnector-based client from this cache. The connector
    /// separates DNS/TCP (to the original URL host) from TLS (with the
    /// SNI override hostname), keeping the default path unchanged.
    ///
    /// Bounded by [`SNI_CLIENT_CACHE_MAX_ENTRIES`] via the shared
    /// [`BoundedClientCache`] mechanics (see `transport::hyper_client`).
    ///
    /// The cache is keyed only by hostname because every other
    /// connection-affecting dimension is immutable at `ClientInner` scope
    /// after `ClientBuilder::build`: `http_version_policy` and `tls_config`
    /// ALPN, `direct_connector_config` socket/local-address policy,
    /// `timeout.connect` phase policy, and the absence of a custom dialer
    /// on this path. If any of those became per-request mutable, this
    /// cache would need to include the changed dimension in the key to
    /// avoid serving a client with stale policy (e.g. H2 vs H1) for the
    /// same hostname. Request-scoped state (total/read/write/pool
    /// deadlines, retry/redirect, body, trace/failure context,
    /// decompression limits, cookies/auth headers) is never keyed and
    /// never retained by the cached connector.
    #[cfg(feature = "advanced-routing")]
    pub(crate) sni_clients:
        Mutex<BoundedClientCache<String, crate::transport::TimeoutDirectClient>>,
    /// Cached custom-dialer clients for per-request SNI overrides.
    #[cfg(feature = "advanced-routing")]
    pub(crate) sni_custom_clients:
        Mutex<BoundedClientCache<String, crate::transport::TimeoutCustomClient>>,
    /// Cached Hyper clients for direct resolved-target routes.
    ///
    /// Bounded by [`RESOLVED_CLIENT_CACHE_MAX_ENTRIES`] via the shared
    /// [`BoundedClientCache`] mechanics (see `transport::hyper_client`).
    /// Hyper remains the physical pool owner: each entry is a configured
    /// Hyper client whose H1 keep-alive / H2 multiplexed connections are
    /// reused only when the full [`ResolvedRouteKey`] (logical origin +
    /// ordered address snapshot + exact SNI override) matches.
    ///
    /// The key contains origin + ordered target + SNI because each of those
    /// dimensions changes the physical/TLS route: different logical origins
    /// must never share a pool even when the addresses coincide, address
    /// order controls failover, and SNI changes certificate identity.
    /// Immutable client-wide TLS/ALPN/socket/connect/lifecycle/idle policy
    /// is intentionally not duplicated into every key because it is fixed at
    /// `ClientInner` scope after `ClientBuilder::build`; if any such field
    /// became request-scoped, the key must be expanded first. Ordinary
    /// DNS/direct clients and resolved-target clients never share an entry,
    /// and proxy resolved routing stays in the SOCKS/forward/CONNECT caches.
    #[cfg(feature = "advanced-routing")]
    pub(crate) resolved_clients:
        Mutex<BoundedClientCache<ResolvedRouteKey, crate::transport::TimeoutDirectClient>>,
    /// Hyper client for Unix domain socket requests.
    #[cfg(unix)]
    #[cfg(feature = "advanced-routing")]
    pub(crate) uds_client: Option<crate::transport::TimeoutUdsClient>,
    /// Persistent Hyper clients keyed by effective SOCKS route.
    ///
    /// Bounded like [`ClientInner::sni_clients`] via the shared
    /// [`BoundedClientCache`] mechanics (see `transport::hyper_client`).
    /// SOCKS route clients share the resolved Hyper idle-pool policy with
    /// every other persistent Hyper family.
    #[cfg(feature = "proxy")]
    pub(crate) socks_clients: Mutex<
        BoundedClientCache<
            crate::transport::socks::SocksRouteKey,
            crate::transport::TimeoutSocksClient,
        >,
    >,
    /// Persistent Hyper clients for ordinary HTTP forward-proxy routes.
    /// Keys include the destination origin and every proxy-leg policy that
    /// can affect a reusable connection. The cache is bounded because
    /// request-scoped proxy overrides are allowed.
    #[cfg(feature = "proxy")]
    pub(crate) forward_clients: Mutex<
        BoundedClientCache<
            crate::transport::proxy::ForwardRouteKey,
            crate::transport::TimeoutForwardClient,
        >,
    >,
    /// Persistent Hyper clients for one HTTPS origin reached through CONNECT.
    #[cfg(feature = "proxy")]
    pub(crate) connect_clients: Mutex<
        BoundedClientCache<
            crate::transport::connect::ConnectRouteKey,
            crate::transport::TimeoutConnectClient,
        >,
    >,
    pub(crate) config: ClientConfig,
    pub(crate) pool: Pool,
    /// Transport observability counters shared by all connectors owned by
    /// this client. Distinct from [`Pool`] logical-permit metrics.
    pub(crate) transport_metrics: Arc<crate::transport::metrics::TransportMetrics>,
    /// Shared admission permits and established-I/O timeout policy.
    #[cfg(any(
        feature = "transport-http1",
        feature = "transport-http2",
        feature = "advanced-routing"
    ))]
    pub(crate) lifecycle: Arc<LifecycleConfig>,
    #[cfg(feature = "http3")]
    pub(crate) h3_connector: Option<crate::transport::http3::H3Connector>,
    /// Bounded Alt-Svc discovery state (separate from QUIC sessions).
    /// Owned here so all routes share one view. Only present with `http3`;
    /// without it there is no discovery and no state to keep.
    #[cfg(feature = "http3")]
    pub(crate) alt_svc_state: Arc<crate::transport::alt_svc::AltSvcState>,
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
    #[cfg(feature = "high-level-url")]
    pub fn get(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::GET, url)
    }

    /// Create a POST request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    #[cfg(feature = "high-level-url")]
    pub fn post(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::POST, url)
    }

    /// Create a PUT request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    #[cfg(feature = "high-level-url")]
    pub fn put(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::PUT, url)
    }

    /// Create a PATCH request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    #[cfg(feature = "high-level-url")]
    pub fn patch(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::PATCH, url)
    }

    /// Create a DELETE request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    #[cfg(feature = "high-level-url")]
    pub fn delete(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::DELETE, url)
    }

    /// Create a HEAD request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    #[cfg(feature = "high-level-url")]
    pub fn head(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::HEAD, url)
    }

    /// Create an OPTIONS request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    #[cfg(feature = "high-level-url")]
    pub fn options(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::OPTIONS, url)
    }

    /// Create a request builder for the given method and URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    #[cfg(feature = "high-level-url")]
    pub fn request(&self, method: Method, url: &str) -> Result<RequestBuilder> {
        let parsed = parse_url(url)?;
        Ok(RequestBuilder::new(self.clone(), method, parsed))
    }

    /// Returns a reference to the connection pool metrics.
    ///
    /// These count logical request-permit waits/cancellations. For
    /// connector/protocol observations see [`Self::transport_metrics`].
    #[must_use]
    pub fn pool_metrics(&self) -> &PoolMetrics {
        self.inner.pool.metrics()
    }

    /// Returns a reference to the transport observability counters.
    ///
    /// Counts connector events (DNS/connect/TLS attempts) and protocol
    /// connections (H3 sessions, 101 upgrades) where observable. Hyper
    /// socket-reuse counts are intentionally absent.
    #[must_use]
    pub fn transport_metrics(&self) -> &crate::transport::metrics::TransportMetrics {
        &self.inner.transport_metrics
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
    /// When the `redirects` feature is enabled, the redirect loop enforces
    /// `max_redirects`, performs method rewrites per HTTP semantics, strips
    /// sensitive headers on cross-origin hops, and records redirect history.
    /// Without it, 3xx responses are returned as ordinary responses.
    ///
    /// When the `logical-retry` feature is enabled and a retry policy is
    /// configured (on the client or request), the entire logical request is
    /// retried on failure according to the policy. Without it, each
    /// high-level request is dispatched once under the outer total deadline.
    /// Hyper's distinct canceled-idle-request retry remains governed by
    /// `retry_canceled_requests` in all profiles.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails at any stage (connect, TLS,
    /// protocol, body) or if a timeout elapses.
    #[cfg(feature = "high-level-url")]
    pub(crate) async fn send(&self, request: Request) -> Result<Response> {
        #[cfg(feature = "logical-retry")]
        {
            Box::pin(crate::pipeline::send_with_retry(self, request)).await
        }
        #[cfg(all(not(feature = "logical-retry"), feature = "redirects"))]
        {
            Box::pin(crate::pipeline::send_with_redirects(self, request)).await
        }
        #[cfg(all(not(feature = "logical-retry"), not(feature = "redirects")))]
        {
            Box::pin(crate::pipeline::send_lean(self, request)).await
        }
    }

    /// Send a request and return optional structured native failure detail.
    ///
    /// The ordinary [`Error`] remains unchanged and can be recovered with
    /// [`RequestFailure::into_error`](crate::RequestFailure::into_error).
    /// Network classifications are reported only when the selected route
    /// exposes typed evidence; callers must not treat an absent subtype as a
    /// positive diagnosis.
    ///
    /// # Errors
    ///
    /// Returns the original request error wrapped in [`crate::RequestFailure`].
    #[cfg(feature = "high-level-url")]
    pub async fn send_detailed(
        &self,
        mut request: Request,
    ) -> std::result::Result<Response, crate::RequestFailure> {
        let context = crate::error::RequestFailureContext::new();
        context.clear();
        request.set_failure_context(Some(context.clone()));
        #[cfg(feature = "logical-retry")]
        let result = Box::pin(crate::pipeline::send_with_retry(self, request)).await;
        #[cfg(all(not(feature = "logical-retry"), feature = "redirects"))]
        let result = Box::pin(crate::pipeline::send_with_redirects(self, request)).await;
        #[cfg(all(not(feature = "logical-retry"), not(feature = "redirects")))]
        let result = Box::pin(crate::pipeline::send_lean(self, request)).await;
        match result {
            Ok(response) => Ok(response),
            Err(error) => Err(crate::RequestFailure::from_context(error, &context)),
        }
    }

    /// Send a single HTTP request and return the streaming response.
    ///
    /// This handles pool acquisition, timeout application, and body
    /// processing for one request/response cycle. It does NOT handle
    /// redirects—that is the responsibility of [`Client::send`].
    #[cfg(feature = "high-level-url")]
    pub(crate) async fn send_single_request(
        &self,
        request: Request,
        timeout: &Timeout,
    ) -> Result<Response> {
        crate::pipeline::send_single_request(&self.inner, request, timeout).await
    }

    /// Execute a native `http_body::Body` using eggfetch's HTTP/TLS
    /// transport engine.
    ///
    /// This is the lower-level native embedding API. Request DATA and
    /// trailer frames are passed through without flattening. The response is
    /// an ordinary `http::Response` whose opaque [`crate::NativeResponseBody`]
    /// preserves DATA and trailer frames and holds the logical pool lease
    /// until the body reaches EOF, errors, or is dropped.
    ///
    /// Native execution does not implicitly apply redirects, logical
    /// retries, cookies, authentication, response decompression, or decoded
    /// body limits. The request body is one-shot. Built-in proxy, HTTP/3,
    /// and protocol-upgrade routes are rejected because their current
    /// adapters do not expose the frame-preserving contract.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid logical URI, unsupported route, pool
    /// or transport failure, or a timeout.
    pub async fn execute_http_body<B>(
        &self,
        request: http::Request<B>,
        options: NativeRequestOptions,
    ) -> Result<http::Response<crate::NativeResponseBody>>
    where
        B: http_body::Body<Data = bytes::Bytes> + Send + 'static,
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        crate::pipeline::send_native_http_body(&self.inner, request, options).await
    }

    /// Execute a native body request with default native transport options.
    ///
    /// # Errors
    ///
    /// Returns the same URI, pool, transport, and timeout errors as
    /// [`Self::execute_http_body`].
    pub async fn execute_http_body_default<B>(
        &self,
        request: http::Request<B>,
    ) -> Result<http::Response<crate::NativeResponseBody>>
    where
        B: http_body::Body<Data = bytes::Bytes> + Send + 'static,
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        self.execute_http_body(request, NativeRequestOptions::default())
            .await
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
    #[cfg(feature = "redirects")]
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
    #[cfg(feature = "tls-rustls")]
    tls_config: Option<crate::tls::TlsConfig>,
    #[cfg(feature = "logical-retry")]
    retry: Option<RetryPolicy>,
    retry_canceled_requests: bool,
    #[cfg(feature = "advanced-routing")]
    dialer: Option<Arc<dyn Dialer>>,
    physical_connection_policy: PhysicalConnectionPolicy,
    transport_io_timeout: TransportIoTimeout,
    http_version_policy: HttpVersionPolicy,
    /// Advanced direct-connector config for socket options / local address.
    #[cfg(feature = "advanced-routing")]
    direct_connector_config: Option<crate::transport::direct_connector::DirectConnectorConfig>,
    /// Unix domain socket path. When set, all requests use UDS transport.
    #[cfg(feature = "advanced-routing")]
    uds_path: Option<String>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
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
            #[cfg(feature = "redirects")]
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
            #[cfg(feature = "tls-rustls")]
            tls_config: None,
            #[cfg(feature = "logical-retry")]
            retry: None,
            retry_canceled_requests: true,
            #[cfg(feature = "advanced-routing")]
            dialer: None,
            physical_connection_policy: PhysicalConnectionPolicy::default(),
            transport_io_timeout: TransportIoTimeout::default(),
            http_version_policy: HttpVersionPolicy::default(),
            #[cfg(feature = "advanced-routing")]
            direct_connector_config: None,
            #[cfg(feature = "advanced-routing")]
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

    /// Set the default headers for all requests made by this client.
    ///
    /// This preserves valid header values containing HTTP `obs-text` bytes.
    #[must_use]
    pub fn default_headers(mut self, headers: Headers) -> Self {
        self.default_headers = headers;
        self
    }

    /// Set the default user-agent header.
    #[must_use]
    pub fn user_agent(mut self, agent: &str) -> Self {
        self.user_agent = Some(agent.to_owned());
        self
    }

    /// Set the maximum number of idle (unused) connections kept per host.
    ///
    /// This is a convenience alias for [`max_idle_connections_per_host`]:
    /// the transport enforces the limit on a per-host basis, so total
    /// idle connections grow with the number of distinct origins. Use
    /// [`max_connections`] for a global concurrency cap.
    ///
    /// [`max_idle_connections_per_host`]: Self::max_idle_connections_per_host
    /// [`max_connections`]: Self::max_connections
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

    /// Set the maximum total number of concurrent in-flight requests.
    ///
    /// Compatibility alias for [`Self::max_in_flight_requests`] kept for
    /// the pre-1.0 line. One permit equals one logical request, not one
    /// TCP connection (H2/H3 multiplex many requests over one
    /// connection). Prefer `max_in_flight_requests` in new code.
    #[must_use]
    pub fn max_connections(mut self, max: usize) -> Self {
        self.pool_config.max_connections = Some(max);
        self
    }

    /// Set the maximum number of concurrent in-flight requests per origin.
    ///
    /// Compatibility alias for [`Self::max_in_flight_requests_per_origin`].
    #[must_use]
    pub fn max_connections_per_host(mut self, max: usize) -> Self {
        self.pool_config.max_connections_per_host = Some(max);
        self
    }

    /// Set the maximum total number of concurrent in-flight requests.
    ///
    /// Preferred native name. Bounds logical request concurrency (one
    /// permit per request). Under H1 one request typically owns its
    /// connection slot; under H2/H3 many permits multiplex over one
    /// TCP/QUIC connection. When both this and `max_connections` are
    /// set, this wins.
    #[must_use]
    pub fn max_in_flight_requests(mut self, max: usize) -> Self {
        self.pool_config.max_in_flight_requests = Some(max);
        self
    }

    /// Set the maximum number of concurrent in-flight requests per origin.
    ///
    /// Preferred native name. When both this and
    /// `max_connections_per_host` are set, this wins.
    #[must_use]
    pub fn max_in_flight_requests_per_origin(mut self, max: usize) -> Self {
        self.pool_config.max_in_flight_requests_per_origin = Some(max);
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

    /// Merge a partial timeout configuration into the currently configured
    /// timeout.
    ///
    /// Unlike [`timeout`](Self::timeout), which replaces the entire
    /// configuration, each phase set in `timeout` overrides only that
    /// phase; unset phases keep any previously configured value (or
    /// remain unset when no timeout was configured yet).
    #[must_use]
    pub fn merge_timeout(mut self, timeout: Timeout) -> Self {
        let merged = match self.timeout {
            Some(existing) => existing.merge(Some(timeout)),
            None => timeout,
        };
        self.timeout = Some(merged);
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
    ///
    /// Only available with the `redirects` feature (enabled by the `http1`/`http2`
    /// compatibility aliases). The lean profile omits redirect following and
    /// returns 3xx responses without a second hop.
    #[cfg(feature = "redirects")]
    #[must_use]
    pub fn follow_redirects(mut self, follow: bool) -> Self {
        self.redirect.follow = follow;
        self
    }

    /// Set the maximum number of redirects to follow.
    ///
    /// Only available with the `redirects` feature.
    #[cfg(feature = "redirects")]
    #[must_use]
    pub fn max_redirects(mut self, max: usize) -> Self {
        self.redirect.max_redirects = max;
        self
    }

    /// Set the full redirect policy.
    ///
    /// Only available with the `redirects` feature.
    #[cfg(feature = "redirects")]
    #[must_use]
    pub fn redirect_policy(mut self, policy: RedirectPolicy) -> Self {
        self.redirect = policy;
        self
    }

    /// Set the HTTPS-downgrade rule for redirects without replacing the
    /// rest of the redirect policy.
    ///
    /// Only available with the `redirects` feature.
    ///
    /// Select
    /// [`RedirectDowngradePolicy::Deny`](crate::redirect::RedirectDowngradePolicy::Deny)
    /// to reject HTTPS -> HTTP downgrades before the downgraded request
    /// is dispatched. The default is
    /// [`Allow`](crate::redirect::RedirectDowngradePolicy::Allow) for
    /// compatibility.
    #[cfg(feature = "redirects")]
    #[must_use]
    pub fn redirect_downgrade_policy(
        mut self,
        policy: crate::redirect::RedirectDowngradePolicy,
    ) -> Self {
        self.redirect.downgrade = policy;
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

    /// Opt into environment-proxy routing from an explicit snapshot.
    ///
    /// The native default stays environment-independent: without this call
    /// the client never reads proxy environment variables. Pass
    /// [`crate::proxy::ProxyEnvironment::from_env`] at the call site to
    /// preserve command-line environment reachability, or
    /// [`crate::proxy::ProxyEnvironment::from_map`] in tests for a pure
    /// snapshot without global environment mutation.
    ///
    /// Each configured value becomes a scheme-scoped fallback proxy
    /// (specific before `ALL_PROXY`); `NO_PROXY` bypass is attached to
    /// every proxy. Invalid values fail closed with a redacted error.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::InvalidProxyUrl`] when a configured
    /// proxy URL or the `NO_PROXY` value cannot be parsed.
    #[cfg(feature = "proxy")]
    pub fn proxy_environment(
        mut self,
        env: &crate::proxy::ProxyEnvironment,
    ) -> crate::error::Result<Self> {
        for proxy in env.client_proxies()? {
            self.environment_proxies.push(proxy);
        }
        Ok(self)
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
    /// TLS connections instead of the default native-root-with-WebPKI-fallback
    /// strategy (or the packaged-WebPKI default when `tls-native-roots` is not
    /// enabled).
    #[cfg(feature = "tls-rustls")]
    #[must_use]
    pub fn tls_config(mut self, config: crate::tls::TlsConfig) -> Self {
        self.tls_config = Some(config);
        self
    }

    /// Enable or disable insecure TLS verification while retaining other
    /// TLS configuration already applied to this builder.
    #[cfg(feature = "tls-rustls")]
    #[must_use]
    pub fn danger_accept_invalid_certs(mut self, accept: bool) -> Self {
        let config = self.tls_config.take().unwrap_or_default();
        self.tls_config = Some(config.danger_accept_invalid_certs(accept));
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
    ///
    /// The limit applies to bytes made available after decoding and also to
    /// ordinary unencoded/identity response bodies, in both buffered and
    /// streaming paths. Exceeding it produces [`Error::DecodedBodyTooLarge`]
    /// without requiring a second caller-side accumulation loop. A wire
    /// `Content-Length` may be used for an early metadata check, but this
    /// body limit remains authoritative when the header is absent, incorrect,
    /// or describes encoded bytes.
    #[must_use]
    pub fn max_decoded_body_size(mut self, max: usize) -> Self {
        self.max_decoded_body_size = Some(max);
        self
    }

    /// Set the maximum decompression ratio (decoded bytes / compressed bytes).
    /// When set, responses whose expansion ratio exceeds this limit produce
    /// an error. This guards against zip-bomb style attacks.
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

    /// Set the retry policy for this client.
    ///
    /// Only available with the `logical-retry` feature (enabled by the `http1`/`http2`
    /// compatibility aliases). The lean profile dispatches once under the outer
    /// total deadline with no retry backoff/jitter/status policy.
    ///
    /// When set, failed requests that match the policy (safe methods,
    /// retryable statuses/errors, replayable bodies) are automatically
    /// retried with exponential backoff.
    #[cfg(feature = "logical-retry")]
    #[must_use]
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    /// Allow or disallow Hyper's implicit retry when a reused idle
    /// connection is found unusable before a request starts writing.
    ///
    /// This is separate from eggfetch's explicit logical retry (the
    /// `logical-retry` feature and [`RetryPolicy`]). The
    /// default is `true`, preserving Hyper's existing behavior.
    #[must_use]
    pub fn retry_canceled_requests(mut self, enabled: bool) -> Self {
        self.retry_canceled_requests = enabled;
        self
    }

    /// Install a client-scoped caller-owned raw-stream dialer.
    ///
    /// Eggfetch continues to own HTTP framing, destination TLS, SNI,
    /// certificate verification, redirects, retries, and response bodies.
    /// The dialer is incompatible with built-in proxy, UDS, resolved-target,
    /// local-address, and socket-option routing; those combinations fail
    /// closed before network I/O.
    ///
    /// Only available with the `advanced-routing` feature (enabled by the
    /// `native-http1`/`native-http2` compatibility slices). The lean
    /// standard-route profile omits custom dialing.
    #[cfg(feature = "advanced-routing")]
    #[must_use]
    pub fn dialer<D>(mut self, dialer: D) -> Self
    where
        D: Dialer,
    {
        self.dialer = Some(Arc::new(dialer));
        self
    }

    /// Install a shared caller-scoped live physical-connection policy.
    ///
    /// This controls established Hyper connections, including idle pooled
    /// connections. It does not change the logical request limits in
    /// [`PoolConfig`]. A `max_live` value of zero fails
    /// requests closed because no useful client can be created from it.
    #[must_use]
    pub fn physical_connection_policy(mut self, policy: PhysicalConnectionPolicy) -> Self {
        self.physical_connection_policy = policy;
        self
    }

    /// Install established-transport read/write inactivity guardrails.
    ///
    /// These timers are distinct from [`Timeout`]'s request
    /// body and response-stream semantics and are disabled by default.
    #[must_use]
    pub fn transport_io_timeout(mut self, timeout: TransportIoTimeout) -> Self {
        self.transport_io_timeout = timeout;
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
    ///
    /// Only available with `advanced-routing`; omitted by the lean profile.
    #[cfg(feature = "advanced-routing")]
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
    ///
    /// Only available with `advanced-routing`; omitted by the lean profile.
    #[cfg(feature = "advanced-routing")]
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
    ///
    /// Only available with `advanced-routing`; omitted by the lean profile.
    #[cfg(feature = "advanced-routing")]
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
        #[cfg(any(
            feature = "transport-http1",
            feature = "transport-http2",
            feature = "advanced-routing"
        ))]
        use crate::http_version::HttpVersionPolicyEnabler;
        #[cfg(any(
            feature = "transport-http1",
            feature = "transport-http2",
            feature = "advanced-routing"
        ))]
        let enabler = HttpVersionPolicyEnabler::from_policy(self.http_version_policy);
        let transport_metrics = Arc::new(crate::transport::metrics::TransportMetrics::new());

        let mut pool_config = self.pool_config;
        if let Some(limits) = self.limits {
            let limits_config: PoolConfig = limits.into();
            if limits_config.max_connections.is_some() {
                pool_config.max_connections = limits_config.max_connections;
            }
            if limits_config.max_connections_per_host.is_some() {
                pool_config.max_connections_per_host = limits_config.max_connections_per_host;
            }
            if limits_config.max_in_flight_requests.is_some() {
                pool_config.max_in_flight_requests = limits_config.max_in_flight_requests;
            }
            if limits_config.max_in_flight_requests_per_origin.is_some() {
                pool_config.max_in_flight_requests_per_origin =
                    limits_config.max_in_flight_requests_per_origin;
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
        #[cfg(any(
            feature = "transport-http1",
            feature = "transport-http2",
            feature = "advanced-routing"
        ))]
        let lifecycle = Arc::new(LifecycleConfig::from_policy(
            self.physical_connection_policy,
            self.transport_io_timeout,
            transport_metrics.clone(),
        ));
        // Shared builder policy for the persistent per-client Hyper
        // singletons below (standard, direct, UDS, custom dialer). Cached and
        // isolated route clients resolve their own narrower policy at use.
        #[cfg(any(
            feature = "transport-http1",
            feature = "transport-http2",
            feature = "advanced-routing"
        ))]
        let persistent_policy = HyperClientPolicy::persistent(
            self.retry_canceled_requests,
            pool_config.idle_timeout,
            pool_config.effective_max_idle_per_host(),
            enabler,
        );

        #[cfg(feature = "cookies")]
        let cookie_jar = self.cookie_jar.unwrap_or_default();

        let automatic_decompression = self.automatic_decompression.unwrap_or(true);
        #[cfg(feature = "tls-rustls")]
        let tls_config_result = self
            .tls_config
            .clone()
            .unwrap_or_default()
            .build_rustls_config();
        #[cfg(feature = "tls-rustls")]
        let tls_config_error = tls_config_result
            .as_ref()
            .err()
            .map(|error| format!("failed to build TLS config: {error}"));
        // `Http3Only` is strict direct-QUIC and needs no Hyper client.
        // `Auto { allow_http3: true }` needs both: H1/H2 for discovery and
        // safe fallback plus QUIC for discovered routes. All other policies
        // need Hyper only.
        #[cfg(feature = "tls-rustls")]
        let hyper_client = if matches!(
            self.http_version_policy,
            crate::HttpVersionPolicy::Http3Only
        ) || tls_config_error.is_some()
        {
            None
        } else if let Ok(rc) = tls_config_result {
            let https = build_standard_connector(rc, enabler);
            let connect_timeout = self.timeout.as_ref().and_then(|t| t.connect);
            Some(build_hyper_client(
                https,
                &persistent_policy,
                connect_timeout,
                &lifecycle,
            ))
        } else {
            None
        };

        #[cfg(all(
            not(feature = "tls-rustls"),
            any(feature = "transport-http1", feature = "transport-http2")
        ))]
        let hyper_client = if matches!(
            self.http_version_policy,
            crate::HttpVersionPolicy::Http3Only
        ) {
            None
        } else {
            let connector = build_standard_connector(enabler);
            let connect_timeout = self.timeout.as_ref().and_then(|t| t.connect);
            Some(build_hyper_client(
                connector,
                &persistent_policy,
                connect_timeout,
                &lifecycle,
            ))
        };
        #[cfg(feature = "http3")]
        let h3_connector = if enabler.use_http3() {
            crate::transport::http3::H3Connector::with_metrics(
                self.tls_config.clone(),
                &pool_config,
                Some(transport_metrics.clone()),
            )
            .ok()
        } else {
            None
        };

        // Build the direct connector client for advanced socket options / local
        // address binding. This uses a custom connector path instead of the
        // standard hyper-rustls connector.
        #[cfg(feature = "advanced-routing")]
        let stored_direct_config = self.direct_connector_config.clone();
        #[cfg(feature = "advanced-routing")]
        let direct_client = if let Some(dc_config) = self.direct_connector_config {
            let connect_timeout = self.timeout.as_ref().and_then(|t| t.connect);

            // Build a TLS connector for HTTPS through the direct path.
            #[cfg(feature = "tls-rustls")]
            let tls_connector = match self
                .tls_config
                .clone()
                .unwrap_or_default()
                .build_rustls_config()
            {
                Ok(rc) => Some(tokio_rustls::TlsConnector::from(Arc::new(
                    configure_tls_alpn(
                        rc,
                        crate::http_version::HttpVersionPolicyEnabler::from_policy(
                            self.http_version_policy,
                        ),
                    ),
                ))),
                Err(_) => None,
            };
            #[cfg(not(feature = "tls-rustls"))]
            let tls_connector = None;

            let direct_connector =
                crate::transport::direct_connector::DirectConnector::with_metrics(
                    dc_config,
                    tls_connector,
                    transport_metrics.clone(),
                );
            Some(build_hyper_client(
                direct_connector,
                &persistent_policy,
                connect_timeout,
                &lifecycle,
            ))
        } else {
            None
        };

        // The UDS path is accepted on all platforms (see `uds_path`) but
        // only consumed on Unix; without this the field is never read on
        // non-Unix targets.
        #[cfg(all(not(unix), feature = "advanced-routing"))]
        let _ = &self.uds_path;

        #[cfg(all(unix, feature = "advanced-routing"))]
        let uds_client = self.uds_path.clone().map(|path| {
            #[cfg(feature = "tls-rustls")]
            let tls_connector = self
                .tls_config
                .clone()
                .unwrap_or_default()
                .build_rustls_config()
                .ok()
                .map(|rc| {
                    tokio_rustls::TlsConnector::from(Arc::new(configure_tls_alpn(
                        rc,
                        crate::http_version::HttpVersionPolicyEnabler::from_policy(
                            self.http_version_policy,
                        ),
                    )))
                });
            #[cfg(not(feature = "tls-rustls"))]
            let tls_connector = None;
            let connector = crate::transport::uds::UdsConnector::with_metrics(
                path,
                tls_connector,
                transport_metrics.clone(),
            );
            let connect_timeout = self.timeout.as_ref().and_then(|timeout| timeout.connect);
            build_hyper_client(connector, &persistent_policy, connect_timeout, &lifecycle)
        });

        #[cfg(feature = "advanced-routing")]
        let custom_client = if let Some(dialer) = self.dialer.clone() {
            #[cfg(feature = "tls-rustls")]
            let custom_config = self
                .tls_config
                .clone()
                .unwrap_or_default()
                .build_rustls_config()
                .ok();
            #[cfg(not(feature = "tls-rustls"))]
            let custom_config = Some(());

            custom_config.and_then(|custom_config| {
                let connector =
                    build_custom_connector(custom_config, dialer, enabler, None).ok()?;
                let connect_timeout = self.timeout.as_ref().and_then(|timeout| timeout.connect);
                Some(build_hyper_client(
                    connector,
                    &persistent_policy,
                    connect_timeout,
                    &lifecycle,
                ))
            })
        } else {
            None
        };
        let config = ClientConfig {
            default_headers: self.default_headers,
            user_agent: self.user_agent,
            timeout: self.timeout,
            #[cfg(feature = "redirects")]
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
            #[cfg(feature = "tls-rustls")]
            tls_config: self.tls_config,
            #[cfg(feature = "logical-retry")]
            retry: self.retry,
            retry_canceled_requests: self.retry_canceled_requests,
            #[cfg(feature = "advanced-routing")]
            dialer: self.dialer,
            #[cfg(feature = "advanced-routing")]
            uds_configured: self.uds_path.is_some(),
            http_version_policy: self.http_version_policy,
        };

        let pool = Pool::new(pool_config);

        Client {
            inner: Arc::new(ClientInner {
                #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
                hyper_client,
                #[cfg(feature = "advanced-routing")]
                custom_client,
                #[cfg(feature = "tls-rustls")]
                tls_config_error,
                #[cfg(feature = "advanced-routing")]
                direct_client,
                #[cfg(feature = "advanced-routing")]
                direct_connector_config: stored_direct_config,
                #[cfg(feature = "advanced-routing")]
                sni_clients: Mutex::new(BoundedClientCache::new(SNI_CLIENT_CACHE_MAX_ENTRIES)),
                #[cfg(feature = "advanced-routing")]
                sni_custom_clients: Mutex::new(BoundedClientCache::new(
                    SNI_CLIENT_CACHE_MAX_ENTRIES,
                )),
                #[cfg(feature = "advanced-routing")]
                resolved_clients: Mutex::new(BoundedClientCache::new(
                    RESOLVED_CLIENT_CACHE_MAX_ENTRIES,
                )),
                #[cfg(all(unix, feature = "advanced-routing"))]
                uds_client,
                #[cfg(feature = "proxy")]
                socks_clients: Mutex::new(BoundedClientCache::new(SOCKS_CLIENT_CACHE_MAX_ENTRIES)),
                #[cfg(feature = "proxy")]
                forward_clients: Mutex::new(BoundedClientCache::new(
                    FORWARD_CLIENT_CACHE_MAX_ENTRIES,
                )),
                #[cfg(feature = "proxy")]
                connect_clients: Mutex::new(BoundedClientCache::new(
                    CONNECT_CLIENT_CACHE_MAX_ENTRIES,
                )),
                config,
                pool,
                transport_metrics,
                #[cfg(any(
                    feature = "transport-http1",
                    feature = "transport-http2",
                    feature = "advanced-routing"
                ))]
                lifecycle,
                #[cfg(feature = "http3")]
                h3_connector,
                #[cfg(feature = "http3")]
                alt_svc_state: Arc::new(crate::transport::alt_svc::AltSvcState::new()),
            }),
        }
    }
}

#[cfg(feature = "high-level-url")]
fn parse_url(url_str: &str) -> Result<url::Url> {
    // Deliberately do not echo url_str into the error: parsing has
    // failed, so redact_url_string would fall back to the raw input
    // (potentially containing credentials). url::ParseError renders
    // only a static description and never embeds the input.
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
    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
    use hyper_util::rt::TokioExecutor;
    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
    use std::future::{ready, Ready};
    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
    use std::net::SocketAddr;
    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
    use std::task::{Context, Poll};
    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
    use tower_service::Service;

    #[test]
    fn client_constructs() {
        let _client = Client::new();
    }

    #[test]
    fn client_config_debug_redacts_secrets() {
        let mut config = ClientConfig::default();
        config
            .default_headers
            .insert("authorization", "Bearer header-secret-token")
            .unwrap();
        #[cfg(feature = "cookies")]
        config
            .cookie_jar
            .set_default_cookie("session".to_owned(), "jar-secret-token".to_owned())
            .expect("valid test cookie");
        let debug = format!("{config:?}");
        assert!(
            !debug.contains("header-secret-token"),
            "ClientConfig Debug must not leak header secrets: {debug}"
        );
        #[cfg(feature = "cookies")]
        assert!(
            !debug.contains("jar-secret-token"),
            "ClientConfig Debug must not leak jar secrets: {debug}"
        );
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn client_default() {
        let _client = Client::default();
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn client_builder() {
        let _client = Client::builder().user_agent("test-agent").build();
    }

    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
    #[cfg(feature = "high-level-url")]
    #[tokio::test]
    async fn standard_connector_preserves_resolver_failure_for_detailed_mapping() {
        #[derive(Clone, Copy)]
        struct FailingResolver;

        impl Service<hyper_util::client::legacy::connect::dns::Name> for FailingResolver {
            type Response = std::vec::IntoIter<SocketAddr>;
            type Error = std::io::Error;
            type Future = Ready<std::result::Result<Self::Response, Self::Error>>;

            fn poll_ready(
                &mut self,
                _: &mut Context<'_>,
            ) -> Poll<std::result::Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }

            fn call(&mut self, _: hyper_util::client::legacy::connect::dns::Name) -> Self::Future {
                ready(Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "synthetic standard resolver failure",
                )))
            }
        }

        let enabler = crate::http_version::HttpVersionPolicyEnabler::from_policy(
            HttpVersionPolicy::Http1Only,
        );
        #[cfg(feature = "tls-rustls")]
        let connector = build_standard_connector_with_resolver(
            rustls::ClientConfig::builder()
                .with_root_certificates(rustls::RootCertStore::empty())
                .with_no_client_auth(),
            enabler,
            FailingResolver,
        );
        #[cfg(not(feature = "tls-rustls"))]
        let connector = build_standard_connector_with_resolver(FailingResolver, enabler);

        let client =
            hyper_util::client::legacy::Client::builder(TokioExecutor::new()).build(connector);
        #[cfg(feature = "tls-rustls")]
        let uri = "https://synthetic.example/";
        #[cfg(not(feature = "tls-rustls"))]
        let uri = "http://synthetic.example/";
        let request = http::Request::get(uri)
            .body(http_body_util::Empty::<Bytes>::new())
            .expect("synthetic request is valid");
        let error = client
            .request(request)
            .await
            .expect_err("synthetic resolver should fail");
        let context = crate::error::RequestFailureContext::new();
        let mapped = crate::transport::direct::map_send_error_with_context(error, Some(&context));

        assert_eq!(mapped.kind(), "hyper_client");
        let failure = crate::error::RequestFailure::from_context(mapped, &context);
        assert_eq!(
            failure.network_failure_kind(),
            Some(crate::error::NetworkFailureKind::Dns)
        );
    }

    #[test]
    fn client_builder_merge_timeout_preserves_other_phases() {
        // Phase-specific setters must merge into any existing timeout
        // configuration instead of replacing it wholesale.
        let client = Client::builder()
            .timeout(Timeout::from_secs(30))
            .merge_timeout(
                Timeout::builder()
                    .connect(std::time::Duration::from_secs(5))
                    .build(),
            )
            .merge_timeout(
                Timeout::builder()
                    .read(std::time::Duration::from_secs(10))
                    .build(),
            )
            .build();

        let timeout = client.inner.config.timeout.expect("timeout configured");
        let d = |secs: u64| std::time::Duration::from_secs(secs);
        assert_eq!(timeout.connect, Some(d(5)), "last connect wins");
        assert_eq!(timeout.read, Some(d(10)), "last read wins");
        assert_eq!(
            timeout.write,
            Some(d(30)),
            "unset phases keep earlier values"
        );
        assert_eq!(timeout.pool, Some(d(30)));
    }

    #[test]
    fn client_builder_merge_timeout_without_prior_config() {
        let client = Client::builder()
            .merge_timeout(
                Timeout::builder()
                    .connect(std::time::Duration::from_secs(5))
                    .build(),
            )
            .build();

        let timeout = client.inner.config.timeout.expect("timeout configured");
        assert_eq!(timeout.connect, Some(std::time::Duration::from_secs(5)));
        assert_eq!(timeout.read, None);
    }

    #[cfg(any(feature = "proxy", feature = "http3"))]
    #[test]
    fn client_builder_danger_accept_invalid_certs_preserves_tls_config() {
        let client = Client::builder()
            .tls_config(crate::tls::TlsConfig::builder().sni(false).build())
            .danger_accept_invalid_certs(true)
            .build();
        let tls = client
            .inner
            .config
            .tls_config
            .as_ref()
            .expect("TLS config should be retained");
        assert!(!tls.sni_enabled());
        assert!(!tls.verify_certificate());
        assert!(!tls.verify_hostname());
    }

    #[test]
    #[cfg(feature = "tls-rustls")]
    fn tls_root_store_paths_construct() {
        // Native roots are environment-dependent; the production connector
        // always receives the same policy-built config as direct connectors.
        let enabler =
            crate::http_version::HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Auto {
                allow_http3: false,
            });
        let config = crate::tls::TlsConfig::default()
            .build_rustls_config()
            .expect("default TLS policy builds");
        let _ = build_standard_connector(config, enabler);
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
    #[cfg(feature = "transport-http2")]
    fn client_builder_http2_only() {
        let client = Client::builder()
            .http_version_policy(HttpVersionPolicy::Http2Only)
            .build();
        assert_eq!(
            client.inner.config.http_version_policy,
            HttpVersionPolicy::Http2Only
        );
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn connector_builds_for_all_policies() {
        for policy in [
            HttpVersionPolicy::Http1Only,
            HttpVersionPolicy::Auto { allow_http3: false },
            #[cfg(feature = "transport-http2")]
            HttpVersionPolicy::Http2Only,
            #[cfg(feature = "http3")]
            HttpVersionPolicy::Http3Only,
        ] {
            let enabler = crate::http_version::HttpVersionPolicyEnabler::from_policy(policy);
            if !enabler.use_http3() {
                #[cfg(feature = "tls-rustls")]
                let config = crate::tls::TlsConfig::default()
                    .build_rustls_config()
                    .expect("default TLS policy builds");
                #[cfg(feature = "tls-rustls")]
                let _ = build_standard_connector(config, enabler);
            }
        }
    }

    #[cfg(feature = "transport-http2")]
    #[cfg(feature = "high-level-url")]
    #[test]
    fn custom_connector_alpn_matches_http_version_policy() {
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(rustls::RootCertStore::empty())
            .with_no_client_auth();
        let h2_only = configure_tls_alpn(
            config.clone(),
            crate::http_version::HttpVersionPolicyEnabler::from_policy(
                HttpVersionPolicy::Http2Only,
            ),
        );
        assert_eq!(h2_only.alpn_protocols, vec![b"h2".to_vec()]);

        let auto = configure_tls_alpn(
            config,
            crate::http_version::HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Auto {
                allow_http3: false,
            }),
        );
        assert_eq!(
            auto.alpn_protocols,
            vec![b"h2".to_vec(), b"http/1.1".to_vec()]
        );
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn parse_valid_urls() {
        assert!(parse_url("https://example.com").is_ok());
        assert!(parse_url("http://localhost:8080").is_ok());
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn parse_invalid_schemes() {
        assert!(parse_url("ftp://example.com").is_err());
        assert!(parse_url("file:///tmp/test").is_err());
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn parse_invalid_urls() {
        assert!(parse_url("not a url").is_err());
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn get_request_builder() {
        let client = Client::new();
        let builder = client.get("https://example.com").unwrap();
        let req = builder.build().unwrap();
        assert_eq!(*req.method(), Method::GET);
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn hyper_canceled_request_retry_defaults_on_and_is_independently_configurable() {
        assert!(Client::new().inner.config.retry_canceled_requests);
        let strict = Client::builder().retry_canceled_requests(false).build();
        assert!(!strict.inner.config.retry_canceled_requests);
        #[cfg(feature = "logical-retry")]
        assert!(strict.inner.config.retry.is_none());
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn physical_policy_and_transport_io_timeout_are_separate_from_logical_pooling() {
        let client = Client::builder()
            .physical_connection_policy(PhysicalConnectionPolicy {
                max_live: Some(2),
                admission_timeout: Some(Duration::from_secs(1)),
            })
            .transport_io_timeout(TransportIoTimeout {
                read: Some(Duration::from_secs(2)),
                write: Some(Duration::from_secs(3)),
            })
            .build();
        assert_eq!(
            client.inner.lifecycle.admission_timeout,
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            client.inner.lifecycle.io_timeout.read,
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            client.inner.lifecycle.io_timeout.write,
            Some(Duration::from_secs(3))
        );
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn apply_content_length_empty_body() {
        let headers = Headers::new();
        let body = crate::body::RequestBody::Empty;
        let out = crate::pipeline::apply_content_length(headers, &body).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "0");
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn apply_content_length_bytes_body() {
        let headers = Headers::new();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let out = crate::pipeline::apply_content_length(headers, &body).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "5");
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn apply_content_length_stream_known() {
        let headers = Headers::new();
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = crate::body::RequestBody::from_stream(stream, Some(7));
        let out = crate::pipeline::apply_content_length(headers, &body).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "7");
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn apply_content_length_stream_unknown() {
        let headers = Headers::new();
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = crate::body::RequestBody::from_stream(stream, None);
        let out = crate::pipeline::apply_content_length(headers, &body).unwrap();
        assert!(out.get("content-length").is_none());
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn apply_content_length_user_matches() {
        let mut headers = Headers::new();
        headers.insert("content-length", "5").unwrap();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let out = crate::pipeline::apply_content_length(headers, &body).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "5");
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn apply_content_length_user_mismatch_errors() {
        let mut headers = Headers::new();
        headers.insert("content-length", "10").unwrap();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let err = crate::pipeline::apply_content_length(headers, &body).unwrap_err();
        assert_eq!(err.kind(), "request_build");
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn apply_content_length_rejects_invalid_value() {
        let mut headers = Headers::new();
        headers.insert("content-length", "not-a-number").unwrap();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let err = crate::pipeline::apply_content_length(headers, &body).unwrap_err();
        assert_eq!(err.kind(), "invalid_header_value");
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn apply_content_length_rejects_unknown_stream_override() {
        let mut headers = Headers::new();
        headers.insert("content-length", "5").unwrap();
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = crate::body::RequestBody::from_stream(stream, None);
        let err = crate::pipeline::apply_content_length(headers, &body).unwrap_err();
        assert_eq!(err.kind(), "request_build");
    }

    #[cfg(feature = "high-level-url")]
    #[test]
    fn parse_url_rejects_userinfo_without_echoing_credentials() {
        let err = parse_url("https://user:secret@example.com").unwrap_err();
        assert_eq!(err.kind(), "invalid_url");
        assert!(!err.to_string().contains("secret"));
    }

    #[cfg(feature = "proxy")]
    #[cfg(feature = "high-level-url")]
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
    #[cfg(feature = "high-level-url")]
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
    #[cfg(feature = "high-level-url")]
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

    #[cfg(feature = "advanced-routing")]
    #[cfg(feature = "high-level-url")]
    #[test]
    fn resolved_route_key_matrix() {
        use crate::transport_hints::ResolvedTarget;

        // Keys intentionally lack `Debug`/`Display` (connection identity
        // only, never request state), so compare with `==`/`!=`.
        let origin = url::Url::parse("http://origin.example/").unwrap();
        let target: ResolvedTarget =
            ResolvedTarget::new(["127.0.0.1:80".parse::<SocketAddr>().unwrap()]).unwrap();
        let base = ResolvedRouteKey::new(&origin, &target, None);
        let same = ResolvedRouteKey::new(&origin, &target, None);
        assert!(
            base == same,
            "same origin + same ordered addresses + same SNI must reuse"
        );

        // Path/query/fragment never participate: same logical origin.
        let with_path = url::Url::parse("http://origin.example/a?b=1#f").unwrap();
        let other_path = url::Url::parse("http://origin.example/other").unwrap();
        assert!(
            ResolvedRouteKey::new(&with_path, &target, None)
                == ResolvedRouteKey::new(&other_path, &target, None),
            "path/query/fragment must not fragment resolved identity"
        );
        // Explicit default port normalizes to the same origin.
        let explicit_default = url::Url::parse("http://origin.example:80/").unwrap();
        assert!(
            ResolvedRouteKey::new(&origin, &target, None)
                == ResolvedRouteKey::new(&explicit_default, &target, None),
            "explicit default port must reuse"
        );

        // HTTP vs HTTPS: logical/TLS origin differs.
        let https = url::Url::parse("https://origin.example/").unwrap();
        let https_target: ResolvedTarget =
            ResolvedTarget::new(["127.0.0.1:443".parse::<SocketAddr>().unwrap()]).unwrap();
        assert!(
            base != ResolvedRouteKey::new(&https, &https_target, None),
            "scheme change must fragment resolved identity"
        );

        // Host differs: logical Host/TLS identity differs.
        let other_host = url::Url::parse("http://other.example/").unwrap();
        assert!(
            base != ResolvedRouteKey::new(&other_host, &target, None),
            "host change must fragment resolved identity"
        );

        // Effective port differs: origin/socket identity differs.
        let other_port = url::Url::parse("http://origin.example:8080/").unwrap();
        let other_port_target: ResolvedTarget =
            ResolvedTarget::new(["127.0.0.1:8080".parse::<SocketAddr>().unwrap()]).unwrap();
        assert!(
            base != ResolvedRouteKey::new(&other_port, &other_port_target, None),
            "effective port change must fragment resolved identity"
        );

        // One address differs: physical route differs.
        let other_addr: ResolvedTarget =
            ResolvedTarget::new(["127.0.0.2:80".parse::<SocketAddr>().unwrap()]).unwrap();
        assert!(
            base != ResolvedRouteKey::new(&origin, &other_addr, None),
            "address change must fragment resolved identity"
        );

        // Same addresses, different order: attempt/failover order differs.
        let ordered: ResolvedTarget = ResolvedTarget::new([
            "127.0.0.1:80".parse::<SocketAddr>().unwrap(),
            "127.0.0.2:80".parse::<SocketAddr>().unwrap(),
        ])
        .unwrap();
        let reordered: ResolvedTarget = ResolvedTarget::new([
            "127.0.0.2:80".parse::<SocketAddr>().unwrap(),
            "127.0.0.1:80".parse::<SocketAddr>().unwrap(),
        ])
        .unwrap();
        assert!(
            ResolvedRouteKey::new(&origin, &ordered, None)
                != ResolvedRouteKey::new(&origin, &reordered, None),
            "address reorder must fragment resolved identity"
        );

        // SNI None vs override: TLS identity differs.
        assert!(
            base != ResolvedRouteKey::new(&origin, &target, Some("sni.example")),
            "adding an SNI override must fragment resolved identity"
        );

        // SNI override A vs B: TLS identity differs.
        assert!(
            ResolvedRouteKey::new(&origin, &target, Some("a.example"))
                != ResolvedRouteKey::new(&origin, &target, Some("b.example")),
            "SNI override change must fragment resolved identity"
        );
        assert!(
            ResolvedRouteKey::new(&origin, &target, Some("a.example"))
                == ResolvedRouteKey::new(&origin, &target, Some("a.example")),
            "same SNI override must reuse"
        );
    }

    #[cfg(feature = "advanced-routing")]
    #[test]
    fn resolved_route_key_from_native_origin() {
        use crate::http_origin::HttpOrigin;
        use crate::transport_hints::ResolvedTarget;

        let uri: http::Uri = "http://origin.example/".parse().expect("valid URI");
        let origin = HttpOrigin::from_uri(&uri).expect("native origin parses");
        let target = ResolvedTarget::new(["127.0.0.1:80".parse::<SocketAddr>().unwrap()]).unwrap();
        let base = ResolvedRouteKey::from_origin(&origin, &target, None);
        let same = ResolvedRouteKey::from_origin(&origin, &target, None);
        // Keys intentionally lack `Debug`/`Display`; compare with `==`/`!=`.
        assert!(
            base == same,
            "same native origin + addresses + SNI must reuse"
        );

        // Explicit default port shares identity; scheme/host/port changes
        // fragment; address order and SNI fragment.
        let explicit: http::Uri = "http://origin.example:80/a?b=1".parse().expect("valid URI");
        let explicit_origin = HttpOrigin::from_uri(&explicit).expect("origin parses");
        assert!(
            base == ResolvedRouteKey::from_origin(&explicit_origin, &target, None),
            "explicit default port must reuse"
        );

        let https: http::Uri = "https://origin.example/".parse().expect("valid URI");
        let https_origin = HttpOrigin::from_uri(&https).expect("origin parses");
        let https_target =
            ResolvedTarget::new(["127.0.0.1:443".parse::<SocketAddr>().unwrap()]).unwrap();
        assert!(
            base != ResolvedRouteKey::from_origin(&https_origin, &https_target, None),
            "scheme change must fragment resolved identity"
        );
        assert!(
            base != ResolvedRouteKey::from_origin(&origin, &target, Some("sni.example")),
            "SNI override must fragment resolved identity"
        );
    }

    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
    #[cfg(feature = "high-level-url")]
    #[test]
    fn resolved_route_cache_is_bounded() {
        use crate::transport::hyper_client::{
            BoundedClientCache, RESOLVED_CLIENT_CACHE_MAX_ENTRIES,
        };
        use crate::transport_hints::ResolvedTarget;

        assert_eq!(
            RESOLVED_CLIENT_CACHE_MAX_ENTRIES, 64,
            "resolved-route cache uses the conservative 64-entry bound"
        );
        let mut cache: BoundedClientCache<ResolvedRouteKey, usize> =
            BoundedClientCache::new(RESOLVED_CLIENT_CACHE_MAX_ENTRIES);
        for index in 0..(RESOLVED_CLIENT_CACHE_MAX_ENTRIES + 16) {
            let origin =
                url::Url::parse(&format!("http://host-{index}.example/")).expect("valid URL");
            let target =
                ResolvedTarget::new(["127.0.0.1:80".parse::<SocketAddr>().expect("valid addr")])
                    .expect("non-empty");
            cache.insert(ResolvedRouteKey::new(&origin, &target, None), index);
            assert!(
                cache.len() <= RESOLVED_CLIENT_CACHE_MAX_ENTRIES,
                "insertion beyond capacity must not grow the cache"
            );
        }
        assert_eq!(cache.len(), RESOLVED_CLIENT_CACHE_MAX_ENTRIES);
    }
}
