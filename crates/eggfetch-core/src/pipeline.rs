//! Request pipeline: redirect loop, retry wrapper, defaults, cookie/auth
//! application, deadline propagation, and body-lease lifecycle.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;

use crate::body::{RequestBody, ResponseBody};
use crate::client::{Client, ClientInner};
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::pool::{OriginKey, PoolGuard};
#[cfg(feature = "proxy")]
use crate::proxy::{Proxy, ProxyConfig};
use crate::redirect;
#[cfg(feature = "proxy")]
use crate::request::ProxyOverride;
use crate::request::Request;
use crate::response::{HistoryEntry, Response};
use crate::retry::{should_retry, RetryCause, RetryPolicy};
use crate::stream::{read_timeout_stream, write_timeout_stream};
use crate::timeout::{Timeout, TimeoutPhase};
#[cfg(feature = "proxy")]
use crate::transport::proxy::send_proxy_request;

/// Sleep for the backoff delay if the total budget allows it.
///
/// Returns `Err(RetryBudgetExhausted)` if sleeping would exceed the
/// remaining total budget.
async fn sleep_if_budget_allows(
    policy: &RetryPolicy,
    delay: Duration,
    attempt: usize,
    start_time: std::time::Instant,
) -> Result<()> {
    if let Some(max_elapsed) = policy.max_elapsed() {
        let elapsed = start_time.elapsed();
        if elapsed + delay > max_elapsed {
            return Err(Error::RetryBudgetExhausted { attempts: attempt });
        }
    }
    tokio::time::sleep(delay).await;
    Ok(())
}

/// Bound a complete transport future only by an explicitly configured native
/// total deadline. Direct Hyper/UDS/H3 transports do not expose a clean
/// response-header boundary here, so their read budget is applied by the
/// response-body stream after transport setup has completed.
async fn send_with_total_timeout<F>(
    send_future: F,
    remaining_total: Option<Duration>,
) -> Result<Response>
where
    F: std::future::Future<Output = Result<Response>>,
{
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
const DRAIN_MAX_BYTES: usize = 256 * 1024;

/// Upper bound on how long a best-effort drain may run when no explicit
/// total deadline governs it. Without this, a slow-drip server could
/// stall retry/redirect processing indefinitely even though only a
/// bounded number of bytes would ever be drained.
const DRAIN_MAX_TIME: Duration = Duration::from_secs(30);

/// Best-effort drain of a discarded response body.
///
/// Uses the raw (encoded) byte stream so a zip-bomb response does not
/// abort the drain via `DecompressionRatioExceeded` and abandon the
/// connection. Drain errors are ignored; draining stops after
/// [`DRAIN_MAX_BYTES`] or [`DRAIN_MAX_TIME`].
async fn drain_response_body(response: &mut Response) {
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

/// Check if there is budget remaining for another retry attempt.
fn has_budget(policy: &RetryPolicy, attempt: usize, start_time: std::time::Instant) -> bool {
    if attempt >= policy.max_attempts() {
        return false;
    }
    if let Some(max_elapsed) = policy.max_elapsed() {
        if start_time.elapsed() >= max_elapsed {
            return false;
        }
    }
    true
}

/// Send a request with optional retry policy.
///
/// This is the top-level entry point called by [`Client::send`]. It
/// resolves the effective retry policy from the request override and
/// client default, then wraps [`send_with_redirects`] in a retry loop
/// with exponential backoff.
///
/// Retries restart the complete logical request (including redirects)
/// under the original total deadline. Stream bodies are never retried.
#[allow(clippy::too_many_lines)]
pub(crate) async fn send_with_retry(client: &Client, request: Request) -> Result<Response> {
    let body_replayable = request.body().is_replayable();

    // Resolve the effective retry policy. Request-level takes precedence.
    let effective_policy = request
        .retry()
        .cloned()
        .or_else(|| client.config().retry.clone());
    let policy = match effective_policy {
        Some(p) if p.is_enabled() => p,
        _ => {
            return Box::pin(send_with_redirects(client, request)).await;
        }
    };

    // If the body is not replayable, we can only attempt once.
    if !body_replayable {
        return Box::pin(send_with_redirects(client, request)).await;
    }

    // Save the complete logical-request state for replay. The typed
    // `retry_request` transformation below reconstructs each attempt from
    // this saved state, so every field (including future additions) is
    // preserved or explicitly transformed rather than copied through a
    // long parameter list.
    let saved = request.into_parts();
    let saved_timeout = saved.timeout;
    let start_time = std::time::Instant::now();
    let mut attempt = 0usize;

    loop {
        attempt += 1;

        // Check max attempts budget first so the `attempts` field in the
        // error reports the same count regardless of which constraint
        // (max_attempts or max_elapsed) ends the loop. Without this order,
        // a tight `max_elapsed` can return `attempts: 0` for the same
        // logical situation that `max_attempts` would report as
        // `attempts: attempt - 1`.
        if attempt > policy.max_attempts() {
            return Err(Error::RetryBudgetExhausted {
                attempts: attempt - 1,
            });
        }

        // Check total budget before attempting.
        // Snapshot elapsed once so the total-deadline remainder below cannot
        // go to zero between two samples and waste a connection on an
        // attempt that fails immediately on the total deadline.
        let elapsed = start_time.elapsed();
        if let Some(max_elapsed) = policy.max_elapsed() {
            if elapsed >= max_elapsed {
                return Err(Error::RetryBudgetExhausted {
                    attempts: attempt - 1,
                });
            }
        }

        // Enforce the original total deadline across attempts: each attempt
        // receives only the remaining wall-clock budget, mirroring how the
        // redirect loop shrinks per-hop deadlines. Without this, an attempt
        // would restart with the full original `total` and the aggregate
        // elapsed time could reach `max_attempts * total`.
        if let Some(total) = saved_timeout.as_ref().and_then(|t| t.total) {
            if elapsed >= total {
                return Err(Error::Timeout {
                    phase: TimeoutPhase::Total,
                    elapsed,
                });
            }
        }
        let attempt_timeout =
            saved_timeout.map(|t| crate::request::RequestParts::shrink_total_deadline(&t, elapsed));
        // Skip an attempt with no remaining total budget instead of spending
        // a pool slot and TCP connect only to fail on the deadline.
        if attempt_timeout
            .as_ref()
            .and_then(|t| t.total)
            .is_some_and(|remaining| remaining.is_zero())
        {
            return Err(Error::Timeout {
                phase: TimeoutPhase::Total,
                elapsed,
            });
        }

        // Reconstruct the attempt through the single typed transformation.
        let attempt_request = saved.retry_request(attempt_timeout)?;

        let result = Box::pin(send_with_redirects(client, attempt_request)).await;

        match result {
            Ok(response) => {
                if response.status().is_success() {
                    return Ok(response);
                }
                let status = response.status().as_u16();

                if let Some(cause) =
                    should_retry(&policy, &saved.method, &saved.body, None, Some(status))
                {
                    if !has_budget(&policy, attempt, start_time) {
                        return Ok(response);
                    }

                    let mut resp = response;
                    // Capture the header before the body drain consumes it.
                    let retry_after = resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned);
                    // Drain errors are intentionally ignored: the body is
                    // being discarded and the request retried regardless.
                    drain_response_body(&mut resp).await;

                    if let Some(dur) =
                        compute_retry_delay(&policy, &cause, attempt, retry_after.as_deref())
                    {
                        sleep_if_budget_allows(&policy, dur, attempt, start_time).await?;
                    }
                    continue;
                }
                return Ok(response);
            }
            Err(err) => {
                if let Some(cause) =
                    should_retry(&policy, &saved.method, &saved.body, Some(&err), None)
                {
                    if !has_budget(&policy, attempt, start_time) {
                        return Err(err);
                    }

                    // No response is available on the error path; only the
                    // configured backoff applies.
                    if let Some(dur) = compute_retry_delay(&policy, &cause, attempt, None) {
                        sleep_if_budget_allows(&policy, dur, attempt, start_time).await?;
                    }
                    continue;
                }
                return Err(err);
            }
        }
    }
}

/// Compute the backoff delay for a retry attempt.
///
/// When the failed attempt carried a `Retry-After` response header and
/// the policy respects it, the server-directed delay takes precedence
/// over the configured exponential backoff.
fn compute_retry_delay(
    policy: &RetryPolicy,
    cause: &RetryCause,
    attempt: usize,
    retry_after: Option<&str>,
) -> Option<Duration> {
    if let Some(value) = retry_after {
        if let Some(delay) = policy.retry_after_delay(value) {
            return Some(delay);
        }
    }
    let base = policy.backoff_delay(attempt)?;
    // 429 (Too Many Requests) without a server-directed Retry-After should
    // back off longer than other retryable statuses per conventional policy.
    if matches!(cause, RetryCause::Status(429)) {
        let max = policy.backoff().max_delay().as_secs_f64();
        let increased = (base.as_secs_f64() * 1.5).min(max);
        return Some(Duration::from_secs_f64(increased));
    }
    Some(base)
}

/// Parameters for building one redirect-loop hop request.
///
/// This is the single policy path shared by the redirects-disabled fast
/// path and every iteration of the redirect-enabled loop. Cookie injection,
/// auth resolution, transport-hint handling, and credential scoping live
/// here so the two entry paths cannot diverge.
#[allow(
    clippy::struct_excessive_bools,
    reason = "hop policy is three orthogonal booleans (first-hop, credentials, cookies); an enum would obscure call sites"
)]
struct HopBuildParams {
    method: http::Method,
    url: url::Url,
    headers: Headers,
    body: RequestBody,
    version: http::Version,
    timeout: Timeout,
    decompress: Option<bool>,
    #[cfg(feature = "proxy")]
    proxy_override: ProxyOverride,
    transport_hints: crate::request::TransportHints,
    auth: Option<crate::auth::AuthScheme>,
    auth_disabled: bool,
    /// True only for the first hop of the logical request.
    is_first_hop: bool,
    /// False after a cross-origin redirect; gates client-auth reapplication.
    credentials_allowed: bool,
    /// False after a cross-origin redirect; gates cookie injection.
    cookie_allowed: bool,
}

