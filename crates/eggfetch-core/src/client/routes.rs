//! Private route identity and cached client acquisition.

#[cfg(feature = "advanced-routing")]
use std::sync::Arc;
#[cfg(all(
    feature = "proxy",
    any(feature = "transport-http1", feature = "transport-http2")
))]
use std::time::Duration;

#[cfg(feature = "tls-rustls")]
use super::configure_tls_alpn;
#[cfg(any(feature = "advanced-routing", feature = "proxy"))]
use super::{ClientConfig, Error, Pool};
use super::{ClientInner, Result};
#[cfg(any(
    feature = "transport-http1",
    feature = "transport-http2",
    feature = "advanced-routing",
    feature = "proxy"
))]
use crate::transport::hyper_client::{build_hyper_client, HyperClientPolicy};
#[cfg(any(
    feature = "transport-http1",
    feature = "transport-http2",
    feature = "advanced-routing"
))]
use crate::transport::lifecycle::LifecycleConfig;

/// Internal cache identity for one direct resolved-target route.
///
/// The type deliberately does not implement `Debug` or `Display`: route keys
/// carry connection identity only and must never render request state in
/// diagnostics. Compare with `==`/`!=` in tests.
///
/// Connection-scoped: normalized logical HTTP origin (scheme, host, effective
/// port as `scheme://host:port` with an always-explicit port and no
/// path/query/fragment), the full ordered physical address snapshot (order
/// controls attempt/failover order, so reordering fragments identity), and
/// the exact optional SNI override (no normalization: a redundant miss is
/// safe, a false hit is a security defect). TLS config/provider/trust
/// policy, HTTP version/ALPN policy, direct socket/local-address
/// configuration, connect timeout, lifecycle policy, canceled-request retry
/// behavior, and idle-pool policy are immutable at `ClientInner` scope after
/// `ClientBuilder::build` and therefore stay outside the per-route key.
/// Request-scoped state (total/read/write/pool deadlines, retry/redirect
/// state, body, cookies/auth headers, decompression limits, trace observers,
/// failure contexts) is never represented. If any currently client-wide
/// connection-affecting field becomes request-scoped, this key must be
/// expanded before the new variability may use the cache.
#[cfg(feature = "advanced-routing")]
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct ResolvedRouteKey {
    origin: String,
    addresses: Arc<[std::net::SocketAddr]>,
    sni_hostname: Option<String>,
}

#[cfg(feature = "advanced-routing")]
impl ResolvedRouteKey {
    /// Canonical constructor from a native origin.
    pub(crate) fn from_origin(
        origin: &crate::http_origin::HttpOrigin,
        target: &crate::transport_hints::ResolvedTarget,
        sni_hostname: Option<&str>,
    ) -> Self {
        Self {
            origin: origin.origin_string(),
            addresses: target.addresses_shared(),
            sni_hostname: sni_hostname.map(str::to_owned),
        }
    }

    /// High-level adapter from `url::Url`; reduces to the same canonical
    /// `scheme://host:port` representation as [`Self::from_origin`].
    ///
    /// Test-only: production high-level routes convert via
    /// `HttpOrigin::from_url` + `from_origin` so pool and route identity
    /// share one canonical constructor.
    #[cfg(all(feature = "high-level-url", any(test, feature = "test-util")))]
    #[allow(dead_code)]
    pub(crate) fn new(
        origin_url: &url::Url,
        target: &crate::transport_hints::ResolvedTarget,
        sni_hostname: Option<&str>,
    ) -> Self {
        match crate::http_origin::HttpOrigin::from_url(origin_url) {
            Some(origin) => Self::from_origin(&origin, target, sni_hostname),
            None => Self {
                // Fall back to the legacy serialization for non-HTTP(S) or
                // hostless URLs; such routes fail closed before I/O.
                origin: origin_url.origin().ascii_serialization(),
                addresses: target.addresses_shared(),
                sni_hostname: sni_hostname.map(str::to_owned),
            },
        }
    }
}

