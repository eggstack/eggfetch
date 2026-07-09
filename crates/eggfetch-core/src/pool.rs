//! Connection pool subsystem.
//!
//! Provides slot-based concurrency limiting for HTTP connections, both
//! globally and per-origin. Actual connection reuse is handled by hyper;
//! this module controls how many concurrent requests may be in flight.
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

use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Configuration for the connection pool.
///
/// All fields are optional. When a field is `None`, the corresponding
/// limit is not applied.
#[derive(Debug, Clone, Default)]
pub struct PoolConfig {
    /// Maximum number of idle (unused) connections to keep in the pool.
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
/// when the URL does not specify a port).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct OriginKey {
    /// URL scheme (`http` or `https`).
    scheme: String,
    /// Hostname.
    host: String,
    /// Effective port (explicit or default-for-scheme).
    port: u16,
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
}

impl fmt::Display for OriginKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://{}:{}", self.scheme, self.host, self.port)
    }
}

/// Observable metrics for the connection pool.
///
/// All counters are atomically updated and may be read concurrently.
///
/// # What is measured
///
/// These counters track **logical permits** held by the pool, not raw
/// TCP sockets. Hyper owns socket lifecycle; eggfetch cannot observe
/// individual socket open/reuse/close events through its current
/// integration. If you need socket-level metrics, those would have to
/// be added at a custom connector layer.
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
            let sem = self
                .inner
                .per_origin
                .entry(origin.clone())
                .or_insert_with(|| Arc::new(Semaphore::new(max_per_origin)))
                .clone();

            // Try immediate acquire first.
            if let Ok(permit) = sem.clone().try_acquire_owned() {
                origin_permit = Some(permit);
            } else {
                // Must wait for a slot.
                waited = true;
                if let Ok(permit) = sem.clone().acquire_owned().await {
                    origin_permit = Some(permit);
                } else {
                    // Semaphore closed (shouldn't happen in practice).
                    self.inner
                        .metrics
                        .acquisition_cancellations
                        .fetch_add(1, Ordering::Relaxed);
                    return PoolGuard::new(self.inner.clone(), Some(origin.clone()), None, None);
                }
            }
        }

        // Acquire global permit if configured.
        if let Some(ref sem) = self.inner.global_semaphore {
            // Try immediate acquire first.
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
}