/// Build one hop request through the shared first-hop policy path.
///
/// - Transport hints (`target`, `sni_hostname`, `trace`) attach only on the
///   first hop; redirect hops clear them because the destination changed.
/// - Per-request decompression and proxy overrides persist across all hops.
/// - Auth follows the cross-origin stripping policy; cookies are injected
///   from the jar unless an explicit `cookie` header exists or the hop no
///   longer allows cookies.
/// - The effective auth is resolved and applied here so both the fast path
///   and the redirect loop observe identical header state on the wire.
///
/// # Errors
///
/// Returns an error if cookie insertion or auth application fails.
fn build_hop_request(client: &Client, params: HopBuildParams) -> Result<Request> {
    let HopBuildParams {
        method,
        url,
        headers,
        body,
        version,
        timeout,
        decompress,
        #[cfg(feature = "proxy")]
        proxy_override,
        transport_hints,
        auth,
        auth_disabled,
        is_first_hop,
        credentials_allowed,
        cookie_allowed,
    } = params;

    let mut hop = Request::new(method, url);
    *hop.headers_mut() = headers;
    hop.set_body(body);
    hop.set_version(version);
    hop.set_timeout(Some(timeout));
    hop.set_decompress(decompress);
    #[cfg(feature = "proxy")]
    hop.set_proxy_override(proxy_override);
    // Transport hints apply only on the first hop; redirects clear them
    // because the destination changed.
    if is_first_hop {
        hop.set_transport_hints(transport_hints);
    }

    hop.set_auth(if credentials_allowed { auth } else { None });
    hop.set_auth_disabled(auth_disabled);

    #[cfg(feature = "cookies")]
    {
        if !cookie_allowed {
            hop.headers_mut().remove("cookie");
        } else if !hop.headers().contains("cookie") {
            if let Some(cookie_header) = client.config().cookie_jar.cookies_for_url(hop.url()) {
                hop.headers_mut().insert("cookie", &cookie_header)?;
            }
        }
    }
    #[cfg(not(feature = "cookies"))]
    let _ = (client, cookie_allowed);

    {
        let effective_auth = crate::auth::resolve_request_auth(
            hop.auth(),
            hop.is_auth_disabled(),
            if credentials_allowed {
                client.config().auth.as_ref()
            } else {
                None
            },
            hop.headers(),
        )?;
        if let Some(auth) = effective_auth {
            auth.apply(hop.headers_mut())?;
        }
    }

    Ok(hop)
}

/// Result of advancing one redirect hop: the next hop's method, URL,
/// headers, body, and version.
#[derive(Debug)]
struct RedirectHop {
    method: http::Method,
    url: url::Url,
    headers: Headers,
    body: RequestBody,
    version: http::Version,
}

/// Compute the next redirect hop's request state in one transformation step.
///
/// Evaluates the method/body policy, enforces one-shot body replayability
/// before any unsafe replay, delegates header stripping to the redirect
/// engine, and returns only the fields that legitimately change across a
/// hop (method/URL/headers/body/version). Destination-specific transport
/// hints, auth, proxy/decompression overrides, and timeouts are managed by
/// the redirect loop itself, not carried through the redirect builder.
///
/// # Errors
///
/// Returns [`Error::BodyNotReplayableForRedirect`] when a method-preserving
/// redirect requires replaying a one-shot stream body, or propagates URL
/// and header errors from the redirect engine.
fn advance_redirect_hop(
    cur_method: &http::Method,
    cur_url: &url::Url,
    cur_headers: &Headers,
    cur_version: http::Version,
    replay_body: &mut Option<Bytes>,
    status: http::StatusCode,
    location: &str,
) -> Result<RedirectHop> {
    let new_method = redirect::redirect_method(status, cur_method);
    let drop_body = redirect::drops_body_on_redirect(status, cur_method);
    if drop_body {
        // The body is dropped on this hop. Clear any stale replay payload
        // so a later method-preserving hop does not resurrect bytes from an
        // earlier method-rewritten hop.
        *replay_body = Some(Bytes::new());
    } else if replay_body.is_none() {
        return Err(Error::BodyNotReplayableForRedirect);
    }

    let mut temp_request = Request::new(cur_method.clone(), cur_url.clone());
    *temp_request.headers_mut() = cur_headers.clone();
    let temp_body = if drop_body {
        RequestBody::Empty
    } else {
        RequestBody::Bytes(
            replay_body
                .as_ref()
                .ok_or(Error::BodyNotReplayableForRedirect)?
                .clone(),
        )
    };
    temp_request.set_body(temp_body);
    temp_request.set_version(cur_version);

    let redirect_req = redirect::build_redirect_request_with_policy(
        &temp_request,
        location,
        new_method.clone(),
        drop_body,
    )?;

    // Destructure exhaustively so a new `RequestParts` field fails to
    // compile here rather than being silently dropped across hops.
    let crate::request::RequestParts {
        method: _,
        url: new_url,
        headers: new_headers,
        body: new_body,
        version: new_version,
        timeout: _,
        redirect: _,
        auth: _,
        auth_disabled: _,
        decompress: _,
        proxy_override: _,
        retry: _,
        transport_hints: _,
    } = redirect_req.into_parts();

    Ok(RedirectHop {
        method: new_method,
        url: new_url,
        headers: new_headers,
        body: new_body,
        version: new_version,
    })
}

/// Transport-ready state after request policy and body/header normalization.
///
/// Built once in `send_single_request`'s preparation phase. Transport
/// dispatch then selects an execution path without reimplementing
/// content-length, version, or header policy. The body is owned so one-shot
/// streams are moved, never cloned, to satisfy the abstraction.
struct PreparedRequest {
    method: http::Method,
    url: url::Url,
    uri: http::Uri,
    headers: Headers,
    body: RequestBody,
    version: http::Version,
    transport_hints: crate::request::TransportHints,
    #[cfg(feature = "proxy")]
    effective_proxy: Option<ProxyConfig>,
    decompression_enabled: bool,
    timeout: Timeout,
    remaining_total: Option<Duration>,
    deadline: Option<std::time::Instant>,
}

/// Declarative transport route selected after preparation.
///
/// Precedence (unchanged): configured UDS, specialized direct connector
/// when applicable (no proxy), effective proxy/SOCKS path, SNI override
/// direct path, H3 where selected, standard Hyper direct path. H3 never
/// bypasses proxy rules because proxy routes are selected first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransportRoute {
    /// Configured Unix-domain-socket client.
    Uds,
    /// Specialized direct connector (socket options / local address).
    Direct,
    /// Effective proxy or SOCKS path.
    Proxy,
    /// Cached SNI-override direct client.
    SniDirect,
    /// HTTP/3 over QUIC.
    H3,
    /// Standard Hyper direct path.
    Standard,
}

