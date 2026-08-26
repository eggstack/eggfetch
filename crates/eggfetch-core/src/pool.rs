//! Connection pool subsystem.
//!
//! Provides slot-based concurrency limiting for HTTP connections, both
//! globally and per-origin. Actual connection reuse is handled by hyper;
//! this module controls how many concurrent requests may be in flight.
//!
//! # Concurrency model
//!
//! eggfetch's pool enforces **logical request concurrency** — the number
//! of concurrent requests the application may have in flight. This is
//! distinct from physical connection counts and, under HTTP/2, from
//! per-connection stream concurrency:
//!
//! - **Logical request concurrency**: Controlled by `max_connections`
//!   and `max_connections_per_host` in this pool. A single semaphore
//!   permit corresponds to one logical request.
//!
//! - **Physical connection count**: Owned by hyper's internal connection
//!   pool. eggfetch cannot observe or control this directly. Under
//!   HTTP/1.1, one connection carries one request at a time. Under
//!   HTTP/2, one connection carries many multiplexed streams.
//!
//! - **Per-origin HTTP/2 stream concurrency**: hyper/h2 respects the
//!   server's `SETTINGS_MAX_CONCURRENT_STREAMS` advertisement and its
//!   own internal limits. eggfetch does not expose or override these.
//!   The logical request limit acts as an upper bound on concurrent
//!   streams because each in-flight request holds a pool permit.
//!
//! - **Per-connection stream limit**: The h2 library enforces a default
//!   of 100 concurrent streams per connection, or whatever the server
//!   advertises via `SETTINGS_MAX_CONCURRENT_STREAMS`. This is
//!   transparent to eggfetch — hyper handles stream multiplexing within
//!   a connection automatically.
//!
//! Under HTTP/2, multiple logical requests may share a single TCP
//! connection (multiplexed streams). The pool's per-origin limit still
//! applies: it bounds the number of concurrent requests, not the number
//! of connections. If the h2 connection's stream limit is reached,
//! hyper internally queues streams until a slot opens, which may cause
//! pool acquisition to wait.
//!
//! # HTTP/3 and QUIC
//!
//! When the `http3` feature is enabled and `HttpVersionPolicy::Http3Only`
//! is selected, requests bypass the hyper transport and are sent over QUIC
//! via Quinn. These requests still acquire pool permits for concurrency
//! limiting, but the underlying QUIC connection lifecycle is managed
//! independently by the H3 connector.
//!
//! - **Logical request concurrency**: Same as HTTP/1.1 and HTTP/2 — one
//!   pool permit per in-flight request.
//! - **QUIC connection caching**: The H3 connector maintains a per-origin
//!   cache of QUIC connections. Idle connections are reused for subsequent
//!   requests to the same origin.
//! - **Per-connection stream concurrency**: Quinn enforces a maximum of
//!   100 concurrent bidirectional streams per QUIC connection by default.
//!   This is independent of the pool's logical permit limit.
//!
//! # Origin keying
//!
//! Per-origin limits are keyed by `(scheme, host, port)`, where the port
//! uses the scheme's default if not explicitly provided:
//!
//! - `http://example.com:80` shares a limit with `http://example.com`
//!   because the default port for `http` is 80.
//! - `http://example.com` and `https://example.com` are distinct origins
//!   and have independent per-origin limits.
//! - `http://example.com:8080` is a distinct origin from
//!   `http://example.com`.
//!
//! When a proxy is involved, the pool key extends to
//! `(proxy_origin, destination_origin, tunnel_mode)`. This means:
//!
//! - Direct and proxied requests to the same destination have
//!   independent concurrency slots.
//! - Different proxies sharing the same destination get independent
//!   slots.
//! - HTTP forwarding and HTTPS CONNECT tunneling through the same
//!   proxy are keyed separately.

use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Upper bound on tracked per-origin semaphores.
///
/// Long-lived processes touching many distinct origins (crawlers,
/// aggregators) would otherwise grow this table without limit. When the
/// cap is exceeded, idle entries (all permits available) are evicted;
/// in-flight entries are never dropped because holders own `Arc` clones
/// of the semaphore and a later request simply recreates the entry.
const PER_ORIGIN_TABLE_MAX_ENTRIES: usize = 1024;

