//! Centralized Hyper legacy-client construction policy and bounded route caches.
//!
//! Every persistent H1/H2 Hyper client owned by `ClientInner` is assembled
//! from a route-specific connector plus the shared policy in this module.
//! Route connectors legitimately differ (standard hyper-rustls, direct with
//! socket options, UDS, caller-owned dialer, SOCKS, forward proxy, CONNECT),
//! but the builder knobs, lifecycle wrapping, and cache mechanics do not.
//!
//! What lives here:
//!
//! - [`HyperClientPolicy`]: the immutable H1/H2 builder policy (canceled-request
//!   retry, H2-only selection, idle timeout, per-host idle cap). It takes
//!   semantic inputs rather than an entire `ClientConfig`.
//! - [`build_hyper_client`]: one generic helper that wraps a route connector
//!   as `route connector -> ConnectTimeout -> LifecycleConnector` and builds
//!   a legacy Hyper client from a [`HyperClientPolicy`]. Wrapping and builder
//!   configuration share one function because the generic bounds are identical;
//!   a separate wrap-only helper would not be clearer.
//! - [`BoundedClientCache`]: the tiny bounded-map mechanics shared by the SNI,
//!   custom-SNI, SOCKS, forward-proxy, and CONNECT route caches, with
//!   route-specific capacities ([`SNI_CLIENT_CACHE_MAX_ENTRIES`] and friends).
//!
//! Baseline note: the corrective/invariant predecessors (proxy TLS cache
//! identity, Hyper idle-pool timer propagation, route-cache invariant tests)
//! have not landed yet, so this module centralizes current behavior exactly
//! as-is. It must not be read as the corrected policy:
//!
//! - no Hyper pool timer is installed here (the pending idle-pool corrective
//!   owns `pool_timer` installation in [`HyperClientPolicy::apply`]);
//! - SOCKS route clients receive no Hyper idle timeout/cap (call sites pass
//!   `None`; the pending corrective owns propagation);
//! - proxy TLS compatibility identity is untouched.
//!
//! Centralizing first means those corrections later land in one place.
//!
//! Locking contract: route caches are held behind an async mutex while the
//! configured client is built. Connector construction (`new`/`with_*`) and
//! TLS policy builds are CPU/local configuration only and perform no network
//! I/O, so holding the lock is safe. If a future construction path performs
//! I/O, it must be restructured to build outside the lock. Construction
//! failures return before insert so they never poison the cache.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

use http::Uri;
use hyper_util::client::legacy::connect::Connection;
use hyper_util::rt::TokioExecutor;
use tower_service::Service;

use super::connect_timeout::ConnectTimeout;
use super::lifecycle::{LifecycleConfig, LifecycleConnector};
use super::HyperRequestBody;
use crate::http_version::HttpVersionPolicyEnabler;

/// Upper bound on cached SNI-keyed hyper clients (direct and custom-dialer).
pub(crate) const SNI_CLIENT_CACHE_MAX_ENTRIES: usize = 256;

/// Upper bound on cached SOCKS-route hyper clients.
#[cfg(feature = "proxy")]
pub(crate) const SOCKS_CLIENT_CACHE_MAX_ENTRIES: usize = 64;

/// Upper bound on cached HTTP forward-proxy Hyper clients.
#[cfg(feature = "proxy")]
pub(crate) const FORWARD_CLIENT_CACHE_MAX_ENTRIES: usize = 64;

/// Upper bound on cached HTTPS CONNECT Hyper clients.
#[cfg(feature = "proxy")]
pub(crate) const CONNECT_CLIENT_CACHE_MAX_ENTRIES: usize = 64;

/// Immutable H1/H2 builder policy shared by every persistent Hyper client.
///
/// Resolved from client configuration at construction time; route call sites
/// pick one of the named constructors so the chosen idle shape stays explicit.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HyperClientPolicy {
    retry_canceled_requests: bool,
    idle_timeout: Option<Duration>,
    max_idle_per_host: Option<usize>,
    http2_only: bool,
}

impl HyperClientPolicy {
    /// H2-only selection shared by every route: HTTP/1 disabled and HTTP/2
    /// enabled. Feature-gating in [`HttpVersionPolicyEnabler`] downgrades
    /// H2-only to H1-only when `http2` is not compiled in, so this is false
    /// there and [`HyperClientPolicy::apply`] becomes a no-op for the flag.
    fn http2_only(enabler: HttpVersionPolicyEnabler) -> bool {
        !enabler.enable_http1() && enabler.enable_http2()
    }

