//! Lean single-hop dispatch for profiles without redirect following.
//!
//! Used when the `redirects` feature is disabled (regardless of whether
//! `logical-retry` is enabled: the retry loop calls [`send_lean`] as its
//! inner attempt, and the client calls it directly when both policy
//! features are absent).
//!
//! The lean path dispatches once under the outer total deadline, returns
//! 3xx responses as ordinary responses without a second hop, constructs no
//! redirect history, and needs no cross-origin credential reconstruction.
//! Request timeout/body-limit/failure semantics are unchanged from the
//! full pipeline's first hop.

use crate::client::Client;
use crate::error::Result;
use crate::headers::Headers;
use crate::request::Request;
use crate::response::Response;

/// Send one request without retry or redirect orchestration.
///
/// Header merging, timeout computation, cookie injection, auth resolution,
/// and transport-hint handling match the full pipeline's first-hop policy
/// so the two entry paths cannot diverge except for policy-loop behavior
/// itself.
pub(crate) async fn send_lean(client: &Client, request: Request) -> Result<Response> {
    let crate::request::RequestParts {
        method,
        url,
        headers: request_headers,
        body,
        version,
        timeout: request_timeout,
        auth: req_auth,
        auth_disabled: req_auth_disabled,
        decompress: request_decompress,
        max_decoded_body_size: request_max_decoded_body_size,
        max_decompression_ratio: request_max_decompression_ratio,
        proxy_override: request_proxy,
        transport_hints: request_transport_hints,
        proxied_target: request_proxied_target,
        failure_context,
        #[cfg(feature = "redirects")]
            redirect: _,
        #[cfg(feature = "logical-retry")]
            retry: _,
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

    let mut hop = Request::new(method, url);
    *hop.headers_mut() = headers;
    hop.set_body(body);
    hop.set_version(version);
    hop.set_timeout(Some(timeout));
    hop.set_decompress(request_decompress);
    hop.set_max_decoded_body_size(request_max_decoded_body_size);
    hop.set_max_decompression_ratio(request_max_decompression_ratio);
    #[cfg(feature = "proxy")]
    hop.set_proxy_override(request_proxy);
    hop.set_transport_hints(request_transport_hints);
    hop.set_proxied_target(request_proxied_target);
    hop.set_auth(req_auth);
    hop.set_auth_disabled(req_auth_disabled);
    hop.set_failure_context(failure_context);

    #[cfg(feature = "cookies")]
    {
        if !hop.headers().contains("cookie") {
            if let Some(cookie_header) = client.config().cookie_jar.cookies_for_url(hop.url()) {
                hop.headers_mut().insert("cookie", &cookie_header)?;
            }
        }
    }

    {
        let effective_auth = crate::auth::resolve_request_auth(
            hop.auth(),
            hop.is_auth_disabled(),
            client.config().auth.as_ref(),
            hop.headers(),
        )?;
        if let Some(auth) = effective_auth {
            auth.apply(hop.headers_mut())?;
        }
    }

    let response = client.send_single_request(hop, &timeout).await?;

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

    Ok(response)
}