impl ClientInner {
    /// Get or create a cached Hyper client for one direct resolved-target
    /// route.
    ///
    /// The cache key is the normalized logical origin plus the full ordered
    /// physical address snapshot plus the exact SNI override. Identical route
    /// identity reuses the configured Hyper client (and therefore Hyper-owned
    /// H1 keep-alive / H2 multiplexed connections); any change in those
    /// dimensions selects a different entry. Ordinary DNS/direct clients never
    /// share these entries.
    #[cfg(feature = "advanced-routing")]
    pub(crate) async fn resolved_client(
        &self,
        origin: &crate::http_origin::HttpOrigin,
        target: &crate::transport_hints::ResolvedTarget,
        sni_hostname: Option<&str>,
    ) -> Result<crate::transport::TimeoutDirectClient> {
        let key = ResolvedRouteKey::from_origin(origin, target, sni_hostname);
        let mut clients = self.resolved_clients.lock().await;
        if let Some(client) = clients.get(&key) {
            return Ok(client.clone());
        }
        let client = Self::build_resolved_client(
            &self.config,
            self.direct_connector_config.as_ref(),
            &self.transport_metrics,
            &self.pool,
            &self.lifecycle,
            target,
            sni_hostname,
        )?;
        clients.insert(key, client.clone());
        Ok(client)
    }

    /// Build a configured resolved-route Hyper client without touching the
    /// route cache.
    ///
    /// CPU/local configuration only (connector + TLS policy + Hyper builder);
    /// performs no network I/O, so callers may hold the route-cache mutex
    /// while building, matching the documented route-cache locking contract.
    /// Construction failures return before insert and never poison the cache.
    #[cfg(feature = "advanced-routing")]
    #[allow(clippy::too_many_arguments)]
    fn build_resolved_client(
        config: &ClientConfig,
        direct_connector_config: Option<&crate::transport::direct_connector::DirectConnectorConfig>,
        transport_metrics: &Arc<crate::transport::metrics::TransportMetrics>,
        pool: &Pool,
        lifecycle: &Arc<LifecycleConfig>,
        target: &crate::transport_hints::ResolvedTarget,
        sni_hostname: Option<&str>,
    ) -> Result<crate::transport::TimeoutDirectClient> {
        let connect_timeout = config.timeout.as_ref().and_then(|t| t.connect);
        #[cfg(feature = "tls-rustls")]
        let tls_connector = {
            let tls_config = config.tls_config.clone().unwrap_or_default();
            let rustls_config = tls_config
                .build_rustls_config()
                .map_err(|e| Error::Tls(format!("failed to build TLS config: {e}")))?;
            let rustls_config = configure_tls_alpn(
                rustls_config,
                crate::http_version::HttpVersionPolicyEnabler::from_policy(
                    config.http_version_policy,
                ),
            );
            Some(tokio_rustls::TlsConnector::from(Arc::new(rustls_config)))
        };
        #[cfg(not(feature = "tls-rustls"))]
        let tls_connector = None;

        let base_config = direct_connector_config.cloned().unwrap_or(
            crate::transport::direct_connector::DirectConnectorConfig {
                local_address: None,
                socket_options: Vec::new(),
            },
        );
        let mut connector = crate::transport::direct_connector::DirectConnector::with_metrics(
            base_config,
            tls_connector,
            transport_metrics.clone(),
        )
        .with_resolved_target(target);
        if let Some(sni_hostname) = sni_hostname {
            connector = connector.with_sni(sni_hostname.to_owned());
        }
        let policy = HyperClientPolicy::cached_route(
            config.retry_canceled_requests,
            pool.idle_timeout(),
            pool.max_idle_per_host(),
            crate::http_version::HttpVersionPolicyEnabler::from_policy(config.http_version_policy),
        );
        Ok(build_hyper_client(
            connector,
            &policy,
            connect_timeout,
            lifecycle,
        ))
    }

