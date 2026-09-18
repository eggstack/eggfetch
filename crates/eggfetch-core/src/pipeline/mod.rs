//! Request pipeline: retry, redirect, preparation, dispatch, finalization.
//!
//! Short orchestration entry points. Each phase has an explicit private
//! owner: `retry` (retry loop, backoff, discard drain), `redirect`
//! (redirect state, hop transformation, first-hop assembly), `prepare`
//! (request normalization, pool acquisition), `route` (precedence),
//! `hyper_dispatch` (ordinary H1/H2 routes), `proxy_dispatch` (proxy
//! routes), `h3_dispatch` (experimental H3 discovery/fallback), and
//! `finalize` (common post-transport policy). This module wires those phases
//! together and keeps the narrower native frame-execution semantics explicit.

#[cfg(feature = "high-level-url")]
mod finalize;
#[cfg(feature = "http3")]
mod h3_dispatch;
#[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
mod hyper_dispatch;
#[cfg(all(feature = "high-level-url", not(feature = "redirects")))]
mod lean;
mod prepare;
#[cfg(feature = "proxy")]
mod proxy_dispatch;
#[cfg(all(feature = "high-level-url", feature = "redirects"))]
mod redirect;
#[cfg(all(feature = "high-level-url", feature = "logical-retry"))]
mod retry;
mod route;

// Preserve the pre-decomposition `crate::pipeline::X` paths used outside
// this module tree. `validate_target` is consumed by the proxy transport
// modules, so its re-export follows the same feature gate.
#[cfg(feature = "proxy")]
pub(crate) use prepare::validate_target;
#[cfg(all(feature = "high-level-url", feature = "logical-retry"))]
pub(crate) use retry::send_with_retry;
// Re-exported for `Client::{send, send_detailed}` only when the retry loop
// is absent; the retry loop itself reaches the redirect/lean modules via
// `super::` paths.
#[cfg(all(
    feature = "high-level-url",
    not(feature = "redirects"),
    not(feature = "logical-retry")
))]
pub(crate) use lean::send_lean;
#[cfg(all(
    feature = "high-level-url",
    feature = "redirects",
    not(feature = "logical-retry")
))]
pub(crate) use redirect::send_with_redirects;
// `apply_content_length` is exercised directly by `client.rs` unit tests
// through the pre-decomposition path; preparation itself uses it in-file.
#[cfg(test)]
pub(crate) use prepare::apply_content_length;

use std::time::Duration;

#[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
use bytes::Bytes;

use crate::body::{NativeRequestBody, NativeResponseBody};
use crate::client::ClientInner;
use crate::error::{Error, Result};
#[cfg(feature = "high-level-url")]
use crate::request::Request;
#[cfg(feature = "high-level-url")]
use crate::response::Response;
use crate::timeout::Timeout;
use crate::transport_hints::NativeRequestOptions;

/// Bound a complete transport future only by an explicitly configured native
/// total deadline. Direct Hyper/UDS/H3 transports do not expose a clean
/// response-header boundary here, so their read budget is applied by the
/// response-body stream after transport setup has completed.
///
/// Shared by every dispatch route; the current request's shrinking total
/// budget remains authoritative here and is never stored in the reusable
/// route caches.
async fn send_with_total_timeout<F, T>(
    send_future: F,
    remaining_total: Option<Duration>,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    use crate::timeout::TimeoutPhase;

    match remaining_total {
        Some(duration) => {
            let started = std::time::Instant::now();
            tokio::time::timeout(duration, send_future)
                .await
                .map_err(|_| Error::Timeout {
                    phase: TimeoutPhase::Total,
                    elapsed: started.elapsed(),
                })?
        }
        None => send_future.await,
    }
}

/// Upper bound on how much of a discarded response body is drained so
/// the underlying connection can be returned to the pool. Bodies larger
/// than this are abandoned (the connection closes instead), matching
/// common client practice (e.g. Go's `net/http` 256 KiB drain cap) and
/// guaranteeing retry/redirect processing cannot be stalled indefinitely
/// by a slow-dripping server when no read timeout is configured.
#[cfg(all(
    feature = "high-level-url",
    any(feature = "logical-retry", feature = "redirects")
))]
const DRAIN_MAX_BYTES: usize = 256 * 1024;

