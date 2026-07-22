//! Phase-aware timeout configuration.
//!
//! Timeouts are applied per-phase during the request lifecycle. Each phase
//! measures a specific segment of the request:
//!
//! - **Pool**: time waiting for a connection slot from the pool.
//! - **Connect**: time to establish the TCP connection and TLS handshake
//!   (including DNS resolution).
//! - **`ProxyConnect`**: TCP connection to the proxy server. Only applies
//!   when a proxy is configured.
//! - **`ProxyTls`**: TLS handshake over a CONNECT tunnel to the proxy.
//!   Only applies to HTTPS targets routed through an HTTP proxy.
//! - **Write**: time for the request body producer to yield each chunk.
//!   Only applies to streamed request bodies; buffered bodies complete
//!   synchronously.
//! - **Read**: time to wait for response headers and each response body
//!   chunk. The deadline resets on every chunk arrival.
//! - **Total**: wall-clock cap across the entire request lifecycle.
//!
//! # Proxy timeout phases
//!
//! When a request is routed through a proxy, the connect phase splits
//! into two sub-phases:
//!
//! 1. `ProxyConnect` — the TCP connection from the client to the proxy.
//! 2. `ProxyTls` — the TLS handshake over the CONNECT tunnel (HTTPS
//!    targets only).
//!
//! After the tunnel is established, regular `Connect`, `Send`, and
//! `Receive` semantics apply to the destination.
//!
//! # Enforcement
//!
//! - **Pool** and **Total** are enforced with `tokio::time::timeout`.
//! - **Read** is enforced by a per-chunk wrapper stream that fires
//!   `Error::Timeout { phase: Read }` if no chunk arrives within the
//!   configured duration. The deadline resets on every chunk.
//! - **Write** is enforced by a per-chunk wrapper stream that fires
//!   `Error::Timeout { phase: Write }` if the producer does not yield
//!   the next chunk within the configured duration. The deadline resets
//!   on every chunk.
//! - **Connect** is enforced by wrapping the underlying connector with a
//!   timeout that bounds the entire connection-establishment phase (DNS
//!   resolution, TCP connect, and TLS handshake). Fires
//!   `Error::Timeout { phase: Connect }` if the connection is not
//!   established within the configured duration.
//!
//! When a scalar timeout is provided (e.g. `Timeout::from_secs(5)`), it
//! applies to pool, connect, write, and read phases. The total timeout is
//! not set by scalar constructors unless explicitly specified.
//!
//! Request-level timeout overrides client-level timeout on a per-field
//! basis: only the fields present in the request-level timeout replace
//! the corresponding client-level fields.

use std::time::Duration;

/// Identifies the phase of a request that timed out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeoutPhase {
    /// Waiting for a connection slot from the pool.
    Pool,
    /// Establishing TCP connection and TLS handshake.
    Connect,
    /// TCP connection to the proxy server.
    ProxyConnect,
    /// TLS handshake over a CONNECT tunnel to the proxy.
    ProxyTls,
    /// Sending request headers and body.
    Write,
    /// Waiting for response headers or a response body chunk.
    Read,
    /// The overall wall-clock deadline was exceeded.
    Total,
}

impl std::fmt::Display for TimeoutPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pool => write!(f, "pool"),
            Self::Connect => write!(f, "connect"),
            Self::ProxyConnect => write!(f, "proxy connect"),
            Self::ProxyTls => write!(f, "proxy TLS"),
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
            Self::Total => write!(f, "total"),
        }
    }
}