    /// Policy for the persistent per-client singletons (standard, direct,
    /// UDS, custom dialer): shared idle timeout plus the configured per-host
    /// idle cap.
    pub(crate) fn persistent(
        retry_canceled_requests: bool,
        idle_timeout: Option<Duration>,
        max_idle_per_host: Option<usize>,
        enabler: HttpVersionPolicyEnabler,
    ) -> Self {
        Self {
            retry_canceled_requests,
            idle_timeout,
            max_idle_per_host,
            http2_only: Self::http2_only(enabler),
        }
    }

    /// Policy for bounded route caches and isolated route clients (resolved
    /// target, SNI, custom-SNI, CONNECT): shared idle timeout, no per-host
    /// idle cap.
    ///
    /// SOCKS call sites pass `None` for `idle_timeout` to preserve current
    /// behavior (SOCKS pools receive no Hyper idle tuning today); the pending
    /// idle-pool corrective owns that propagation.
    pub(crate) fn cached_route(
        retry_canceled_requests: bool,
        idle_timeout: Option<Duration>,
        enabler: HttpVersionPolicyEnabler,
    ) -> Self {
        Self {
            retry_canceled_requests,
            idle_timeout,
            max_idle_per_host: None,
            http2_only: Self::http2_only(enabler),
        }
    }

    /// Policy for the HTTP forward-proxy route cache.
    ///
    /// The proxy leg uses H1 absolute-form framing owned by Hyper, so
    /// `http2_only` is never set even under an H2-only client policy. This is
    /// an intentional route exception, not an oversight.
    #[cfg(feature = "proxy")]
    pub(crate) fn forward_route(
        retry_canceled_requests: bool,
        idle_timeout: Option<Duration>,
    ) -> Self {
        Self {
            retry_canceled_requests,
            idle_timeout,
            max_idle_per_host: None,
            http2_only: false,
        }
    }

    /// Apply the shared knobs to a legacy Hyper builder. Only configured
    /// values are set so Hyper defaults apply otherwise.
    ///
    /// When HTTP/1 is disabled and HTTP/2 is enabled, the legacy client is
    /// marked HTTP/2-only. This is what enforces the protocol contract:
    /// hyper-util attempts an HTTP/2 handshake on every socket (including
    /// cleartext, where it falls through to HTTP/2 prior knowledge). When
    /// ALPN does not negotiate `h2`, the HTTP/2 handshake fails with a
    /// `Connect` error rather than silently downgrading to HTTP/1.1.
    pub(crate) fn apply(&self, builder: &mut hyper_util::client::legacy::Builder) {
        builder.retry_canceled_requests(self.retry_canceled_requests);
        #[cfg(feature = "http2")]
        if self.http2_only {
            builder.http2_only(true);
        }
        #[cfg(not(feature = "http2"))]
        let _ = self.http2_only;
        if let Some(idle_timeout) = self.idle_timeout {
            builder.pool_idle_timeout(idle_timeout);
        }
        if let Some(max_idle_per_host) = self.max_idle_per_host {
            builder.pool_max_idle_per_host(max_idle_per_host);
        }
    }
}

/// Build a legacy Hyper client from a route connector and shared policy.
///
/// The connector is wrapped as `route connector` -> `ConnectTimeout` ->
/// `LifecycleConnector`, then built with a Tokio executor and the given
/// policy. Monomorphized concrete connector types are used; no boxed dynamic
/// connectors are introduced to share this path.
pub(crate) fn build_hyper_client<C>(
    connector: C,
    policy: &HyperClientPolicy,
    connect_timeout: Option<Duration>,
    lifecycle: &Arc<LifecycleConfig>,
) -> hyper_util::client::legacy::Client<LifecycleConnector<ConnectTimeout<C>>, HyperRequestBody>
where
    C: Service<Uri> + Clone + Send + Sync + 'static,
    C::Response: hyper::rt::Read + hyper::rt::Write + Connection + Unpin + Send + 'static,
    C::Error: Into<Box<dyn std::error::Error + Send + Sync>> + 'static,
    C::Future: Send + 'static,
{
    let connector = ConnectTimeout::new(connector, connect_timeout);
    let connector = LifecycleConnector::new(connector, lifecycle.clone());
    let mut builder = hyper_util::client::legacy::Client::builder(TokioExecutor::new());
    policy.apply(&mut builder);
    builder.build(connector)
}

