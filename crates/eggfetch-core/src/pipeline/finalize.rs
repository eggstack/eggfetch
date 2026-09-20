//! Response finalization: the one common post-transport policy applied to
//! every successful route.
//!
//! Owns decompression wrapping, decoded-body size/ratio limits, the
//! read/total-timeout attachment, pool-lease attachment, and common metadata
//! finalization (Alt-Svc learning on learnable routes, 101 upgrade
//! accounting). Transport-specific response metadata must already be present
//! before this step. The total deadline is route-neutral: H1, H2, proxy,
//! direct/advanced routes, and H3 share this one policy.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;

use super::route::TransportRoute;
use crate::body::ResponseBody;
use crate::client::ClientInner;
use crate::error::Result;
use crate::pool::PoolGuard;
use crate::response::Response;
use crate::timeout::ResponseDeadline;

/// Attach read/total timeouts plus the pool permit to the streaming
/// response body.
///
/// The absolute total deadline (when configured) is retained with
/// response-body state; the unified timeout wrapper enforces it at the final
/// stream boundary used by the chosen consumption mode. No Tokio timer is
/// created here, so the body can cross runtime boundaries before first poll.
pub(super) fn apply_timeouts_and_lease(
    response: &mut Response,
    mut guard: PoolGuard,
    read_timeout: Option<Duration>,
    total_deadline: Option<ResponseDeadline>,
) {
    // Timeout metadata lives behind the private lease lifecycle, never in
    // the public `ResponseBody` variant shape. Initialize before the guard
    // is placed behind the lease `Arc`; consumption reads it back when the
    // final raw/decoded stream is selected.
    guard.set_response_timeouts(read_timeout, total_deadline);
    if !guard.has_response_state() {
        drop(guard);
        return;
    }
    let body = std::mem::replace(&mut response.body, ResponseBody::buffered(Bytes::new()));
    let new_body = body.attach_lease(Arc::new(guard));
    response.set_body(new_body);
}

/// Apply the common post-transport policy to every successful route.
///
/// Receives the transported response plus the prepared policy: Alt-Svc
/// learning (learnable routes only), 101 upgrade accounting, decompression
/// wrapping, decoded-size limiting, then read/total-timeout + pool-lease
/// attachment.
#[allow(clippy::too_many_arguments)]
pub(super) fn finalize_response(
    inner: &ClientInner,
    mut response: Response,
    route: TransportRoute,
    url: &url::Url,
    via_proxy: bool,
    decompression_enabled: bool,
    max_decoded_body_size: Option<usize>,
    max_decompression_ratio: Option<f64>,
    guard: PoolGuard,
    read_timeout: Option<Duration>,
    total_deadline: Option<ResponseDeadline>,
) -> Result<Response> {
    // Authenticated Alt-Svc learning: only from learnable routes
    // (Standard/Direct/H3) with `https`, verified TLS, no proxy. UDS, SNI,
    // and proxy routes never install alternatives. Logical origin only;
    // alternative authorities never change cookies/auth/Host/security.
    #[cfg(feature = "http3")]
    {
        let learnable = matches!(
            route,
            TransportRoute::Standard | TransportRoute::Direct | TransportRoute::H3
        );
        if learnable {
            super::h3_dispatch::learn_altsvc_from_response(
                inner,
                url,
                response.headers(),
                via_proxy,
                true,
            );
        }
    }
    #[cfg(not(feature = "http3"))]
    let _ = (route, url, via_proxy);

    // Record successfully captured 101 upgrades as protocol connections.
    // Ordinary pooled responses remain uncounted here; logical requests are
    // counted by `PoolMetrics`.
    if response
        .network_stream()
        .is_some_and(crate::network_stream::NetworkStream::is_upgraded)
    {
        inner.transport_metrics.record_upgraded();
    }

    if decompression_enabled {
        let content_encoding = response
            .headers()
            .get("content-encoding")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        let limit = crate::compression::DecompressionLimit {
            max_decoded_body_size,
            max_decompression_ratio,
        };

        response = crate::response_decode::apply_decompression(
            response,
            content_encoding.as_deref(),
            limit,
        )?;
    }

    if let Some(max) = max_decoded_body_size {
        // EncodedStreaming merges this value into its existing decompression
        // limit; unencoded streaming bodies get the limiting wrapper here.
        // This does not add a second decoded-size stream layer.
        response.body = response.body.limit_decoded_size(max)?;
    }

    apply_timeouts_and_lease(&mut response, guard, read_timeout, total_deadline);

    Ok(response)
}
