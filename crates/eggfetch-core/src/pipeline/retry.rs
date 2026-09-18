//! Retry policy: retry loop, backoff delay, and bounded discard drain.
//!
//! Retries restart the complete logical request (including redirects) under
//! the original total deadline. Stream bodies are never retried. Backoff,
//! `Retry-After`, replayability, retryable status/error classification, and
//! total-deadline behavior live here and must not change in a pure
//! decomposition.

use std::time::Duration;

use crate::client::Client;
use crate::error::{Error, Result};
use crate::request::Request;
use crate::response::Response;
use crate::retry::{should_retry, RetryCause, RetryPolicy};
use crate::timeout::TimeoutPhase;

use super::drain_response_body;

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
/// client default, then wraps the redirect loop in a retry loop with
/// exponential backoff.
///
/// Retries restart the complete logical request (including redirects)
/// under the original total deadline. Stream bodies are never retried.
///
/// Retry eligibility is intentionally original-method-scoped: `should_retry`
/// is evaluated with the logical (pre-redirect) method and replayable body,
/// not the post-redirect wire method. A `POST → 301 → GET → 503` therefore
/// does not retry under a `GET`-only policy, because retrying would replay
/// the original non-idempotent `POST`.
/// Send one logical attempt through the inner policy layer.
///
/// When the `redirects` feature is enabled this is the redirect loop;
/// otherwise it is the lean single-hop dispatch. The retry loop itself is
/// unchanged: it restarts the complete logical request under the original
/// total deadline.
async fn dispatch_attempt(client: &Client, request: Request) -> Result<Response> {
    #[cfg(feature = "redirects")]
    {
        Box::pin(super::redirect::send_with_redirects(client, request)).await
    }
    #[cfg(not(feature = "redirects"))]
    {
        Box::pin(super::lean::send_lean(client, request)).await
    }
}

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
            return dispatch_attempt(client, request).await;
        }
    };

    // If the body is not replayable, we can only attempt once.
    if !body_replayable {
        return dispatch_attempt(client, request).await;
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
        if let Some(context) = saved.failure_context.as_deref() {
            context.clear();
        }

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

        let result = dispatch_attempt(client, attempt_request).await;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::RequestBody;
    use crate::request::{RequestParts, ResolvedTarget, TransportHints};
    use crate::timeout::Timeout;
    use bytes::Bytes;
    use std::sync::Arc;

    fn test_url() -> url::Url {
        url::Url::parse("https://example.com/path").expect("valid test URL")
    }

    fn full_request() -> Request {
        #[cfg(not(feature = "basic-auth"))]
        use crate::auth::AuthScheme;
        #[cfg(feature = "basic-auth")]
        use crate::auth::{AuthScheme, BasicAuth};
        #[cfg(feature = "redirects")]
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
        #[cfg(feature = "redirects")]
        req.set_redirect(Some(RedirectPolicy::new(true, 7)));
        #[cfg(feature = "basic-auth")]
        req.set_auth(Some(AuthScheme::Basic(
            BasicAuth::new("user", "pass").expect("valid auth"),
        )));
        #[cfg(not(feature = "basic-auth"))]
        req.set_auth(Some(AuthScheme::bearer("tok").expect("valid auth")));
        req.set_auth_disabled(false);
        req.set_decompress(Some(false));
        req.set_max_decoded_body_size(Some(1024));
        req.set_max_decompression_ratio(Some(12.5));
        #[cfg(feature = "proxy")]
        req.set_proxy_override(crate::request::ProxyOverride::Direct);
        req.set_retry(Some(RetryPolicy::default()));
        req.set_transport_hints(TransportHints {
            target: Some(Bytes::from("/override-target")),
            sni_hostname: Some("sni.example".to_owned()),
            resolved_target: None,
            trace: Some(Arc::new(crate::trace::NoopTraceObserver)),
        });
        req.set_proxied_target(Some(
            ResolvedTarget::new(["192.0.2.10:443".parse().expect("valid address")])
                .expect("non-empty target"),
        ));
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
        #[cfg(feature = "redirects")]
        let expected_redirect = req.redirect().cloned();
        let expected_auth = req.auth().cloned();
        let expected_auth_disabled = req.is_auth_disabled();
        let expected_decompress = req.decompress();
        let expected_max_decoded_body_size = req.max_decoded_body_size();
        let expected_max_decompression_ratio = req.max_decompression_ratio();
        let expected_retry = req.retry().cloned();
        let expected_target = req.transport_hints().target.clone();
        let expected_sni = req.transport_hints().sni_hostname.clone();
        let expected_has_trace = req.transport_hints().trace.is_some();
        let expected_proxied_target = req.proxied_target().cloned();

        let parts = req.into_parts();
        // Touch every field so wildcard destructuring cannot silently pass.
        let RequestParts {
            method: _,
            url: _,
            headers: _,
            body: _,
            version: _,
            timeout: _,
            #[cfg(feature = "redirects")]
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
        #[cfg(feature = "redirects")]
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
        assert_eq!(
            rebuilt.max_decoded_body_size(),
            expected_max_decoded_body_size
        );
        assert_eq!(
            rebuilt.max_decompression_ratio(),
            expected_max_decompression_ratio
        );
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
        assert_eq!(rebuilt.proxied_target(), expected_proxied_target.as_ref());
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
        #[cfg(feature = "redirects")]
        assert!(attempt.redirect().is_some());
        assert!(attempt.auth().is_some());
        assert!(!attempt.is_auth_disabled());
        assert_eq!(attempt.decompress(), Some(false));
        assert_eq!(attempt.max_decoded_body_size(), Some(1024));
        assert_eq!(attempt.max_decompression_ratio(), Some(12.5));
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
        let expected_proxied_target =
            ResolvedTarget::new(["192.0.2.10:443".parse().expect("valid address")])
                .expect("non-empty target");
        assert_eq!(attempt.proxied_target(), Some(&expected_proxied_target));
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
}