/// Configuration for the connection pool.
///
/// All fields are optional. When a field is `None`, the corresponding
/// limit is not applied.
#[derive(Debug, Clone, Default)]
pub struct PoolConfig {
    /// Maximum number of idle (unused) connections kept per host.
    ///
    /// Applied as a per-host cap (the transport has no global idle
    /// limit); see [`Self::max_connections`] for a global bound.
    pub max_idle_connections: Option<usize>,
    /// Maximum number of idle connections per individual host.
    pub max_idle_connections_per_host: Option<usize>,
    /// Maximum total number of concurrent connections (active + idle).
    pub max_connections: Option<usize>,
    /// Maximum number of concurrent connections per individual origin.
    ///
    /// Origins are keyed by `(scheme, host, port)`. Two URLs with
    /// different schemes or ports are distinct origins.
    pub max_connections_per_host: Option<usize>,
    /// Duration after which an idle connection is closed.
    pub idle_timeout: Option<std::time::Duration>,
}

/// Origin key used for per-host pool slot acquisition.
///
/// Combines scheme, host, and effective port (using the scheme default
/// when the URL does not specify a port). When a proxy is involved,
/// the proxy endpoint and tunnel mode are also part of the key so that
/// different proxies or tunnel vs direct connections get independent
/// concurrency slots.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct OriginKey {
    /// URL scheme (`http` or `https`).
    scheme: String,
    /// Hostname.
    host: String,
    /// Effective port (explicit or default-for-scheme).
    port: u16,
    /// Proxy hostname, if routed through a proxy.
    proxy_host: Option<String>,
    /// Proxy port, if routed through a proxy.
    proxy_port: Option<u16>,
    /// Proxy endpoint scheme, which distinguishes plain and TLS-to-proxy
    /// routes even when host and port are identical.
    proxy_scheme: Option<String>,
    /// Whether this is a CONNECT tunnel (HTTPS through proxy) as
    /// opposed to HTTP forwarding.
    is_tunnel: bool,
}

impl OriginKey {
    /// Build an `OriginKey` from a scheme and `url::Url`.
    pub(crate) fn from_url(scheme: &str, url: &url::Url) -> Option<Self> {
        let host = url.host_str()?.to_owned();
        let port = url.port_or_known_default()?;
        Some(Self {
            scheme: scheme.to_owned(),
            host,
            port,
            proxy_host: None,
            proxy_port: None,
            proxy_scheme: None,
            is_tunnel: false,
        })
    }

    /// Build an `OriginKey` that includes proxy route information.
    #[allow(dead_code)]
    pub(crate) fn from_url_with_proxy(
        scheme: &str,
        url: &url::Url,
        proxy_host: Option<&str>,
        proxy_port: Option<u16>,
        is_tunnel: bool,
    ) -> Option<Self> {
        Self::from_url_with_proxy_scheme(scheme, url, proxy_host, proxy_port, None, is_tunnel)
    }

    /// Build an origin key including the proxy endpoint scheme.
    #[allow(dead_code)]
    pub(crate) fn from_url_with_proxy_scheme(
        scheme: &str,
        url: &url::Url,
        proxy_host: Option<&str>,
        proxy_port: Option<u16>,
        proxy_scheme: Option<&str>,
        is_tunnel: bool,
    ) -> Option<Self> {
        let host = url.host_str()?.to_owned();
        let port = url.port_or_known_default()?;
        Some(Self {
            scheme: scheme.to_owned(),
            host,
            port,
            proxy_host: proxy_host.map(str::to_owned),
            proxy_port,
            proxy_scheme: proxy_scheme.map(str::to_owned),
            is_tunnel,
        })
    }

    /// Build an `OriginKey` for tests or callers that already have the
    /// components.
    #[allow(dead_code)] // Kept for future callers; tests cover the path via from_url.
    pub(crate) fn from_parts(scheme: &str, host: &str, port: u16) -> Self {
        Self {
            scheme: scheme.to_owned(),
            host: host.to_owned(),
            port,
            proxy_host: None,
            proxy_port: None,
            proxy_scheme: None,
            is_tunnel: false,
        }
    }

