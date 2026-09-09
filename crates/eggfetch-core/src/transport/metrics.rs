//! Transport observability counters.
//!
//! [`TransportMetrics`] counts connector and protocol events where the
//! engine can observe them reliably. It is deliberately separate from
//! [`crate::pool::PoolMetrics`], which counts logical pool-permit
//! waits/cancellations.
//!
//! # What is (and is not) counted
//!
//! - **Connector events**: `*_connector_attempts/successes/failures`,
//!   `*_dns_attempts/failures`, `*_tls_attempts/failures`,
//!   `proxy_*`, `uds_*`. One increment per connector `call` or per
//!   DNS/TLS phase within that call.
//! - **Protocol connections**: `h3_connections_created`,
//!   `upgraded_connections_created`. One per established QUIC/H3 session
//!   or captured 101 upgrade.
//! - **Logical requests** are counted by `PoolMetrics`, not here.
//!
//! Hyper's internal socket-reuse counts and per-connection H2 stream
//! counts are intentionally absent: the legacy pool owns socket lifecycle
//! and eggfetch cannot observe reuse reliably. No estimate is reported.
//!
//! All counters are `AtomicUsize` with `Relaxed` ordering (low overhead,
//! concurrency-safe). Tests assert exact counts in deterministic local
//! scenarios (loopback listeners, blackholed UDP for H3).

use std::sync::atomic::{AtomicUsize, Ordering};

/// Observable transport counters.
///
/// Shared via `Arc` across connectors owned by one [`crate::Client`].
/// Cloning the `Arc` is cheap; all methods are lock-free.
#[derive(Debug, Default)]
pub struct TransportMetrics {
    /// Direct-connector `call` attempts (connector events).
    pub direct_connector_attempts: AtomicUsize,
    /// Direct-connector TCP successes (at least one address connected).
    pub direct_connector_successes: AtomicUsize,
    /// Direct-connector TCP failures (all addresses failed).
    pub direct_connector_failures: AtomicUsize,
    /// Direct-connector DNS resolution attempts.
    pub direct_dns_attempts: AtomicUsize,
    /// Direct-connector DNS resolution failures.
    pub direct_dns_failures: AtomicUsize,
    /// Direct-connector TLS handshake attempts (HTTPS only).
    pub direct_tls_attempts: AtomicUsize,
    /// Direct-connector TLS handshake failures.
    pub direct_tls_failures: AtomicUsize,
    /// UDS connector attempts (connector events).
    pub uds_connector_attempts: AtomicUsize,
    /// UDS connector failures.
    pub uds_connector_failures: AtomicUsize,
    /// Proxy TCP connect attempts (HTTP forward + CONNECT + SOCKS TCP).
    pub proxy_connector_attempts: AtomicUsize,
    /// Proxy TCP connect failures.
    pub proxy_connector_failures: AtomicUsize,
    /// Proxy-endpoint TLS handshake attempts (`https://` proxy only).
    pub proxy_tls_attempts: AtomicUsize,
    /// Proxy-endpoint TLS handshake failures.
    pub proxy_tls_failures: AtomicUsize,
    /// H3/QUIC sessions successfully established (protocol connections).
    pub h3_connections_created: AtomicUsize,
    /// H3 cache evictions (bounded-cache + stale-generation removals).
    pub h3_cache_evictions: AtomicUsize,
    /// Successfully captured 101 upgrades (protocol connections).
    pub upgraded_connections_created: AtomicUsize,
}

impl TransportMetrics {
    /// Create empty metrics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn inc(counter: &AtomicUsize) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a direct-connector call start.
    pub fn record_direct_attempt(&self) {
        Self::inc(&self.direct_connector_attempts);
    }

    /// Record a direct-connector TCP success.
    pub fn record_direct_success(&self) {
        Self::inc(&self.direct_connector_successes);
    }

    /// Record a direct-connector TCP failure.
    pub fn record_direct_failure(&self) {
        Self::inc(&self.direct_connector_failures);
    }

    /// Record a direct DNS attempt.
    pub fn record_direct_dns_attempt(&self) {
        Self::inc(&self.direct_dns_attempts);
    }

    /// Record a direct DNS failure.
    pub fn record_direct_dns_failure(&self) {
        Self::inc(&self.direct_dns_failures);
    }

    /// Record a direct TLS handshake attempt.
    pub fn record_direct_tls_attempt(&self) {
        Self::inc(&self.direct_tls_attempts);
    }

    /// Record a direct TLS handshake failure.
    pub fn record_direct_tls_failure(&self) {
        Self::inc(&self.direct_tls_failures);
    }

    /// Record a UDS connect attempt.
    pub fn record_uds_attempt(&self) {
        Self::inc(&self.uds_connector_attempts);
    }

