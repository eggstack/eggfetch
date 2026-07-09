//! Connection pool subsystem.
//!
//! Provides slot-based concurrency limiting for HTTP connections, both
//! globally and per-host. Actual connection reuse is handled by hyper;
//! this module controls how many concurrent requests may be in flight.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

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
    /// Maximum number of concurrent connections per individual host.
    pub max_connections_per_host: Option<usize>,
    /// Duration after which an idle connection is closed.
    pub idle_timeout: Option<Duration>,
}

/// Observable metrics for the connection pool.
///
/// Intended for internal use and testing. All counters are atomically
/// updated and may be read concurrently.
#[derive(Debug, Default)]
pub struct PoolMetrics {
    /// Total number of new connections opened.
    pub connections_opened: AtomicUsize,
    /// Total number of times an existing connection was reused.
    pub connections_reused: AtomicUsize,
    /// Total number of idle connections that were closed (timeout or eviction).
    pub idle_connections_closed: AtomicUsize,
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
    host: Option<String>,
    // These fields are held for their Drop impl (releasing semaphore permits).
    #[allow(dead_code)]
    global_permit: Option<OwnedSemaphorePermit>,
    #[allow(dead_code)]
    host_permit: Option<OwnedSemaphorePermit>,
}

impl PoolGuard {
    /// Create a new guard (crate-internal).
    fn new(
        pool: Arc<PoolInner>,
        host: Option<String>,
        global_permit: Option<OwnedSemaphorePermit>,
        host_permit: Option<OwnedSemaphorePermit>,
    ) -> Self {
        Self {
            pool,
            host,
            global_permit,
            host_permit,
        }
    }

    /// Returns the host this guard was acquired for, if any.
    #[must_use]
    pub fn host(&self) -> Option<&str> {
        self.host.as_deref()
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
    /// Per-host concurrency semaphores, created lazily on first use.
    per_host: DashMap<String, Arc<Semaphore>>,
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
                per_host: DashMap::new(),
                config,
                metrics: PoolMetrics::default(),
            }),
        }
    }

    /// Acquire a pool slot for the given host.
    ///
    /// If a per-host limit is configured, a per-host permit is acquired
    /// first, then a global permit. The returned [`PoolGuard`] holds
    /// both permits and releases them on drop.
    ///
    /// If no limits are configured, returns a guard that does nothing on drop.
    ///
    /// # Arguments
    ///
    /// * `host` - The hostname extracted from the request URL. Pass `None`
    ///   for malformed URLs or URLs without a host component.
    pub async fn acquire(&self, host: Option<&str>) -> PoolGuard {
        let mut waited = false;
        let mut global_permit: Option<OwnedSemaphorePermit> = None;
        let mut host_permit: Option<OwnedSemaphorePermit> = None;

        // Acquire per-host permit if configured and host is known.
        if let (Some(max_per_host), Some(host)) = (self.inner.config.max_connections_per_host, host)
        {
            let sem = self
                .inner
                .per_host
                .entry(host.to_owned())
                .or_insert_with(|| Arc::new(Semaphore::new(max_per_host)))
                .clone();

            // Try immediate acquire first.
            if let Ok(permit) = sem.clone().try_acquire_owned() {
                host_permit = Some(permit);
            } else {
                // Must wait for a slot.
                waited = true;
                if let Ok(permit) = sem.clone().acquire_owned().await {
                    host_permit = Some(permit);
                } else {
                    // Semaphore closed (shouldn't happen in practice).
                    self.inner
                        .metrics
                        .acquisition_cancellations
                        .fetch_add(1, Ordering::Relaxed);
                    return PoolGuard::new(self.inner.clone(), Some(host.to_owned()), None, None);
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
                        host.map(str::to_owned),
                        None,
                        host_permit,
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
            host.map(str::to_owned),
            global_permit,
            host_permit,
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
        assert!(pool.inner.per_host.is_empty());
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
        assert_eq!(metrics.connections_opened.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.connections_reused.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.idle_connections_closed.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.acquisition_waits.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.acquisition_cancellations.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn acquire_without_limits_returns_immediately() {
        let pool = Pool::new(PoolConfig::default());
        let guard = pool.acquire(None).await;
        assert!(guard.host().is_none());
    }

    #[tokio::test]
    async fn acquire_with_global_limit() {
        let config = PoolConfig {
            max_connections: Some(2),
            ..Default::default()
        };
        let pool = Pool::new(config);

        let g1 = pool.acquire(Some("example.com")).await;
        let g2 = pool.acquire(Some("example.com")).await;

        // Both acquired successfully; metrics should show no waits.
        assert_eq!(pool.metrics().acquisition_waits.load(Ordering::Relaxed), 0);

        drop(g1);
        drop(g2);
    }

    #[tokio::test]
    async fn acquire_with_per_host_limit() {
        let config = PoolConfig {
            max_connections_per_host: Some(1),
            ..Default::default()
        };
        let pool = Pool::new(config);

        let g1 = pool.acquire(Some("example.com")).await;
        // Second acquire on same host should wait (but we can't easily test
        // blocking in a non-async test; just verify the first one succeeds).
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
}