    /// Build an `OriginKey` with proxy route info for tests or callers
    /// that already have all components.
    #[cfg(feature = "proxy")]
    #[allow(dead_code)]
    pub(crate) fn from_parts_with_proxy(
        scheme: &str,
        host: &str,
        port: u16,
        proxy_host: Option<&str>,
        proxy_port: Option<u16>,
        is_tunnel: bool,
    ) -> Self {
        Self {
            scheme: scheme.to_owned(),
            host: host.to_owned(),
            port,
            proxy_host: proxy_host.map(str::to_owned),
            proxy_port,
            proxy_scheme: None,
            is_tunnel,
        }
    }

    /// Returns the scheme.
    #[allow(dead_code)]
    pub(crate) fn scheme(&self) -> &str {
        &self.scheme
    }

    /// Returns the host.
    pub(crate) fn host(&self) -> &str {
        &self.host
    }

    /// Returns the effective port.
    #[allow(dead_code)] // Exposed for future diagnostics and metrics.
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    /// Returns the proxy hostname, if any.
    #[allow(dead_code)]
    pub(crate) fn proxy_host(&self) -> Option<&str> {
        self.proxy_host.as_deref()
    }

    /// Returns the proxy port, if any.
    #[allow(dead_code)]
    pub(crate) fn proxy_port(&self) -> Option<u16> {
        self.proxy_port
    }

    /// Returns whether this key represents a CONNECT tunnel.
    #[allow(dead_code)]
    pub(crate) fn is_tunnel(&self) -> bool {
        self.is_tunnel
    }
}

impl fmt::Display for OriginKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://{}:{}", self.scheme, self.host, self.port)?;
        if let (Some(ref ph), Some(pp)) = (&self.proxy_host, self.proxy_port) {
            if let Some(ref scheme) = self.proxy_scheme {
                write!(f, " via {scheme}://{ph}:{pp}")?;
            } else {
                write!(f, " via {ph}:{pp}")?;
            }
            if self.is_tunnel {
                write!(f, " tunnel")?;
            }
        }
        Ok(())
    }
}

/// Observable metrics for the connection pool.
///
/// All counters are atomically updated and may be read concurrently.
///
/// # What is measured
///
/// These counters track **logical permits** held by the pool, not raw
/// TCP sockets or HTTP/2 streams. Hyper owns socket lifecycle and h2
/// stream multiplexing; eggfetch cannot observe individual socket
/// open/reuse/close events or per-connection stream counts through its
/// current integration.
///
/// Under HTTP/2, a single TCP connection may carry multiple concurrent
/// streams, but the pool still tracks one permit per logical request.
/// The server's `SETTINGS_MAX_CONCURRENT_STREAMS` limit is enforced
/// internally by hyper/h2, not by this pool. If the stream limit is
/// reached, hyper queues new streams, which may cause pool acquisition
/// to wait even though logical permit slots are available.
///
/// If you need socket-level or stream-level metrics, those would have
/// to be added at a custom connector layer or by instrumenting h2
/// directly.
#[derive(Debug, Default)]
pub struct PoolMetrics {
    /// Total number of times an acquire call had to wait for a permit.
    pub acquisition_waits: AtomicUsize,
    /// Total number of times an acquire call was cancelled while waiting.
    pub acquisition_cancellations: AtomicUsize,
}

/// RAII guard representing an acquired pool slot.
///
/// Dropping the guard releases all held semaphore permits back to the pool.
pub struct PoolGuard {
    pool: Arc<PoolInner>,
    pub(crate) origin: Option<OriginKey>,
    // These fields are held for their Drop impl (releasing semaphore permits).
    // The `dead_code` allow is justified: the field value is read implicitly
    // by `OwnedSemaphorePermit::drop`, not by source code.
    #[allow(dead_code)]
    pub(crate) global_permit: Option<OwnedSemaphorePermit>,
    #[allow(dead_code)]
    pub(crate) origin_permit: Option<OwnedSemaphorePermit>,
}

impl PoolGuard {
    /// Create a new guard (crate-internal).
    fn new(
        pool: Arc<PoolInner>,
        origin: Option<OriginKey>,
        global_permit: Option<OwnedSemaphorePermit>,
        origin_permit: Option<OwnedSemaphorePermit>,
    ) -> Self {
        Self {
            pool,
            origin,
            global_permit,
            origin_permit,
        }
    }

    /// Returns the origin this guard was acquired for, if any.
    #[must_use]
    #[allow(dead_code)] // Exposed for future diagnostics and metrics.
    pub(crate) fn origin(&self) -> Option<&OriginKey> {
        self.origin.as_ref()
    }

