//! Resource limits for the connection pool.
//!
//! Distinguishes between logical request concurrency (pool permits)
//! and physical connection behavior (hyper's internal pool).

use std::time::Duration;

use crate::pool::PoolConfig;

/// Resource limits for the connection pool.
///
/// Distinguishes between logical request concurrency (pool permits,
/// one per in-flight request) and physical connection behavior
/// (Hyper's internal idle pool).
///
/// # Logical vs physical
///
/// - `max_in_flight_requests` / `max_in_flight_requests_per_origin` bound
///   logical requests. Under H1 one request usually owns its connection
///   slot; under H2/H3 many requests multiplex over one connection/QUIC
///   session while each still holds a permit.
/// - `max_idle_connections*` / `keepalive_expiry` are physical idle-pool
///   policy passed to Hyper, not concurrency bounds.
///
/// `max_connections` / `max_connections_per_host` are compatibility
/// aliases for the pre-1.0 line (same semantics as the new names; the
/// HTTPX facade keeps `Limits(max_connections=...)` unchanged). When both
/// alias and new name are set, the new name wins.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Limits {
    /// Maximum concurrent logical requests (pool permits).
    ///
    /// Compatibility alias for `max_in_flight_requests`. Maps to pool
    /// `max_connections`.
    pub max_connections: Option<usize>,
    /// Maximum concurrent logical requests per origin.
    ///
    /// Compatibility alias for `max_in_flight_requests_per_origin`. Maps
    /// to pool `max_connections_per_host`.
    pub max_connections_per_host: Option<usize>,
    /// Maximum concurrent in-flight requests (logical, preferred name).
    ///
    /// One permit per request, not one TCP connection. H1/H2/H3 examples:
    /// H1 serializes one request per connection; H2 multiplexes many
    /// streams over one TCP connection up to the server's
    /// `SETTINGS_MAX_CONCURRENT_STREAMS`; H3 multiplexes over QUIC with
    /// bidi streams derived from the per-origin limit.
    pub max_in_flight_requests: Option<usize>,
    /// Maximum concurrent in-flight requests per origin (logical,
    /// preferred name).
    pub max_in_flight_requests_per_origin: Option<usize>,
    /// Maximum number of idle (kept-alive) connections.
    /// Maps to hyper's pool idle connection limit (physical).
    pub max_idle_connections: Option<usize>,
    /// Maximum number of idle connections per host (physical).
    /// Maps to hyper's pool per-host idle limit.
    pub max_idle_connections_per_host: Option<usize>,
    /// Duration after which idle connections are closed (physical).
    pub keepalive_expiry: Option<Duration>,
}

impl Limits {
    /// Effective global logical-request limit (new name wins).
    #[must_use]
    pub fn effective_max_in_flight(&self) -> Option<usize> {
        self.max_in_flight_requests.or(self.max_connections)
    }

    /// Effective per-origin logical-request limit (new name wins).
    #[must_use]
    pub fn effective_max_in_flight_per_origin(&self) -> Option<usize> {
        self.max_in_flight_requests_per_origin
            .or(self.max_connections_per_host)
    }
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
            max_in_flight_requests: None,
            max_in_flight_requests_per_origin: None,
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
            max_in_flight_requests: limits.max_in_flight_requests,
            max_in_flight_requests_per_origin: limits.max_in_flight_requests_per_origin,
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
        assert!(limits.max_in_flight_requests.is_none());
        assert!(limits.max_in_flight_requests_per_origin.is_none());
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
        assert_eq!(config.max_in_flight_requests, None);
        assert_eq!(config.max_in_flight_requests_per_origin, None);
        assert_eq!(config.max_idle_connections, None);
        assert_eq!(config.max_idle_connections_per_host, None);
        assert_eq!(config.idle_timeout, None);
    }

    #[test]
    fn new_names_win_over_aliases() {
        let limits = Limits {
            max_connections: Some(10),
            max_in_flight_requests: Some(7),
            max_connections_per_host: Some(5),
            max_in_flight_requests_per_origin: Some(3),
            ..Limits::default()
        };
        assert_eq!(limits.effective_max_in_flight(), Some(7));
        assert_eq!(limits.effective_max_in_flight_per_origin(), Some(3));
        let config: PoolConfig = limits.into();
        assert_eq!(config.effective_max_in_flight(), Some(7));
        assert_eq!(config.effective_max_in_flight_per_origin(), Some(3));
    }

    #[test]
    fn aliases_apply_when_new_names_absent() {
        let limits = Limits {
            max_connections: Some(11),
            max_connections_per_host: Some(4),
            ..Limits::default()
        };
        assert_eq!(limits.effective_max_in_flight(), Some(11));
        assert_eq!(limits.effective_max_in_flight_per_origin(), Some(4));
    }
}
