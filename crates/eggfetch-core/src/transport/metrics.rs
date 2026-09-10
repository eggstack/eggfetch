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
//! - **Alt-Svc discovery** (`http3`): `altsvc_learned/expired/cleared/rejected`,
//!   `h3_route_attempted/suppressed`, `h3_fallback_selected`,
//!   `h3_drain_observed/closed/reconnected`. One per routing decision or
//!   cache transition; no secrets, no URLs, only counts.
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
    /// Alt-Svc advertisements learned (fresh H3 alternatives cached).
    pub altsvc_learned: AtomicUsize,
    /// Alt-Svc entries expired on read (lazy expiry).
    pub altsvc_expired: AtomicUsize,
    /// Alt-Svc entries cleared via `clear` or replaced by empty advertisement.
    pub altsvc_cleared: AtomicUsize,
    /// Alt-Svc header values rejected (malformed, oversized, untrusted context).
    pub altsvc_rejected: AtomicUsize,
    /// H3 route selections attempted (explicit or discovered).
    pub h3_route_attempted: AtomicUsize,
    /// H3 route selections skipped due to broken-route suppression.
    pub h3_route_suppressed: AtomicUsize,
    /// Safe H3-to-H2/H1 fallbacks selected (replayable, pre-commit only).
    pub h3_fallback_selected: AtomicUsize,
    /// H3 graceful drains observed (GOAWAY / `RemoteClosing`).
    pub h3_drain_observed: AtomicUsize,
    /// H3 connections closed (driver `poll_close` terminal).
    pub h3_closed: AtomicUsize,
    /// H3 reconnect generations created after drain/eviction.
    pub h3_reconnected: AtomicUsize,
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

    /// Record a learned Alt-Svc alternative.
    pub fn record_altsvc_learned(&self) {
        Self::inc(&self.altsvc_learned);
    }

    /// Record an Alt-Svc lazy expiry.
    pub fn record_altsvc_expired(&self) {
        Self::inc(&self.altsvc_expired);
    }

    /// Record an Alt-Svc clear.
    pub fn record_altsvc_cleared(&self) {
        Self::inc(&self.altsvc_cleared);
    }

    /// Record a rejected Alt-Svc value.
    pub fn record_altsvc_rejected(&self) {
        Self::inc(&self.altsvc_rejected);
    }

    /// Record an H3 route attempt.
    pub fn record_h3_attempted(&self) {
        Self::inc(&self.h3_route_attempted);
    }

    /// Record an H3 suppression skip.
    pub fn record_h3_suppressed(&self) {
        Self::inc(&self.h3_route_suppressed);
    }

    /// Record a safe H3-to-H2/H1 fallback.
    pub fn record_h3_fallback(&self) {
        Self::inc(&self.h3_fallback_selected);
    }

    /// Record an observed H3 graceful drain.
    pub fn record_h3_drain(&self) {
        Self::inc(&self.h3_drain_observed);
    }

    /// Record an H3 connection close.
    pub fn record_h3_closed(&self) {
        Self::inc(&self.h3_closed);
    }

    /// Record an H3 reconnect generation.
    pub fn record_h3_reconnected(&self) {
        Self::inc(&self.h3_reconnected);
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
            altsvc_learned: self.altsvc_learned.load(Ordering::Relaxed),
            altsvc_expired: self.altsvc_expired.load(Ordering::Relaxed),
            altsvc_cleared: self.altsvc_cleared.load(Ordering::Relaxed),
            altsvc_rejected: self.altsvc_rejected.load(Ordering::Relaxed),
            h3_route_attempted: self.h3_route_attempted.load(Ordering::Relaxed),
            h3_route_suppressed: self.h3_route_suppressed.load(Ordering::Relaxed),
            h3_fallback_selected: self.h3_fallback_selected.load(Ordering::Relaxed),
            h3_drain_observed: self.h3_drain_observed.load(Ordering::Relaxed),
            h3_closed: self.h3_closed.load(Ordering::Relaxed),
            h3_reconnected: self.h3_reconnected.load(Ordering::Relaxed),
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
    /// Alt-Svc learned.
    pub altsvc_learned: usize,
    /// Alt-Svc expired.
    pub altsvc_expired: usize,
    /// Alt-Svc cleared.
    pub altsvc_cleared: usize,
    /// Alt-Svc rejected.
    pub altsvc_rejected: usize,
    /// H3 route attempted.
    pub h3_route_attempted: usize,
    /// H3 route suppressed.
    pub h3_route_suppressed: usize,
    /// H3 fallback selected.
    pub h3_fallback_selected: usize,
    /// H3 drain observed.
    pub h3_drain_observed: usize,
    /// H3 closed.
    pub h3_closed: usize,
    /// H3 reconnected.
    pub h3_reconnected: usize,
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
        assert_eq!(s.altsvc_learned, 0);
        assert_eq!(s.h3_route_attempted, 0);
    }

    #[test]
    fn increments_are_exact() {
        let m = TransportMetrics::new();
        m.record_direct_attempt();
        m.record_direct_attempt();
        m.record_direct_success();
        m.record_upgraded();
        m.record_altsvc_learned();
        m.record_h3_attempted();
        m.record_h3_fallback();
        let s = m.snapshot();
        assert_eq!(s.direct_connector_attempts, 2);
        assert_eq!(s.direct_connector_successes, 1);
        assert_eq!(s.upgraded_connections_created, 1);
        assert_eq!(s.altsvc_learned, 1);
        assert_eq!(s.h3_route_attempted, 1);
        assert_eq!(s.h3_fallback_selected, 1);
    }
}
