//! HTTP/3 dispatch: Alt-Svc discovery, suppression, fallback, and metrics.
//!
//! Owns Alt-Svc discovery lookup, the explicit-vs-discovered H3 distinction,
//! replay-safe pre-commit fallback, suppression generation/failure
//! classification, and H3-specific metrics. Behind the `http3` feature gate.

use std::time::Duration;

use super::hyper_dispatch::send_hyper_request;
use crate::body::RequestBody;
use crate::client::ClientInner;
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::timeout::Timeout;
use crate::transport::http3::H3AltTarget;

/// Resolve an Alt-Svc-discovered H3 target for `Auto` discovery.
///
/// Returns `None` when no fresh alternative exists, when suppressed, when
/// the URL is not `https`, or when `http3` is disabled. Counts lazy expiry
/// and suppression exactly once per call. Explicit `Http3Only` never calls
/// this (direct route, no discovery).
pub(super) fn h3_discovered_target(
    inner: &ClientInner,
    url: &url::Url,
) -> Option<crate::transport::http3::H3AltTarget> {
    if url.scheme() != "https" {
        return None;
    }
    let origin = crate::transport::alt_svc::AltSvcOrigin::from_url(url)?;
    let now = std::time::Instant::now();
    let (fresh, expired) = inner.alt_svc_state.cache().get_fresh_counted(&origin, now);
    if expired {
        inner.transport_metrics.record_altsvc_expired();
    }
    let entry = fresh?;
    // Suppression is routing-only: skip broken alternatives during backoff.
    // A generation change re-enables without waiting (checked inside).
    if inner
        .alt_svc_state
        .suppressor()
        .is_suppressed(&origin, entry.generation, now)
    {
        inner.transport_metrics.record_h3_suppressed();
        return None;
    }
    Some(crate::transport::http3::H3AltTarget {
        alt_host: entry.alt_host,
        alt_port: entry.alt_port,
        sni_host: origin.host().to_owned(),
        generation: Some(entry.generation),
    })
}

/// Learn Alt-Svc advertisements from a response when trustworthy.
///
/// Trust requires: `https` scheme, TLS actually verified per policy (not
/// `danger_accept_invalid_certs`), no proxy route, learnable transport
/// route (Standard/Direct/H3 only; UDS/SNI/proxy never learn), and hop-local
/// origin (always true here since this is per-hop). Alternative endpoints
/// never affect cookies/auth/`Host`/security; only UDP routing uses them.
pub(super) fn learn_altsvc_from_response(
    inner: &ClientInner,
    url: &url::Url,
    headers: &http::HeaderMap,
    via_proxy: bool,
    route_learnable: bool,
) {
    if url.scheme() != "https" || via_proxy || !route_learnable {
        return;
    }
    // TLS authentication: default (no custom config) is verified via system
    // roots; custom configs must verify both cert and hostname.
    let tls_authenticated = match inner.config.tls_config.as_ref() {
        Some(cfg) => cfg.verify_certificate() && cfg.verify_hostname(),
        None => true,
    };
    // If the client failed to build TLS policy, HTTPS would already have
    // failed; a response here implies no config error, but gate anyway.
    if inner.tls_config_error.is_some() {
        inner.transport_metrics.record_altsvc_rejected();
        return;
    }
    let ctx = crate::transport::alt_svc::AltSvcLearnContext {
        is_https: true,
        tls_authenticated,
        via_proxy: false,
        origin_matches_hop: true,
    };
    if !ctx.trustworthy() {
        inner.transport_metrics.record_altsvc_rejected();
        return;
    }
    let Some(origin) = crate::transport::alt_svc::AltSvcOrigin::from_url(url) else {
        inner.transport_metrics.record_altsvc_rejected();
        return;
    };
    // Collect all `alt-svc` values (multiple lines allowed).
    let values: Vec<String> = headers
        .get_all("alt-svc")
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_owned))
        .collect();
    if values.is_empty() {
        return;
    }
    let now = std::time::Instant::now();
    let (outcome, had_cleared) = inner
        .alt_svc_state
        .cache()
        .learn_counted(&origin, &values, now);
    match outcome {
        crate::transport::alt_svc::AltSvcLearnOutcome::Learned => {
            inner.transport_metrics.record_altsvc_learned();
            // New generation re-enables the route without waiting on stale
            // suppression.
            if let Some(entry) = inner.alt_svc_state.cache().get_fresh(&origin, now) {
                inner
                    .alt_svc_state
                    .suppressor()
                    .note_new_advertisement(&origin, entry.generation);
            }
        }
        crate::transport::alt_svc::AltSvcLearnOutcome::Cleared => {
            if had_cleared {
                inner.transport_metrics.record_altsvc_cleared();
            }
        }
        crate::transport::alt_svc::AltSvcLearnOutcome::Rejected => {
            inner.transport_metrics.record_altsvc_rejected();
        }
        crate::transport::alt_svc::AltSvcLearnOutcome::Absent => {}
    }
}