/// Upper bound on how long a best-effort drain may run when no explicit
/// total deadline governs it. Without this, a slow-drip server could
/// stall retry/redirect processing indefinitely even though only a
/// bounded number of bytes would ever be drained.
#[cfg(all(
    feature = "high-level-url",
    any(feature = "logical-retry", feature = "redirects")
))]
const DRAIN_MAX_TIME: Duration = Duration::from_secs(30);

/// Best-effort drain of a discarded response body.
///
/// Uses the raw (encoded) byte stream so a zip-bomb response does not
/// abort the drain via `DecompressionRatioExceeded` and abandon the
/// connection. Drain errors are ignored; draining stops after
/// [`DRAIN_MAX_BYTES`] or [`DRAIN_MAX_TIME`].
///
/// Shared by the retry loop and the redirect loop; both discard a response
/// body before the next attempt/hop.
#[cfg(all(
    feature = "high-level-url",
    any(feature = "logical-retry", feature = "redirects")
))]
pub(super) async fn drain_response_body(response: &mut Response) {
    let Ok(stream) = response.raw_bytes_stream() else {
        return; // body already consumed; nothing to drain
    };
    let mut remaining = DRAIN_MAX_BYTES;
    let mut stream = std::pin::pin!(stream);
    let drain = async {
        while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
            match chunk {
                Ok(bytes) => {
                    if bytes.len() >= remaining {
                        break;
                    }
                    remaining -= bytes.len();
                }
                Err(_) => break,
            }
        }
    };
    let _ = tokio::time::timeout(DRAIN_MAX_TIME, drain).await;
}

