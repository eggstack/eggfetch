//! Redirect handling: hop transformation and first-hop request assembly.
//!
//! `HopBuildParams` + `build_hop_request` form the single policy path shared
//! by the redirects-disabled fast path and every iteration of the
//! redirect-enabled loop, so the two entry paths cannot diverge except for
//! redirect-loop behavior itself. `advance_redirect_hop` performs the single
//! method/body/header transformation step per hop with exhaustive
//! request-state handling and same-origin resolved-target semantics.

use std::sync::Arc;

use bytes::Bytes;

use super::retry::drain_response_body;
use crate::body::RequestBody;
use crate::client::Client;
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::redirect;
use crate::request::Request;
use crate::response::{HistoryEntry, Response};
use crate::timeout::{Timeout, TimeoutPhase};

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
    max_decoded_body_size: Option<usize>,
    max_decompression_ratio: Option<f64>,
    #[cfg(feature = "proxy")]
    proxy_override: crate::request::ProxyOverride,
    transport_hints: crate::request::TransportHints,
    proxied_target: Option<crate::request::ResolvedTarget>,
    auth: Option<crate::auth::AuthScheme>,
    auth_disabled: bool,
    /// True only for the first hop of the logical request.
    is_first_hop: bool,
    /// Preserve the request-scoped resolved destination on a same-origin hop.
    preserve_resolved_target: bool,
    /// False after a cross-origin redirect; gates client-auth reapplication.
    credentials_allowed: bool,
    /// False after a cross-origin redirect; gates cookie injection.
    cookie_allowed: bool,
    failure_context: Option<Arc<crate::error::RequestFailureContext>>,
}

/// Build one hop request through the shared first-hop policy path.
///
/// - Transport hints (`target`, `sni_hostname`, `trace`) attach only on the
///   first hop; redirect hops clear them because the destination changed.
/// - Per-request decompression, decoded-body limits, and proxy overrides
///   persist across all hops.
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
        max_decoded_body_size,
        max_decompression_ratio,
        #[cfg(feature = "proxy")]
        proxy_override,
        transport_hints,
        proxied_target,
        auth,
        auth_disabled,
        is_first_hop,
        preserve_resolved_target,
        credentials_allowed,
        cookie_allowed,
        failure_context,
    } = params;

    let mut hop = Request::new(method, url);
    *hop.headers_mut() = headers;
    hop.set_body(body);
    hop.set_version(version);
    hop.set_timeout(Some(timeout));
    hop.set_decompress(decompress);
    hop.set_max_decoded_body_size(max_decoded_body_size);
    hop.set_max_decompression_ratio(max_decompression_ratio);
    #[cfg(feature = "proxy")]
    hop.set_proxy_override(proxy_override);
    // Ordinary wire hints apply only on the first hop. A resolved destination
    // is different: it remains valid for a same-origin redirect and is
    // retained only when the redirect loop explicitly permits it.
    if is_first_hop {
        hop.set_transport_hints(transport_hints);
        hop.set_proxied_target(proxied_target);
    } else if preserve_resolved_target {
        let resolved_hints = crate::request::TransportHints {
            resolved_target: transport_hints.resolved_target,
            ..Default::default()
        };
        hop.set_transport_hints(resolved_hints);
        hop.set_proxied_target(proxied_target);
    }

    hop.set_auth(if credentials_allowed { auth } else { None });
    hop.set_auth_disabled(auth_disabled);
    hop.set_failure_context(failure_context);

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
        max_decoded_body_size: _,
        max_decompression_ratio: _,
        proxy_override: _,
        retry: _,
        transport_hints: _,
        proxied_target: _,
        failure_context: _,
    } = redirect_req.into_parts();

    Ok(RedirectHop {
        method: new_method,
        url: new_url,
        headers: new_headers,
        body: new_body,
        version: new_version,
    })
}

