//! Resource limits for the connection pool.
//!
//! Distinguishes between logical request concurrency (pool permits)
//! and physical connection behavior (hyper's internal pool).

use std::time::Duration;

use crate::pool::PoolConfig;

/// Resource limits for the connection pool.
///
/// Distinguishes between logical request concurrency (pool permits)
/// and physical connection behavior (hyper's internal pool).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Limits {
    /// Maximum concurrent logical requests (pool permits).
    /// Maps to pool `max_connections`.
    pub max_connections: Option<usize>,
    /// Maximum concurrent logical requests per origin.
    /// Maps to pool `max_connections_per_host`.
    pub max_connections_per_host: Option<usize>,
    /// Maximum number of idle (kept-alive) connections.
    /// Maps to hyper's pool idle connection limit.
    pub max_idle_connections: Option<usize>,
    /// Maximum number of idle connections per host.
    /// Maps to hyper's pool per-host idle limit.
    pub max_idle_connections_per_host: Option<usize>,
    /// Duration after which idle connections are closed.
    pub keepalive_expiry: Option<Duration>,
}

impl Limits {
    /// Create HTTPX-compatible limits.
    ///
    /// HTTPX 0.28.1 defaults to 100 total connections with a 5-second
    /// keepalive expiry. Per-host idle connections default to 20. No
    /// per-host concurrency limit is set.
    ///
    /// # Examples
    ///
    /// ```
    /// use eggfetch_core::Limits;
    ///
    /// let limits = Limits::compat();
    /// assert_eq!(limits.max_connections, Some(100));
    /// assert!(limits.max_connections_per_host.is_none());
    /// assert_eq!(limits.max_idle_connections, Some(20));
    /// assert_eq!(limits.max_idle_connections_per_host, Some(20));
    /// assert_eq!(limits.keepalive_expiry, Some(std::time::Duration::from_secs(5)));
    /// ```
    #[must_use]
    pub fn compat() -> Self {
        Self {
            max_connections: Some(100),
            max_connections_per_host: None,
            max_idle_connections: Some(20),
            max_idle_connections_per_host: Some(20),
            keepalive_expiry: Some(Duration::from_secs(5)),
        }
    }

    /// Create unlimited defaults (all `None`).
    ///
    /// No concurrency limits or idle connection caps are applied. Idle
    /// connections are never proactively closed.
    ///
    /// # Examples
    ///
    /// ```
    /// use eggfetch_core::Limits;
    ///
    /// let limits = Limits::native();
    /// assert!(limits.max_connections.is_none());
    /// assert!(limits.max_connections_per_host.is_none());
    /// assert!(limits.max_idle_connections.is_none());
    /// assert!(limits.max_idle_connections_per_host.is_none());
    /// assert!(limits.keepalive_expiry.is_none());
    /// ```
    #[must_use]
    pub fn native() -> Self {
        Self::default()
    }
}

impl From<Limits> for PoolConfig {
    fn from(limits: Limits) -> Self {
        Self {
            max_connections: limits.max_connections,
            max_connections_per_host: limits.max_connections_per_host,
            max_idle_connections: limits.max_idle_connections,
            max_idle_connections_per_host: limits.max_idle_connections_per_host,
            idle_timeout: limits.keepalive_expiry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits_are_all_none() {
        let limits = Limits::default();
        assert!(limits.max_connections.is_none());
        assert!(limits.max_connections_per_host.is_none());
        assert!(limits.max_idle_connections.is_none());
        assert!(limits.max_idle_connections_per_host.is_none());
        assert!(limits.keepalive_expiry.is_none());
    }

    #[test]
    fn compat_limits_match_httpx() {
        let limits = Limits::compat();
        assert_eq!(limits.max_connections, Some(100));
        assert!(limits.max_connections_per_host.is_none());
        assert_eq!(limits.max_idle_connections, Some(20));
        assert_eq!(limits.max_idle_connections_per_host, Some(20));
        assert_eq!(limits.keepalive_expiry, Some(Duration::from_secs(5)));
    }

    #[test]
    fn native_limits_are_unlimited() {
        let limits = Limits::native();
        assert_eq!(limits, Limits::default());
    }

    #[test]
    fn limits_into_pool_config() {
        let limits = Limits::compat();
        let config: PoolConfig = limits.into();
        assert_eq!(config.max_connections, Some(100));
        assert!(config.max_connections_per_host.is_none());
        assert_eq!(config.max_idle_connections, Some(20));
        assert_eq!(config.max_idle_connections_per_host, Some(20));
        assert_eq!(config.idle_timeout, Some(Duration::from_secs(5)));
    }

    #[test]
    fn native_limits_into_pool_config() {
        let limits = Limits::native();
        let config: PoolConfig = limits.into();
        assert_eq!(config.max_connections, None);
        assert_eq!(config.max_connections_per_host, None);
        assert_eq!(config.max_idle_connections, None);
        assert_eq!(config.max_idle_connections_per_host, None);
        assert_eq!(config.idle_timeout, None);
    }
}