/// Send a single HTTP request and return the streaming response.
///
/// This handles pool acquisition, timeout application, and body
/// processing for one request/response cycle. It does NOT handle
/// redirects—that is the responsibility of the redirect loop.
///
/// The implementation separates a preparation phase
/// (`prepare::prepare_single_request`) from declarative transport selection
/// (`route::select_route`); one common post-transport policy
/// (`finalize::finalize_response`: decompression, decoded-size limiting,
/// read-timeout and pool-lease attachment) applies to every route.
#[allow(clippy::too_many_lines)]
#[cfg(all(
    any(feature = "transport-http1", feature = "transport-http2"),
    feature = "high-level-url"
))]
pub(crate) async fn send_single_request(
    inner: &ClientInner,
    request: Request,
    timeout: &Timeout,
) -> Result<Response> {
    let (prepared, guard) = prepare::prepare_single_request(inner, request, timeout).await?;
    let prepare::PreparedRequest {
        method,
        url,
        uri,
        headers,
        body,
        version,
        transport_hints,
        proxied_target,
        #[cfg(feature = "proxy")]
        effective_proxy,
        decompression_enabled,
        max_decoded_body_size,
        max_decompression_ratio,
        timeout: hop_timeout,
        remaining_total,
        deadline,
        failure_context,
    } = prepared;

    // `deadline` feeds the proxy multi-phase context; without the `proxy`
    // feature no route consumes it.
    #[cfg(not(feature = "proxy"))]
    let _ = &deadline;
    #[cfg(not(feature = "proxy"))]
    let _ = &proxied_target;

    #[cfg(all(unix, feature = "advanced-routing"))]
    let has_uds = inner.uds_client.is_some();
    #[cfg(all(unix, not(feature = "advanced-routing")))]
    let has_uds = false;
    #[cfg(not(unix))]
    let has_uds = false;
    #[cfg(feature = "proxy")]
    let has_proxy = effective_proxy.is_some();
    #[cfg(not(feature = "proxy"))]
    let has_proxy = false;
    // Advanced-route availability: in lean standard-route profiles the
    // corresponding client state does not exist; advanced hints fail closed
    // below instead of selecting an absent route.
    #[cfg(all(feature = "proxy", feature = "advanced-routing"))]
    let has_direct_no_proxy = !has_proxy
        && (transport_hints.resolved_target.is_some() || inner.direct_client.is_some())
        && !has_uds;
    #[cfg(all(feature = "proxy", not(feature = "advanced-routing")))]
    let has_direct_no_proxy = false;
    #[cfg(all(not(feature = "proxy"), feature = "advanced-routing"))]
    let has_direct_no_proxy =
        !has_uds && (transport_hints.resolved_target.is_some() || inner.direct_client.is_some());
    #[cfg(all(not(feature = "proxy"), not(feature = "advanced-routing")))]
    let has_direct_no_proxy = false;
    #[cfg(feature = "advanced-routing")]
    let has_sni = transport_hints.sni_hostname.is_some();
    #[cfg(not(feature = "advanced-routing"))]
    let has_sni = false;
    #[cfg(feature = "advanced-routing")]
    let has_custom = inner.config.dialer.is_some();
    #[cfg(not(feature = "advanced-routing"))]
    let has_custom = false;
    #[cfg(not(feature = "advanced-routing"))]
    {
        if transport_hints.resolved_target.is_some() {
            return Err(Error::Unsupported(
                "caller-supplied resolved destinations require the advanced-routing feature".into(),
            ));
        }
        if transport_hints.sni_hostname.is_some() {
            return Err(Error::Unsupported(
                "SNI override requires the advanced-routing feature".into(),
            ));
        }
    }
    #[cfg(all(feature = "advanced-routing", feature = "proxy"))]
    if has_custom && has_proxy {
        return Err(Error::Unsupported(
            "custom dialing is incompatible with built-in proxy routing".into(),
        ));
    }
    #[cfg(feature = "http3")]
    let (use_h3, h3_alt) =
        h3_dispatch::decide_h3_use(inner, &url, has_uds, has_proxy, has_sni, has_custom);
    #[cfg(not(feature = "http3"))]
    let use_h3 = false;
    let route = route::select_route(
        has_uds,
        has_custom,
        has_direct_no_proxy,
        has_proxy,
        has_sni,
        use_h3,
    );
    #[cfg(feature = "http3")]
    if transport_hints.resolved_target.is_some() && use_h3 {
        return Err(Error::Unsupported(
            "caller-supplied resolved destinations are incompatible with HTTP/3".into(),
        ));
    }
    #[cfg(feature = "http3")]
    if use_h3 {
        inner.transport_metrics.record_h3_attempted();
    }

    // Declarative transport dispatch. Precedence is encoded in
    // `route::select_route` and covered by direct unit tests; H3 never bypasses
    // proxy rules because proxy routes are selected first.
    let response = match route {
        #[cfg(feature = "advanced-routing")]
        route::TransportRoute::Uds => {
            hyper_dispatch::send_uds_route(
                inner,
                &method,
                uri,
                &headers,
                body,
                version,
                url.clone(),
                &transport_hints,
                remaining_total,
            )
            .await?
        }
        #[cfg(feature = "advanced-routing")]
        route::TransportRoute::Custom => {
            hyper_dispatch::send_custom_route(
                inner,
                &method,
                uri,
                &headers,
                body,
                version,
                url.clone(),
                &transport_hints,
                failure_context.as_deref(),
                remaining_total,
            )
            .await?
        }
        #[cfg(feature = "advanced-routing")]
        route::TransportRoute::Direct => {
            hyper_dispatch::send_direct_route(
                inner,
                &method,
                uri,
                &headers,
                body,
                version,
                url.clone(),
                &transport_hints,
                failure_context.as_deref(),
                remaining_total,
            )
            .await?
        }
        route::TransportRoute::Proxy => {
            #[cfg(feature = "proxy")]
            {
                proxy_dispatch::send_proxy_route(
                    inner,
                    &method,
                    &url,
                    &headers,
                    body,
                    version,
                    effective_proxy.as_ref(),
                    &transport_hints,
                    proxied_target.as_ref(),
                    hop_timeout,
                    remaining_total,
                    deadline,
                    failure_context.as_deref(),
                )
                .await?
            }
            #[cfg(not(feature = "proxy"))]
            {
                let _ = (method, uri, headers, body, version);
                return Err(Error::Unsupported("proxy support is not enabled".into()));
            }
        }
        #[cfg(feature = "advanced-routing")]
        route::TransportRoute::SniDirect => {
            hyper_dispatch::send_sni_route(
                inner,
                &method,
                uri,
                &headers,
                body,
                version,
                url.clone(),
                &transport_hints,
                failure_context.as_deref(),
                remaining_total,
            )
            .await?
        }
        route::TransportRoute::H3 => {
            #[cfg(feature = "http3")]
            {
                h3_dispatch::send_h3_route(
                    inner,
                    &method,
                    &url,
                    &headers,
                    body,
                    version,
                    uri,
                    h3_alt,
                    hop_timeout,
                    remaining_total,
                    &transport_hints,
                    failure_context.as_deref(),
                )
                .await?
            }
            #[cfg(not(feature = "http3"))]
            {
                let _ = (method, uri, headers, body, version);
                return Err(Error::Unsupported("HTTP/3 support is not enabled".into()));
            }
        }
        route::TransportRoute::Standard => {
            hyper_dispatch::send_hyper_request(
                inner,
                &method,
                url.clone(),
                &headers,
                body,
                version,
                remaining_total,
                &transport_hints,
                failure_context.as_deref(),
            )
            .await?
        }
    };

    #[cfg(feature = "proxy")]
    let via_proxy = effective_proxy.is_some();
    #[cfg(not(feature = "proxy"))]
    let via_proxy = false;

    finalize::finalize_response(
        inner,
        response,
        route,
        &url,
        via_proxy,
        decompression_enabled,
        max_decoded_body_size,
        max_decompression_ratio,
        guard,
        hop_timeout.read,
    )
}