/// Decide whether the H3 route applies and resolve discovery state.
///
/// Discovery semantics:
/// - `Http3Only` = explicit direct QUIC route, strict, no discovery, no
///   fallback, no suppression.
/// - `Auto { allow_http3: true }` = discovery: H2/H1 unless a fresh
///   Alt-Svc alternative is cached and not suppressed. No fresh entry
///   means never invent an H3 endpoint.
/// - `Auto { allow_http3: false }` (default) never attempts H3.
///
/// H3 never bypasses proxy/UDS/SNI precedence (selected first).
#[allow(
    clippy::fn_params_excessive_bools,
    reason = "H3 discovery gating mirrors the route-selection flag vector so precedence stays directly testable"
)]
pub(super) fn decide_h3_use(
    inner: &ClientInner,
    url: &url::Url,
    has_uds: bool,
    has_proxy: bool,
    has_sni: bool,
    has_custom: bool,
) -> (bool, Option<H3AltTarget>) {
    use crate::HttpVersionPolicy;
    match inner.config.http_version_policy {
        HttpVersionPolicy::Http3Only => (true, None),
        HttpVersionPolicy::Auto { allow_http3: true } => {
            // Only HTTPS origins can be discovered; plaintext never
            // learns and never routes H3. Earlier routes (UDS/proxy/SNI)
            // already win via route selection, so discovery here only
            // decides H3-vs-standard.
            if has_uds || has_proxy || has_sni || has_custom {
                (false, None)
            } else if let Some(target) = h3_discovered_target(inner, url) {
                (true, Some(target))
            } else {
                (false, None)
            }
        }
        _ => (false, None),
    }
}