/// Phase-aware timeout configuration for a request.
///
/// Each field is optional. When `None`, the corresponding phase has no
/// timeout. When a field is `Some(duration)`, the phase will fail with
/// [`crate::Error::Timeout`] if it exceeds the given duration.
///
/// # Enforcement
///
/// - `pool`: enforced via `tokio::time::timeout` around the pool acquire.
/// - `read`: enforced per chunk by a wrapper stream that fires
///   `Error::Timeout { phase: Read }` when no data arrives within the
///   configured duration. Resets on every chunk arrival.
/// - `write`: enforced per chunk by a wrapper stream that fires
///   `Error::Timeout { phase: Write }` when the request body producer
///   does not yield a chunk within the configured duration. Resets on
///   every chunk delivery. Only applies to streamed request bodies;
///   buffered bodies complete synchronously.
/// - `total`: enforced via `tokio::time::timeout` around the full send.
/// - `connect`: enforced by wrapping the underlying connector with a
///   timeout that bounds DNS resolution, TCP connect, and TLS handshake.
///   Fires `Error::Timeout { phase: Connect }` if the connection is not
///   established within the configured duration.
///
/// # Default
///
/// The default `Timeout` has all phases disabled (all `None`).
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use eggfetch_core::Timeout;
///
/// // All phases get 5 seconds.
/// let t = Timeout::from_secs(5);
///
/// // Only read and total are configured.
/// let t = Timeout {
///     read: Some(Duration::from_secs(30)),
///     total: Some(Duration::from_secs(60)),
///     ..Timeout::default()
/// };
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct Timeout {
    /// Time allowed to wait for a connection slot from the pool.
    pub pool: Option<Duration>,
    /// Time allowed to establish TCP connection and TLS handshake.
    /// Includes DNS resolution when performed as part of connect.
    /// Enforced by wrapping the underlying connector with a deadline.
    pub connect: Option<Duration>,
    /// Time allowed for the request body producer to yield each chunk.
    /// Only applies to streamed request bodies.
    pub write: Option<Duration>,
    /// Time allowed between response body chunks. Resets on every chunk.
    pub read: Option<Duration>,
    /// Optional wall-clock cap across the entire request lifecycle.
    pub total: Option<Duration>,
}

impl Timeout {
    /// Create a timeout with all phases disabled.
    ///
    /// This is the same as `Timeout::default()`.
    #[must_use]
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Create a timeout where pool, connect, write, and read each get the
    /// given duration.
    ///
    /// The total timeout is not set. Use [`Timeout::builder`] or the
    /// struct literal syntax to set it explicitly.
    ///
    /// # Examples
    ///
    /// ```
    /// use eggfetch_core::Timeout;
    ///
    /// let t = Timeout::from_secs(5);
    /// assert_eq!(t.pool, Some(std::time::Duration::from_secs(5)));
    /// assert_eq!(t.connect, Some(std::time::Duration::from_secs(5)));
    /// assert_eq!(t.write, Some(std::time::Duration::from_secs(5)));
    /// assert_eq!(t.read, Some(std::time::Duration::from_secs(5)));
    /// assert!(t.total.is_none());
    /// ```
    #[must_use]
    pub fn from_secs(secs: u64) -> Self {
        let d = Duration::from_secs(secs);
        Self {
            pool: Some(d),
            connect: Some(d),
            write: Some(d),
            read: Some(d),
            total: None,
        }
    }

    /// Create an HTTPX-compatible timeout configuration.
    ///
    /// HTTPX 0.28.1 uses 5 seconds for all phases including a total
    /// wall-clock cap. Use this when migrating from HTTPX to preserve
    /// the same timeout semantics.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use eggfetch_core::Timeout;
    ///
    /// let t = Timeout::compat();
    /// assert_eq!(t.pool, Some(Duration::from_secs(5)));
    /// assert_eq!(t.connect, Some(Duration::from_secs(5)));
    /// assert_eq!(t.write, Some(Duration::from_secs(5)));
    /// assert_eq!(t.read, Some(Duration::from_secs(5)));
    /// assert_eq!(t.total, Some(Duration::from_secs(5)));
    /// ```
    #[must_use]
    pub fn compat() -> Self {
        let d = Duration::from_secs(5);
        Self {
            pool: Some(d),
            connect: Some(d),
            write: Some(d),
            read: Some(d),
            total: Some(d),
        }
    }