/// Select the transport route from prepared state.
///
/// Pure function over availability flags so precedence is directly tested
/// without constructing clients.
#[allow(
    clippy::fn_params_excessive_bools,
    reason = "route selection is a pure precedence predicate over five availability flags; bundling into a struct adds indirection for tests"
)]
fn select_route(
    has_uds: bool,
    has_direct_no_proxy: bool,
    has_proxy: bool,
    has_sni: bool,
    use_h3: bool,
) -> TransportRoute {
    if has_uds {
        TransportRoute::Uds
    } else if has_direct_no_proxy {
        TransportRoute::Direct
    } else if has_proxy {
        TransportRoute::Proxy
    } else if has_sni {
        TransportRoute::SniDirect
    } else if use_h3 {
        TransportRoute::H3
    } else {
        TransportRoute::Standard
    }
}

/// Resolve an Alt-Svc-discovered H3 target for `Auto` discovery.
///
/// Returns `None` when no fresh alternative exists, when suppressed, when
/// the URL is not `https`, or when `http3` is disabled. Counts lazy expiry
/// and suppression exactly once per call. Explicit `Http3Only` never calls
/// this (direct route, no discovery).
#[cfg(feature = "http3")]
fn h3_discovered_target(
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
#[cfg(feature = "http3")]
fn learn_altsvc_from_response(
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

/// Build a Hyper request from prepared parts.
///
/// Shared by the UDS, specialized-direct, SNI-direct, and standard Hyper
/// paths so `http::Request` scaffolding is not rebuilt in each branch.
///
/// # Errors
///
/// Returns [`Error::RequestBuild`] if the Hyper request cannot be built.
fn build_hyper_request(
    method: &http::Method,
    uri: http::Uri,
    version: http::Version,
    headers: &Headers,
    body: RequestBody,
) -> Result<http::Request<crate::transport::HyperRequestBody>> {
    let mut builder = http::Request::builder()
        .method(method)
        .uri(uri)
        .version(version);
    for (name, value) in headers.iter() {
        builder = builder.header(name, value);
    }
    builder
        .body(body.into_http_body())
        .map_err(|e| Error::RequestBuild(e.to_string()))
}

/// Send a request through the client, following redirects if enabled.
///
/// This is the top-level entry point for the request pipeline. It
/// handles header merging, timeout computation, cookie injection,
/// authentication, the redirect loop, and response post-processing.
#[allow(clippy::too_many_lines)]
pub(crate) async fn send_with_redirects(client: &Client, request: Request) -> Result<Response> {
    let crate::request::RequestParts {
        method,
        url,
        headers: request_headers,
        body,
        version,
        timeout: request_timeout,
        redirect: request_redirect,
        auth: req_auth,
        auth_disabled: req_auth_disabled,
        decompress: request_decompress,
        proxy_override: request_proxy,
        retry: _request_retry,
        transport_hints: request_transport_hints,
    } = request.into_parts();

    // Without the `proxy` feature the override has no transport effect.
    #[cfg(not(feature = "proxy"))]
    let _ = &request_proxy;

    let mut merged_headers = client.config().default_headers.clone().into_inner();
    for name in request_headers.keys() {
        merged_headers.remove(name);
    }
    merged_headers.extend(request_headers.into_inner());
    let headers = Headers::from(merged_headers);

    let timeout = match client.config().timeout {
        Some(client_timeout) => client_timeout.merge(request_timeout),
        None => request_timeout.unwrap_or_default(),
    };

    let effective_redirect = request_redirect
        .as_ref()
        .unwrap_or(&client.config().redirect);

    // Fast path: redirects disabled — send through the same first-hop
    // builder as the redirect-enabled path so the two cannot diverge except
    // for redirect-loop behavior itself.
    if !effective_redirect.follow {
        let request = build_hop_request(
            client,
            HopBuildParams {
                method,
                url,
                headers,
                body,
                version,
                timeout,
                decompress: request_decompress,
                #[cfg(feature = "proxy")]
                proxy_override: request_proxy,
                transport_hints: request_transport_hints,
                auth: req_auth,
                auth_disabled: req_auth_disabled,
                is_first_hop: true,
                credentials_allowed: true,
                cookie_allowed: true,
            },
        )?;

        let response = client.send_single_request(request, &timeout).await?;

        #[cfg(feature = "cookies")]
        {
            let set_cookie_headers: Vec<String> = response
                .headers()
                .get_all("set-cookie")
                .iter()
                .filter_map(|v| v.to_str().ok().map(str::to_owned))
                .collect();
            if !set_cookie_headers.is_empty() {
                client
                    .config()
                    .cookie_jar
                    .update_from_response(response.url(), &set_cookie_headers);
            }
        }

        return Ok(response);
    }

    let mut history = Vec::new();
    let mut redirect_count = 0usize;
    let start_time = std::time::Instant::now();

    let (mut replay_body, mut cur_body) = match body {
        RequestBody::Empty => (Some(Bytes::new()), RequestBody::Empty),
        RequestBody::Bytes(bytes) => (Some(bytes.clone()), RequestBody::Bytes(bytes)),
        stream @ RequestBody::Stream { .. } => (None, stream),
    };

    let mut cur_method = method;
    let mut cur_url = url;
    let mut cur_headers = headers;
    let mut cur_version = version;
    #[cfg(feature = "cookies")]
    let mut cookie_header_allowed = true;

    let mut prev_url: Option<url::Url> = None;
    let mut credentials_allowed = true;

    loop {
        let hop_timeout = if let Some(total_dur) = timeout.total {
            let elapsed = start_time.elapsed();
            if elapsed >= total_dur {
                return Err(Error::Timeout {
                    phase: TimeoutPhase::Total,
                    elapsed,
                });
            }
            let remaining = total_dur.saturating_sub(elapsed);
            let mut hop = timeout;
            hop.total = Some(remaining);
            hop
        } else {
            timeout
        };

        let is_first_hop = prev_url.is_none();
        let is_cross_origin_redirect = prev_url
            .as_ref()
            .is_some_and(|prev| prev.origin() != cur_url.origin());
        if is_cross_origin_redirect {
            credentials_allowed = false;
            #[cfg(feature = "cookies")]
            {
                cookie_header_allowed = false;
            }
        }

        let hop_request = build_hop_request(
            client,
            HopBuildParams {
                method: cur_method.clone(),
                url: cur_url.clone(),
                headers: cur_headers.clone(),
                body: cur_body,
                version: cur_version,
                timeout: hop_timeout,
                decompress: request_decompress,
                #[cfg(feature = "proxy")]
                proxy_override: request_proxy.clone(),
                transport_hints: request_transport_hints.clone(),
                auth: req_auth.clone(),
                auth_disabled: req_auth_disabled,
                is_first_hop,
                credentials_allowed,
                #[cfg(feature = "cookies")]
                cookie_allowed: cookie_header_allowed,
                #[cfg(not(feature = "cookies"))]
                cookie_allowed: true,
            },
        )?;

        let mut response = client
            .send_single_request(hop_request, &hop_timeout)
            .await?;

        #[cfg(feature = "cookies")]
        {
            let set_cookie_headers: Vec<String> = response
                .headers()
                .get_all("set-cookie")
                .iter()
                .filter_map(|v| v.to_str().ok().map(str::to_owned))
                .collect();
            if !set_cookie_headers.is_empty() {
                client
                    .config()
                    .cookie_jar
                    .update_from_response(response.url(), &set_cookie_headers);
            }
        }

        if !redirect::is_redirect_status(response.status()) || !effective_redirect.follow {
            response.set_history(history);
            return Ok(response);
        }

        let location = if let Some(v) = response.headers().get("location") {
            v.to_str()
                .map_err(|e| Error::InvalidRedirectLocation(e.to_string()))?
                .to_owned()
        } else {
            response.set_history(history);
            return Ok(response);
        };

        if let Some(total) = timeout.total {
            // Bound the drain by the remaining total deadline, but drain
            // with the byte-capped helper rather than buffering the whole
            // redirect body in memory just to release the connection.
            let dur = total.saturating_sub(start_time.elapsed());
            if tokio::time::timeout(dur, drain_response_body(&mut response))
                .await
                .is_err()
            {
                return Err(Error::Timeout {
                    phase: TimeoutPhase::Total,
                    elapsed: start_time.elapsed(),
                });
            }
        } else {
            drain_response_body(&mut response).await;
        }

        redirect_count += 1;
        if redirect_count > effective_redirect.max_redirects {
            return Err(Error::TooManyRedirects {
                followed: redirect_count - 1,
                max: effective_redirect.max_redirects,
            });
        }

        let redirect_status = response.status();
        // Single redirect transformation step: method/body policy, replay
        // check, and header stripping live in `advance_redirect_hop`.
        let hop = advance_redirect_hop(
            &cur_method,
            &cur_url,
            &cur_headers,
            cur_version,
            &mut replay_body,
            redirect_status,
            &location,
        )?;

        history.push(HistoryEntry::from_response(&response));

        prev_url = Some(cur_url);

        cur_method = hop.method;
        cur_url = hop.url;
        cur_headers = hop.headers;
        cur_body = hop.body;
        cur_version = hop.version;
    }
}

/// Apply `Content-Length` header to known-size request bodies when the
/// user has not provided one. For known-size bodies with a user-supplied
/// `Content-Length`, reject mismatches.
pub(crate) fn apply_content_length(headers: Headers, body: &RequestBody) -> Result<Headers> {
    let known =
        match body {
            RequestBody::Empty => Some(0u64),
            RequestBody::Bytes(b) => Some(u64::try_from(b.len()).map_err(|_| {
                Error::RequestBuild("request body length does not fit in u64".into())
            })?),
            RequestBody::Stream {
                length: Some(n), ..
            } => Some(u64::try_from(*n).map_err(|_| {
                Error::RequestBuild("request body length does not fit in u64".into())
            })?),
            RequestBody::Stream { length: None, .. } => None,
        };

    let supplied = headers.get("content-length").map(|value| {
        value
            .to_str()
            .map_err(|e| Error::InvalidHeaderValue(format!("invalid Content-Length: {e}")))?
            .parse::<u64>()
            .map_err(|e| Error::InvalidHeaderValue(format!("invalid Content-Length: {e}")))
    });
    let supplied = supplied.transpose()?;

    if let Some(known_len) = known {
        if let Some(user_len) = supplied {
            if user_len != known_len {
                return Err(Error::RequestBuild(format!(
                    "Content-Length mismatch: user supplied {user_len} but body is {known_len}"
                )));
            }
        } else {
            let mut h = headers;
            h.insert("content-length", &known_len.to_string())?;
            return Ok(h);
        }
    } else if supplied.is_some() {
        return Err(Error::RequestBuild(
            "Content-Length cannot be supplied for an unknown-length stream body".into(),
        ));
    }

    Ok(headers)
}

/// Apply the read-timeout wrapper to the streaming response body and
/// attach the pool permit so it is held until the body is consumed or
/// dropped.
pub(crate) fn apply_read_timeout_and_lease(
    response: &mut Response,
    guard: PoolGuard,
    read_timeout: Option<Duration>,
) {
    let body = std::mem::replace(&mut response.body, ResponseBody::buffered(Bytes::new()));

    let new_body = match body {
        ResponseBody::Streaming { mut stream, .. } => {
            if let Some(dur) = read_timeout {
                let inner = std::mem::replace(
                    &mut stream,
                    Box::pin(futures_util::stream::empty::<crate::error::Result<Bytes>>()),
                );
                stream = read_timeout_stream(inner, dur);
            }
            ResponseBody::streaming_with_lease(stream, Arc::new(guard))
        }
        ResponseBody::EncodedStreaming {
            mut stream,
            content_encoding,
            limit,
            ..
        } => {
            if let Some(dur) = read_timeout {
                let inner = std::mem::replace(
                    &mut stream,
                    Box::pin(futures_util::stream::empty::<crate::error::Result<Bytes>>()),
                );
                stream = read_timeout_stream(inner, dur);
            }
            ResponseBody::encoded_streaming_with_lease(
                stream,
                Arc::new(guard),
                content_encoding,
                limit,
            )
        }
        other => {
            drop(guard);
            other
        }
    };

    response.set_body(new_body);
}

/// Resolve the effective proxy configuration for a request.
///
/// Applies the tri-state override model:
/// - `Inherit`: use client-level proxy
/// - `Direct`: direct, no proxy
/// - `Override(config)`: use request-level proxy
#[cfg(feature = "proxy")]
pub(crate) fn resolve_proxy(
    inner: &ClientInner,
    url: &url::Url,
    proxy_override: &ProxyOverride,
) -> Option<ProxyConfig> {
    match proxy_override {
        ProxyOverride::Override(config) => Some(config.clone()),
        ProxyOverride::Direct => None,
        ProxyOverride::Inherit => {
            let candidates = inner
                .config
                .proxy
                .iter()
                .chain(inner.config.environment_proxies.iter());
            candidates
                .filter(|p| p.should_use_for_scheme(url.scheme()))
                .find(|p| p.no_proxy_rules().map_or(true, |np| !np.should_bypass(url)))
                .map(Proxy::config)
        }
    }
}

/// Preparation phase for [`send_single_request`]: normalize headers, body,
/// version, and proxy/pool/timeout state into a transport-ready form.
///
/// Centralizes content-length, user-agent, accept-encoding, H2 header, and
/// request-size policy so transport modules own only connection/protocol
/// work. The pool guard is acquired here and returned alongside the prepared
/// request; the caller attaches it to the response body after transport
/// completes.
///
/// # Errors
///
/// Returns an error for TLS misconfiguration, proxy origin resolution,
/// pool timeouts, content-length mismatches, or oversized requests.
#[allow(
    clippy::too_many_lines,
    reason = "preparation centralizes header/body/version/proxy/pool policy in one place so transports do not reimplement it"
)]
async fn prepare_single_request(
    inner: &ClientInner,
    request: Request,
    timeout: &Timeout,
) -> Result<(PreparedRequest, PoolGuard)> {
    // Destructure exhaustively so new `RequestParts` fields fail to compile
    // here rather than being silently dropped before transport.
    let crate::request::RequestParts {
        method,
        url,
        headers,
        body,
        version,
        timeout: _request_timeout,
        redirect: _request_redirect,
        auth: _,
        auth_disabled: _,
        decompress: request_decompress,
        proxy_override,
        retry: _,
        transport_hints,
    } = request.into_parts();

    #[cfg(feature = "tls-rustls")]
    if url.scheme() == "https" {
        if let Some(error) = &inner.tls_config_error {
            return Err(Error::Tls(error.clone()));
        }
    }

    let decompression_enabled = request_decompress.unwrap_or(inner.config.automatic_decompression);

    let mut headers = headers;
    if decompression_enabled && !headers.contains("accept-encoding") {
        if let Some(value) = crate::compression::accept_encoding_value() {
            headers.insert("accept-encoding", value)?;
        }
    }

    #[cfg(feature = "proxy")]
    let effective_proxy = resolve_proxy(inner, &url, &proxy_override);
    #[cfg(not(feature = "proxy"))]
    {
        let _ = proxy_override;
    }

    #[cfg(feature = "proxy")]
    let origin = match effective_proxy {
        Some(ref proxy_config) => {
            let is_tunnel = url.scheme() == "https";
            OriginKey::from_url_with_proxy_scheme(
                url.scheme(),
                &url,
                proxy_config.host(),
                Some(proxy_config.port()?),
                Some(proxy_config.scheme()),
                is_tunnel,
            )
        }
        None => OriginKey::from_url(url.scheme(), &url),
    };
    #[cfg(not(feature = "proxy"))]
    let origin = OriginKey::from_url(url.scheme(), &url);

    let started = std::time::Instant::now();
    let pool_deadline = match (timeout.pool, timeout.total) {
        (Some(pool), Some(total)) if total < pool => Some((total, TimeoutPhase::Total)),
        (Some(pool), _) => Some((pool, TimeoutPhase::Pool)),
        (None, Some(total)) => Some((total, TimeoutPhase::Total)),
        (None, None) => None,
    };
    let guard = match pool_deadline {
        Some((duration, phase)) => {
            match tokio::time::timeout(duration, inner.pool.acquire(origin.as_ref())).await {
                Ok(guard) => guard?,
                Err(_) => {
                    return Err(Error::Timeout {
                        phase,
                        elapsed: started.elapsed(),
                    })
                }
            }
        }
        None => inner.pool.acquire(origin.as_ref()).await?,
    };

    let body = match (body, timeout.write) {
        (RequestBody::Stream { stream, length }, Some(write_dur)) => {
            let wrapped = write_timeout_stream(stream, write_dur);
            RequestBody::Stream {
                stream: wrapped,
                length,
            }
        }
        (b, _) => b,
    };

    let headers = apply_content_length(headers, &body)?;

    let mut headers = headers;
    if let Some(ref ua) = inner.config.user_agent {
        if !headers.contains("user-agent") {
            headers.insert(http::header::USER_AGENT.as_str(), ua.as_str())?;
        }
    }

    let enabler = crate::http_version::HttpVersionPolicyEnabler::from_policy(
        inner.config.http_version_policy,
    );
    // Only strip H2-forbidden headers when the request is known to use
    // HTTP/2 (explicit version or H2-only policy). For `Auto`, ALPN
    // negotiation is per-connection; stripping here would drop
    // `Connection: close` etc. on H1.1. Hyper will enforce H2 header
    // validity if `Auto` negotiates h2.
    if version == http::Version::HTTP_2 || (!enabler.enable_http1() && enabler.enable_http2()) {
        crate::h2_headers::strip_h2_forbidden_headers(&mut headers);
    }
    let request_uri = resolve_request_uri(&url, &transport_hints)?;
    headers.validate_request_size(&method, request_uri.to_string().as_bytes())?;

    let remaining_total = timeout
        .total
        .map(|total| total.saturating_sub(started.elapsed()));
    let deadline = timeout.total.map(|total| started + total);

    let prepared = PreparedRequest {
        method,
        url,
        uri: request_uri,
        headers,
        body,
        version,
        transport_hints,
        #[cfg(feature = "proxy")]
        effective_proxy,
        decompression_enabled,
        timeout: *timeout,
        remaining_total,
        deadline,
    };
    Ok((prepared, guard))
}