    /// Record a UDS connect failure.
    pub fn record_uds_failure(&self) {
        Self::inc(&self.uds_connector_failures);
    }

    /// Record a proxy TCP connect attempt.
    pub fn record_proxy_attempt(&self) {
        Self::inc(&self.proxy_connector_attempts);
    }

    /// Record a proxy TCP connect failure.
    pub fn record_proxy_failure(&self) {
        Self::inc(&self.proxy_connector_failures);
    }

    /// Record a proxy TLS handshake attempt.
    pub fn record_proxy_tls_attempt(&self) {
        Self::inc(&self.proxy_tls_attempts);
    }

    /// Record a proxy TLS handshake failure.
    pub fn record_proxy_tls_failure(&self) {
        Self::inc(&self.proxy_tls_failures);
    }

    /// Record an established H3 session.
    pub fn record_h3_created(&self) {
        Self::inc(&self.h3_connections_created);
    }

    /// Record an H3 cache eviction.
    pub fn record_h3_eviction(&self) {
        Self::inc(&self.h3_cache_evictions);
    }

    /// Record a captured 101 upgrade.
    pub fn record_upgraded(&self) {
        Self::inc(&self.upgraded_connections_created);
    }

    /// Snapshot all counters for assertions.
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "snapshot groups all counters for deterministic test assertions"
    )]
    pub fn snapshot(&self) -> TransportSnapshot {
        TransportSnapshot {
            direct_connector_attempts: self.direct_connector_attempts.load(Ordering::Relaxed),
            direct_connector_successes: self.direct_connector_successes.load(Ordering::Relaxed),
            direct_connector_failures: self.direct_connector_failures.load(Ordering::Relaxed),
            direct_dns_attempts: self.direct_dns_attempts.load(Ordering::Relaxed),
            direct_dns_failures: self.direct_dns_failures.load(Ordering::Relaxed),
            direct_tls_attempts: self.direct_tls_attempts.load(Ordering::Relaxed),
            direct_tls_failures: self.direct_tls_failures.load(Ordering::Relaxed),
            uds_connector_attempts: self.uds_connector_attempts.load(Ordering::Relaxed),
            uds_connector_failures: self.uds_connector_failures.load(Ordering::Relaxed),
            proxy_connector_attempts: self.proxy_connector_attempts.load(Ordering::Relaxed),
            proxy_connector_failures: self.proxy_connector_failures.load(Ordering::Relaxed),
            proxy_tls_attempts: self.proxy_tls_attempts.load(Ordering::Relaxed),
            proxy_tls_failures: self.proxy_tls_failures.load(Ordering::Relaxed),
            h3_connections_created: self.h3_connections_created.load(Ordering::Relaxed),
            h3_cache_evictions: self.h3_cache_evictions.load(Ordering::Relaxed),
            upgraded_connections_created: self.upgraded_connections_created.load(Ordering::Relaxed),
        }
    }
}

/// Point-in-time snapshot of [`TransportMetrics`] for tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportSnapshot {
    /// Direct-connector attempts.
    pub direct_connector_attempts: usize,
    /// Direct-connector successes.
    pub direct_connector_successes: usize,
    /// Direct-connector failures.
    pub direct_connector_failures: usize,
    /// Direct DNS attempts.
    pub direct_dns_attempts: usize,
    /// Direct DNS failures.
    pub direct_dns_failures: usize,
    /// Direct TLS attempts.
    pub direct_tls_attempts: usize,
    /// Direct TLS failures.
    pub direct_tls_failures: usize,
    /// UDS attempts.
    pub uds_connector_attempts: usize,
    /// UDS failures.
    pub uds_connector_failures: usize,
    /// Proxy attempts.
    pub proxy_connector_attempts: usize,
    /// Proxy failures.
    pub proxy_connector_failures: usize,
    /// Proxy TLS attempts.
    pub proxy_tls_attempts: usize,
    /// Proxy TLS failures.
    pub proxy_tls_failures: usize,
    /// H3 created.
    pub h3_connections_created: usize,
    /// H3 evictions.
    pub h3_cache_evictions: usize,
    /// Upgraded created.
    pub upgraded_connections_created: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_start_zeroed() {
        let m = TransportMetrics::new();
        let s = m.snapshot();
        assert_eq!(s.direct_connector_attempts, 0);
        assert_eq!(s.upgraded_connections_created, 0);
    }

    #[test]
    fn increments_are_exact() {
        let m = TransportMetrics::new();
        m.record_direct_attempt();
        m.record_direct_attempt();
        m.record_direct_success();
        m.record_upgraded();
        let s = m.snapshot();
        assert_eq!(s.direct_connector_attempts, 2);
        assert_eq!(s.direct_connector_successes, 1);
        assert_eq!(s.upgraded_connections_created, 1);
    }
}