/// Tiny bounded map for cached configured Hyper clients.
///
/// Centralizes the get-or-build-and-evict protocol: lookups return the stored
/// configured client, misses insert the freshly built client, and an arbitrary
/// entry is evicted when the map is at capacity so long-lived processes
/// touching many routes cannot grow it without limit. Entries hold pooled
/// connections but no unsynchronized state, so any entry can be dropped safely
/// and recreated lazily; hash-order arbitrary eviction avoids an LRU
/// dependency for these low-churn caches.
///
/// Keys must never render secrets: route-key types carry connection identity
/// only and this cache performs no logging or `Debug` of its contents beyond
/// the derived implementation used in diagnostics.
#[derive(Debug)]
pub(crate) struct BoundedClientCache<K, V> {
    entries: HashMap<K, V>,
    capacity: usize,
}

impl<K, V> BoundedClientCache<K, V> {
    /// Create an empty cache holding at most `capacity` entries.
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            capacity,
        }
    }

    /// Number of cached entries. Test-only helper for bound regressions.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache holds no entries. Test-only helper.
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<K, V> BoundedClientCache<K, V>
where
    K: Eq + Hash,
{
    /// Return the cached client for `key`, if present. Accepts borrowed
    /// lookups (`str` for `String` keys) exactly like [`HashMap::get`].
    pub(crate) fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: std::borrow::Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.entries.get(key)
    }
}

impl<K, V> BoundedClientCache<K, V>
where
    K: Eq + Hash + Clone,
{
    /// Insert `value` under `key`, evicting an arbitrary entry first when the
    /// cache is already at capacity. Replacing an existing key never evicts.
    pub(crate) fn insert(&mut self, key: K, value: V) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            // `capacity` is always non-zero in practice; a zero capacity
            // degrades to a single-entry cache rather than growing.
            if let Some(victim) = self.entries.keys().next().cloned() {
                self.entries.remove(&victim);
            }
        }
        self.entries.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HttpVersionPolicy;

    #[test]
    fn bounded_cache_returns_hits_without_growth() {
        let mut cache = BoundedClientCache::new(2);
        cache.insert("a".to_owned(), 1_u32);
        assert_eq!(cache.get("a"), Some(&1));
        assert_eq!(cache.len(), 1);
        // Replacing an existing key updates in place and never evicts.
        cache.insert("a".to_owned(), 2_u32);
        assert_eq!(cache.get("a"), Some(&2));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn bounded_cache_evicts_arbitrary_entry_at_capacity() {
        let mut cache = BoundedClientCache::new(2);
        cache.insert("a".to_owned(), 1_u32);
        cache.insert("b".to_owned(), 2_u32);
        cache.insert("c".to_owned(), 3_u32);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get("c"), Some(&3));
        // Exactly one of the earlier entries survived eviction.
        assert_eq!(
            usize::from(cache.get("a").is_some()) + usize::from(cache.get("b").is_some()),
            1
        );
    }

    #[test]
    fn bounded_cache_miss_returns_none() {
        let cache: BoundedClientCache<String, u32> = BoundedClientCache::new(64);
        assert!(cache.is_empty());
        assert_eq!(cache.get("missing"), None);
    }

    #[test]
    fn policy_http2_only_matches_h2_only_client() {
        let h2_only = HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Http2Only);
        let auto =
            HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Auto { allow_http3: false });
        let h1_only = HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Http1Only);
        #[cfg(feature = "http2")]
        assert!(HyperClientPolicy::cached_route(true, None, h2_only).http2_only);
        #[cfg(not(feature = "http2"))]
        assert!(!HyperClientPolicy::cached_route(true, None, h2_only).http2_only);
        assert!(!HyperClientPolicy::cached_route(true, None, auto).http2_only);
        assert!(!HyperClientPolicy::cached_route(true, None, h1_only).http2_only);
    }

    #[test]
    fn forward_route_never_selects_http2_only() {
        let h2_only = HttpVersionPolicyEnabler::from_policy(HttpVersionPolicy::Http2Only);
        // The forward constructor takes no version policy at all: the proxy
        // leg is H1 absolute-form regardless of client configuration.
        let _ = h2_only;
        assert!(!HyperClientPolicy::forward_route(true, None).http2_only);
    }
}