/// Send a single HTTP request and return the streaming response.
///
/// This handles pool acquisition, timeout application, and body
/// processing for one request/response cycle. It does NOT handle
/// redirects—that is the responsibility of [`send_with_redirects`].
///
/// The implementation separates a preparation phase
/// ([`prepare_single_request`]) from declarative transport selection
/// ([`select_route`]); one common post-transport policy (decompression,
/// decoded-size limiting, read-timeout and pool-lease attachment) applies
/// to every route.
#[allow(clippy::too_many_lines)]
#[cfg(any(feature = "http1", feature = "http2"))]
pub(crate) async fn send_single_request(
    inner: &ClientInner,
    request: Request,
    timeout: &Timeout,
) -> Result<Response> {
    let (prepared, guard) = prepare_single_request(inner, request, timeout).await?;
    let PreparedRequest {
        method,
        url,
        uri,
        headers,
        body,
        version,
        transport_hints,
        #[cfg(feature = "proxy")]
        effective_proxy,
        decompression_enabled,
        timeout: hop_timeout,
        remaining_total,
        deadline,
    } = prepared;

    // `deadline` feeds the proxy multi-phase context; without the `proxy`
    // feature no route consumes it.
    #[cfg(not(feature = "proxy"))]
    let _ = &deadline;

    #[cfg(unix)]
    let has_uds = inner.uds_client.is_some();
    #[cfg(not(unix))]
    let has_uds = false;
    #[cfg(feature = "proxy")]
    let has_proxy = effective_proxy.is_some();
    #[cfg(not(feature = "proxy"))]
    let has_proxy = false;
    #[cfg(feature = "proxy")]
    let has_direct_no_proxy = !has_proxy && inner.direct_client.is_some() && !has_uds;
    #[cfg(not(feature = "proxy"))]
    let has_direct_no_proxy = !has_uds && inner.direct_client.is_some();
    let has_sni = transport_hints.sni_hostname.is_some();
    // Discovery semantics for `Auto` (plan §4):
    // - `Http3Only` = explicit direct QUIC route, strict, no discovery, no
    //   fallback, no suppression.
    // - `Auto { allow_http3: true }` = discovery: H2/H1 unless a fresh
    //   Alt-Svc alternative is cached and not suppressed. No fresh entry
    //   means never invent an H3 endpoint.
    // - `Auto { allow_http3: false }` (default) never attempts H3.
    // H3 never bypasses proxy/UDS/SNI precedence (selected first).
    #[cfg(feature = "http3")]
    let (use_h3, h3_alt) = {
        use crate::HttpVersionPolicy;
        match inner.config.http_version_policy {
            HttpVersionPolicy::Http3Only => (true, None),
            HttpVersionPolicy::Auto { allow_http3: true } => {
                // Only HTTPS origins can be discovered; plaintext never
                // learns and never routes H3. Earlier routes (UDS/proxy/SNI)
                // already win via `select_route`, so discovery here only
                // decides H3-vs-standard.
                if has_uds || has_proxy || has_sni {
                    (false, None)
                } else if let Some(target) = h3_discovered_target(inner, &url) {
                    (true, Some(target))
                } else {
                    (false, None)
                }
            }
            _ => (false, None),
        }
    };
    #[cfg(not(feature = "http3"))]
    let use_h3 = false;
    let route = select_route(has_uds, has_direct_no_proxy, has_proxy, has_sni, use_h3);
    #[cfg(feature = "http3")]
    if use_h3 {
        inner.transport_metrics.record_h3_attempted();
    }

    // Declarative transport dispatch. Precedence is encoded in
    // `select_route` and covered by direct unit tests; H3 never bypasses
    // proxy rules because proxy routes are selected first.
    let response = match route {
        TransportRoute::Uds => {
            #[cfg(unix)]
            {
                let uds_client = inner
                    .uds_client
                    .as_ref()
                    .ok_or_else(|| Error::Unsupported("UDS client not available".into()))?;
                let hyper_request = build_hyper_request(&method, uri, version, &headers, body)?;
                let send_future = crate::transport::uds::send_request(
                    uds_client,
                    hyper_request,
                    url.clone(),
                    transport_hints.trace.as_deref(),
                );
                send_with_total_timeout(send_future, remaining_total).await?
            }
            #[cfg(not(unix))]
            {
                let _ = (method, uri, headers, body, version);
                return Err(Error::Unsupported(
                    "Unix domain sockets are not supported on this platform".into(),
                ));
            }
        }
        TransportRoute::Direct => {
            let direct_client = inner
                .direct_client
                .as_ref()
                .ok_or_else(|| Error::Unsupported("direct client not available".into()))?;
            let hyper_request = build_hyper_request(&method, uri, version, &headers, body)?;
            let send_future = crate::transport::direct::send_direct_request(
                direct_client,
                hyper_request,
                url.clone(),
                transport_hints.trace.as_deref(),
            );
            send_with_total_timeout(send_future, remaining_total).await?
        }
        TransportRoute::Proxy => {
            #[cfg(feature = "proxy")]
            {
                let proxy_config = effective_proxy.as_ref().ok_or_else(|| {
                    Error::Unsupported("proxy configuration not available".into())
                })?;
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
                        Some(proxy) => Some(inner.socks_client(proxy).await?),
                        None => None,
                    }
                };
                Box::pin(send_proxy_request(
                    &url,
                    &method,
                    &headers,
                    body,
                    version,
                    proxy_config,
                    &transport_hints,
                    &crate::transport::proxy::ProxyRequestContext {
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
                        // config. When the proxy endpoint has no explicit
                        // TLS configuration we use the proxy endpoint's own
                        // default trust roots rather than reusing the origin
                        // CA / client identity / verification policy. This
                        // prevents a custom origin CA, origin mTLS identity,
                        // or origin verify=False from leaking into the proxy
                        // handshake.
                        proxy_tls_config: proxy_config.proxy_tls_config(),
                        socks_client,
                        transport_metrics: Some(inner.transport_metrics.clone()),
                    },
                ))
                .await?
            }
            #[cfg(not(feature = "proxy"))]
            {
                let _ = (method, uri, headers, body, version);
                return Err(Error::Unsupported("proxy support is not enabled".into()));
            }
        }
        TransportRoute::SniDirect => {
            // SNI override separates DNS/TCP (to the original host) from TLS
            // (with the override hostname). The CONNECT tunnel handles its
            // own SNI in the proxy path above.
            let sni_hostname = transport_hints.sni_hostname.clone().ok_or_else(|| {
                Error::RequestBuild("SNI route selected without sni_hostname".into())
            })?;
            let sni_client = inner.sni_client(&sni_hostname).await?;
            let hyper_request = build_hyper_request(&method, uri, version, &headers, body)?;
            let send_future = crate::transport::direct::send_direct_request(
                &sni_client,
                hyper_request,
                url.clone(),
                transport_hints.trace.as_deref(),
            );
            send_with_total_timeout(send_future, remaining_total).await?
        }
        TransportRoute::H3 => {
            #[cfg(feature = "http3")]
            {
                use crate::body::RequestBody;
                let h3_connector = inner.h3_connector.as_ref().ok_or_else(|| {
                    Error::Unsupported(
                        "HTTP/3 connector not available; ensure http3 feature is enabled".into(),
                    )
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
                    .method(&method)
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
                // does not bloat the shared `send_single_request` future.
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
                    Response,
                    crate::transport::http3::H3DispatchError,
                > = match remaining_total {
                    Some(dur) => {
                        let started = std::time::Instant::now();
                        match tokio::time::timeout(dur, h3_future).await {
                            Ok(r) => r,
                            Err(_) => {
                                Err(crate::transport::http3::H3DispatchError::for_total_timeout(
                                    started.elapsed(),
                                ))
                            }
                        }
                    }
                    None => h3_future.await,
                };
                match h3_outcome {
                    Ok(resp) => {
                        // Successful H3 use clears broken-route suppression
                        // (both explicit and discovered; explicit never
                        // suppresses but may clear stale discovered state).
                        if let Some(origin) =
                            crate::transport::alt_svc::AltSvcOrigin::from_url(&url)
                        {
                            inner.alt_svc_state.suppressor().record_success(&origin);
                        }
                        resp
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
                            if let Some(origin) =
                                crate::transport::alt_svc::AltSvcOrigin::from_url(&url)
                            {
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
                            let fb_body = fallback_body.expect("checked above");
                            send_hyper_request(
                                inner,
                                &method,
                                url.clone(),
                                &headers,
                                fb_body,
                                version,
                                remaining_total,
                                &transport_hints,
                            )
                            .await?
                        } else {
                            // Post-commit, one-shot, or request-scoped:
                            // never silently duplicate. Retry policy remains
                            // the only later-attempt mechanism.
                            return Err(dispatch.error);
                        }
                    }
                }
            }
            #[cfg(not(feature = "http3"))]
            {
                let _ = (method, uri, headers, body, version);
                return Err(Error::Unsupported("HTTP/3 support is not enabled".into()));
            }
        }
        TransportRoute::Standard => {
            send_hyper_request(
                inner,
                &method,
                url.clone(),
                &headers,
                body,
                version,
                remaining_total,
                &transport_hints,
            )
            .await?
        }
    };

    let mut response = response;

    // Authenticated Alt-Svc learning (plan §3): only from learnable routes
    // (Standard/Direct/H3) with `https`, verified TLS, no proxy. UDS, SNI,
    // and proxy routes never install alternatives. Logical origin only;
    // alternative authorities never change cookies/auth/Host/security.
    #[cfg(feature = "http3")]
    {
        let learnable = matches!(
            route,
            TransportRoute::Standard | TransportRoute::Direct | TransportRoute::H3
        );
        #[cfg(feature = "proxy")]
        let via_proxy = effective_proxy.is_some();
        #[cfg(not(feature = "proxy"))]
        let via_proxy = false;
        if learnable {
            learn_altsvc_from_response(inner, &url, response.headers(), via_proxy, true);
        }
    }

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
            max_decoded_body_size: inner.config.max_decoded_body_size,
            max_decompression_ratio: inner.config.max_decompression_ratio,
        };

        response = crate::response_decode::apply_decompression(
            response,
            content_encoding.as_deref(),
            limit,
        )?;
    }

    if let Some(max) = inner.config.max_decoded_body_size {
        // EncodedStreaming merges this value into its existing decompression
        // limit; unencoded streaming bodies get the limiting wrapper here.
        // This does not add a second decoded-size stream layer.
        response.body = response.body.limit_decoded_size(max)?;
    }

    apply_read_timeout_and_lease(&mut response, guard, hop_timeout.read);

    Ok(response)
}