/// Execute a caller-owned `http_body::Body` through eggfetch's transport
/// engine without applying high-level request policy.
#[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
#[allow(
    clippy::too_many_lines,
    reason = "native dispatch keeps route validation, pool admission, and the shared route matrix together"
)]
pub(crate) async fn send_native_http_body<B>(
    inner: &ClientInner,
    request: http::Request<B>,
    options: NativeRequestOptions,
) -> Result<http::Response<NativeResponseBody>>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: std::error::Error + Send + Sync + 'static,
{
    use http_body_util::BodyExt;

    use crate::headers::Headers;
    use crate::http_origin::HttpOrigin;
    use crate::pool::OriginKey;
    use crate::timeout::TimeoutPhase;

    // Native transport facts come directly from `http::Uri`; never reparse
    // through `url::Url`.
    let http_origin = HttpOrigin::from_uri(request.uri())?;
    #[cfg(feature = "proxy")]
    let explicit_port = request.uri().port_u16();

    let NativeRequestOptions {
        timeout: request_timeout,
        transport_hints,
    } = options;
    let timeout = match inner.config.timeout {
        Some(client_timeout) => client_timeout.merge(request_timeout),
        None => request_timeout.unwrap_or_default(),
    };

    if let Some(target) = &transport_hints.resolved_target {
        let expected_port = http_origin.port_ref();
        if target
            .addresses()
            .iter()
            .any(|address| address.port() != expected_port)
        {
            return Err(Error::InvalidResolvedTarget(format!(
                "all resolved destinations must use the URL's effective port {expected_port}"
            )));
        }
    }

    #[cfg(feature = "proxy")]
    if prepare::native_would_use_proxy(inner, &http_origin, explicit_port) {
        return Err(Error::Unsupported(
            "native frame bodies are not supported through the built-in proxy routes".into(),
        ));
    }

    #[cfg(feature = "advanced-routing")]
    if inner.config.dialer.is_some() {
        if transport_hints.resolved_target.is_some() {
            return Err(Error::Unsupported(
                "custom dialing is incompatible with caller-supplied resolved destinations".into(),
            ));
        }
        if inner.direct_connector_config.is_some() {
            return Err(Error::Unsupported(
                "custom dialing is incompatible with local-address or socket-option routing".into(),
            ));
        }
        if inner.config.uds_configured {
            return Err(Error::Unsupported(
                "custom dialing is incompatible with Unix-domain routing".into(),
            ));
        }
        #[cfg(feature = "http3")]
        if matches!(
            inner.config.http_version_policy,
            crate::HttpVersionPolicy::Http3Only
        ) {
            return Err(Error::Unsupported(
                "custom dialing is incompatible with HTTP/3".into(),
            ));
        }
    }
    #[cfg(not(feature = "advanced-routing"))]
    {
        if transport_hints.resolved_target.is_some() {
            return Err(Error::Unsupported(
                "caller-supplied resolved destinations require the advanced-routing feature".into(),
            ));
        }
        if transport_hints.sni_hostname.is_some() {
            return Err(Error::Unsupported(
                "SNI override requires the advanced-routing feature".into(),
            ));
        }
    }

    #[cfg(feature = "http3")]
    if matches!(
        inner.config.http_version_policy,
        crate::HttpVersionPolicy::Http3Only
    ) {
        return Err(Error::Unsupported(
            "native frame bodies are not supported by the experimental HTTP/3 route".into(),
        ));
    }

    #[cfg(feature = "tls-rustls")]
    if http_origin.is_https() {
        if let Some(error) = &inner.tls_config_error {
            return Err(Error::Tls(error.clone()));
        }
    }

    let uri = prepare::resolve_native_request_uri(request.uri(), &transport_hints)?;
    let method = request.method().clone();
    let version = request.version();
    let headers = Headers::from(request.headers().clone());
    let body = BodyExt::map_err(request.into_body(), |error| {
        Box::new(error) as Box<dyn std::error::Error + Send + Sync>
    })
    .boxed_unsync();
    let body = NativeRequestBody::new(Box::pin(body), timeout.write).boxed_unsync();

    let origin_key = OriginKey::from_origin(&http_origin);
    let started = std::time::Instant::now();
    let pool_deadline = match (timeout.pool, timeout.total) {
        (Some(pool), Some(total)) if total < pool => Some((total, TimeoutPhase::Total)),
        (Some(pool), _) => Some((pool, TimeoutPhase::Pool)),
        (None, Some(total)) => Some((total, TimeoutPhase::Total)),
        (None, None) => None,
    };
    let guard = match pool_deadline {
        Some((duration, phase)) => {
            match tokio::time::timeout(duration, inner.pool.acquire(Some(&origin_key))).await {
                Ok(guard) => guard?,
                Err(_) => {
                    return Err(Error::Timeout {
                        phase,
                        elapsed: started.elapsed(),
                    })
                }
            }
        }
        None => inner.pool.acquire(Some(&origin_key)).await?,
    };

    let remaining_total = timeout
        .total
        .map(|total| total.saturating_sub(started.elapsed()));
    let trace = transport_hints.trace.as_deref();
    #[cfg(all(unix, feature = "advanced-routing"))]
    let has_uds = inner.uds_client.is_some();
    #[cfg(any(not(unix), not(feature = "advanced-routing")))]
    let has_uds = false;
    #[cfg(feature = "advanced-routing")]
    let (has_dialer, has_direct) = (
        inner.config.dialer.is_some(),
        transport_hints.resolved_target.is_some() || inner.direct_client.is_some(),
    );
    #[cfg(not(feature = "advanced-routing"))]
    let (has_dialer, has_direct) = (false, false);
    #[cfg(feature = "advanced-routing")]
    let has_sni = transport_hints.sni_hostname.is_some();
    #[cfg(not(feature = "advanced-routing"))]
    let has_sni = false;
    let route = route::select_route(has_uds, has_dialer, has_direct, false, has_sni, false);

    let raw_response = match route {
        #[cfg(feature = "advanced-routing")]
        route::TransportRoute::Uds => {
            #[cfg(unix)]
            {
                let uds_client = inner
                    .uds_client
                    .as_ref()
                    .ok_or_else(|| Error::Unsupported("UDS client not available".into()))?;
                let hyper_request =
                    hyper_dispatch::build_http_request(&method, uri, version, &headers, body)?;
                send_with_total_timeout(
                    crate::transport::direct::send_raw_request(uds_client, hyper_request, trace),
                    remaining_total,
                )
                .await?
            }
            #[cfg(not(unix))]
            {
                let _ = (method, uri, headers, body, version);
                return Err(Error::Unsupported(
                    "Unix domain sockets are not supported on this platform".into(),
                ));
            }
        }
        #[cfg(feature = "advanced-routing")]
        route::TransportRoute::Custom => {
            let custom_client = if let Some(sni_hostname) = transport_hints.sni_hostname.as_deref()
            {
                inner.sni_custom_client(sni_hostname).await?
            } else {
                inner
                    .custom_client
                    .as_ref()
                    .ok_or_else(|| Error::Unsupported("custom dialer client not available".into()))?
                    .clone()
            };
            let hyper_request =
                hyper_dispatch::build_http_request(&method, uri, version, &headers, body)?;
            send_with_total_timeout(
                crate::transport::direct::send_raw_request(&custom_client, hyper_request, trace),
                remaining_total,
            )
            .await?
        }
        #[cfg(feature = "advanced-routing")]
        route::TransportRoute::Direct => {
            let resolved_client;
            let direct_client = if let Some(target) = transport_hints.resolved_target.as_ref() {
                resolved_client = inner
                    .resolved_client(
                        &http_origin,
                        target,
                        transport_hints.sni_hostname.as_deref(),
                    )
                    .await?;
                &resolved_client
            } else {
                inner
                    .direct_client
                    .as_ref()
                    .ok_or_else(|| Error::Unsupported("direct client not available".into()))?
            };
            let hyper_request =
                hyper_dispatch::build_http_request(&method, uri, version, &headers, body)?;
            send_with_total_timeout(
                crate::transport::direct::send_raw_request(direct_client, hyper_request, trace),
                remaining_total,
            )
            .await?
        }
        #[cfg(feature = "advanced-routing")]
        route::TransportRoute::SniDirect => {
            let sni_hostname = transport_hints.sni_hostname.as_deref().ok_or_else(|| {
                Error::RequestBuild("SNI route selected without sni_hostname".into())
            })?;
            let sni_client = inner.sni_client(sni_hostname).await?;
            let hyper_request =
                hyper_dispatch::build_http_request(&method, uri, version, &headers, body)?;
            send_with_total_timeout(
                crate::transport::direct::send_raw_request(&sni_client, hyper_request, trace),
                remaining_total,
            )
            .await?
        }
        route::TransportRoute::Standard => {
            let hyper_client = inner.hyper_client.as_ref().ok_or_else(|| {
                Error::Unsupported("HTTP client not available for this protocol".into())
            })?;
            let hyper_request =
                hyper_dispatch::build_http_request(&method, uri, version, &headers, body)?;
            send_with_total_timeout(
                crate::transport::direct::send_raw_request(hyper_client, hyper_request, trace),
                remaining_total,
            )
            .await?
        }
        route::TransportRoute::Proxy | route::TransportRoute::H3 => {
            return Err(Error::Unsupported(
                "native frame body route is not available".into(),
            ));
        }
    };

    if raw_response.status() == http::StatusCode::SWITCHING_PROTOCOLS {
        return Err(Error::Unsupported(
            "native frame bodies do not expose protocol upgrades; use the high-level upgrade API"
                .into(),
        ));
    }

    let (parts, incoming) = raw_response.into_parts();
    let native_body =
        NativeResponseBody::from_incoming(incoming, std::sync::Arc::new(guard), timeout.read);
    Ok(http::Response::from_parts(parts, native_body))
}

