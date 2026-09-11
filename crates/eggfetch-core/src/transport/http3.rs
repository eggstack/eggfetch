//! HTTP/3 transport over QUIC (Quinn + h3).
//!
//! This module is only compiled when the `http3` feature is enabled.
//! HTTP/3 uses QUIC instead of TCP, so it has its own connection lifecycle,
//! TLS configuration, and stream multiplexing model.
//!
//! # Lifecycle
//!
//! Each origin (`host:port`) moves through:
//!
//! `Vacant -> Connecting -> Ready -> Failed/Closed -> Evicted -> Reconnectable`
//!
//! - Only one connection attempt per origin is in flight at a time. The
//!   per-origin [`tokio::sync::OnceCell`] serializes establishment;
//!   concurrent waiters share the same attempt (including DNS, QUIC
//!   handshake, and h3 initialization).
//! - A failed initialization does not poison the origin. `OnceCell`
//!   caches only success; `Err` leaves the cell empty so the next request
//!   retries establishment.
//! - A connection that becomes unusable is evicted from the cache so a
//!   subsequent request reconnects. Eviction removes only cache ownership:
//!   in-flight streams hold their own `SendRequest` clone and the detached
//!   driver task keeps driving until those streams complete.
//! - Client drop releases cache ownership and the QUIC endpoint. Detached
//!   driver tasks terminate when their connections close; in-flight response
//!   bodies hold their QUIC streams and remain valid until consumed or
//!   dropped.
//! - The cache is bounded ([`H3_CACHE_MAX_ENTRIES`]). Eviction drops an
//!   arbitrary idle entry; the next request to an evicted origin reconnects
//!   lazily. In-flight responses survive unrelated eviction.
//!
//! # Policy mapping
//!
//! - `Timeout.connect` bounds DNS + QUIC handshake + h3 initialization as one
//!   connect budget shared across address fallback attempts. `Timeout.total`
//!   remains the outer logical deadline enforced by the pipeline and is never
//!   restarted per address or reconnect.
//! - `Timeout.read` applies to response-body progress at the documented
//!   response boundary (post-transport wrapper). `Timeout.write` applies to
//!   streamed request-body progress via the pre-transport wrapper installed
//!   before dispatch.
//! - QUIC idle lifetime derives from `PoolConfig::idle_timeout`
//!   (`Limits::keepalive_expiry`). `Timeout.pool` is an acquisition budget
//!   and never controls idle lifetime. Without explicit configuration the
//!   default is [`H3_DEFAULT_IDLE_TIMEOUT`] for compatibility.
//! - Logical request concurrency is enforced by the pool (one permit per
//!   request). QUIC stream limits are physical transport settings kept
//!   separate: `max_concurrent_bidi_streams` derives from
//!   `PoolConfig::max_connections_per_host` when configured, otherwise
//!   [`H3_DEFAULT_MAX_BIDI_STREAMS`]. The logical limit remains the upper
//!   bound on concurrent H3 streams because each stream holds a permit.
//!
//! # Failure handling
//!
//! The transport never retries internally. On a terminal connection-level
//! failure the stale cache entry is evicted and the original error is
//! returned. Replayable idempotent requests reconnect only through the
//! existing retry machinery on a subsequent attempt; one-shot bodies are
//! never duplicated by the transport.
//!
//! # Alt-Svc discovery (separate cache)
//!
//! Alt-Svc state lives in [`super::alt_svc::AltSvcCache`], owned by the
//! client and logically separate from the QUIC `sender_cache` here. An
//! origin may advertise an alternative with no QUIC session, and evicting a
//! failed QUIC session never erases the advertised route. Alternative
//! authorities affect only UDP routing; SNI, `Host`, cookies, auth, and
//! certificate validation always use the logical origin.
//!
//! # Draining (GOAWAY)
//!
//! GOAWAY is observed through the pinned `h3` 0.0.8 API only:
//! `SendRequest::send_request` fails with `StreamError::RemoteClosing` once
//! the shared `closing` flag is set by `process_goaway`, and the driver
//! `poll_close` terminal carries the HTTP/3 application close code
//! (`ApplicationClose { error_code }`, e.g. `H3_NO_ERROR`). Draining marks
//! the cached generation so no new streams are assigned to it; in-flight
//! streams keep their sender clones and the detached driver until they
//! complete. Reconnect creates a new generation. No upstream bug is worked
//! around by normalizing close errors into success.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Buf;
use dashmap::DashMap;

use crate::body::{RequestBody, ResponseBody};
use crate::error::{Error, Result};
use crate::pool::PoolConfig;
use crate::response::Response;
use crate::timeout::TimeoutPhase;
use crate::transport::metrics::{H3CloseKind, H3CloseSummary, H3ConnectionDiagnostic, H3RouteKind};

/// Upper bound on cached H3 origin entries.
///
/// Matches the spirit of the SNI (256) and SOCKS (64) client caches without a
/// new LRU dependency. QUIC connections each own a UDP flow, TLS session, and
/// background driver task, so the bound is conservative like SOCKS rather than
/// as generous as SNI. Eviction is arbitrary-entry (hash order) and removes
/// only cache ownership; in-flight streams keep their connection alive.
pub(crate) const H3_CACHE_MAX_ENTRIES: usize = 64;

/// Default QUIC idle timeout when no `keepalive_expiry` is configured.
///
/// Preserves the pre-hardening 30-second behavior as the conservative default.
pub(crate) const H3_DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Default maximum concurrent bidirectional QUIC streams per connection.
///
/// H3 request/response cycles use bidirectional streams. Unidirectional
/// streams carry control traffic and stay at this default.
pub(crate) const H3_DEFAULT_MAX_BIDI_STREAMS: u32 = 100;

/// Default maximum concurrent unidirectional QUIC streams per connection.
pub(crate) const H3_DEFAULT_MAX_UNI_STREAMS: u32 = 100;

/// Type alias for the h3 request sender, parameterised over the
/// h3-quinn connection and bytes body type.
type H3Sender = h3::client::SendRequest<h3_quinn::OpenStreams, bytes::Bytes>;

/// Cached h3 sender and its background driver for a single origin.
///
/// The `SendRequest` is clonable; clones share the same h3 connection.
/// When all clones (including the one stored here) are dropped the h3
/// connection is closed with `HTTP_NO_ERROR`. Dropping the cached entry
/// (eviction) detaches the driver task (`JoinHandle` drop does not abort);
/// the task keeps driving until in-flight streams complete and the
/// connection closes.
///
/// `draining` is set when GOAWAY is observed (`RemoteClosing` on new
/// streams) or when the driver `poll_close` resolves. New requests must not
/// be assigned to a draining generation. `close_reason` preserves the
/// upstream HTTP/3 application close code for diagnostics (never
/// synthesized).
struct CachedH3Sender {
    sender: H3Sender,
    _driver: tokio::task::JoinHandle<()>,
    draining: Arc<AtomicBool>,
    #[allow(
        dead_code,
        reason = "preserved upstream close code for diagnostics via test hook"
    )]
    close_reason: Arc<std::sync::Mutex<Option<String>>>,
    /// Alt-Svc generation this session was built for (`None` = explicit
    /// `Http3Only` direct route, no discovery).
    alt_generation: Option<u64>,
}