    /// Returns the host portion of the origin, if any.
    #[allow(dead_code)]
    pub fn host(&self) -> Option<&str> {
        self.origin.as_ref().map(OriginKey::host)
    }

    /// Returns a reference to the pool metrics.
    #[must_use]
    pub fn metrics(&self) -> &PoolMetrics {
        &self.pool.metrics
    }
}

impl Drop for PoolGuard {
    fn drop(&mut self) {
        // Both permits are dropped automatically by OwnedSemaphorePermit::drop,
        // releasing the slots back to their respective semaphores.
    }
}

/// Shared state for the connection pool.
struct PoolInner {
    /// Global concurrency semaphore. `None` when no global limit is set.
    global_semaphore: Option<Arc<Semaphore>>,
    /// Per-origin concurrency semaphores, created lazily on first use.
    per_origin: DashMap<OriginKey, Arc<Semaphore>>,
    /// Pool configuration.
    config: PoolConfig,
    /// Observable metrics.
    metrics: PoolMetrics,
}

/// Connection pool that limits concurrent requests.
///
/// The pool does **not** manage actual TCP connections; hyper handles
/// that. It only enforces concurrency limits via semaphores.
///
/// # Example
///
/// ```
/// use eggfetch_core::pool::{Pool, PoolConfig};
///
/// let pool = Pool::new(PoolConfig {
///     max_connections: Some(100),
///     max_connections_per_host: Some(10),
///     ..Default::default()
/// });
/// ```
#[derive(Clone)]
pub struct Pool {
    inner: Arc<PoolInner>,
}

impl Pool {
    /// Create a new pool from the given configuration.
    #[must_use]
    pub fn new(config: PoolConfig) -> Self {
        let global_semaphore = config.max_connections.map(|n| Arc::new(Semaphore::new(n)));

        Self {
            inner: Arc::new(PoolInner {
                global_semaphore,
                per_origin: DashMap::new(),
                config,
                metrics: PoolMetrics::default(),
            }),
        }
    }

    /// Returns the configured idle-connection timeout, if any.
    ///
    /// Transport paths that build their own hyper client (SNI override)
    /// use this instead of the request `Timeout.pool`/`total` phases,
    /// which mean "wait to acquire", not "idle lifetime".
    #[must_use]
    pub(crate) fn idle_timeout(&self) -> Option<std::time::Duration> {
        self.inner.config.idle_timeout
    }

