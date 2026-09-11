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

#[cfg(feature = "http3")]
use std::net::{IpAddr, SocketAddr};
#[cfg(feature = "http3")]
use std::sync::Mutex;
#[cfg(feature = "http3")]
use std::time::Duration;

/// The H3 route that established a diagnostic snapshot.
#[cfg(feature = "http3")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H3RouteKind {
    /// An explicit `Http3Only` direct route.
    Explicit,
    /// An authenticated Alt-Svc-discovered route.
    AltSvc,
}

/// Sanitized classification of a QUIC connection close.
#[cfg(feature = "http3")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H3CloseKind {
    /// The peer did not support a negotiated QUIC version.
    VersionMismatch,
    /// QUIC transport error; `code` contains the protocol code.
    TransportError,
    /// The peer aborted the QUIC connection; `code` contains the transport code.
    ConnectionClosed,
    /// An HTTP/3 application close; `code` contains the application code.
    ApplicationClosed,
    /// The peer reset the connection without a close code.
    Reset,
    /// The negotiated idle timeout elapsed.
    TimedOut,
    /// The local application closed the connection.
    LocallyClosed,
    /// The connection exhausted its connection-ID space.
    CidsExhausted,
}

/// A bounded, connection-level H3 diagnostic snapshot.
///
/// These values are copied from Quinn and intentionally contain no origin
/// hostname, request headers, cookies, bodies, or peer-provided close reason.
/// `open_streams` is `None` because the pinned Quinn API does not expose a
/// reliable H3 stream count for a reused multiplexed connection.
#[cfg(feature = "http3")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct H3ConnectionDiagnostic {
    /// Quinn's stable identifier for this connection.
    pub stable_id: usize,
    /// Last selected remote UDP address observed by Quinn.
    pub remote_address: SocketAddr,
    /// Local address information when the platform exposes it.
    pub local_ip: Option<IpAddr>,
    /// Whether the route was explicit or Alt-Svc discovered.
    pub route: H3RouteKind,
    /// Alt-Svc generation, or `None` for an explicit route.
    pub alt_svc_generation: Option<u64>,
    /// Quinn's current smoothed RTT at snapshot time.
    pub rtt: Duration,
    /// QUIC packets sent on the current path.
    pub sent_packets: u64,
    /// QUIC packets received on the current path.
    pub received_packets: u64,
    /// QUIC packets declared lost on the current path.
    pub lost_packets: u64,
    /// QUIC bytes sent in UDP datagrams.
    pub sent_bytes: u64,
    /// QUIC bytes received in UDP datagrams.
    pub received_bytes: u64,
    /// QUIC bytes declared lost on the current path.
    pub lost_bytes: u64,
    /// H3/QUIC close classification, available after the connection closes.
    pub close: Option<H3CloseSummary>,
    /// H3 stream count is not exposed by the pinned Quinn/h3 APIs.
    pub open_streams: Option<usize>,
}

/// Sanitized close information associated with an H3 diagnostic snapshot.
#[cfg(feature = "http3")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct H3CloseSummary {
    /// Close category.
    pub kind: H3CloseKind,
    /// QUIC or HTTP/3 close code when the upstream API provides one.
    pub code: Option<u64>,
    /// Whether h3 classified the close as `H3_NO_ERROR`.
    pub graceful: bool,
}

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
    /// Bounded H3 connection snapshots, when HTTP/3 is enabled.
    #[cfg(feature = "http3")]
    h3_diagnostics: Mutex<Vec<H3ConnectionDiagnostic>>,
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

    /// Store the latest snapshot for an H3 connection.
    #[cfg(feature = "http3")]
    pub(crate) fn record_h3_diagnostic(&self, diagnostic: H3ConnectionDiagnostic) {
        let Ok(mut diagnostics) = self.h3_diagnostics.lock() else {
            return;
        };
        if let Some(existing) = diagnostics
            .iter_mut()
            .find(|existing| existing.stable_id == diagnostic.stable_id)
        {
            *existing = diagnostic;
            return;
        }
        if diagnostics.len() >= 64 {
            diagnostics.remove(0);
        }
        diagnostics.push(diagnostic);
    }

    /// Return bounded H3 connection snapshots without retaining live Quinn
    /// connection handles.
    #[cfg(feature = "http3")]
    #[must_use]
    pub fn h3_diagnostics(&self) -> Vec<H3ConnectionDiagnostic> {
        self.h3_diagnostics
            .lock()
            .map(|diagnostics| diagnostics.clone())
            .unwrap_or_default()
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

    #[cfg(feature = "http3")]
    #[test]
    fn h3_diagnostics_replace_by_connection_and_stay_bounded() {
        let m = TransportMetrics::new();
        let address = "127.0.0.1:443".parse().expect("address");
        for stable_id in 0..65 {
            m.record_h3_diagnostic(H3ConnectionDiagnostic {
                stable_id,
                remote_address: address,
                local_ip: None,
                route: H3RouteKind::Explicit,
                alt_svc_generation: None,
                rtt: Duration::from_millis(1),
                sent_packets: 1,
                received_packets: 1,
                lost_packets: 0,
                sent_bytes: 1,
                received_bytes: 1,
                lost_bytes: 0,
                close: None,
                open_streams: None,
            });
        }
        let diagnostics = m.h3_diagnostics();
        assert_eq!(diagnostics.len(), 64);
        assert_eq!(diagnostics[0].stable_id, 1);

        m.record_h3_diagnostic(H3ConnectionDiagnostic {
            stable_id: 64,
            remote_address: address,
            local_ip: None,
            route: H3RouteKind::AltSvc,
            alt_svc_generation: Some(7),
            rtt: Duration::from_millis(2),
            sent_packets: 2,
            received_packets: 2,
            lost_packets: 1,
            sent_bytes: 2,
            received_bytes: 2,
            lost_bytes: 1,
            close: Some(H3CloseSummary {
                kind: H3CloseKind::ApplicationClosed,
                code: Some(0x100),
                graceful: true,
            }),
            open_streams: None,
        });
        let updated = m.h3_diagnostics();
        assert_eq!(updated.len(), 64);
        assert_eq!(
            updated.last().expect("last diagnostic").alt_svc_generation,
            Some(7)
        );
        assert_eq!(updated.last().expect("last diagnostic").lost_packets, 1);
    }
}