/// Report that no HTTP protocol feature was selected.
#[cfg(not(any(feature = "http1", feature = "http2")))]
pub(crate) async fn send_single_request(
    _inner: &ClientInner,
    _request: Request,
    _timeout: &Timeout,
) -> Result<Response> {
    Err(Error::Unsupported(
        "no HTTP protocol feature is enabled; enable http1 or http2".into(),
    ))
}

/// Send a request through the hyper/HTTP-1.1/2 transport.
///
/// Shared standard-path helper; UDS, specialized-direct, and SNI-direct
/// paths build their Hyper requests through [`build_hyper_request`] and call
/// their respective `transport::direct`/`transport::uds` converters, so this
/// helper exists only to keep the standard Hyper dispatch explicit.
#[allow(clippy::too_many_arguments)] // Keeps the direct-send helper explicit; timeout wrapping is the only added phase input.
#[cfg(any(feature = "http1", feature = "http2"))]
async fn send_hyper_request(
    inner: &ClientInner,
    method: &http::Method,
    url: url::Url,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    remaining_total: Option<Duration>,
    transport_hints: &crate::request::TransportHints,
) -> Result<Response> {
    let hyper_client = inner
        .hyper_client
        .as_ref()
        .ok_or_else(|| Error::Unsupported("HTTP client not available for this protocol".into()))?;

    let uri = resolve_request_uri(&url, transport_hints)?;
    let hyper_request = build_hyper_request(method, uri, version, headers, body)?;

    let send_future = crate::transport::direct::send_request(
        hyper_client,
        hyper_request,
        url.clone(),
        transport_hints.trace.as_deref(),
    );

    send_with_total_timeout(send_future, remaining_total).await
}