    /// Create eggfetch-native timeout defaults.
    ///
    /// Uses 30 seconds for pool, connect, write, and read phases with
    /// no total timeout. This is appropriate for general-purpose use
    /// where the caller controls overall deadlines.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use eggfetch_core::Timeout;
    ///
    /// let t = Timeout::native();
    /// assert_eq!(t.pool, Some(Duration::from_secs(30)));
    /// assert_eq!(t.connect, Some(Duration::from_secs(30)));
    /// assert_eq!(t.write, Some(Duration::from_secs(30)));
    /// assert_eq!(t.read, Some(Duration::from_secs(30)));
    /// assert!(t.total.is_none());
    /// ```
    #[must_use]
    pub fn native() -> Self {
        let d = Duration::from_secs(30);
        Self {
            pool: Some(d),
            connect: Some(d),
            write: Some(d),
            read: Some(d),
            total: None,
        }
    }

    /// Create a [`TimeoutBuilder`] for configuring individual phases.
    #[must_use]
    pub fn builder() -> TimeoutBuilder {
        TimeoutBuilder {
            pool: None,
            connect: None,
            write: None,
            read: None,
            total: None,
        }
    }

    /// Merge request-level overrides into client-level defaults.
    ///
    /// For each phase, if the request-level value is `Some`, it replaces
    /// the client-level value. If `None`, the client-level value is kept.
    #[must_use]
    pub(crate) fn merge(self, override_timeout: Option<Self>) -> Self {
        match override_timeout {
            Some(req) => Self {
                pool: req.pool.or(self.pool),
                connect: req.connect.or(self.connect),
                write: req.write.or(self.write),
                read: req.read.or(self.read),
                total: req.total.or(self.total),
            },
            None => self,
        }
    }

    /// Returns `true` if any phase has a timeout set.
    #[must_use]
    pub fn has_any(&self) -> bool {
        self.pool.is_some()
            || self.connect.is_some()
            || self.write.is_some()
            || self.read.is_some()
            || self.total.is_some()
    }
}

/// Builder for constructing a [`Timeout`] with individual phase durations.
///
/// Created by [`Timeout::builder()`].
#[derive(Debug, Clone)]
pub struct TimeoutBuilder {
    pool: Option<Duration>,
    connect: Option<Duration>,
    write: Option<Duration>,
    read: Option<Duration>,
    total: Option<Duration>,
}

impl TimeoutBuilder {
    /// Set the pool acquisition timeout.
    #[must_use]
    pub fn pool(mut self, timeout: Duration) -> Self {
        self.pool = Some(timeout);
        self
    }

    /// Set the connect timeout.
    #[must_use]
    pub fn connect(mut self, timeout: Duration) -> Self {
        self.connect = Some(timeout);
        self
    }

    /// Set the write timeout.
    #[must_use]
    pub fn write(mut self, timeout: Duration) -> Self {
        self.write = Some(timeout);
        self
    }

    /// Set the read timeout.
    #[must_use]
    pub fn read(mut self, timeout: Duration) -> Self {
        self.read = Some(timeout);
        self
    }

    /// Set the total request timeout.
    #[must_use]
    pub fn total(mut self, timeout: Duration) -> Self {
        self.total = Some(timeout);
        self
    }