    /// Get or create a cached hyper client with TLS SNI hostname override.
    ///
    /// The returned client uses a [`DirectConnector`](crate::transport::direct_connector::DirectConnector)
    /// that separates DNS/TCP resolution (to the original URL host) from
    /// TLS negotiation (with the SNI hostname). Clients are cached by
    /// SNI hostname for connection reuse.
    #[cfg(feature = "advanced-routing")]
    pub(crate) async fn sni_client(
        &self,
        sni_hostname: &str,
    ) -> Result<crate::transport::TimeoutDirectClient> {
        let mut clients = self.sni_clients.lock().await;
        if let Some(client) = clients.get(sni_hostname) {
            return Ok(client.clone());
        }

        let connect_timeout = self.config.timeout.as_ref().and_then(|t| t.connect);
        let enabler = crate::http_version::HttpVersionPolicyEnabler::from_policy(
            self.config.http_version_policy,
        );

        #[cfg(feature = "tls-rustls")]
        let tls_connector = {
            let tls_config = self.config.tls_config.clone().unwrap_or_default();
            let rc = tls_config
                .build_rustls_config()
                .map_err(|e| Error::Tls(format!("failed to build TLS config: {e}")))?;
            let rc = configure_tls_alpn(rc, enabler);
            Some(tokio_rustls::TlsConnector::from(Arc::new(rc)))
        };
        #[cfg(not(feature = "tls-rustls"))]
        let tls_connector = None;

        let base_config = self.direct_connector_config.clone().unwrap_or(
            crate::transport::direct_connector::DirectConnectorConfig {
                local_address: None,
                socket_options: Vec::new(),
            },
        );
        let base_connector = crate::transport::direct_connector::DirectConnector::with_metrics(
            base_config,
            tls_connector,
            self.transport_metrics.clone(),
        );
        let sni_connector = base_connector.with_sni(sni_hostname.to_owned());
        // Idle-connection lifetime comes from the pool configuration
        // (`Limits::keepalive_expiry`), matching the standard, direct,
        // and UDS paths. `Timeout.pool`/`total` are acquisition budgets
        // and must not close idle connections early. The effective
        // per-host idle cap is shared as well; see `Pool::max_idle_per_host`.
        let policy = HyperClientPolicy::cached_route(
            self.config.retry_canceled_requests,
            self.pool.idle_timeout(),
            self.pool.max_idle_per_host(),
            enabler,
        );
        let client = build_hyper_client(sni_connector, &policy, connect_timeout, &self.lifecycle);
        clients.insert(sni_hostname.to_owned(), client.clone());
        Ok(client)
    }

    /// Get or create a custom-dialer Hyper client with a fixed SNI override.
    #[cfg(feature = "advanced-routing")]
    pub(crate) async fn sni_custom_client(
        &self,
        sni_hostname: &str,
    ) -> Result<crate::transport::TimeoutCustomClient> {
        let mut clients = self.sni_custom_clients.lock().await;
        if let Some(client) = clients.get(sni_hostname) {
            return Ok(client.clone());
        }
        let dialer = self
            .config
            .dialer
            .clone()
            .ok_or_else(|| Error::Unsupported("custom dialer is not configured".into()))?;
        #[cfg(feature = "tls-rustls")]
        let tls_config = self
            .config
            .tls_config
            .clone()
            .unwrap_or_default()
            .build_rustls_config()
            .map_err(|error| Error::Tls(format!("failed to build TLS config: {error}")))?;
        #[cfg(not(feature = "tls-rustls"))]
        let tls_config = ();
        let connector = super::connectors::build_custom_connector(
            tls_config,
            dialer,
            crate::http_version::HttpVersionPolicyEnabler::from_policy(
                self.config.http_version_policy,
            ),
            Some(sni_hostname),
        )?;
        let policy = HyperClientPolicy::cached_route(
            self.config.retry_canceled_requests,
            self.pool.idle_timeout(),
            self.pool.max_idle_per_host(),
            crate::http_version::HttpVersionPolicyEnabler::from_policy(
                self.config.http_version_policy,
            ),
        );
        let client = build_hyper_client(
            connector,
            &policy,
            self.config
                .timeout
                .as_ref()
                .and_then(|timeout| timeout.connect),
            &self.lifecycle,
        );
        clients.insert(sni_hostname.to_owned(), client.clone());
        Ok(client)
    }
}