/// Validate a `target` extension value for request smuggling safety.
///
/// Rejects C0 control characters and DEL bytes. The target must also be
/// valid UTF-8 because it is converted to Hyper's text-based URI type before
/// dispatch; callers should percent-encode non-ASCII octets when necessary.
pub(crate) fn validate_target(target: &[u8]) -> Result<()> {
    if target.is_empty() {
        return Err(Error::RequestBuild(
            "target extension must not be empty".into(),
        ));
    }
    if target.iter().any(|&b| b < 0x20 || b == 0x7f) {
        return Err(Error::RequestBuild(
            "target extension contains forbidden characters (C0 controls/DEL; includes CR/LF/NUL)"
                .into(),
        ));
    }
    // Reject whitespace-only or leading/trailing whitespace targets that would
    // otherwise pass the C0 check (space 0x20) and produce a late `InvalidUrl`
    // or bypass proxy routing checks.
    if target.iter().all(|&b| b == b' ') {
        return Err(Error::RequestBuild(
            "target extension must not be whitespace-only".into(),
        ));
    }
    if target.first() == Some(&b' ') || target.last() == Some(&b' ') {
        return Err(Error::RequestBuild(
            "target extension must not contain leading or trailing whitespace".into(),
        ));
    }
    Ok(())
}

