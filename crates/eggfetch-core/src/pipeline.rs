//! Request pipeline: redirect loop, defaults, cookie/auth application,
//! deadline propagation, and body-lease lifecycle.

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
use crate::stream::{read_timeout_stream, write_timeout_stream};
use crate::timeout::{Timeout, TimeoutPhase};
#[cfg(feature = "proxy")]
use crate::transport::proxy::send_proxy_request;

/// Send a request through the client, following redirects if enabled.
///
/// This is the top-level entry point for the request pipeline. It
/// handles header merging, timeout computation, cookie injection,
/// authentication, the redirect loop, and response post-processing.
#[allow(clippy::too_many_lines)]
pub(crate) async fn send_with_redirects(client: &Client, request: Request) -> Result<Response> {
    let (
        method,
        url,
        request_headers,
        body,
        version,
        request_timeout,
        request_redirect,
        req_auth,
        req_auth_disabled,
        _request_decompress,
        _request_proxy,
    ) = request.into_parts();

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

    // Fast path: redirects disabled — send directly without buffering.
    if !effective_redirect.follow {
        let mut request = Request::new(method, url);

        #[cfg(feature = "cookies")]
        let has_cookie_header = headers.contains("cookie");

        *request.headers_mut() = headers;
        request.set_body(body);
        request.set_version(version);
        request.set_timeout(Some(timeout));

        {
            request.set_auth(req_auth);
            request.set_auth_disabled(req_auth_disabled);
        }

        #[cfg(feature = "cookies")]
        if !has_cookie_header {
            if let Some(cookie_header) = client.config().cookie_jar.cookies_for_url(request.url()) {
                request.headers_mut().insert("cookie", &cookie_header)?;
            }
        }

        {
            let effective_auth = crate::auth::resolve_request_auth(
                request.auth(),
                request.is_auth_disabled(),
                client.config().auth.as_ref(),
                request.headers(),
            )?;
            if let Some(auth) = effective_auth {
                auth.apply(request.headers_mut())?;
            }
        }

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
    let mut cookie_header_allowed = cur_headers.contains("cookie");

    let mut prev_url: Option<url::Url> = None;
    let mut credentials_allowed = true;

    loop {
        let hop_timeout = if let Some(total_dur) = timeout.total {
            let elapsed = start_time.elapsed();
            if elapsed >= total_dur {
                return Err(Error::Timeout {
                    phase: TimeoutPhase::Total,
                    elapsed: total_dur,
                });
            }
            let remaining = total_dur.saturating_sub(elapsed);
            let mut hop = timeout;
            hop.total = Some(remaining);
            hop
        } else {
            timeout
        };

        let mut hop_request = Request::new(cur_method.clone(), cur_url.clone());
        *hop_request.headers_mut() = cur_headers.clone();
        hop_request.set_body(cur_body);
        hop_request.set_version(cur_version);
        hop_request.set_timeout(Some(hop_timeout));

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

        hop_request.set_auth(if credentials_allowed {
            req_auth.clone()
        } else {
            None
        });
        hop_request.set_auth_disabled(req_auth_disabled);

        #[cfg(feature = "cookies")]
        {
            if !cookie_header_allowed {
                hop_request.headers_mut().remove("cookie");
            }
            if !cookie_header_allowed && !hop_request.headers().contains("cookie") {
                if let Some(cookie_header) = client
                    .config()
                    .cookie_jar
                    .cookies_for_url(hop_request.url())
                {
                    hop_request.headers_mut().insert("cookie", &cookie_header)?;
                }
            }
        }

        {
            let effective_auth = crate::auth::resolve_request_auth(
                hop_request.auth(),
                hop_request.is_auth_disabled(),
                if credentials_allowed {
                    client.config().auth.as_ref()
                } else {
                    None
                },
                hop_request.headers(),
            )?;
            if let Some(auth) = effective_auth {
                auth.apply(hop_request.headers_mut())?;
            }
        }

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
            let dur = total.saturating_sub(start_time.elapsed());
            if tokio::time::timeout(dur, response.bytes()).await.is_err() {
                return Err(Error::Timeout {
                    phase: TimeoutPhase::Total,
                    elapsed: dur,
                });
            }
        } else {
            let _ = response.bytes().await;
        }

        redirect_count += 1;
        if redirect_count > effective_redirect.max_redirects {
            return Err(Error::TooManyRedirects {
                followed: redirect_count - 1,
                max: effective_redirect.max_redirects,
            });
        }

        let redirect_status = response.status();
        let drop_body = redirect::drops_body_on_redirect(redirect_status, &cur_method);
        if drop_body && replay_body.is_none() {
            replay_body = Some(Bytes::new());
        }
        if !drop_body && replay_body.is_none() {
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

        let (redirect_req, _) =
            redirect::build_redirect_request(&temp_request, redirect_status, &location)?;

        history.push(HistoryEntry::from_response(&response));

        let new_method = redirect::redirect_method(redirect_status, &cur_method);

        let (_, new_url, new_headers, new_body, new_version, _, _, _, _, _, _) =
            redirect_req.into_parts();

        prev_url = Some(cur_url.clone());

        cur_method = new_method;
        cur_url = new_url;
        cur_headers = new_headers;
        cur_body = new_body;
        cur_version = new_version;
    }
}

/// Apply `Content-Length` header to known-size request bodies when the
/// user has not provided one. For known-size bodies with a user-supplied
/// `Content-Length`, reject mismatches.
pub(crate) fn apply_content_length(headers: Headers, body: &RequestBody) -> Result<Headers> {
    let known = match body {
        RequestBody::Empty => Some(0u64),
        RequestBody::Bytes(b) => Some(b.len() as u64),
        RequestBody::Stream {
            length: Some(n), ..
        } => Some(*n as u64),
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
        ProxyOverride::Inherit => inner
            .config
            .proxy
            .as_ref()
            .filter(|p| p.should_use_for_scheme(url.scheme()))
            .filter(|p| p.no_proxy_rules().map_or(true, |np| !np.should_bypass(url)))
            .map(Proxy::config),
    }
}

/// Send a single HTTP request and return the streaming response.
///
/// This handles pool acquisition, timeout application, and body
/// processing for one request/response cycle. It does NOT handle
/// redirects—that is the responsibility of [`send_with_redirects`].
#[allow(clippy::too_many_lines)]
pub(crate) async fn send_single_request(
    inner: &ClientInner,
    request: Request,
    timeout: &Timeout,
) -> Result<Response> {
    let (
        method,
        url,
        headers,
        body,
        version,
        _request_timeout,
        _request_redirect,
        _,
        _,
        request_decompress,
        proxy_override,
    ) = request.into_parts();

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
    #[cfg(not(feature = "proxy"))]
    let effective_proxy: Option<()> = None;

    #[cfg(feature = "proxy")]
    let origin = match effective_proxy {
        Some(ref proxy_config) => {
            let is_tunnel = url.scheme() == "https";
            OriginKey::from_url_with_proxy(
                url.scheme(),
                &url,
                proxy_config.host(),
                Some(proxy_config.port()),
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
                Ok(guard) => guard,
                Err(_) => {
                    return Err(Error::Timeout {
                        phase,
                        elapsed: duration,
                    })
                }
            }
        }
        None => inner.pool.acquire(origin.as_ref()).await,
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

    let remaining_total = timeout
        .total
        .map(|total| total.saturating_sub(started.elapsed()));

    let response = match effective_proxy {
        #[cfg(feature = "proxy")]
        Some(ref proxy_config) => {
            if headers.contains("proxy-authorization") && proxy_config.auth().is_some() {
                return Err(Error::ConflictingAuth(
                    "conflict: both request Proxy-Authorization header and proxy auth are configured; remove one".into(),
                ));
            }
            send_proxy_request(
                &url,
                &method,
                &headers,
                body,
                version,
                proxy_config,
                &crate::transport::proxy::ProxyRequestContext {
                    remaining_total,
                    tls_config: inner.config.tls_config.as_ref(),
                },
            )
            .await?
        }
        _ => {
            let uri: http::Uri = url
                .as_str()
                .parse()
                .map_err(|e| Error::InvalidUrl(format!("failed to convert url to URI: {e}")))?;

            let mut http_request = http::Request::builder()
                .method(method)
                .uri(uri)
                .version(version);

            for (name, value) in headers.iter() {
                http_request = http_request.header(name, value);
            }

            let hyper_request = http_request
                .body(body.into_http_body())
                .map_err(|e| Error::RequestBuild(e.to_string()))?;

            let send_future = crate::transport::direct::send_request(
                &inner.hyper_client,
                hyper_request,
                url.clone(),
            );

            match remaining_total {
                Some(dur) => match tokio::time::timeout(dur, send_future).await {
                    Ok(Ok(resp)) => resp,
                    Ok(Err(e)) => return Err(e),
                    Err(_) => {
                        return Err(Error::Timeout {
                            phase: TimeoutPhase::Total,
                            elapsed: dur,
                        });
                    }
                },
                None => send_future.await?,
            }
        }
    };

    let mut response = response;

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

    apply_read_timeout_and_lease(&mut response, guard, timeout.read);

    Ok(response)
}
