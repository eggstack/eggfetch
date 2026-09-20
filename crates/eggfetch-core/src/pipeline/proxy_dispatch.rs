//! Proxy dispatch: proxy-specific preparation after the common prepared
//! request exists.
//!
//! Owns SOCKS/forward/CONNECT cached client acquisition, the proxy auth
//! conflict check, `ProxyRequestContext` construction, and the legacy
//! multi-target fallback route selection (inside the transport layer). The
//! current request's shrinking total budget stays authoritative at the outer
//! dispatch boundary and is never stored in the reusable route caches.

use std::time::Duration;

use crate::body::RequestBody;
use crate::client::ClientInner;
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::proxy::ProxyConfig;
use crate::response::Response;
use crate::timeout::Timeout;
use crate::transport::proxy::send_proxy_request;
use crate::transport_hints::{ResolvedTarget, TransportHints};

/// Proxy route: assemble proxy context and dispatch.
///
/// `effective_proxy` is the prepared proxy decision; `None` is a caller
/// error because route selection only picks this path when a proxy applies.
#[allow(clippy::too_many_arguments)]
pub(super) async fn send_proxy_route(
    inner: &ClientInner,
    method: &http::Method,
    url: &url::Url,
    headers: Headers,
    body: RequestBody,
    version: http::Version,
    effective_proxy: Option<&ProxyConfig>,
    transport_hints: &TransportHints,
    proxied_target: Option<&ResolvedTarget>,
    hop_timeout: Timeout,
    remaining_total: Option<Duration>,
    deadline: Option<std::time::Instant>,
    failure_context: Option<&crate::error::RequestFailureContext>,
) -> Result<Response> {
    let proxy_config = effective_proxy
        .as_ref()
        .ok_or_else(|| Error::Unsupported("proxy configuration not available".into()))?;
    if !proxy_config.is_socks()
        && headers.contains("proxy-authorization")
        && proxy_config.auth().is_some()
    {
        return Err(Error::ConflictingAuth(
            "conflict: both request Proxy-Authorization header and proxy auth are configured; remove one".into(),
        ));
    }
    let socks_client = {
        let socks_proxy = effective_proxy.as_ref().filter(|proxy| proxy.is_socks());
        match socks_proxy {
            Some(proxy) => Some(inner.socks_client(proxy, proxied_target).await?),
            None => None,
        }
    };
    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
    let forward_client = if url.scheme() == "http" && !proxy_config.is_socks() {
        Some(
            inner
                .forward_client(proxy_config, url, hop_timeout.connect)
                .await?,
        )
    } else {
        None
    };
    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
    let connect_client = if url.scheme() == "https"
        && !proxy_config.is_socks()
        && transport_hints
            .target
            .as_ref()
            .is_none_or(|target| target.starts_with(b"/"))
        && proxied_target
            .as_ref()
            .is_none_or(|target| target.addresses().len() <= 1)
    {
        let target = proxied_target
            .as_ref()
            .and_then(|target| target.addresses().first().copied());
        Some(
            inner
                .connect_client(
                    proxy_config,
                    url,
                    transport_hints,
                    target,
                    hop_timeout.connect,
                )
                .await?,
        )
    } else {
        None
    };
    let proxy_context = crate::transport::proxy::ProxyRequestContext {
        remaining_total,
        deadline,
        connect_timeout: hop_timeout.connect,
        proxy_connect_timeout: hop_timeout.connect,
        proxy_tls_timeout: hop_timeout.connect,
        write_timeout: hop_timeout.write,
        read_timeout: hop_timeout.read,
        http_version_policy: inner.config.http_version_policy,
        origin_tls_config: inner.config.tls_config.as_ref(),
        // Proxy TLS config is independent from origin TLS
        // config. When the proxy endpoint has no explicit TLS
        // configuration we use its own default trust roots.
        proxy_tls_config: proxy_config.proxy_tls_config(),
        proxied_target,
        socks_client,
        #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
        forward_client,
        #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
        connect_client,
        failure_context,
        transport_metrics: Some(inner.transport_metrics.clone()),
    };
    let proxy_future = Box::pin(send_proxy_request(
        url,
        method,
        headers,
        body,
        version,
        proxy_config,
        transport_hints,
        &proxy_context,
    ));
    super::send_with_total_timeout(proxy_future, remaining_total).await
}
