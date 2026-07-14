//! Request pipeline: redirect loop, defaults, cookie/auth application,
//! deadline propagation, and body-lease lifecycle.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;

use crate::body::{RequestBody, ResponseBody};
use crate::client::Client;
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::pool::PoolGuard;
use crate::redirect;
use crate::request::Request;
use crate::response::{HistoryEntry, Response};
use crate::stream::read_timeout_stream;
use crate::timeout::TimeoutPhase;

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