    /// Build the [`Timeout`].
    #[must_use]
    pub fn build(self) -> Timeout {
        Timeout {
            pool: self.pool,
            connect: self.connect,
            write: self.write,
            read: self.read,
            total: self.total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn timeout_default_all_none() {
        let t = Timeout::default();
        assert!(t.pool.is_none());
        assert!(t.connect.is_none());
        assert!(t.write.is_none());
        assert!(t.read.is_none());
        assert!(t.total.is_none());
    }

    #[test]
    fn timeout_disabled_same_as_default() {
        let t = Timeout::disabled();
        assert!(!t.has_any());
    }

    #[test]
    fn timeout_from_secs_sets_phases() {
        let t = Timeout::from_secs(10);
        assert_eq!(t.pool, Some(Duration::from_secs(10)));
        assert_eq!(t.connect, Some(Duration::from_secs(10)));
        assert_eq!(t.write, Some(Duration::from_secs(10)));
        assert_eq!(t.read, Some(Duration::from_secs(10)));
        assert!(t.total.is_none());
    }

    #[test]
    fn timeout_builder_sets_all() {
        let t = Timeout::builder()
            .pool(Duration::from_secs(1))
            .connect(Duration::from_secs(2))
            .write(Duration::from_secs(3))
            .read(Duration::from_secs(4))
            .total(Duration::from_secs(5))
            .build();
        assert_eq!(t.pool, Some(Duration::from_secs(1)));
        assert_eq!(t.connect, Some(Duration::from_secs(2)));
        assert_eq!(t.write, Some(Duration::from_secs(3)));
        assert_eq!(t.read, Some(Duration::from_secs(4)));
        assert_eq!(t.total, Some(Duration::from_secs(5)));
    }

    #[test]
    fn timeout_merge_request_overrides_client() {
        let client = Timeout::from_secs(10);
        let request = Timeout {
            read: Some(Duration::from_secs(30)),
            ..Timeout::default()
        };
        let merged = client.merge(Some(request));
        assert_eq!(merged.pool, Some(Duration::from_secs(10)));
        assert_eq!(merged.connect, Some(Duration::from_secs(10)));
        assert_eq!(merged.write, Some(Duration::from_secs(10)));
        assert_eq!(merged.read, Some(Duration::from_secs(30)));
        assert!(merged.total.is_none());
    }

    #[test]
    fn timeout_merge_no_override() {
        let client = Timeout::from_secs(5);
        let merged = client.merge(None);
        assert_eq!(merged.pool, Some(Duration::from_secs(5)));
        assert_eq!(merged.connect, Some(Duration::from_secs(5)));
    }

    #[test]
    fn timeout_compat_sets_all_phases_with_total() {
        let t = Timeout::compat();
        let d = Duration::from_secs(5);
        assert_eq!(t.pool, Some(d));
        assert_eq!(t.connect, Some(d));
        assert_eq!(t.write, Some(d));
        assert_eq!(t.read, Some(d));
        assert_eq!(t.total, Some(d));
    }

    #[test]
    fn timeout_native_sets_phases_without_total() {
        let t = Timeout::native();
        let d = Duration::from_secs(30);
        assert_eq!(t.pool, Some(d));
        assert_eq!(t.connect, Some(d));
        assert_eq!(t.write, Some(d));
        assert_eq!(t.read, Some(d));
        assert!(t.total.is_none());
    }

    #[test]
    fn timeout_has_any() {
        assert!(!Timeout::default().has_any());
        assert!(Timeout::from_secs(1).has_any());
        assert!(Timeout {
            total: Some(Duration::from_secs(1)),
            ..Timeout::default()
        }
        .has_any());
    }

    #[test]
    fn timeout_phase_display() {
        assert_eq!(TimeoutPhase::Pool.to_string(), "pool");
        assert_eq!(TimeoutPhase::Connect.to_string(), "connect");
        assert_eq!(TimeoutPhase::ProxyConnect.to_string(), "proxy connect");
        assert_eq!(TimeoutPhase::ProxyTls.to_string(), "proxy TLS");
        assert_eq!(TimeoutPhase::Write.to_string(), "write");
        assert_eq!(TimeoutPhase::Read.to_string(), "read");
        assert_eq!(TimeoutPhase::Total.to_string(), "total");
    }

    #[test]
    fn timeout_merge_empty_override_preserves_client() {
        let client = Timeout {
            pool: Some(Duration::from_secs(1)),
            connect: Some(Duration::from_secs(2)),
            write: Some(Duration::from_secs(3)),
            read: Some(Duration::from_secs(4)),
            total: Some(Duration::from_secs(5)),
        };
        let merged = client.merge(Some(Timeout::default()));
        assert_eq!(merged.pool, Some(Duration::from_secs(1)));
        assert_eq!(merged.connect, Some(Duration::from_secs(2)));
        assert_eq!(merged.write, Some(Duration::from_secs(3)));
        assert_eq!(merged.read, Some(Duration::from_secs(4)));
        assert_eq!(merged.total, Some(Duration::from_secs(5)));
    }

    #[test]
    fn timeout_builder_matches_from_secs_for_phases() {
        let scalar = Timeout::from_secs(5);
        let builder = Timeout::builder()
            .pool(Duration::from_secs(5))
            .connect(Duration::from_secs(5))
            .write(Duration::from_secs(5))
            .read(Duration::from_secs(5))
            .build();
        assert_eq!(scalar.pool, builder.pool);
        assert_eq!(scalar.connect, builder.connect);
        assert_eq!(scalar.write, builder.write);
        assert_eq!(scalar.read, builder.read);
        assert_eq!(scalar.total, builder.total);
    }

    #[test]
    fn timeout_merge_request_overrides_total() {
        let client = Timeout::from_secs(10);
        let request = Timeout {
            total: Some(Duration::from_secs(60)),
            ..Timeout::default()
        };
        let merged = client.merge(Some(request));
        assert_eq!(merged.total, Some(Duration::from_secs(60)));
        assert_eq!(merged.pool, Some(Duration::from_secs(10)));
    }

    #[test]
    fn zero_duration_timeout_is_valid() {
        // Zero is a valid Duration in Rust (unsigned). It produces an immediate
        // timeout when used with tokio::time::timeout. This is documented
        // behavior, not an error.
        let t = Timeout::from_secs(0);
        assert_eq!(t.pool, Some(Duration::ZERO));
        assert_eq!(t.connect, Some(Duration::ZERO));
        assert_eq!(t.write, Some(Duration::ZERO));
        assert_eq!(t.read, Some(Duration::ZERO));
        assert_eq!(t.total, None);
    }

    proptest::proptest! {
        #[test]
        fn from_secs_sets_correct_phases(secs in 0u64..86400) {
            let t = Timeout::from_secs(secs);
            let d = Duration::from_secs(secs);
            prop_assert_eq!(t.pool, Some(d));
            prop_assert_eq!(t.connect, Some(d));
            prop_assert_eq!(t.write, Some(d));
            prop_assert_eq!(t.read, Some(d));
            prop_assert!(t.total.is_none());
        }

        #[test]
        fn disabled_has_no_phases(_ in 0u64..100) {
            let t = Timeout::disabled();
            prop_assert!(!t.has_any());
            prop_assert!(t.pool.is_none());
            prop_assert!(t.connect.is_none());
            prop_assert!(t.write.is_none());
            prop_assert!(t.read.is_none());
            prop_assert!(t.total.is_none());
        }

        #[test]
        fn has_any_consistent_with_fields(
            pool in proptest::option::of(0u64..3600),
            connect in proptest::option::of(0u64..3600),
            write in proptest::option::of(0u64..3600),
            read in proptest::option::of(0u64..3600),
            total in proptest::option::of(0u64..3600),
        ) {
            let t = Timeout {
                pool: pool.map(Duration::from_secs),
                connect: connect.map(Duration::from_secs),
                write: write.map(Duration::from_secs),
                read: read.map(Duration::from_secs),
                total: total.map(Duration::from_secs),
            };
            let expected = t.pool.is_some() || t.connect.is_some() || t.write.is_some() || t.read.is_some() || t.total.is_some();
            prop_assert_eq!(t.has_any(), expected);
        }

        #[test]
        fn builder_round_trip(
            pool in 0u64..3600,
            connect in 0u64..3600,
            write in 0u64..3600,
            read in 0u64..3600,
            total in 0u64..3600,
        ) {
            let t = Timeout::builder()
                .pool(Duration::from_secs(pool))
                .connect(Duration::from_secs(connect))
                .write(Duration::from_secs(write))
                .read(Duration::from_secs(read))
                .total(Duration::from_secs(total))
                .build();
            prop_assert_eq!(t.pool, Some(Duration::from_secs(pool)));
            prop_assert_eq!(t.connect, Some(Duration::from_secs(connect)));
            prop_assert_eq!(t.write, Some(Duration::from_secs(write)));
            prop_assert_eq!(t.read, Some(Duration::from_secs(read)));
            prop_assert_eq!(t.total, Some(Duration::from_secs(total)));
        }
    }
}