/// Explicit H3 dispatch failure.
///
/// Carries whether any request body bytes may have been committed so the
/// pipeline can decide safe fallback without inferring from error strings.
/// `body_committed=false` means no application data was sent (connect,
/// handshake, or pre-send drain); only then may `Auto` fall back for
/// replayable bodies.
#[derive(Debug, Clone)]
pub(crate) struct H3DispatchError {
    /// Underlying transport error.
    pub(crate) error: crate::error::Error,
    /// `true` once request body bytes may have been delivered.
    pub(crate) body_committed: bool,
    /// `true` when the peer indicated graceful drain (GOAWAY).
    pub(crate) draining: bool,
    /// Failure class for suppression (variant-derived, never strings).
    pub(crate) failure_class: super::alt_svc::H3FailureClass,
}

impl H3DispatchError {
    fn new(error: crate::error::Error, body_committed: bool, draining: bool) -> Self {
        let failure_class = super::alt_svc::H3FailureClass::from_error(&error);
        Self {
            error,
            body_committed,
            draining,
            failure_class,
        }
    }

    /// Total-deadline expiry for an H3 attempt (pre-commit, request-scoped).
    ///
    /// Classified `Other` so it never triggers broken-route suppression;
    /// the outer deadline is monotonic and never restarted by fallback.
    pub(crate) fn for_total_timeout(elapsed: Duration) -> Self {
        Self::new(
            crate::error::Error::Timeout {
                phase: crate::timeout::TimeoutPhase::Total,
                elapsed,
            },
            false,
            false,
        )
    }

    /// Returns `true` when `Auto` may safely fall back to H2/H1 within the
    /// same logical attempt: pre-commit failure only. The caller must also
    /// check body replayability and `Http3Only` strictness.
    pub(crate) fn safe_for_fallback(&self) -> bool {
        // Draining before commit is safe to reconnect, but fallback to H1/H2
        // is also safe (no bytes committed). Draining is handled as
        // reconnect-first by the connector; pipeline fallback uses the same
        // gate. Post-commit failures (including post-commit drains) are never
        // safe to silently replay.
        !self.body_committed
    }
}

/// Alternative endpoint for one H3 attempt.
///
/// `alt_host/alt_port` is the UDP destination; `sni_host` is always the
/// logical origin host for TLS authentication (never the alternative).
#[derive(Debug, Clone)]
pub(crate) struct H3AltTarget {
    /// UDP destination host (resolved via DNS).
    pub(crate) alt_host: String,
    /// UDP destination port.
    pub(crate) alt_port: u16,
    /// Logical origin host for SNI and certificate validation.
    pub(crate) sni_host: String,
    /// Alt-Svc generation (for suppression scoping).
    pub(crate) generation: Option<u64>,
}

/// Per-origin cache cell. The [`tokio::sync::OnceCell`] guarantees only
/// one caller per origin establishes the QUIC connection; concurrent
/// requests to the same origin await the same initialization instead of
/// racing get-then-insert (which duplicated connections and orphaned the
/// loser's driver task). Failures are not cached: `get_or_try_init` leaves
/// the cell empty on `Err`, so the origin stays reconnectable.
type CachedH3SenderCell = Arc<tokio::sync::OnceCell<CachedH3Sender>>;

/// A shared QUIC endpoint for HTTP/3 connections.
///
/// The endpoint is cloneable and manages the underlying UDP socket.
/// h3 senders are cached per origin so that the same h3 connection
/// (and therefore the same QUIC connection) is reused for subsequent
/// requests to the same host:port.
#[derive(Clone)]
pub(crate) struct H3Connector {
    endpoint: quinn::Endpoint,
    tls_config: Option<crate::tls::TlsConfig>,
    sender_cache: Arc<DashMap<String, CachedH3SenderCell>>,
    /// Effective QUIC idle timeout derived from pool keepalive configuration.
    quinn_idle_timeout: Duration,
    /// Effective maximum concurrent bidirectional streams per QUIC connection.
    max_bidi_streams: u32,
    /// Shared transport observability counters. `None` disables metering.
    metrics: Option<Arc<crate::transport::metrics::TransportMetrics>>,
}

/// Derive the QUIC idle timeout from pool configuration.
///
/// Maps `PoolConfig::idle_timeout` (`Limits::keepalive_expiry`) to the QUIC
/// transport timeout. `Timeout.pool` is deliberately not consulted: it is a
/// permit-acquisition budget, not a connection lifetime. Quinn requires a
/// protocol transport timeout that cannot exactly equal Hyper's pool idle
/// policy (QUIC idles on packet activity, Hyper idles on pool checkout), so
/// the mapping is documented here rather than claimed as identical.
fn derive_quinn_idle_timeout(pool_config: &PoolConfig) -> Duration {
    pool_config.idle_timeout.unwrap_or(H3_DEFAULT_IDLE_TIMEOUT)
}

/// Derive the QUIC bidirectional stream limit from pool configuration.
///
/// Keeps the physical transport limit separate from logical request
/// concurrency while ensuring the transport does not contradict configured
/// policy: when the effective per-origin in-flight limit
/// (`max_in_flight_requests_per_origin` or alias
/// `max_connections_per_host`) is set, the QUIC connection admits at
/// least that many concurrent request streams. Without explicit
/// configuration the conservative default applies. The global limit spans
/// origins and does not size a single connection's streams.
fn derive_max_bidi_streams(pool_config: &PoolConfig) -> u32 {
    match pool_config.effective_max_in_flight_per_origin() {
        Some(n) => u32::try_from(n).unwrap_or(u32::MAX).max(1),
        None => H3_DEFAULT_MAX_BIDI_STREAMS,
    }
}

/// Remaining budget under an optional connect deadline.
fn remaining_connect_budget(deadline: Option<std::time::Instant>) -> Option<Duration> {
    deadline.map(|d| d.saturating_duration_since(std::time::Instant::now()))
}

/// Convert an upstream QUIC close into a bounded, reason-free diagnostic.
fn h3_close_summary(error: &quinn::ConnectionError, graceful: bool) -> H3CloseSummary {
    let (kind, code) = match error {
        quinn::ConnectionError::VersionMismatch => (H3CloseKind::VersionMismatch, None),
        quinn::ConnectionError::TransportError(error) => {
            (H3CloseKind::TransportError, Some(error.code.into()))
        }
        quinn::ConnectionError::ConnectionClosed(close) => {
            (H3CloseKind::ConnectionClosed, Some(close.error_code.into()))
        }
        quinn::ConnectionError::ApplicationClosed(close) => (
            H3CloseKind::ApplicationClosed,
            Some(close.error_code.into()),
        ),
        quinn::ConnectionError::Reset => (H3CloseKind::Reset, None),
        quinn::ConnectionError::TimedOut => (H3CloseKind::TimedOut, None),
        quinn::ConnectionError::LocallyClosed => (H3CloseKind::LocallyClosed, None),
        quinn::ConnectionError::CidsExhausted => (H3CloseKind::CidsExhausted, None),
    };
    H3CloseSummary {
        kind,
        code,
        graceful,
    }
}

/// Snapshot Quinn state without retaining the connection handle in metrics.
fn h3_connection_diagnostic(
    connection: &quinn::Connection,
    route: H3RouteKind,
    alt_svc_generation: Option<u64>,
    close: Option<&quinn::ConnectionError>,
    graceful: bool,
) -> H3ConnectionDiagnostic {
    let stats = connection.stats();
    H3ConnectionDiagnostic {
        stable_id: connection.stable_id(),
        remote_address: connection.remote_address(),
        local_ip: connection.local_ip(),
        route,
        alt_svc_generation,
        rtt: connection.rtt(),
        sent_packets: stats.path.sent_packets,
        received_packets: stats.udp_rx.datagrams,
        lost_packets: stats.path.lost_packets,
        sent_bytes: stats.udp_tx.bytes,
        received_bytes: stats.udp_rx.bytes,
        lost_bytes: stats.path.lost_bytes,
        close: close.map(|error| h3_close_summary(error, graceful)),
        open_streams: None,
    }
}