/// Report that no HTTP protocol feature was selected for native bodies.
#[cfg(not(any(feature = "transport-http1", feature = "transport-http2")))]
pub(crate) async fn send_native_http_body<B>(
    _inner: &ClientInner,
    _request: http::Request<B>,
    _options: NativeRequestOptions,
) -> Result<http::Response<NativeResponseBody>> {
    Err(Error::Unsupported(
        "no HTTP protocol feature is enabled; enable http1 or http2".into(),
    ))
}

/// Report that no HTTP protocol feature was selected.
#[cfg(all(
    not(any(feature = "transport-http1", feature = "transport-http2")),
    feature = "high-level-url"
))]
pub(crate) async fn send_single_request(
    _inner: &ClientInner,
    _request: Request,
    _timeout: &Timeout,
) -> Result<Response> {
    Err(Error::Unsupported(
        "no HTTP protocol feature is enabled; enable http1 or http2".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn total_timeout_wraps_transport_future() {
        // No budget: the inner future passes through unchanged.
        let ok = send_with_total_timeout(async { Ok::<u32, Error>(7) }, None)
            .await
            .expect("passthrough");
        assert_eq!(ok, 7);

        // Expired budget: the outer Total deadline wins without running long.
        let err = send_with_total_timeout(
            async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                Ok::<u32, Error>(7)
            },
            Some(Duration::from_millis(20)),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err, Error::Timeout { .. }),
            "expected total timeout, got {err:?}"
        );
    }
}