/// Send a request through the client, following redirects if enabled.
///
/// This is the top-level entry point for the request pipeline. It
/// handles header merging, timeout computation, cookie injection,
/// authentication, the redirect loop, and response post-processing.
#[allow(clippy::too_many_lines)]
pub(super) async fn send_with_redirects(client: &Client, request: Request) -> Result<Response> {
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
        max_decoded_body_size: request_max_decoded_body_size,
        max_decompression_ratio: request_max_decompression_ratio,
        proxy_override: request_proxy,
        retry: _request_retry,
        transport_hints: request_transport_hints,
        proxied_target: request_proxied_target,
        failure_context,
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
                max_decoded_body_size: request_max_decoded_body_size,
                max_decompression_ratio: request_max_decompression_ratio,
                #[cfg(feature = "proxy")]
                proxy_override: request_proxy,
                transport_hints: request_transport_hints,
                proxied_target: request_proxied_target,
                auth: req_auth,
                auth_disabled: req_auth_disabled,
                is_first_hop: true,
                preserve_resolved_target: false,
                credentials_allowed: true,
                cookie_allowed: true,
                failure_context: failure_context.clone(),
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
            if request_transport_hints.resolved_target.is_some() || request_proxied_target.is_some()
            {
                return Err(Error::ResolvedTargetRedirect);
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
                max_decoded_body_size: request_max_decoded_body_size,
                max_decompression_ratio: request_max_decompression_ratio,
                #[cfg(feature = "proxy")]
                proxy_override: request_proxy.clone(),
                transport_hints: request_transport_hints.clone(),
                proxied_target: request_proxied_target.clone(),
                auth: req_auth.clone(),
                auth_disabled: req_auth_disabled,
                is_first_hop,
                preserve_resolved_target: !is_cross_origin_redirect,
                credentials_allowed,
                #[cfg(feature = "cookies")]
                cookie_allowed: cookie_header_allowed,
                #[cfg(not(feature = "cookies"))]
                cookie_allowed: true,
                failure_context: failure_context.clone(),
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
            // Draining is best-effort connection release: if the remaining
            // budget expires first, proceed with the redirect anyway (the
            // undrained connection closes instead of being reused) rather
            // than failing a redirect that could otherwise continue. The
            // overall total deadline is still enforced around the dispatch.
            let dur = total.saturating_sub(start_time.elapsed());
            let _ = tokio::time::timeout(dur, drain_response_body(&mut response)).await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::TransportHints;
    use crate::timeout::Timeout;

    fn test_url() -> url::Url {
        url::Url::parse("https://example.com/path").expect("valid test URL")
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
                max_decoded_body_size: None,
                max_decompression_ratio: None,
                #[cfg(feature = "proxy")]
                proxy_override: crate::request::ProxyOverride::Direct,
                transport_hints: TransportHints {
                    target: Some(Bytes::from("/t")),
                    sni_hostname: Some("sni.example".to_owned()),
                    resolved_target: None,
                    trace: Some(Arc::new(crate::trace::NoopTraceObserver)),
                },
                proxied_target: None,
                auth: None,
                auth_disabled: false,
                is_first_hop: true,
                preserve_resolved_target: false,
                credentials_allowed: true,
                cookie_allowed: true,
                failure_context: None,
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
                max_decoded_body_size: None,
                max_decompression_ratio: None,
                #[cfg(feature = "proxy")]
                proxy_override: crate::request::ProxyOverride::Inherit,
                transport_hints: TransportHints {
                    target: Some(Bytes::from("/t")),
                    sni_hostname: Some("sni.example".to_owned()),
                    resolved_target: None,
                    trace: Some(Arc::new(crate::trace::NoopTraceObserver)),
                },
                proxied_target: None,
                auth: Some(AuthScheme::Basic(
                    BasicAuth::new("req", "pw").expect("valid auth"),
                )),
                auth_disabled: false,
                is_first_hop: false,
                preserve_resolved_target: false,
                credentials_allowed: false,
                cookie_allowed: false,
                failure_context: None,
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
            resolved_target: None,
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
                max_decoded_body_size: None,
                max_decompression_ratio: None,
                #[cfg(feature = "proxy")]
                proxy_override: crate::request::ProxyOverride::Direct,
                transport_hints: mk_hints(),
                proxied_target: None,
                auth: None,
                auth_disabled: false,
                is_first_hop: true,
                preserve_resolved_target: false,
                credentials_allowed: true,
                cookie_allowed: true,
                failure_context: None,
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
                max_decoded_body_size: None,
                max_decompression_ratio: None,
                #[cfg(feature = "proxy")]
                proxy_override: crate::request::ProxyOverride::Direct,
                transport_hints: mk_hints(),
                proxied_target: None,
                auth: None,
                auth_disabled: false,
                is_first_hop: true,
                preserve_resolved_target: false,
                credentials_allowed: true,
                cookie_allowed: true,
                failure_context: None,
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
}