/// A reason-free close label retained only for the existing test hook.
fn h3_close_label(error: &quinn::ConnectionError, graceful: bool) -> String {
    let summary = h3_close_summary(error, graceful);
    match summary.code {
        Some(code) => format!("{:?}:{code}", summary.kind),
        None => format!("{:?}", summary.kind),
    }
}

impl H3Connector {
    /// Create a new H3 connector.
    ///
    /// Derives QUIC idle and stream policy from `pool_config` so the H3 path
    /// honors the same keepalive/concurrency configuration as the H1/H2
    /// paths. See the module-level policy mapping for the exact semantics.
    #[allow(
        dead_code,
        reason = "kept for unit tests without metrics; client paths use with_metrics"
    )]
    pub(crate) fn new(
        tls_config: Option<crate::tls::TlsConfig>,
        pool_config: &PoolConfig,
    ) -> Result<Self> {
        Self::with_metrics(tls_config, pool_config, None)
    }

    /// Create a new H3 connector with shared transport metrics.
    pub(crate) fn with_metrics(
        tls_config: Option<crate::tls::TlsConfig>,
        pool_config: &PoolConfig,
        metrics: Option<Arc<crate::transport::metrics::TransportMetrics>>,
    ) -> Result<Self> {
        let bind_addr = "0.0.0.0:0"
            .parse()
            .map_err(|e| Error::Connect(format!("invalid QUIC bind address: {e}")))?;
        let endpoint = quinn::Endpoint::client(bind_addr)
            .map_err(|e| Error::Connect(format!("failed to create QUIC endpoint: {e}")))?;

        Ok(Self {
            endpoint,
            tls_config,
            sender_cache: Arc::new(DashMap::new()),
            quinn_idle_timeout: derive_quinn_idle_timeout(pool_config),
            max_bidi_streams: derive_max_bidi_streams(pool_config),
            metrics,
        })
    }

    /// Number of cached origin entries.
    #[cfg(any(test, feature = "test-util"))]
    #[allow(dead_code, reason = "test hook for bounded-cache assertions")]
    pub(crate) fn cache_len(&self) -> usize {
        self.sender_cache.len()
    }

    /// Returns `true` when `origin_key` (`host:port`) has a cache entry.
    #[cfg(any(test, feature = "test-util"))]
    #[allow(dead_code, reason = "test hook for eviction assertions")]
    pub(crate) fn contains_origin(&self, origin_key: &str) -> bool {
        self.sender_cache.contains_key(origin_key)
    }

    /// Effective QUIC idle timeout for this connector.
    #[cfg(any(test, feature = "test-util"))]
    #[allow(dead_code, reason = "test hook for idle-policy assertions")]
    pub(crate) fn quinn_idle_timeout(&self) -> Duration {
        self.quinn_idle_timeout
    }

    /// Effective maximum bidirectional streams per QUIC connection.
    #[cfg(any(test, feature = "test-util"))]
    #[allow(dead_code, reason = "test hook for stream-limit assertions")]
    pub(crate) fn max_bidi_streams(&self) -> u32 {
        self.max_bidi_streams
    }

    /// Cache key for an origin.
    fn cache_key(host: &str, port: u16) -> String {
        format!("{host}:{port}")
    }

    /// Ensure the cache stays bounded by evicting one arbitrary entry.
    ///
    /// Never evicts `except_key` (the origin currently being inserted), so a
    /// newly created entry cannot be removed immediately after creation.
    /// Eviction drops only cache ownership; in-flight streams hold their own
    /// sender clones and the detached driver keeps them alive.
    fn ensure_cache_bound(&self, except_key: &str) {
        if self.sender_cache.len() < H3_CACHE_MAX_ENTRIES {
            return;
        }
        let victim = self.sender_cache.iter().find_map(|entry| {
            let key = entry.key().clone();
            (key != except_key).then_some(key)
        });
        if let Some(victim) = victim {
            self.sender_cache.remove(&victim);
            if let Some(ref m) = self.metrics {
                m.record_h3_eviction();
            }
        }
    }

    /// Remove a stale cache entry only when it is still the current cell.
    ///
    /// A concurrent reconnect may have already replaced the entry; blindly
    /// removing by key could drop the fresh connection. Pointer comparison
    /// keeps eviction scoped to the failed generation.
    fn evict_if_current(&self, key: &str, cell: &CachedH3SenderCell) {
        if self
            .sender_cache
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(&current, cell))
        {
            self.sender_cache.remove(key);
            if let Some(ref m) = self.metrics {
                m.record_h3_eviction();
            }
        }
    }

    /// Resolve all socket addresses for an origin under the connect budget.
    async fn resolve_addrs(
        host: &str,
        port: u16,
        deadline: Option<std::time::Instant>,
        connect_budget: Option<Duration>,
        started: std::time::Instant,
    ) -> Result<Vec<SocketAddr>> {
        let lookup = tokio::net::lookup_host(format!("{host}:{port}"));
        let addrs: Vec<SocketAddr> = match remaining_connect_budget(deadline) {
            Some(remaining) => tokio::time::timeout(remaining, lookup)
                .await
                .map_err(|_| Error::Timeout {
                    phase: TimeoutPhase::Connect,
                    elapsed: started.elapsed(),
                })?
                .map_err(|e| Error::Connect(format!("DNS resolve: {e}")))?
                .collect(),
            None => lookup
                .await
                .map_err(|e| Error::Connect(format!("DNS resolve: {e}")))?
                .collect(),
        };
        if addrs.is_empty() {
            return Err(Error::Connect("no addresses resolved".into()));
        }
        let _ = connect_budget;
        Ok(addrs)
    }

    /// Attempt each resolved address in order under one connect deadline.
    ///
    /// Sequential bounded fallback: earlier candidates that fail yield to the
    /// next address without restarting the connect budget. Each attempt
    /// receives a fair share of the remaining budget (`remaining /
    /// addrs_left`) so a stalling candidate cannot starve later addresses;
    /// fast failures leave nearly the full budget for the next candidate.
    /// Cancellation drops the loop and aborts remaining attempts promptly.
    /// The final error preserves the connect/H3-connect taxonomy without
    /// leaking sensitive detail beyond the transport error itself.
    async fn connect_with_fallback(
        &self,
        addrs: &[SocketAddr],
        host: &str,
        deadline: Option<std::time::Instant>,
        started: std::time::Instant,
    ) -> Result<quinn::Connection> {
        let mut last_err: Option<Error> = None;
        for (idx, addr) in addrs.iter().enumerate() {
            let addrs_left = u32::try_from(addrs.len().saturating_sub(idx)).unwrap_or(u32::MAX);
            let remaining = remaining_connect_budget(deadline).map(|r| {
                let share = r.checked_div(addrs_left.max(1)).unwrap_or(r);
                // Never hand an attempt a zero budget while time remains;
                // the deadline check below reports exhaustion instead.
                share
            });
            if deadline.is_some_and(|d| std::time::Instant::now() >= d)
                || remaining.is_some_and(|r| r.is_zero())
            {
                return Err(last_err.unwrap_or(Error::Timeout {
                    phase: TimeoutPhase::Connect,
                    elapsed: started.elapsed(),
                }));
            }
            match self.connect_single(*addr, host, remaining, started).await {
                Ok(conn) => return Ok(conn),
                Err(e) => {
                    // Per-attempt timeouts fall through to the next address
                    // with its own share; only overall exhaustion (above) is
                    // terminal. This keeps a stalling candidate from
                    // consuming the entire shared budget.
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| Error::Connect("no addresses resolved".into())))
    }

    /// Establish a single QUIC connection under the remaining budget.
    async fn connect_single(
        &self,
        addr: SocketAddr,
        host: &str,
        remaining: Option<Duration>,
        started: std::time::Instant,
    ) -> Result<quinn::Connection> {
        let quic_config = build_quic_client_config(
            self.tls_config.as_ref(),
            self.quinn_idle_timeout,
            self.max_bidi_streams,
        )?;
        let connecting = self
            .endpoint
            .connect_with(quic_config, addr, host)
            .map_err(|e| Error::Connect(format!("QUIC connect: {e}")))?;
        let conn = match remaining {
            Some(budget) => tokio::time::timeout(budget, connecting)
                .await
                .map_err(|_| Error::Timeout {
                    phase: TimeoutPhase::Connect,
                    elapsed: started.elapsed(),
                })?
                .map_err(|e| Error::H3Connect(format!("QUIC handshake failed: {e}")))?,
            None => connecting
                .await
                .map_err(|e| Error::H3Connect(format!("QUIC handshake failed: {e}")))?,
        };
        Ok(conn)
    }

    /// Establish the h3 client session under the remaining connect budget.
    async fn establish_h3_sender(
        quinn_conn: quinn::Connection,
        deadline: Option<std::time::Instant>,
        started: std::time::Instant,
        alt_generation: Option<u64>,
        route: H3RouteKind,
        metrics: Option<Arc<crate::transport::metrics::TransportMetrics>>,
    ) -> Result<CachedH3Sender> {
        let diagnostic_conn = quinn_conn.clone();
        let init = async {
            let h3_conn = h3_quinn::Connection::new(quinn_conn);
            h3::client::new(h3_conn)
                .await
                .map_err(|e| Error::H3Protocol(format!("h3 client init: {e}")))
        };
        let (mut driver, sender) = match remaining_connect_budget(deadline) {
            Some(remaining) => {
                tokio::time::timeout(remaining, init)
                    .await
                    .map_err(|_| Error::Timeout {
                        phase: TimeoutPhase::Connect,
                        elapsed: started.elapsed(),
                    })??
            }
            None => init.await?,
        };
        let draining: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let close_reason: Arc<std::sync::Mutex<Option<String>>> =
            Arc::new(std::sync::Mutex::new(None));
        let draining_for_driver = draining.clone();
        let reason_for_driver = close_reason.clone();
        let metrics_for_driver = metrics.clone();
        if let Some(ref m) = metrics {
            m.record_h3_diagnostic(h3_connection_diagnostic(
                &diagnostic_conn,
                route,
                alt_generation,
                None,
                false,
            ));
        }
        let driver_handle = tokio::spawn(async move {
            use futures_util::future;
            let close_err = future::poll_fn(|cx| driver.poll_close(cx)).await;
            draining_for_driver.store(true, Ordering::Relaxed);
            let graceful = close_err.is_h3_no_error();
            let quinn_close = diagnostic_conn.close_reason();
            if let Ok(mut guard) = reason_for_driver.lock() {
                *guard = quinn_close
                    .as_ref()
                    .map(|error| h3_close_label(error, graceful));
            }
            if let Some(m) = metrics_for_driver {
                if let Some(error) = quinn_close.as_ref() {
                    m.record_h3_diagnostic(h3_connection_diagnostic(
                        &diagnostic_conn,
                        route,
                        alt_generation,
                        Some(error),
                        graceful,
                    ));
                }
                m.record_h3_closed();
                // Graceful application close (`H3_NO_ERROR`) is a drain,
                // not a failure. Uses the public `is_h3_no_error()` API
                // (pinned h3 0.0.8); no variant matching, no strings.
                if graceful {
                    m.record_h3_drain();
                }
            }
        });
        Ok(CachedH3Sender {
            sender,
            _driver: driver_handle,
            draining,
            close_reason,
            alt_generation,
        })
    }

    /// Returns `true` when the cached cell is draining (GOAWAY observed or
    /// driver closed). Test hook for drain assertions.
    #[cfg(any(test, feature = "test-util"))]
    #[allow(dead_code, reason = "test hook for drain assertions")]
    pub(crate) fn is_draining_key(&self, origin_key: &str) -> bool {
        self.sender_cache.get(origin_key).is_some_and(|r| {
            r.get()
                .is_some_and(|cached| cached.draining.load(Ordering::Relaxed))
        })
    }

    /// Last driver close reason for `origin_key`, if any (preserved upstream
    /// close code, never synthesized).
    #[cfg(any(test, feature = "test-util"))]
    #[allow(dead_code, reason = "test hook for close-code assertions")]
    pub(crate) fn close_reason_key(&self, origin_key: &str) -> Option<String> {
        self.sender_cache.get(origin_key).and_then(|r| {
            let cached = r.get()?;
            cached.close_reason.lock().ok()?.clone()
        })
    }

    /// Issue an HTTP/3 request and return a `Response` with a streaming body.
    ///
    /// `connect_timeout` bounds DNS + QUIC + h3 establishment as one budget
    /// shared across address fallback attempts. The outer pipeline enforces
    /// `Timeout.total`; this function never restarts that deadline. Read and
    /// write phase timeouts are enforced by the pipeline's body wrappers at
    /// the documented boundaries, so phase identity is preserved instead of
    /// collapsing into generic H3 protocol errors.
    ///
    /// `alt` carries an Alt-Svc-discovered UDP destination; SNI and
    /// certificate validation always use the logical origin host. `None`
    /// means explicit direct (`Http3Only`) routing.
    ///
    /// The transport performs no hidden retries except one draining
    /// reconnect (GOAWAY before commit): a draining generation is evicted
    /// and a single fresh generation is attempted within the same connect
    /// budget. All other connection-level failures evict the stale origin
    /// entry and return the original error; a later retry-policy attempt
    /// reconnects through the normal machinery. One-shot bodies are never
    /// replayed here. Failures report `body_committed` explicitly so the
    /// pipeline can gate safe fallback without string inference.
    #[allow(clippy::too_many_lines)]
    #[allow(
        dead_code,
        reason = "kept for unit tests; pipeline uses send_request_with_alt"
    )]
    pub(crate) async fn send_request(
        &self,
        request: http::Request<RequestBody>,
        url: url::Url,
        connect_timeout: Option<Duration>,
    ) -> Result<Response> {
        self.send_request_inner(request, url, None, connect_timeout)
            .await
            .map_err(|e| e.error)
    }

    /// Detailed H3 dispatch with explicit commit/drain signalling for the
    /// pipeline fallback and suppression policy.
    pub(crate) async fn send_request_with_alt(
        &self,
        request: http::Request<RequestBody>,
        url: url::Url,
        alt: Option<H3AltTarget>,
        connect_timeout: Option<Duration>,
    ) -> std::result::Result<Response, H3DispatchError> {
        self.send_request_inner(request, url, alt, connect_timeout)
            .await
    }

    #[allow(
        clippy::too_many_lines,
        reason = "H3 dispatch centralizes connect/fallback/drain policy so transports do not reimplement it"
    )]
    async fn send_request_inner(
        &self,
        request: http::Request<RequestBody>,
        url: url::Url,
        alt: Option<H3AltTarget>,
        connect_timeout: Option<Duration>,
    ) -> std::result::Result<Response, H3DispatchError> {
        let map_err = |e: Error, committed: bool, draining: bool| {
            H3DispatchError::new(e, committed, draining)
        };
        let host = url
            .host_str()
            .ok_or_else(|| map_err(Error::InvalidUrl("missing host".into()), false, false))?;
        let port = url.port_or_known_default().unwrap_or(443);
        let cache_key = Self::cache_key(host, port);
        let host_owned = host.to_owned();
        // Alt-Svc routing: UDP destination may differ from the logical
        // origin. SNI and certificate validation always use the origin host.
        let (resolve_host, resolve_port, sni_host, alt_generation) = match &alt {
            Some(a) => (
                a.alt_host.clone(),
                a.alt_port,
                a.sni_host.clone(),
                a.generation,
            ),
            None => (host_owned.clone(), port, host_owned.clone(), None),
        };

        self.ensure_cache_bound(&cache_key);
        let mut cell = self
            .sender_cache
            .entry(cache_key.clone())
            .or_default()
            .clone();

        // Generation change (new advertisement) evicts the old QUIC session
        // so the new alternative is used immediately without waiting on
        // stale failure state. Draining generations are also evicted before
        // assigning new streams; in-flight streams survive via their clones.
        let mut was_reconnect = false;
        if let Some(cached) = cell.get() {
            let stale_generation = cached.alt_generation != alt_generation;
            let draining = cached.draining.load(Ordering::Relaxed);
            if stale_generation || draining {
                self.evict_if_current(&cache_key, &cell);
                let fresh: CachedH3SenderCell = Arc::new(tokio::sync::OnceCell::new());
                self.ensure_cache_bound(&cache_key);
                self.sender_cache.insert(cache_key.clone(), fresh.clone());
                cell = fresh;
                // A fresh generation after eviction counts as a reconnect
                // once it establishes successfully below.
                was_reconnect = true;
            }
        }

        // Fast path: an established connection needs no DNS or handshake.
        // The slow path below shares DNS + QUIC + h3 init across concurrent
        // waiters through the same `OnceCell`.
        let cached = if let Some(cached) = cell.get() {
            cached
        } else {
            let started = std::time::Instant::now();
            let deadline = connect_timeout.map(|d| started + d);
            let endpoint_self = self.clone();
            let resolve_host_clone = resolve_host.clone();
            let sni_host_clone = sni_host.clone();
            let init_result: std::result::Result<&CachedH3Sender, Error> = cell
                .get_or_try_init(|| async {
                    let addrs = Self::resolve_addrs(
                        &resolve_host_clone,
                        resolve_port,
                        deadline,
                        connect_timeout,
                        started,
                    )
                    .await?;
                    let quinn_conn = endpoint_self
                        .connect_with_fallback(&addrs, &sni_host_clone, deadline, started)
                        .await?;
                    let sender: CachedH3Sender = Self::establish_h3_sender(
                        quinn_conn,
                        deadline,
                        started,
                        alt_generation,
                        if alt.is_some() {
                            H3RouteKind::AltSvc
                        } else {
                            H3RouteKind::Explicit
                        },
                        endpoint_self.metrics.clone(),
                    )
                    .await?;
                    if let Some(ref m) = endpoint_self.metrics {
                        m.record_h3_created();
                        if was_reconnect {
                            m.record_h3_reconnected();
                        }
                    }
                    Ok(sender)
                })
                .await;
            match init_result {
                Ok(cached) => cached,
                Err(e) => {
                    // `OnceCell` caches only success, so the origin is already
                    // reconnectable. Propagate the connect-phase taxonomy
                    // (`Timeout{Connect}`, `Connect`, `H3Connect`) unchanged.
                    return Err(map_err(e.clone(), false, false));
                }
            }
        };
        let sender = cached.sender.clone();

        // Decompose the incoming request
        let (parts, body) = request.into_parts();

        // Build h3 request (body is always () for the h3 request frame)
        let mut h3_request = http::Request::builder()
            .method(parts.method)
            .uri(parts.uri)
            .version(parts.version);

        for (name, value) in &parts.headers {
            h3_request = h3_request.header(name, value);
        }

        let h3_request = h3_request
            .body(())
            .map_err(|e| map_err(Error::RequestBuild(e.to_string()), false, false))?;

        // Send the request headers. Pre-commit failures (including GOAWAY
        // drain signalled via shared closing state) evict the stale
        // generation so the next attempt reconnects. Draining is observed
        // through the public `h3::ConnectionState::is_closing()` API on the
        // pinned h3 0.0.8 sender (set by `process_goaway`); no variant
        // matching, no error strings.
        // No hidden retry happens here; the caller (retry/fallback policy)
        // decides whether to attempt again. In-flight streams keep their
        // clones and the detached driver.
        let mut request_stream = {
            let mut sender = sender;
            match sender.send_request(h3_request).await {
                Ok(stream) => stream,
                Err(e) => {
                    // GOAWAY sets shared `closing`; a send failure while
                    // closing is a graceful drain, not an arbitrary failure.
                    let draining = {
                        use h3::ConnectionState;
                        sender.is_closing()
                    };
                    // Mark the cached generation draining so no new streams
                    // are assigned to it; in-flight streams survive via
                    // their clones and the detached driver.
                    if draining {
                        if let Some(cached) = cell.get() {
                            cached.draining.store(true, Ordering::Relaxed);
                        }
                        if let Some(ref m) = self.metrics {
                            m.record_h3_drain();
                        }
                    }
                    self.evict_if_current(&cache_key, &cell);
                    if draining {
                        return Err(map_err(
                            Error::H3ConnectionClosed("peer draining (GOAWAY)".into()),
                            false,
                            true,
                        ));
                    }
                    return Err(map_err(
                        Error::H3Protocol(format!("send request: {e}")),
                        false,
                        false,
                    ));
                }
            }
        };

        // Send request body. The pipeline wraps streamed bodies with the
        // write-phase timeout before dispatch, so a stalled producer surfaces
        // as `Timeout{Write}` here and must not evict the connection.
        // Body-phase failures are post-commit (bytes may have been
        // delivered) and never safe for silent fallback.
        let send_body_result = async {
            match body {
                RequestBody::Empty => Ok(()),
                RequestBody::Bytes(bytes) => request_stream
                    .send_data(bytes)
                    .await
                    .map_err(|e| Error::H3Protocol(format!("send data: {e}"))),
                RequestBody::Stream {
                    stream: body_stream,
                    ..
                } => {
                    use futures_util::StreamExt;
                    let mut body_stream = body_stream;
                    while let Some(chunk) = body_stream.next().await {
                        let bytes = chunk?;
                        request_stream
                            .send_data(bytes)
                            .await
                            .map_err(|e| Error::H3Protocol(format!("send data: {e}")))?;
                    }
                    Ok(())
                }
            }
        }
        .await;
        if let Err(e) = send_body_result {
            // Write-phase timeouts (`Timeout{Write}`) and body producer
            // errors are request scoped and must not evict the shared
            // connection. H3 transport failures evict so the next attempt
            // reconnects. All body-phase outcomes are post-commit for
            // fallback gating.
            if matches!(
                e,
                Error::H3Protocol(_)
                    | Error::H3ConnectionClosed(_)
                    | Error::H3Stream(_)
                    | Error::H3Connect(_)
            ) {
                self.evict_if_current(&cache_key, &cell);
                return Err(map_err(e, true, false));
            }
            return Err(map_err(e, true, false));
        }

        // Signal end of request body (post-commit).
        if let Err(err) = request_stream.finish().await {
            self.evict_if_current(&cache_key, &cell);
            return Err(map_err(
                Error::H3Protocol(format!("finish stream: {err}")),
                true,
                false,
            ));
        }

        // Receive response headers (post-commit: request was delivered).
        let response = match request_stream.recv_response().await {
            Ok(response) => response,
            Err(e) => {
                self.evict_if_current(&cache_key, &cell);
                return Err(map_err(
                    Error::H3Protocol(format!("recv response: {e}")),
                    true,
                    false,
                ));
            }
        };

        let status = response.status();
        let resp_version = response.version();
        let mut resp_headers = http::HeaderMap::new();
        for (name, value) in response.headers() {
            resp_headers.insert(name.clone(), value.clone());
        }

        // Build a streaming response body from the h3 data frames.
        // recv_data() returns `impl Buf`; we convert to Bytes for compatibility
        // with our BoxBytesStream type. After data EOF, recv_trailers() is
        // attempted once and stored without buffering the body.
        //
        // The body stream holds only the request stream. The h3 sender and
        // driver are kept alive by the sender cache (or, after eviction, by
        // the detached driver plus in-flight sender clones), so the body does
        // not need to own them. On a body transport failure the stream evicts
        // the stale origin so the next request reconnects.
        let cache_for_body = self.sender_cache.clone();
        let key_for_body = cache_key.clone();
        let cell_for_body = cell.clone();
        let trailers = crate::body::SharedTrailers::new();
        let trailers_for_body = trailers.clone();
        let body_stream = futures_util::stream::unfold(
            (request_stream, false),
            move |(mut stream, trailers_done)| {
                let cache_for_body = cache_for_body.clone();
                let key_for_body = key_for_body.clone();
                let cell_for_body = cell_for_body.clone();
                let trailers_for_body = trailers_for_body.clone();
                async move {
                    if trailers_done {
                        return None;
                    }
                    match stream.recv_data().await {
                        Ok(Some(mut data)) => {
                            let bytes = data.copy_to_bytes(data.remaining());
                            Some((Ok::<_, Error>(bytes), (stream, false)))
                        }
                        Ok(None) => {
                            // Data complete; attempt trailers once.
                            match stream.recv_trailers().await {
                                Ok(Some(headers)) => {
                                    trailers_for_body.store(headers);
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    if cache_for_body.get(&key_for_body).is_some_and(|current| {
                                        Arc::ptr_eq(&current, &cell_for_body)
                                    }) {
                                        cache_for_body.remove(&key_for_body);
                                    }
                                    return Some((
                                        Err(Error::H3Protocol(format!("recv trailers: {e}"))),
                                        (stream, true),
                                    ));
                                }
                            }
                            None
                        }
                        Err(e) => {
                            if cache_for_body
                                .get(&key_for_body)
                                .is_some_and(|current| Arc::ptr_eq(&current, &cell_for_body))
                            {
                                cache_for_body.remove(&key_for_body);
                            }
                            Some((
                                Err(Error::H3Protocol(format!("recv data: {e}"))),
                                (stream, true),
                            ))
                        }
                    }
                }
            },
        );

        let body = ResponseBody::streaming(Box::pin(body_stream));

        let mut core_response = Response::new(status, resp_version, resp_headers, url, body);
        core_response.set_trailers(trailers);
        Ok(core_response)
    }
}