#[cfg(feature = "proxy")]
impl ClientInner {
    pub(crate) async fn socks_client(
        &self,
        proxy: &crate::proxy::ProxyConfig,
        target: Option<&crate::transport_hints::ResolvedTarget>,
    ) -> Result<crate::transport::TimeoutSocksClient> {
        let key = crate::transport::socks::SocksRouteKey::from_proxy(proxy, target)?;
        let mut clients = self.socks_clients.lock().await;
        if let Some(client) = clients.get(&key) {
            return Ok(client.clone());
        }

        let enabler = crate::http_version::HttpVersionPolicyEnabler::from_policy(
            self.config.http_version_policy,
        );

        let tls_config = if let Some(config) = self.config.tls_config.as_ref() {
            config
                .build_rustls_config()
                .map_err(|e| Error::Tls(format!("failed to build SOCKS TLS config: {e}")))?
        } else {
            crate::tls::TlsConfig::default()
                .build_rustls_config()
                .map_err(|e| Error::Tls(format!("failed to build default SOCKS TLS config: {e}")))?
        };
        let tls_config = configure_tls_alpn(tls_config, enabler);
        let connector = crate::transport::socks::SocksConnector::new(
            proxy.clone(),
            Some(tokio_rustls::TlsConnector::from(Arc::new(tls_config))),
            None,
            target,
        );
        // SOCKS route clients share the resolved Hyper idle-pool policy
        // (timeout plus effective per-host cap) with every other persistent
        // Hyper family. `HyperClientPolicy::apply` installs the pool timer
        // the timeout requires.
        let policy = HyperClientPolicy::cached_route(
            self.config.retry_canceled_requests,
            self.pool.idle_timeout(),
            self.pool.max_idle_per_host(),
            enabler,
        );
        let client = build_hyper_client(
            connector,
            &policy,
            self.config
                .timeout
                .as_ref()
                .and_then(|timeout| timeout.connect),
            &self.lifecycle,
        );
        clients.insert(key, client.clone());
        Ok(client)
    }

    /// Get or create a Hyper client for one HTTP forward-proxy route.
    ///
    /// The client is scoped to the destination origin rather than sharing a
    /// pool across arbitrary origins. Hyper still owns keep-alive handling,
    /// framing, response bodies, and stale-idle recovery for each entry.
    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
    pub(crate) async fn forward_client(
        &self,
        proxy: &crate::proxy::ProxyConfig,
        origin: &url::Url,
        connect_timeout: Option<Duration>,
    ) -> Result<crate::transport::TimeoutForwardClient> {
        let proxy_tls_timeout = connect_timeout;
        let key = crate::transport::proxy::ForwardRouteKey::new(
            proxy,
            origin,
            connect_timeout,
            proxy_tls_timeout,
        );
        let mut clients = self.forward_clients.lock().await;
        if let Some(client) = clients.get(&key) {
            return Ok(client.clone());
        }

        let connector = crate::transport::proxy::ForwardProxyConnector::new(
            proxy.clone(),
            connect_timeout,
            proxy_tls_timeout,
            self.transport_metrics.clone(),
        );
        let policy = HyperClientPolicy::forward_route(
            self.config.retry_canceled_requests,
            self.pool.idle_timeout(),
            self.pool.max_idle_per_host(),
        );
        let client = build_hyper_client(connector, &policy, connect_timeout, &self.lifecycle);
        clients.insert(key, client.clone());
        Ok(client)
    }

    /// Get or create a Hyper client for one compatible HTTPS CONNECT route.
    /// Multi-address target snapshots remain on the legacy path so their
    /// typed 502/504 fallback semantics are not weakened.
    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
    pub(crate) async fn connect_client(
        &self,
        proxy: &crate::proxy::ProxyConfig,
        origin: &url::Url,
        transport_hints: &crate::transport_hints::TransportHints,
        target: Option<std::net::SocketAddr>,
        connect_timeout: Option<Duration>,
    ) -> Result<crate::transport::TimeoutConnectClient> {
        let key = crate::transport::connect::ConnectRouteKey::new(
            proxy,
            origin,
            target,
            self.config.tls_config.as_ref(),
            transport_hints.sni_hostname.as_deref(),
            self.config.http_version_policy,
            connect_timeout,
            connect_timeout,
        );
        let mut clients = self.connect_clients.lock().await;
        if let Some(client) = clients.get(&key) {
            return Ok(client.clone());
        }
        let connector = crate::transport::connect::ConnectProxyConnector::new(
            origin.clone(),
            proxy.clone(),
            transport_hints,
            target,
            self.config.tls_config.clone(),
            connect_timeout,
            connect_timeout,
            connect_timeout,
            self.config.http_version_policy,
            self.transport_metrics.clone(),
        );
        let policy = HyperClientPolicy::cached_route(
            self.config.retry_canceled_requests,
            self.pool.idle_timeout(),
            self.pool.max_idle_per_host(),
            crate::http_version::HttpVersionPolicyEnabler::from_policy(
                self.config.http_version_policy,
            ),
        );
        let client = build_hyper_client(connector, &policy, connect_timeout, &self.lifecycle);
        clients.insert(key, client.clone());
        Ok(client)
    }
}