/// H3 route: attempt QUIC dispatch with suppression and safe fallback.
///
/// Explicit (`Http3Only`, `h3_alt=None`) is strict: no fallback, no
/// suppression. Discovered (`Auto`, `Some`) may suppress and safely fall
/// back pre-commit for replayable bodies.
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::too_many_lines,
    reason = "H3 attempt keeps suppression/fallback accounting in one place so the orchestration match arm stays a single call"
)]
pub(super) async fn send_h3_route(
    inner: &ClientInner,
    method: &http::Method,
    url: &url::Url,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    uri: http::Uri,
    h3_alt: Option<H3AltTarget>,
    hop_timeout: Timeout,
    remaining_total: Option<Duration>,
    transport_hints: &crate::transport_hints::TransportHints,
    failure_context: Option<&crate::error::RequestFailureContext>,
) -> Result<crate::response::Response> {
    let h3_connector = inner.h3_connector.as_ref().ok_or_else(|| {
        Error::Unsupported("HTTP/3 connector not available; ensure http3 feature is enabled".into())
    })?;
    // Explicit (`Http3Only`, `h3_alt=None`) is strict: no
    // fallback, no suppression. Discovered (`Auto`, `Some`) may
    // suppress and safely fall back pre-commit for replayable
    // bodies.
    let is_explicit = h3_alt.is_none();
    // Clone replayable bodies before the H3 move so safe fallback
    // can reuse them without re-consuming a stream. One-shot
    // streams yield `None` and never fall back.
    let fallback_body: Option<RequestBody> = if is_explicit {
        None
    } else {
        match &body {
            RequestBody::Empty => Some(RequestBody::Empty),
            RequestBody::Bytes(b) => Some(RequestBody::Bytes(b.clone())),
            RequestBody::Stream { .. } => None,
        }
    };
    let mut builder = http::Request::builder()
        .method(method)
        .uri(uri)
        .version(version);
    for (name, value) in headers.iter() {
        builder = builder.header(name, value);
    }
    let h3_request = builder
        .body(body)
        .map_err(|e| Error::RequestBuild(e.to_string()))?;
    // `Timeout.connect` bounds H3 DNS + QUIC + h3 establishment
    // inside the connector as one budget shared across address
    // fallback attempts. `remaining_total` stays the outer
    // deadline enforced here and is never restarted per address.
    // Read/write phases are enforced by the body wrappers at the
    // documented boundaries. Boxed so the QUIC handshake state
    // does not bloat the shared dispatch future.
    let h3_alt_clone = h3_alt.clone();
    let connect_budget = hop_timeout.connect;
    let h3_future = Box::pin(h3_connector.send_request_with_alt(
        h3_request,
        url.clone(),
        h3_alt_clone.clone(),
        connect_budget,
    ));
    // Total deadline applies to the H3 attempt without
    // restarting; on expiry report `Total` as a pre-commit
    // dispatch error (safe to consider for fallback when
    // replayable, but classified `Other` so it does not
    // suppress the route).
    let h3_outcome: std::result::Result<
        crate::response::Response,
        crate::transport::http3::H3DispatchError,
    > = match remaining_total {
        Some(dur) => {
            let started = std::time::Instant::now();
            match tokio::time::timeout(dur, h3_future).await {
                Ok(r) => r,
                Err(_) => Err(crate::transport::http3::H3DispatchError::for_total_timeout(
                    started.elapsed(),
                )),
            }
        }
        None => h3_future.await,
    };
    match h3_outcome {
        Ok(resp) => {
            // Successful H3 use clears broken-route suppression
            // (both explicit and discovered; explicit never
            // suppresses but may clear stale discovered state).
            if let Some(origin) = crate::transport::alt_svc::AltSvcOrigin::from_url(url) {
                inner.alt_svc_state.suppressor().record_success(&origin);
            }
            Ok(resp)
        }
        Err(dispatch) => {
            // Broken-route suppression (discovered only, never
            // for draining grace, never for request-scoped
            // `Other` like write/read/total timeouts).
            let suppressible = !is_explicit
                && !dispatch.draining
                && !matches!(
                    dispatch.failure_class,
                    crate::transport::alt_svc::H3FailureClass::Other
                );
            if suppressible {
                if let Some(origin) = crate::transport::alt_svc::AltSvcOrigin::from_url(url) {
                    // Generation scopes suppression: a new
                    // advertisement re-enables without waiting.
                    // Use the attempted generation directly (no
                    // re-lookup that could observe expiry).
                    let generation = h3_alt_clone
                        .as_ref()
                        .and_then(|a| a.generation)
                        .unwrap_or(0);
                    inner.alt_svc_state.suppressor().record_failure(
                        &origin,
                        generation,
                        std::time::Instant::now(),
                        dispatch.failure_class,
                    );
                }
            }
            if is_explicit {
                // `Http3Only` remains strict: never fall back.
                return Err(dispatch.error);
            }
            // Safe H3-to-H2/H1 fallback (Auto only): pre-commit
            // + replayable + route failure (not `Other`). Total,
            // connect, write, read deadlines are monotonic
            // (reused, never restarted). Proxy/TLS policy
            // unchanged (H3 never had a proxy; standard uses the
            // same origin TLS).
            let can_fallback = dispatch.safe_for_fallback()
                && fallback_body.is_some()
                && !matches!(
                    dispatch.failure_class,
                    crate::transport::alt_svc::H3FailureClass::Other
                );
            if can_fallback {
                inner.transport_metrics.record_h3_fallback();
                if let Some(context) = failure_context {
                    context.clear();
                }
                let Some(fb_body) = fallback_body else {
                    return Err(dispatch.error);
                };
                send_hyper_request(
                    inner,
                    method.clone(),
                    url.clone(),
                    headers.clone(),
                    fb_body,
                    version,
                    remaining_total,
                    transport_hints,
                    failure_context,
                )
                .await
            } else {
                // Post-commit, one-shot, or request-scoped:
                // never silently duplicate. Retry policy remains
                // the only later-attempt mechanism.
                Err(dispatch.error)
            }
        }
    }
}