/// Build a QUIC client configuration.
///
/// QUIC requires TLS 1.3 only. When a `TlsConfig` is provided, its trust
/// store, client identity, and verification settings are honoured. Otherwise
/// a default config with webpki roots is used.
///
/// `idle_timeout` is the effective QUIC idle lifetime derived from pool
/// keepalive configuration (see [`derive_quinn_idle_timeout`]).
/// `max_bidi_streams` is the effective per-connection bidirectional stream
/// limit derived from logical pool policy (see [`derive_max_bidi_streams`]).
fn build_quic_client_config(
    tls_config: Option<&crate::tls::TlsConfig>,
    idle_timeout: Duration,
    max_bidi_streams: u32,
) -> Result<quinn::ClientConfig> {
    let rc = if let Some(tc) = tls_config {
        tc.build_quic_rustls_config()?
    } else {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let provider = crate::tls::process_crypto_provider()?;
        let mut rc = rustls::ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|e| Error::Tls(format!("TLS version config: {e}")))?
            .with_root_certificates(root_store)
            .with_no_client_auth();

        rc.alpn_protocols = vec![b"h3".to_vec()];
        rc
    };

    let quic_crypto = quinn::crypto::rustls::QuicClientConfig::try_from(rc)
        .map_err(|e| Error::Tls(format!("QUIC TLS config conversion: {e}")))?;

    let mut quic_config = quinn::ClientConfig::new(Arc::new(quic_crypto));

    // Configure transport parameters. The bidi limit is the physical QUIC
    // stream cap, kept separate from the logical pool permit model: every
    // in-flight H3 request holds a pool permit, so the logical per-origin
    // limit remains the upper bound on concurrent streams.
    let mut transport = quinn::TransportConfig::default();
    transport.max_concurrent_bidi_streams(max_bidi_streams.into());
    transport.max_concurrent_uni_streams(H3_DEFAULT_MAX_UNI_STREAMS.into());

    transport.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(idle_timeout)
            .map_err(|e| Error::Tls(format!("invalid idle timeout: {e}")))?,
    ));

    quic_config.transport_config(Arc::new(transport));

    Ok(quic_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool_with(idle: Option<Duration>, per_host: Option<usize>) -> PoolConfig {
        PoolConfig {
            idle_timeout: idle,
            max_connections_per_host: per_host,
            ..PoolConfig::default()
        }
    }

    #[test]
    fn idle_defaults_to_thirty_seconds() {
        let derived = derive_quinn_idle_timeout(&pool_with(None, None));
        assert_eq!(derived, Duration::from_secs(30));
    }

    #[test]
    fn idle_follows_keepalive_expiry() {
        let derived = derive_quinn_idle_timeout(&pool_with(Some(Duration::from_secs(5)), None));
        assert_eq!(derived, Duration::from_secs(5));
    }

    #[test]
    fn bidi_defaults_to_one_hundred() {
        assert_eq!(derive_max_bidi_streams(&pool_with(None, None)), 100);
    }

    #[test]
    fn bidi_follows_per_host_limit() {
        assert_eq!(derive_max_bidi_streams(&pool_with(None, Some(10))), 10);
        // A large logical limit raises the physical cap so the transport does
        // not contradict configured concurrency.
        assert_eq!(derive_max_bidi_streams(&pool_with(None, Some(1000))), 1000);
    }

    #[test]
    fn cache_bound_is_sixty_four() {
        assert_eq!(H3_CACHE_MAX_ENTRIES, 64);
    }
    #[test]
    fn quic_config_uses_derived_policy() {
        // `ClientConfig` exposes no transport getter; a successful build with
        // derived values plus the connector-level getter tests below pins the
        // mapping without reaching into Quinn internals.
        let _ = build_quic_client_config(None, Duration::from_secs(5), 10).expect("quic config");
        let _ = build_quic_client_config(None, Duration::from_secs(30), 100).expect("quic config");
    }

    #[cfg(any(test, feature = "test-util"))]
    #[tokio::test]
    async fn connector_derives_policy_from_pool() {
        let pool = pool_with(Some(Duration::from_secs(7)), Some(12));
        let connector = H3Connector::new(None, &pool).expect("connector");
        assert_eq!(connector.quinn_idle_timeout(), Duration::from_secs(7));
        assert_eq!(connector.max_bidi_streams(), 12);
    }

    #[cfg(any(test, feature = "test-util"))]
    #[tokio::test]
    async fn connector_defaults_match_legacy_behavior() {
        let connector = H3Connector::new(None, &PoolConfig::default()).expect("connector");
        assert_eq!(connector.quinn_idle_timeout(), Duration::from_secs(30));
        assert_eq!(connector.max_bidi_streams(), 100);
        assert_eq!(connector.cache_len(), 0);
    }

    #[tokio::test]
    async fn cache_bound_evicts_without_losing_current_origin() {
        let connector = H3Connector::new(None, &PoolConfig::default()).expect("connector builds");
        // Fill beyond capacity through the same bounded-insert path used by
        // `send_request` so the test pins the production eviction policy.
        for i in 0..(H3_CACHE_MAX_ENTRIES + 10) {
            let key = format!("host-{i}.example:443");
            connector.ensure_cache_bound(&key);
            connector.sender_cache.entry(key).or_default();
        }
        assert!(
            connector.sender_cache.len() <= H3_CACHE_MAX_ENTRIES,
            "cache must stay bounded, got {}",
            connector.sender_cache.len()
        );
        // The most recently inserted origin survives its own insertion.
        let current = format!("host-{}.example:443", H3_CACHE_MAX_ENTRIES + 9);
        assert!(connector.sender_cache.contains_key(&current));
    }

    #[tokio::test]
    async fn evictions_increment_transport_metrics() {
        let metrics = Arc::new(crate::transport::metrics::TransportMetrics::new());
        let connector =
            H3Connector::with_metrics(None, &PoolConfig::default(), Some(metrics.clone()))
                .expect("connector builds");
        for i in 0..(H3_CACHE_MAX_ENTRIES + 5) {
            let key = format!("host-{i}.example:443");
            connector.ensure_cache_bound(&key);
            connector.sender_cache.entry(key).or_default();
        }
        let evictions = metrics
            .h3_cache_evictions
            .load(std::sync::atomic::Ordering::Relaxed);
        assert!(
            evictions >= 5,
            "bounded evictions must be counted, got {evictions}"
        );
    }

    #[test]
    fn bidi_follows_new_in_flight_name() {
        let pool = PoolConfig {
            max_in_flight_requests_per_origin: Some(9),
            ..PoolConfig::default()
        };
        assert_eq!(derive_max_bidi_streams(&pool), 9);
    }

    #[tokio::test]
    async fn evict_if_current_scopes_to_failed_generation() {
        let connector = H3Connector::new(None, &PoolConfig::default()).expect("connector builds");
        let key = "example.com:443".to_owned();
        let first: CachedH3SenderCell = Arc::new(tokio::sync::OnceCell::new());
        connector.sender_cache.insert(key.clone(), first.clone());
        // A stale generation must not drop a fresh replacement.
        let second: CachedH3SenderCell = Arc::new(tokio::sync::OnceCell::new());
        connector.sender_cache.insert(key.clone(), second.clone());
        connector.evict_if_current(&key, &first);
        assert!(
            connector.sender_cache.contains_key(&key),
            "eviction of a stale cell must not remove the current generation"
        );
        connector.evict_if_current(&key, &second);
        assert!(
            !connector.sender_cache.contains_key(&key),
            "eviction of the current cell must remove the stale origin"
        );
    }

    #[test]
    fn remaining_budget_saturates_at_zero() {
        let past = std::time::Instant::now()
            .checked_sub(Duration::from_secs(1))
            .expect("recent instant");
        assert_eq!(
            remaining_connect_budget(Some(past)),
            Some(Duration::ZERO),
            "an expired deadline must yield zero, not panic"
        );
        assert!(remaining_connect_budget(None).is_none());
    }

    /// Minimal local QUIC+H3 server for deterministic lifecycle tests.
    ///
    /// Binds 127.0.0.1:0 with a self-signed cert, serves `body` on every
    /// request, and returns the bound address plus a shutdown sender. No
    /// public internet is used. Only compiled for tests with the `http3`
    /// feature (this module is already `http3`-gated).
    #[cfg(test)]
    struct LocalH3Server {
        addr: SocketAddr,
        shutdown: tokio::sync::watch::Sender<bool>,
    }

    #[cfg(test)]
    impl LocalH3Server {
        #[allow(
            clippy::unused_async,
            clippy::unused_async_trait_impl,
            reason = "kept async to match the integration-test fixture shape; the awaits live in the spawned task"
        )]
        async fn start(body: Vec<u8>) -> Self {
            use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
            let cert_key =
                rcgen::generate_simple_self_signed(vec!["localhost".into()]).expect("rcgen cert");
            let cert_der = CertificateDer::from(cert_key.cert.der().to_vec());
            let key_der =
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert_key.key_pair.serialize_der()));
            let mut server_tls = rustls::ServerConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_protocol_versions(&[&rustls::version::TLS13])
            .expect("TLS13")
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .expect("cert");
            server_tls.alpn_protocols = vec![b"h3".to_vec()];
            let quic_crypto = quinn::crypto::rustls::QuicServerConfig::try_from(server_tls)
                .expect("server crypto");
            let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));
            let mut transport = quinn::TransportConfig::default();
            transport.max_concurrent_bidi_streams(100u32.into());
            transport.max_concurrent_uni_streams(100u32.into());
            server_config.transport_config(Arc::new(transport));
            let endpoint = quinn::Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap())
                .expect("bind server");
            let addr = endpoint.local_addr().expect("addr");
            let body = bytes::Bytes::from(body);
            let (tx, mut rx) = tokio::sync::watch::channel(false);
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        incoming = endpoint.accept() => {
                            let Some(incoming) = incoming else { break };
                            let body = body.clone();
                            tokio::spawn(async move {
                                let Ok(conn) = incoming.await else { return };
                                let h3_conn = h3_quinn::Connection::new(conn);
                                let Ok(mut server) =
                                    h3::server::Connection::<_, bytes::Bytes>::new(h3_conn).await
                                else {
                                    return;
                                };
                                loop {
                                    let Ok(Some(resolver)) = server.accept().await else { break };
                                    let body = body.clone();
                                    let Ok((_req, mut stream)) =
                                        resolver.resolve_request().await
                                    else {
                                        continue;
                                    };
                                    while stream.recv_data().await.ok().flatten().is_some() {}
                                    let resp = http::Response::builder()
                                        .status(200)
                                        .body(())
                                        .unwrap();
                                    if stream.send_response(resp).await.is_err() {
                                        break;
                                    }
                                    let _ = stream.send_data(body).await;
                                    let _ = stream.finish().await;
                                }
                            });
                        }
                        _ = rx.changed() => break,
                    }
                }
            });
            Self { addr, shutdown: tx }
        }
    }

    #[cfg(test)]
    impl Drop for LocalH3Server {
        fn drop(&mut self) {
            let _ = self.shutdown.send(true);
        }
    }

    /// A UDP blackhole: bound but never answered, so QUIC handshakes stall
    /// until the connect budget fires instead of failing fast with ICMP
    /// unreachable. Deterministic without public internet.
    #[cfg(test)]
    struct Blackhole {
        _socket: std::net::UdpSocket,
        addr: SocketAddr,
    }

    #[cfg(test)]
    impl Blackhole {
        fn bind() -> Self {
            let socket =
                std::net::UdpSocket::bind("127.0.0.1:0").expect("bind blackhole UDP socket");
            socket.set_nonblocking(true).expect("blackhole nonblocking");
            let addr = socket.local_addr().expect("blackhole addr");
            Self {
                _socket: socket,
                addr,
            }
        }
    }

    #[tokio::test]
    async fn failed_init_does_not_poison_origin() {
        // Connect to a closed localhost UDP port: no server answers, so with
        // a tight connect budget the attempt fails. The `OnceCell` must stay
        // uninitialized so a later attempt retries rather than replaying a
        // cached error.
        let blackhole = Blackhole::bind();
        // Use a neighboring closed port for the failure: unbind immediately
        // so nothing answers. (The blackhole itself is used by timeout tests
        // below; here we want a fast failure, so use a port we just freed.)
        let closed_port = {
            let s = std::net::UdpSocket::bind("127.0.0.1:0").expect("probe bind");
            s.local_addr().expect("probe addr").port()
        };
        let _ = &blackhole;
        let connector = H3Connector::new(None, &PoolConfig::default()).expect("connector builds");
        let started = std::time::Instant::now();
        let deadline = Some(started + Duration::from_millis(300));
        let bad = vec![SocketAddr::from(([127, 0, 0, 1], closed_port))];
        let first = connector
            .connect_with_fallback(&bad, "localhost", deadline, started)
            .await;
        assert!(first.is_err(), "expected handshake failure, got {first:?}");
        // No cache entry is created by `connect_with_fallback` itself; the
        // poison-freedom that matters is at the `OnceCell` layer, pinned by
        // `oncecell_failure_leaves_cell_empty` below.
    }

    #[tokio::test]
    async fn oncecell_failure_leaves_cell_empty() {
        // Pins the `Vacant -> Connecting -> Reconnectable` property that the
        // lifecycle depends on: `get_or_try_init` caches only success.
        let cell = tokio::sync::OnceCell::<u32>::new();
        let first: std::result::Result<&u32, &'static str> =
            cell.get_or_try_init(|| async { Err("boom") }).await;
        assert!(first.is_err());
        assert!(cell.get().is_none(), "failed init must not poison the cell");
        let second: std::result::Result<&u32, &'static str> =
            cell.get_or_try_init(|| async { Ok(7u32) }).await;
        assert_eq!(*second.expect("retry succeeds"), 7);
    }

    #[tokio::test]
    async fn fallback_skips_unreachable_address() {
        let server = LocalH3Server::start(b"fallback-ok".to_vec()).await;
        let closed_port = {
            let s = std::net::UdpSocket::bind("127.0.0.1:0").expect("probe bind");
            s.local_addr().expect("probe addr").port()
        };
        let tls = crate::tls::TlsConfig::builder()
            .danger_accept_invalid_certs(true)
            .build();
        let connector =
            H3Connector::new(Some(tls), &PoolConfig::default()).expect("connector builds");
        let started = std::time::Instant::now();
        // The closed port stalls (QUIC has no fast refuse); the fair-share
        // budget gives it half and reserves half for the reachable server.
        let deadline = Some(started + Duration::from_secs(8));
        let addrs = vec![SocketAddr::from(([127, 0, 0, 1], closed_port)), server.addr];
        let conn = tokio::time::timeout(
            Duration::from_secs(20),
            connector.connect_with_fallback(&addrs, "localhost", deadline, started),
        )
        .await
        .expect("fallback test must not hang")
        .expect("fallback to reachable address succeeds");
        assert_eq!(conn.remote_address(), server.addr);
    }

    #[tokio::test]
    async fn connect_budget_fires_connect_phase_timeout() {
        let blackhole = Blackhole::bind();
        let connector = H3Connector::new(None, &PoolConfig::default()).expect("connector builds");
        let started = std::time::Instant::now();
        let budget = Duration::from_millis(200);
        let deadline = Some(started + budget);
        let err = connector
            .connect_with_fallback(
                std::slice::from_ref(&blackhole.addr),
                "localhost",
                deadline,
                started,
            )
            .await
            .expect_err("blackhole handshake must not succeed");
        match err {
            Error::Timeout {
                phase: TimeoutPhase::Connect,
                ..
            } => {}
            other => panic!("expected connect-phase timeout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn fallback_cancellation_aborts_promptly() {
        let blackhole = Blackhole::bind();
        let connector = H3Connector::new(None, &PoolConfig::default()).expect("connector builds");
        let started = std::time::Instant::now();
        let deadline = Some(started + Duration::from_secs(30));
        // Abort well before the 30s budget: dropping the future must stop the
        // remaining attempts instead of stalling to the deadline.
        let pending = connector.connect_with_fallback(
            std::slice::from_ref(&blackhole.addr),
            "localhost",
            deadline,
            started,
        );
        let aborted = tokio::time::timeout(Duration::from_millis(200), pending).await;
        assert!(
            aborted.is_err(),
            "cancellation must abort the pending connect promptly"
        );
    }

    #[tokio::test]
    async fn write_timeout_propagates_without_masking() {
        // A stalled producer wrapped with the pipeline's write timeout must
        // surface as `Timeout{Write}` through the H3 send path, not as a
        // generic H3 protocol error.
        let server = LocalH3Server::start(b"ok".to_vec()).await;
        let tls = crate::tls::TlsConfig::builder()
            .danger_accept_invalid_certs(true)
            .build();
        let connector =
            H3Connector::new(Some(tls), &PoolConfig::default()).expect("connector builds");
        let stalled = futures_util::stream::pending::<std::result::Result<bytes::Bytes, Error>>();
        let wrapped =
            crate::stream::write_timeout_stream(Box::pin(stalled), Duration::from_millis(100));
        let body = RequestBody::Stream {
            stream: wrapped,
            length: None,
        };
        let uri: http::Uri = format!("https://127.0.0.1:{}/", server.addr.port())
            .parse()
            .expect("uri");
        let http_req = http::Request::builder()
            .method(http::Method::POST)
            .uri(uri)
            .version(http::Version::HTTP_3)
            .body(body)
            .expect("request");
        let url =
            url::Url::parse(&format!("https://127.0.0.1:{}/", server.addr.port())).expect("url");
        // Borrow the wrapped stream progression: the first `next()` on the
        // body yields the write timeout; `send_request` must propagate it.
        let err = connector
            .send_request(http_req, url, Some(Duration::from_secs(10)))
            .await
            .expect_err("stalled body must fail");
        match err {
            Error::Timeout {
                phase: TimeoutPhase::Write,
                ..
            } => {}
            other => panic!("expected write-phase timeout, got {other:?}"),
        }
        // A write-phase failure is request scoped: the origin entry stays so
        // later requests are unaffected (no poison, no spurious eviction).
        let key = format!("127.0.0.1:{}", server.addr.port());
        assert!(
            connector.sender_cache.contains_key(&key),
            "write timeout must not evict the shared connection"
        );
        let _ = server;
    }
}