/// Build the HTTP request URI, applying `target` transport hint if present.
///
/// When `target` is set, the wire URI is overridden while the logical URL
/// is preserved for routing, Host header, cookies, and auth decisions.
fn resolve_request_uri(
    url: &url::Url,
    transport_hints: &crate::request::TransportHints,
) -> Result<http::Uri> {
    if let Some(ref target) = transport_hints.target {
        validate_target(target)?;
        std::str::from_utf8(target)
            .map_err(|_| Error::InvalidUrl("target extension is not valid UTF-8".into()))?
            .parse()
            .map_err(|e| Error::InvalidUrl(format!("failed to convert target to URI: {e}")))
    } else {
        url.as_str()
            .parse()
            .map_err(|e| Error::InvalidUrl(format!("failed to convert url to URI: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::{RequestParts, TransportHints};
    use bytes::Bytes;
    use std::sync::Arc;
    use std::time::Duration;

    fn test_url() -> url::Url {
        url::Url::parse("https://example.com/path").expect("valid test URL")
    }

    fn full_request() -> Request {
        use crate::auth::{AuthScheme, BasicAuth};
        use crate::redirect::RedirectPolicy;
        use crate::retry::RetryPolicy;

        let mut req = Request::new(http::Method::POST, test_url());
        req.headers_mut().insert("x-custom", "keep").unwrap();
        req.headers_mut()
            .insert("content-type", "text/plain")
            .unwrap();
        req.set_body(RequestBody::Bytes(Bytes::from("payload")));
        req.set_version(http::Version::HTTP_11);
        req.set_timeout(Some(
            Timeout::builder()
                .pool(Duration::from_secs(1))
                .connect(Duration::from_secs(2))
                .write(Duration::from_secs(3))
                .read(Duration::from_secs(4))
                .total(Duration::from_secs(30))
                .build(),
        ));
        req.set_redirect(Some(RedirectPolicy::new(true, 7)));
        req.set_auth(Some(AuthScheme::Basic(
            BasicAuth::new("user", "pass").expect("valid auth"),
        )));
        req.set_auth_disabled(false);
        req.set_decompress(Some(false));
        #[cfg(feature = "proxy")]
        req.set_proxy_override(crate::request::ProxyOverride::Direct);
        req.set_retry(Some(RetryPolicy::default()));
        req.set_transport_hints(TransportHints {
            target: Some(Bytes::from("/override-target")),
            sni_hostname: Some("sni.example".to_owned()),
            trace: Some(Arc::new(crate::trace::NoopTraceObserver)),
        });
        req
    }

    #[test]
    fn into_request_round_trip_preserves_every_field() {
        // Exhaustive round-trip: if a field is added to `RequestParts` but
        // omitted from `into_request`, this test (and compilation) fails.
        let req = full_request();
        let expected_method = req.method().clone();
        let expected_url = req.url().clone();
        let expected_headers = req.headers().clone();
        let expected_version = req.version();
        let expected_timeout = req.timeout().copied();
        let expected_redirect = req.redirect().cloned();
        let expected_auth = req.auth().cloned();
        let expected_auth_disabled = req.is_auth_disabled();
        let expected_decompress = req.decompress();
        let expected_retry = req.retry().cloned();
        let expected_target = req.transport_hints().target.clone();
        let expected_sni = req.transport_hints().sni_hostname.clone();
        let expected_has_trace = req.transport_hints().trace.is_some();

        let parts = req.into_parts();
        // Touch every field so wildcard destructuring cannot silently pass.
        let RequestParts {
            method: _,
            url: _,
            headers: _,
            body: _,
            version: _,
            timeout: _,
            redirect: _,
            auth: _,
            auth_disabled: _,
            decompress: _,
            proxy_override: _,
            retry: _,
            transport_hints: _,
        } = &parts;
        let rebuilt = parts.into_request();

        assert_eq!(rebuilt.method(), &expected_method);
        assert_eq!(rebuilt.url(), &expected_url);
        assert_eq!(
            rebuilt.headers().get("x-custom").unwrap().to_str().unwrap(),
            "keep"
        );
        let _ = expected_headers;
        assert_eq!(rebuilt.version(), expected_version);
        match (rebuilt.timeout(), expected_timeout.as_ref()) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert_eq!(a.pool, b.pool);
                assert_eq!(a.connect, b.connect);
                assert_eq!(a.write, b.write);
                assert_eq!(a.read, b.read);
                assert_eq!(a.total, b.total);
            }
            (a, b) => panic!("timeout mismatch: {a:?} vs {b:?}"),
        }
        assert_eq!(
            rebuilt.redirect().map(|r| (r.follow, r.max_redirects)),
            expected_redirect.map(|r| (r.follow, r.max_redirects))
        );
        assert_eq!(
            rebuilt.auth().map(|a| format!("{a:?}")),
            expected_auth.map(|a| format!("{a:?}"))
        );
        assert_eq!(rebuilt.is_auth_disabled(), expected_auth_disabled);
        assert_eq!(rebuilt.decompress(), expected_decompress);
        #[cfg(feature = "proxy")]
        assert!(matches!(
            rebuilt.proxy_override(),
            crate::request::ProxyOverride::Direct
        ));
        assert_eq!(rebuilt.retry().is_some(), expected_retry.is_some());
        assert_eq!(rebuilt.transport_hints().target, expected_target);
        assert_eq!(rebuilt.transport_hints().sni_hostname, expected_sni);
        assert_eq!(
            rebuilt.transport_hints().trace.is_some(),
            expected_has_trace
        );
        match rebuilt.body() {
            RequestBody::Bytes(b) => assert_eq!(b, "payload"),
            other => panic!("expected bytes body, got {other:?}"),
        }
    }

    #[test]
    fn retry_request_preserves_every_override_and_shrinks_total() {
        let req = full_request();
        let parts = req.into_parts();
        let attempt_timeout = Some(
            Timeout::builder()
                .pool(Duration::from_secs(1))
                .total(Duration::from_secs(25))
                .build(),
        );
        let attempt = parts.retry_request(attempt_timeout).expect("replayable");

        // All request-local configuration survives the retry transformation.
        assert_eq!(attempt.method(), &http::Method::POST);
        assert_eq!(attempt.url().as_str(), "https://example.com/path");
        assert_eq!(
            attempt.headers().get("x-custom").unwrap().to_str().unwrap(),
            "keep"
        );
        assert_eq!(attempt.version(), http::Version::HTTP_11);
        assert_eq!(
            attempt.timeout().and_then(|t| t.total),
            Some(Duration::from_secs(25)),
            "total deadline is replaced by the shrunk attempt budget"
        );
        assert!(attempt.redirect().is_some());
        assert!(attempt.auth().is_some());
        assert!(!attempt.is_auth_disabled());
        assert_eq!(attempt.decompress(), Some(false));
        #[cfg(feature = "proxy")]
        assert!(matches!(
            attempt.proxy_override(),
            crate::request::ProxyOverride::Direct
        ));
        assert!(attempt.retry().is_some());
        assert_eq!(
            attempt.transport_hints().target.as_deref(),
            Some(b"/override-target".as_slice())
        );
        assert_eq!(
            attempt.transport_hints().sni_hostname.as_deref(),
            Some("sni.example")
        );
        assert!(attempt.transport_hints().trace.is_some());
        match attempt.body() {
            RequestBody::Bytes(b) => assert_eq!(b, "payload"),
            other => panic!("expected replayed bytes, got {other:?}"),
        }
    }

    #[test]
    fn retry_request_rejects_one_shot_stream() {
        let mut req = Request::new(http::Method::POST, test_url());
        req.set_body(RequestBody::from_stream(
            futures_util::stream::empty::<crate::error::Result<Bytes>>(),
            None,
        ));
        let parts = req.into_parts();
        let err = parts.retry_request(None).unwrap_err();
        assert!(
            matches!(err, Error::BodyNotReplayableForRetry),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn retry_preserves_auth_disabled_state() {
        let mut req = Request::new(http::Method::GET, test_url());
        req.set_auth_disabled(true);
        let parts = req.into_parts();
        let attempt = parts.retry_request(None).expect("empty body replays");
        assert!(attempt.is_auth_disabled());
        assert!(attempt.auth().is_none());
    }

    #[test]
    fn shrink_total_deadline_subtracts_elapsed() {
        let t = Timeout::builder()
            .pool(Duration::from_secs(1))
            .total(Duration::from_secs(10))
            .build();
        let shrunk = RequestParts::shrink_total_deadline(&t, Duration::from_secs(3));
        assert_eq!(shrunk.total, Some(Duration::from_secs(7)));
        assert_eq!(shrunk.pool, Some(Duration::from_secs(1)));

        let saturated = RequestParts::shrink_total_deadline(&t, Duration::from_secs(20));
        assert_eq!(saturated.total, Some(Duration::ZERO));
    }

    #[test]
    fn select_route_precedence_matches_pipeline_order() {
        // UDS wins over everything.
        assert_eq!(
            select_route(true, true, true, true, true),
            TransportRoute::Uds
        );
        // Specialized direct wins when no proxy (even with SNI/H3 present).
        assert_eq!(
            select_route(false, true, false, true, true),
            TransportRoute::Direct
        );
        // Proxy wins over SNI/H3; H3 never bypasses proxy rules.
        assert_eq!(
            select_route(false, false, true, true, true),
            TransportRoute::Proxy
        );
        assert_eq!(
            select_route(false, false, true, false, true),
            TransportRoute::Proxy
        );
        // SNI wins over H3/standard when no proxy/direct.
        assert_eq!(
            select_route(false, false, false, true, true),
            TransportRoute::SniDirect
        );
        // H3 only when selected and no earlier route applies.
        assert_eq!(
            select_route(false, false, false, false, true),
            TransportRoute::H3
        );
        assert_eq!(
            select_route(false, false, false, false, false),
            TransportRoute::Standard
        );
        // `has_direct_no_proxy` already encodes "no proxy": callers compute
        // it as `direct.is_some() && !has_proxy`, so a proxy-present call
        // always passes `false` here and selects Proxy (H3 never bypasses
        // proxy rules).
    }

    #[test]
    fn hop_builder_first_hop_preserves_hints_and_applies_auth() {
        let client = crate::client::Client::new();
        let mut headers = Headers::new();
        headers.insert("x-custom", "keep").unwrap();

        let hop = build_hop_request(
            &client,
            HopBuildParams {
                method: http::Method::GET,
                url: test_url(),
                headers,
                body: RequestBody::Empty,
                version: http::Version::HTTP_11,
                timeout: Timeout::default(),
                decompress: Some(true),
                #[cfg(feature = "proxy")]
                proxy_override: crate::request::ProxyOverride::Direct,
                transport_hints: TransportHints {
                    target: Some(Bytes::from("/t")),
                    sni_hostname: Some("sni.example".to_owned()),
                    trace: Some(Arc::new(crate::trace::NoopTraceObserver)),
                },
                auth: None,
                auth_disabled: false,
                is_first_hop: true,
                credentials_allowed: true,
                cookie_allowed: true,
            },
        )
        .expect("first hop builds");

        assert_eq!(
            hop.transport_hints().target.as_deref(),
            Some(b"/t".as_slice())
        );
        assert_eq!(
            hop.transport_hints().sni_hostname.as_deref(),
            Some("sni.example")
        );
        assert!(hop.transport_hints().trace.is_some());
        assert_eq!(hop.decompress(), Some(true));
        assert_eq!(
            hop.headers().get("x-custom").unwrap().to_str().unwrap(),
            "keep"
        );
    }

    #[test]
    fn hop_builder_redirect_hop_clears_hints_and_drops_credentials() {
        use crate::auth::{AuthScheme, BasicAuth};

        let client = crate::client::Client::builder()
            .auth(AuthScheme::basic("client", "secret").expect("valid auth"))
            .build();
        let mut headers = Headers::new();
        headers.insert("cookie", "session=abc").unwrap();
        headers.insert("x-custom", "keep").unwrap();

        let hop = build_hop_request(
            &client,
            HopBuildParams {
                method: http::Method::GET,
                url: test_url(),
                headers,
                body: RequestBody::Empty,
                version: http::Version::HTTP_11,
                timeout: Timeout::default(),
                decompress: None,
                #[cfg(feature = "proxy")]
                proxy_override: crate::request::ProxyOverride::Inherit,
                transport_hints: TransportHints {
                    target: Some(Bytes::from("/t")),
                    sni_hostname: Some("sni.example".to_owned()),
                    trace: Some(Arc::new(crate::trace::NoopTraceObserver)),
                },
                auth: Some(AuthScheme::Basic(
                    BasicAuth::new("req", "pw").expect("valid auth"),
                )),
                auth_disabled: false,
                is_first_hop: false,
                credentials_allowed: false,
                cookie_allowed: false,
            },
        )
        .expect("redirect hop builds");

        // Destination-specific hints are cleared after the first hop.
        assert!(hop.transport_hints().target.is_none());
        assert!(hop.transport_hints().sni_hostname.is_none());
        assert!(hop.transport_hints().trace.is_none());
        // Cross-origin policy drops credentials and cookies.
        assert!(hop.auth().is_none());
        // Cookie stripping lives behind the `cookies` feature; without it
        // the hop builder preserves explicit headers verbatim.
        #[cfg(feature = "cookies")]
        assert!(hop.headers().get("cookie").is_none());
        #[cfg(not(feature = "cookies"))]
        assert!(hop.headers().get("cookie").is_some());
        // Non-sensitive headers survive.
        assert_eq!(
            hop.headers().get("x-custom").unwrap().to_str().unwrap(),
            "keep"
        );
    }

    #[test]
    fn no_redirect_fast_path_matches_redirect_first_hop() {
        // Both entry paths call the same `build_hop_request` with
        // `is_first_hop: true`; construct both and assert identical wire
        // state so a future divergence fails here.
        let client = crate::client::Client::new();
        let mk_headers = || {
            let mut h = Headers::new();
            h.insert("x-custom", "keep").unwrap();
            h
        };
        let mk_hints = || TransportHints {
            target: Some(Bytes::from("/t")),
            sni_hostname: None,
            trace: None,
        };

        let fast = build_hop_request(
            &client,
            HopBuildParams {
                method: http::Method::GET,
                url: test_url(),
                headers: mk_headers(),
                body: RequestBody::Empty,
                version: http::Version::HTTP_11,
                timeout: Timeout::default(),
                decompress: Some(true),
                #[cfg(feature = "proxy")]
                proxy_override: crate::request::ProxyOverride::Direct,
                transport_hints: mk_hints(),
                auth: None,
                auth_disabled: false,
                is_first_hop: true,
                credentials_allowed: true,
                cookie_allowed: true,
            },
        )
        .unwrap();
        let first_loop = build_hop_request(
            &client,
            HopBuildParams {
                method: http::Method::GET,
                url: test_url(),
                headers: mk_headers(),
                body: RequestBody::Empty,
                version: http::Version::HTTP_11,
                timeout: Timeout::default(),
                decompress: Some(true),
                #[cfg(feature = "proxy")]
                proxy_override: crate::request::ProxyOverride::Direct,
                transport_hints: mk_hints(),
                auth: None,
                auth_disabled: false,
                is_first_hop: true,
                credentials_allowed: true,
                cookie_allowed: true,
            },
        )
        .unwrap();

        assert_eq!(fast.method(), first_loop.method());
        assert_eq!(fast.url(), first_loop.url());
        assert_eq!(fast.version(), first_loop.version());
        assert_eq!(fast.decompress(), first_loop.decompress());
        assert_eq!(
            fast.transport_hints().target,
            first_loop.transport_hints().target
        );
        assert_eq!(
            format!("{:?}", fast.headers()),
            format!("{:?}", first_loop.headers())
        );
    }

    #[test]
    fn redirect_hop_post_drops_body_and_clears_replay() {
        let mut replay = Some(Bytes::from("payload"));
        let mut headers = Headers::new();
        headers.insert("content-length", "7").unwrap();
        headers.insert("content-type", "text/plain").unwrap();

        let hop = advance_redirect_hop(
            &http::Method::POST,
            &test_url(),
            &headers,
            http::Version::HTTP_11,
            &mut replay,
            http::StatusCode::MOVED_PERMANENTLY,
            "https://example.com/other",
        )
        .expect("301 POST redirects");

        assert_eq!(hop.method, http::Method::GET);
        assert!(hop.body.is_empty());
        assert!(hop.headers.get("content-length").is_none());
        assert!(hop.headers.get("content-type").is_none());
        assert_eq!(replay, Some(Bytes::new()));
    }

    #[test]
    fn redirect_hop_307_preserves_replayable_body() {
        let mut replay = Some(Bytes::from("payload"));
        let headers = Headers::new();

        let hop = advance_redirect_hop(
            &http::Method::POST,
            &test_url(),
            &headers,
            http::Version::HTTP_11,
            &mut replay,
            http::StatusCode::TEMPORARY_REDIRECT,
            "https://example.com/other",
        )
        .expect("307 POST preserves body");

        assert_eq!(hop.method, http::Method::POST);
        match hop.body {
            RequestBody::Bytes(b) => assert_eq!(b, "payload"),
            other => panic!("expected bytes, got {other:?}"),
        }
        assert_eq!(replay, Some(Bytes::from("payload")));
    }

    #[test]
    fn redirect_hop_rejects_one_shot_stream_before_replay() {
        let mut replay: Option<Bytes> = None;
        let headers = Headers::new();
        let err = advance_redirect_hop(
            &http::Method::POST,
            &test_url(),
            &headers,
            http::Version::HTTP_11,
            &mut replay,
            http::StatusCode::TEMPORARY_REDIRECT,
            "https://example.com/other",
        )
        .unwrap_err();
        assert!(
            matches!(err, Error::BodyNotReplayableForRedirect),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn build_hyper_request_carries_method_uri_version_headers() {
        let mut headers = Headers::new();
        headers.insert("x-custom", "keep").unwrap();
        let uri: http::Uri = "https://example.com/path".parse().unwrap();
        let req = build_hyper_request(
            &http::Method::GET,
            uri.clone(),
            http::Version::HTTP_11,
            &headers,
            RequestBody::Empty,
        )
        .expect("hyper request builds");
        assert_eq!(req.method(), &http::Method::GET);
        assert_eq!(req.uri(), &uri);
        assert_eq!(req.version(), http::Version::HTTP_11);
        assert_eq!(req.headers().get("x-custom").unwrap(), "keep");
    }
}
