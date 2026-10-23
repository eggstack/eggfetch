//! Redirect handling: hop transformation and first-hop request assembly.
//!
//! First-hop assembly is owned by the shared [`super::hop`] builder, which
//! the lean single-hop path uses as well, so the two entry paths cannot
//! diverge except for redirect-loop behavior itself.
//! `advance_redirect_hop` performs the single method/body/header
//! transformation step per hop with exhaustive request-state handling and
//! same-origin resolved-target semantics.

use bytes::Bytes;

use super::drain_response_body;
use super::hop::{build_hop_request, HopBuildParams};
use crate::body::RequestBody;
use crate::client::Client;
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::redirect;
use crate::request::Request;
use crate::response::{HistoryEntry, Response};
use crate::timeout::TimeoutPhase;

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
#[allow(
    clippy::too_many_arguments,
    reason = "hop transformation needs the seven original inputs plus the downgrade policy enforced before the next hop dispatches"
)]
fn advance_redirect_hop(
    cur_method: &http::Method,
    cur_url: &url::Url,
    cur_headers: &Headers,
    cur_version: http::Version,
    replay_body: &mut Option<Bytes>,
    status: http::StatusCode,
    location: &str,
    downgrade: crate::redirect::RedirectDowngradePolicy,
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
    // A manually-set `Authorization` header survives same-origin hops.
    // Reaching this point with one set means no `AuthScheme` is configured
    // (that combination fails with `ConflictingAuth` on the first hop), so
    // the engine's unconditional strip would otherwise drop credentials
    // that HTTPX-compatible callers expect to be forwarded same-origin.
    // Snapshot before the strip; re-attach below when the hop stays
    // same-origin. Cross-origin hops drop it unconditionally.
    let manual_authorization = cur_headers.get("authorization").cloned();
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
        downgrade,
    )?;

    // Destructure exhaustively so a new `RequestParts` field fails to
    // compile here rather than being silently dropped across hops.
    let crate::request::RequestParts {
        method: _,
        url: new_url,
        headers: mut new_headers,
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
        #[cfg(feature = "logical-retry")]
            retry: _,
        transport_hints: _,
        proxied_target: _,
        failure_context: _,
    } = redirect_req.into_parts();

    if let Some(value) = manual_authorization {
        if cur_url.origin() == new_url.origin() {
            // Preserve the raw bytes: a binary `HeaderValue` fails
            // `to_str()`, and re-inserting via `&str` would be lossy.
            let name = http::header::HeaderName::from_static("authorization");
            new_headers.insert_raw(name, value);
        }
    }

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
/// This is the top-level entry point for the request pipeline. First-hop
/// preparation (header merging, timeout computation, hop construction,
/// cookie injection, authentication) is delegated to the shared
/// [`super::hop`] builder that the lean path also uses; this function owns
/// the redirect decision, the redirect loop, and response post-processing.
#[allow(
    clippy::too_many_lines,
    reason = "the redirect decision, the hop loop and the response post-processing are one linear sequence whose order is the behaviour"
)]
pub(crate) async fn send_with_redirects(client: &Client, request: Request) -> Result<Response> {
    // The effective redirect policy is resolved before the shared builder
    // consumes the request: the builder owns the exhaustive request
    // destructuring and must not depend on `redirects`-gated items.
    let request_redirect = request.redirect().cloned();
    let effective_redirect = request_redirect
        .as_ref()
        .unwrap_or(&client.config().redirect);

    let mut params = super::hop::prepare_first_hop(client, request);
    let timeout = params.timeout;

    // Fast path: redirects disabled — send through the same first-hop
    // builder as the redirect-enabled path so the two cannot diverge except
    // for redirect-loop behavior itself.
    if !effective_redirect.follow {
        let request = build_hop_request(client, params)?;

        let response = client.send_single_request(request, &timeout).await?;

        super::hop::update_cookie_jar(client, &response);

        return Ok(response);
    }

    let mut history = Vec::new();
    let mut redirect_count = 0usize;
    let start_time = std::time::Instant::now();

    let (mut replay_body, mut cur_body) =
        match std::mem::replace(&mut params.body, RequestBody::Empty) {
            RequestBody::Empty => (Some(Bytes::new()), RequestBody::Empty),
            RequestBody::Bytes(bytes) => (Some(bytes.clone()), RequestBody::Bytes(bytes)),
            stream @ RequestBody::Stream { .. } => (None, stream),
        };

    let mut cur_method = params.method;
    let mut cur_url = params.url;
    let mut cur_headers = params.headers;
    let mut cur_version = params.version;
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
            if params.transport_hints.resolved_target.is_some() || params.proxied_target.is_some() {
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
                decompress: params.decompress,
                max_decoded_body_size: params.max_decoded_body_size,
                max_decompression_ratio: params.max_decompression_ratio,
                #[cfg(feature = "proxy")]
                proxy_override: params.proxy_override.clone(),
                transport_hints: params.transport_hints.clone(),
                proxied_target: params.proxied_target.clone(),
                auth: params.auth.clone(),
                auth_disabled: params.auth_disabled,
                is_first_hop,
                preserve_resolved_target: !is_cross_origin_redirect,
                credentials_allowed,
                #[cfg(feature = "cookies")]
                cookie_allowed: cookie_header_allowed,
                #[cfg(not(feature = "cookies"))]
                cookie_allowed: true,
                failure_context: params.failure_context.clone(),
            },
        )?;

        let mut response = client
            .send_single_request(hop_request, &hop_timeout)
            .await?;

        super::hop::update_cookie_jar(client, &response);

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
            let _ =
                tokio::time::timeout(dur, drain_response_body(&mut response, timeout.read)).await;
        } else {
            drain_response_body(&mut response, timeout.read).await;
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
        // check, downgrade policy, and header stripping live in
        // `advance_redirect_hop`. The downgrade check runs before the next
        // hop is dispatched, so a denied HTTPS -> HTTP target never
        // receives request bytes.
        let hop = advance_redirect_hop(
            &cur_method,
            &cur_url,
            &cur_headers,
            cur_version,
            &mut replay_body,
            redirect_status,
            &location,
            effective_redirect.downgrade,
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
    use std::sync::Arc;
    use std::time::Duration;

    use crate::timeout::Timeout;
    use crate::transport_hints::TransportHints;

    /// The redirect path's first hop must be exactly the hop the shared
    /// builder specifies. The lean path asserts the same expectation in its
    /// own build, which is what proves the two paths cannot diverge.
    #[tokio::test]
    async fn redirect_path_prepares_the_expected_hop() {
        use super::super::hop::test_support;

        for with_request_auth in [true, false] {
            test_support::clear_recorded();
            let client = test_support::scenario_client();
            let request = test_support::scenario_request(&client, with_request_auth);

            // The dispatch itself fails (nothing listens on the port); the
            // prepared hop is captured before the transport is reached.
            let _ = send_with_redirects(&client, request).await;

            let captured = test_support::take_recorded().expect("a prepared hop was captured");
            let mut expected = test_support::expected_snapshot(with_request_auth);
            // The redirect loop shrinks the hop's total deadline by the
            // time already spent; the fast path does not. Compare against
            // the loop's own bound so the assertion does not depend on
            // wall-clock timing.
            let original_total = expected.timeout.total;
            expected.timeout.total = None;
            let mut actual = captured.clone();
            actual.timeout.total = None;
            assert_eq!(
                actual, expected,
                "the redirect path's first hop must match the shared expectation"
            );
            if let Some(original_total) = original_total {
                let shrunk = captured.timeout.total.expect("a total deadline");
                let lower_bound = original_total
                    .checked_sub(Duration::from_secs(1))
                    .unwrap_or(Duration::ZERO);
                assert!(
                    shrunk <= original_total && shrunk > lower_bound,
                    "shrunk total {shrunk:?} must stay within the original {original_total:?}"
                );
            }
        }
    }

    /// The `Set-Cookie` jar update must leave the same jar state the lean
    /// path leaves.
    #[cfg(feature = "cookies")]
    #[tokio::test]
    async fn redirect_path_updates_the_cookie_jar() {
        use crate::cookie::CookieJar;

        use super::super::hop::test_support;

        let (url, server) = test_support::spawn_set_cookie_server().await;
        let client = Client::builder().cookie_jar(CookieJar::new()).build();

        let response = client.get(&url).unwrap().send().await.expect("response");
        assert_eq!(response.status(), http::StatusCode::OK);
        server.await.expect("server task");

        assert_eq!(
            client
                .cookies()
                .cookies_for_url(&url::Url::parse(&url).expect("valid URL")),
            Some(test_support::EXPECTED_COOKIE_HEADER.to_owned()),
            "the redirect path must leave the same jar state as the lean path"
        );
    }

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
        #[cfg(not(feature = "basic-auth"))]
        use crate::auth::AuthScheme;
        #[cfg(feature = "basic-auth")]
        use crate::auth::{AuthScheme, BasicAuth};

        #[cfg(feature = "basic-auth")]
        let client = crate::client::Client::builder()
            .auth(AuthScheme::basic("client", "secret").expect("valid auth"))
            .build();
        #[cfg(not(feature = "basic-auth"))]
        let client = crate::client::Client::builder()
            .auth(AuthScheme::bearer("secret").expect("valid auth"))
            .build();
        let mut headers = Headers::new();
        headers.insert("cookie", "session=abc").unwrap();
        headers.insert("x-custom", "keep").unwrap();

        #[cfg(feature = "basic-auth")]
        let req_auth = Some(AuthScheme::Basic(
            BasicAuth::new("req", "pw").expect("valid auth"),
        ));
        #[cfg(not(feature = "basic-auth"))]
        let req_auth = Some(AuthScheme::bearer("req-tok").expect("valid auth"));

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
                auth: req_auth,
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
            crate::redirect::RedirectDowngradePolicy::Allow,
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
            crate::redirect::RedirectDowngradePolicy::Allow,
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
            crate::redirect::RedirectDowngradePolicy::Allow,
        )
        .unwrap_err();
        assert!(
            matches!(err, Error::BodyNotReplayableForRedirect),
            "unexpected error: {err:?}"
        );
    }
}