    /// Acquire a pool slot for the given origin.
    ///
    /// If a per-origin limit is configured, a per-origin permit is
    /// acquired first, then a global permit. The returned [`PoolGuard`]
    /// holds both permits and releases them on drop.
    ///
    /// If no limits are configured, returns a guard that does nothing on drop.
    ///
    /// # Arguments
    ///
    /// * `origin` - The origin derived from the request URL. Pass `None`
    ///   for malformed URLs or URLs without a host component.
    pub(crate) async fn acquire(&self, origin: Option<&OriginKey>) -> PoolGuard {
        let mut waited = false;
        let mut global_permit: Option<OwnedSemaphorePermit> = None;
        let mut origin_permit: Option<OwnedSemaphorePermit> = None;

        // Acquire per-origin permit if configured and origin is known.
        if let (Some(max_per_origin), Some(origin)) =
            (self.inner.config.max_connections_per_host, origin)
        {
            // Bound table growth before inserting so the entry created
            // below can never be evicted immediately after creation
            // (which would let the next request to this origin build a
            // fresh semaphore and briefly bypass the per-host limit).
            if self.inner.per_origin.len() >= PER_ORIGIN_TABLE_MAX_ENTRIES {
                self.inner
                    .per_origin
                    .retain(|_, sem| sem.available_permits() < max_per_origin);
            }

            let sem = self
                .inner
                .per_origin
                .entry(origin.clone())
                .or_insert_with(|| Arc::new(Semaphore::new(max_per_origin)))
                .clone();

            // Try immediate acquire first. `try_acquire_owned` consumes an
            // `Arc`, so the fast path clones once; the wait path then moves
            // `sem` into `acquire_owned` without a further bump.
            if let Ok(permit) = sem.clone().try_acquire_owned() {
                origin_permit = Some(permit);
            } else {
                // Must wait for a slot.
                waited = true;
                if let Ok(permit) = sem.acquire_owned().await {
                    origin_permit = Some(permit);
                } else {
                    // Semaphore closed (shouldn't happen in practice).
                    // Record it and continue without an origin permit so
                    // the request remains bounded by at least the global
                    // concurrency limit instead of proceeding with no
                    // permits at all.
                    self.inner
                        .metrics
                        .acquisition_cancellations
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        // Acquire global permit if configured.
        if let Some(ref sem) = self.inner.global_semaphore {
            // Try immediate acquire first; on failure, move the `Arc` into
            // the awaited acquire instead of cloning again.
            if let Ok(permit) = sem.clone().try_acquire_owned() {
                global_permit = Some(permit);
            } else {
                // Must wait for a slot.
                waited = true;
                if let Ok(permit) = sem.clone().acquire_owned().await {
                    global_permit = Some(permit);
                } else {
                    self.inner
                        .metrics
                        .acquisition_cancellations
                        .fetch_add(1, Ordering::Relaxed);
                    return PoolGuard::new(
                        self.inner.clone(),
                        origin.cloned(),
                        None,
                        origin_permit,
                    );
                }
            }
        }

        if waited {
            self.inner
                .metrics
                .acquisition_waits
                .fetch_add(1, Ordering::Relaxed);
        }

        PoolGuard::new(
            self.inner.clone(),
            origin.cloned(),
            global_permit,
            origin_permit,
        )
    }

    /// Returns a reference to the pool metrics.
    #[must_use]
    pub fn metrics(&self) -> &PoolMetrics {
        &self.inner.metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_no_limits() {
        let config = PoolConfig::default();
        assert!(config.max_idle_connections.is_none());
        assert!(config.max_idle_connections_per_host.is_none());
        assert!(config.max_connections.is_none());
        assert!(config.max_connections_per_host.is_none());
        assert!(config.idle_timeout.is_none());
    }

    #[test]
    fn pool_constructs_with_defaults() {
        let pool = Pool::new(PoolConfig::default());
        assert!(pool.inner.global_semaphore.is_none());
        assert!(pool.inner.per_origin.is_empty());
    }

    #[test]
    fn pool_constructs_with_global_limit() {
        let config = PoolConfig {
            max_connections: Some(10),
            ..Default::default()
        };
        let pool = Pool::new(config);
        assert!(pool.inner.global_semaphore.is_some());
    }

    #[test]
    fn pool_metrics_starts_zeroed() {
        let pool = Pool::new(PoolConfig::default());
        let metrics = pool.metrics();
        assert_eq!(metrics.acquisition_waits.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.acquisition_cancellations.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn acquire_without_limits_returns_immediately() {
        let pool = Pool::new(PoolConfig::default());
        let guard = pool.acquire(None).await;
        assert!(guard.origin().is_none());
    }

    #[tokio::test]
    async fn acquire_with_global_limit() {
        let config = PoolConfig {
            max_connections: Some(2),
            ..Default::default()
        };
        let pool = Pool::new(config);

        let origin = OriginKey::from_parts("http", "example.com", 80);
        let g1 = pool.acquire(Some(&origin)).await;
        let g2 = pool.acquire(Some(&origin)).await;

        // Both acquired successfully; metrics should show no waits.
        assert_eq!(pool.metrics().acquisition_waits.load(Ordering::Relaxed), 0);

        drop(g1);
        drop(g2);
    }

    #[tokio::test]
    async fn acquire_with_per_origin_limit() {
        let config = PoolConfig {
            max_connections_per_host: Some(1),
            ..Default::default()
        };
        let pool = Pool::new(config);

        let origin = OriginKey::from_parts("http", "example.com", 80);
        let g1 = pool.acquire(Some(&origin)).await;
        assert_eq!(pool.metrics().acquisition_waits.load(Ordering::Relaxed), 0);

        drop(g1);
    }

    #[test]
    fn pool_is_clone() {
        let pool = Pool::new(PoolConfig::default());
        let pool2 = pool.clone();
        // Both share the same inner state.
        assert!(std::ptr::eq(&*pool.inner, &*pool2.inner));
    }

    #[tokio::test]
    async fn acquire_per_origin_limit_separate_origins() {
        let config = PoolConfig {
            max_connections_per_host: Some(1),
            ..Default::default()
        };
        let pool = Pool::new(config);
        let o1 = OriginKey::from_parts("http", "host-a", 80);
        let o2 = OriginKey::from_parts("http", "host-b", 80);
        let g1 = pool.acquire(Some(&o1)).await;
        let g2 = pool.acquire(Some(&o2)).await;
        assert!(g1.origin().is_some());
        assert!(g2.origin().is_some());
        drop(g1);
        drop(g2);
    }

    #[test]
    fn origin_key_from_url_http() {
        let url = url::Url::parse("http://example.com:8080/path").unwrap();
        let key = OriginKey::from_url("http", &url).unwrap();
        assert_eq!(key.scheme(), "http");
        assert_eq!(key.host(), "example.com");
        assert_eq!(key.port(), 8080);
    }

    #[test]
    fn origin_key_from_url_https_default_port() {
        let url = url::Url::parse("https://example.com/path").unwrap();
        let key = OriginKey::from_url("https", &url).unwrap();
        assert_eq!(key.scheme(), "https");
        assert_eq!(key.host(), "example.com");
        assert_eq!(key.port(), 443);
    }

    #[test]
    fn origin_key_from_url_http_default_port() {
        let url = url::Url::parse("http://example.com/path").unwrap();
        let key = OriginKey::from_url("http", &url).unwrap();
        assert_eq!(key.scheme(), "http");
        assert_eq!(key.port(), 80);
    }

    #[test]
    fn origin_keys_distinguish_scheme() {
        let http = OriginKey::from_parts("http", "example.com", 80);
        let https = OriginKey::from_parts("https", "example.com", 443);
        assert_ne!(http, https);
    }

    #[test]
    fn origin_keys_distinguish_port() {
        let p80 = OriginKey::from_parts("http", "example.com", 80);
        let p8080 = OriginKey::from_parts("http", "example.com", 8080);
        assert_ne!(p80, p8080);
    }

    #[test]
    fn origin_keys_equal_for_same_origin() {
        let a = OriginKey::from_parts("http", "example.com", 80);
        let b = OriginKey::from_parts("http", "example.com", 80);
        assert_eq!(a, b);
    }

    #[test]
    fn origin_key_display() {
        let key = OriginKey::from_parts("https", "example.com", 443);
        assert_eq!(key.to_string(), "https://example.com:443");
    }

    #[tokio::test]
    async fn origin_keys_separate_per_origin_map_entries() {
        let config = PoolConfig {
            max_connections_per_host: Some(1),
            ..Default::default()
        };
        let pool = Pool::new(config);
        let a = OriginKey::from_parts("http", "example.com", 80);
        let b = OriginKey::from_parts("http", "example.com", 8080);
        assert!(pool.acquire(Some(&a)).await.origin().is_some());
        assert!(pool.acquire(Some(&b)).await.origin().is_some());
    }

    #[test]
    fn origin_keys_distinct_by_scheme() {
        // Same host and port, different scheme = different origin.
        let http = OriginKey::from_parts("http", "example.com", 443);
        let https = OriginKey::from_parts("https", "example.com", 443);
        assert_ne!(http, https);
    }

    #[test]
    fn origin_key_from_url_uses_scheme_default_port() {
        // URL with explicit port is honored; URL without port uses scheme default.
        let explicit = url::Url::parse("http://example.com:8080/path").unwrap();
        let default = url::Url::parse("http://example.com/path").unwrap();
        let k_explicit = OriginKey::from_url("http", &explicit).unwrap();
        let k_default = OriginKey::from_url("http", &default).unwrap();
        assert_eq!(k_explicit.port(), 8080);
        assert_eq!(k_default.port(), 80);
    }

    #[test]
    fn origin_key_direct_has_no_proxy() {
        let key = OriginKey::from_parts("http", "example.com", 80);
        assert!(key.proxy_host().is_none());
        assert!(key.proxy_port().is_none());
        assert!(!key.is_tunnel());
    }

    #[cfg(feature = "proxy")]
    #[test]
    fn origin_key_proxy_a_not_equal_proxy_b() {
        let url = url::Url::parse("http://example.com/path").unwrap();
        let a = OriginKey::from_url_with_proxy("http", &url, Some("proxy-a"), Some(8080), false)
            .unwrap();
        let b = OriginKey::from_url_with_proxy("http", &url, Some("proxy-b"), Some(8080), false)
            .unwrap();
        assert_ne!(a, b);
    }

    #[cfg(feature = "proxy")]
    #[test]
    fn origin_key_tunnel_not_equal_non_tunnel() {
        let url = url::Url::parse("http://example.com/path").unwrap();
        let tunnel =
            OriginKey::from_url_with_proxy("http", &url, Some("proxy"), Some(8080), true).unwrap();
        let direct =
            OriginKey::from_url_with_proxy("http", &url, Some("proxy"), Some(8080), false).unwrap();
        assert_ne!(tunnel, direct);
    }

    #[cfg(feature = "proxy")]
    #[test]
    fn origin_key_proxy_same_route_equal() {
        let url = url::Url::parse("http://example.com/path").unwrap();
        let a =
            OriginKey::from_url_with_proxy("http", &url, Some("proxy"), Some(8080), false).unwrap();
        let b =
            OriginKey::from_url_with_proxy("http", &url, Some("proxy"), Some(8080), false).unwrap();
        assert_eq!(a, b);
    }

    #[cfg(feature = "proxy")]
    #[test]
    fn origin_key_display_with_proxy() {
        let url = url::Url::parse("https://example.com/path").unwrap();
        let key =
            OriginKey::from_url_with_proxy("https", &url, Some("proxy"), Some(8080), true).unwrap();
        assert_eq!(
            key.to_string(),
            "https://example.com:443 via proxy:8080 tunnel"
        );
    }

    #[cfg(feature = "proxy")]
    #[test]
    fn origin_key_display_without_tunnel() {
        let url = url::Url::parse("http://example.com/path").unwrap();
        let key =
            OriginKey::from_url_with_proxy("http", &url, Some("proxy"), Some(8080), false).unwrap();
        assert_eq!(key.to_string(), "http://example.com:80 via proxy:8080");
    }

    #[cfg(feature = "proxy")]
    #[tokio::test]
    async fn proxy_route_separate_from_direct() {
        let config = PoolConfig {
            max_connections_per_host: Some(1),
            ..Default::default()
        };
        let pool = Pool::new(config);
        let direct = OriginKey::from_parts("http", "example.com", 80);
        let proxied = OriginKey::from_parts_with_proxy(
            "http",
            "example.com",
            80,
            Some("proxy"),
            Some(8080),
            false,
        );
        let g1 = pool.acquire(Some(&direct)).await;
        let g2 = pool.acquire(Some(&proxied)).await;
        assert!(g1.origin().is_some());
        assert!(g2.origin().is_some());
    }

    #[cfg(feature = "proxy")]
    #[tokio::test]
    async fn tunnel_separate_from_non_tunnel() {
        let config = PoolConfig {
            max_connections_per_host: Some(1),
            ..Default::default()
        };
        let pool = Pool::new(config);
        let http_fwd = OriginKey::from_parts_with_proxy(
            "http",
            "example.com",
            80,
            Some("proxy"),
            Some(8080),
            false,
        );
        let https_tunnel = OriginKey::from_parts_with_proxy(
            "http",
            "example.com",
            80,
            Some("proxy"),
            Some(8080),
            true,
        );
        let g1 = pool.acquire(Some(&http_fwd)).await;
        let g2 = pool.acquire(Some(&https_tunnel)).await;
        assert!(g1.origin().is_some());
        assert!(g2.origin().is_some());
    }

    #[cfg(feature = "proxy")]
    #[tokio::test]
    async fn different_proxies_separate_semaphores() {
        let config = PoolConfig {
            max_connections_per_host: Some(1),
            ..Default::default()
        };
        let pool = Pool::new(config);
        let a = OriginKey::from_parts_with_proxy(
            "http",
            "example.com",
            80,
            Some("proxy-a"),
            Some(8080),
            false,
        );
        let b = OriginKey::from_parts_with_proxy(
            "http",
            "example.com",
            80,
            Some("proxy-b"),
            Some(8080),
            false,
        );
        let g1 = pool.acquire(Some(&a)).await;
        let g2 = pool.acquire(Some(&b)).await;
        assert!(g1.origin().is_some());
        assert!(g2.origin().is_some());
    }
}
