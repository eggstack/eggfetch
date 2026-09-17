//! Protocol-neutral native transport controls.
//!
//! `ResolvedTarget`, `TransportHints` and `NativeRequestOptions` carry only
//! wire-level behavior (request-target override, SNI override, pinned
//! destinations, trace observer, timeout override). They do not depend on
//! high-level URL semantics and therefore compile in the minimal native
//! transport slice without the `url` dependency.
//!
//! The historical `crate::request::{ResolvedTarget, TransportHints,
//! NativeRequestOptions}` paths remain available as re-exports when the
//! high-level URL feature is enabled; new code should prefer this module
//! directly.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;

use crate::timeout::Timeout;
use crate::trace::TraceObserver;

/// A validated set of remote socket addresses for one logical HTTP origin.
///
/// The addresses control only the physical TCP destination. The logical URI,
/// HTTP authority, TLS SNI, certificate verification, and origin policy still
/// use the logical origin. An empty set cannot be constructed. Callers are
/// responsible for validating that the addresses are appropriate for their
/// application and redirect policy.
#[derive(Clone, PartialEq, Eq)]
pub struct ResolvedTarget {
    addresses: Arc<[SocketAddr]>,
}

impl ResolvedTarget {
    /// Construct a resolved destination from one or more socket addresses.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::InvalidResolvedTarget`] for an empty set.
    pub fn new<I>(addresses: I) -> crate::Result<Self>
    where
        I: IntoIterator<Item = SocketAddr>,
    {
        let addresses: Vec<_> = addresses.into_iter().collect();
        if addresses.is_empty() {
            return Err(crate::Error::InvalidResolvedTarget(
                "resolved destination set must not be empty".into(),
            ));
        }
        Ok(Self {
            addresses: addresses.into(),
        })
    }

    /// Return the caller-supplied destination addresses in their attempt order.
    #[must_use]
    pub fn addresses(&self) -> &[SocketAddr] {
        &self.addresses
    }

    /// Cheaply clone the ordered address snapshot for route-cache keying.
    ///
    /// Crate-private so the public API does not grow merely for `HashMap`
    /// key convenience. The order is preserved exactly; callers must not
    /// sort, deduplicate, or otherwise canonicalize it.
    pub(crate) fn addresses_shared(&self) -> Arc<[SocketAddr]> {
        Arc::clone(&self.addresses)
    }
}

impl std::fmt::Debug for ResolvedTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedTarget")
            .field("address_count", &self.addresses.len())
            .finish()
    }
}

/// Typed transport-level hints carried on a request.
///
/// These are *not* HTTP headers and do not affect logical URL semantics
/// (routing, cookies, auth-origin comparisons, redirects, proxy selection).
/// They control only the wire-level behavior of the transport layer.
///
/// Timeout is kept in the existing [`Timeout`] model and must not be
/// duplicated here.
#[derive(Default, Clone)]
pub struct TransportHints {
    /// Override the wire request target (e.g. `OPTIONS *`, absolute-form).
    ///
    /// When present this replaces only the URI sent on the wire; the
    /// logical origin used for connection routing, Host header defaults,
    /// cookies, auth, redirects, and proxy selection remains unchanged.
    pub target: Option<Bytes>,
    /// Override the TLS Server Name Indication hostname.
    ///
    /// TCP connects to the URI host/IP; TLS uses this name for SNI and
    /// certificate verification.
    pub sni_hostname: Option<String>,
    /// Caller-supplied physical destinations for the logical origin.
    ///
    /// When present, direct routing uses exactly these addresses and never
    /// performs a DNS lookup. Same-origin redirects preserve this snapshot;
    /// cross-origin redirects fail closed. Proxy, UDS, and HTTP/3 routes are
    /// rejected while this hint is active.
    pub resolved_target: Option<ResolvedTarget>,
    /// Optional trace observer for transport lifecycle events.
    ///
    /// When present, the transport layer emits typed events at each
    /// lifecycle boundary (TCP connect, TLS handshake, request/response
    /// headers, body chunks, connection close). The observer is invoked
    /// synchronously within the async context and must not block.
    pub trace: Option<Arc<dyn TraceObserver>>,
}

impl std::fmt::Debug for TransportHints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransportHints")
            .field("target", &self.target)
            .field("sni_hostname", &self.sni_hostname)
            .field("resolved_target", &self.resolved_target)
            .field("trace", &self.trace.as_ref().map(|_| "..."))
            .finish()
    }
}

/// Transport-only options for [`Client::execute_http_body`](crate::Client::execute_http_body).
///
/// The native body API intentionally does not apply redirects, retries,
/// cookies, authentication, decompression, or decoded-body limits. Those
/// policies can change or replay wire bytes and remain part of the
/// high-level request API instead.
#[derive(Default, Clone)]
pub struct NativeRequestOptions {
    /// Optional timeout override. It is merged with the client timeout using
    /// the same phase-aware rules as ordinary requests.
    pub timeout: Option<Timeout>,
    /// Typed transport hints such as a resolved destination or SNI name.
    pub transport_hints: TransportHints,
}

impl std::fmt::Debug for NativeRequestOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeRequestOptions")
            .field("timeout", &self.timeout)
            .field("transport_hints", &self.transport_hints)
            .finish()
    }
}

impl NativeRequestOptions {
    /// Set a request-scoped timeout override.
    #[must_use]
    pub fn timeout(mut self, timeout: Timeout) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set typed transport hints for this request.
    #[must_use]
    pub fn transport_hints(mut self, transport_hints: TransportHints) -> Self {
        self.transport_hints = transport_hints;
        self
    }
}
