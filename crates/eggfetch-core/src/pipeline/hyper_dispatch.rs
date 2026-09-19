//! Ordinary Hyper dispatch: direct, custom-dialer, UDS, and SNI routes.
//!
//! Owns direct/custom/UDS/SNI ordinary H1/H2 request building and send
//! calls. Hyper request scaffolding is built once via `build_hyper_request`
//! so branches do not rebuild it. No branch duplicates common response
//! finalization; the caller applies the shared post-transport policy.

use std::time::Duration;

#[cfg(feature = "high-level-url")]
use super::prepare::resolve_request_uri;
use crate::body::RequestBody;
use crate::client::ClientInner;
use crate::error::{Error, Result};
use crate::headers::Headers;
#[cfg(feature = "high-level-url")]
use crate::response::Response;
use crate::transport_hints::TransportHints;

/// Build a Hyper request from prepared parts.
///
/// Shared by the UDS, custom-dialer, specialized-direct, SNI-direct, and
/// standard Hyper paths so `http::Request` scaffolding is not rebuilt in each
/// branch.
///
/// # Errors
///
/// Returns [`Error::RequestBuild`] if the Hyper request cannot be built.
pub(super) fn build_hyper_request(
    method: &http::Method,
    uri: http::Uri,
    version: http::Version,
    headers: &Headers,
    body: RequestBody,
) -> Result<http::Request<crate::transport::HyperRequestBody>> {
    build_http_request(method, uri, version, headers, body.into_http_body())
}

pub(super) fn build_http_request<B>(
    method: &http::Method,
    uri: http::Uri,
    version: http::Version,
    headers: &Headers,
    body: B,
) -> Result<http::Request<B>> {
    let mut builder = http::Request::builder()
        .method(method)
        .uri(uri)
        .version(version);
    for (name, value) in headers.iter() {
        builder = builder.header(name, value);
    }
    builder
        .body(body)
        .map_err(|e| Error::RequestBuild(e.to_string()))
}

/// UDS route: configured Unix-domain-socket client.
#[allow(clippy::too_many_arguments)]
#[cfg(all(feature = "high-level-url", feature = "advanced-routing"))]
pub(super) async fn send_uds_route(
    inner: &ClientInner,
    method: &http::Method,
    uri: http::Uri,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    url: url::Url,
    transport_hints: &TransportHints,
    remaining_total: Option<Duration>,
) -> Result<Response> {
    #[cfg(all(unix, feature = "advanced-routing"))]
    {
        let uds_client = inner
            .uds_client
            .as_ref()
            .ok_or_else(|| Error::Unsupported("UDS client not available".into()))?;
        let hyper_request = build_hyper_request(method, uri, version, headers, body)?;
        let send_future = crate::transport::uds::send_request(
            uds_client,
            hyper_request,
            url,
            transport_hints.trace.as_deref(),
        );
        super::send_with_total_timeout(send_future, remaining_total).await
    }
    #[cfg(not(unix))]
    {
        let _ = (
            inner,
            method,
            uri,
            headers,
            body,
            version,
            url,
            transport_hints,
            remaining_total,
        );
        Err(Error::Unsupported(
            "Unix domain sockets are not supported on this platform".into(),
        ))
    }
}

/// Custom-dialer route: caller-supplied raw-stream client, with per-request
/// SNI override support.
#[allow(clippy::too_many_arguments)]
#[cfg(all(feature = "high-level-url", feature = "advanced-routing"))]
pub(super) async fn send_custom_route(
    inner: &ClientInner,
    method: &http::Method,
    uri: http::Uri,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    url: url::Url,
    transport_hints: &TransportHints,
    failure_context: Option<&crate::error::RequestFailureContext>,
    remaining_total: Option<Duration>,
) -> Result<Response> {
    let custom_client = if let Some(sni_hostname) = transport_hints.sni_hostname.as_deref() {
        inner.sni_custom_client(sni_hostname).await?
    } else {
        inner
            .custom_client
            .as_ref()
            .ok_or_else(|| Error::Unsupported("custom dialer client not available".into()))?
            .clone()
    };
    let hyper_request = build_hyper_request(method, uri, version, headers, body)?;
    let send_future = crate::transport::direct::send_request(
        &custom_client,
        hyper_request,
        url,
        transport_hints.trace.as_deref(),
        failure_context,
    );
    super::send_with_total_timeout(send_future, remaining_total).await
}

/// Specialized-direct route: static resolved destination or configured
/// direct connector (socket options / local address).
#[allow(clippy::too_many_arguments)]
#[cfg(all(feature = "high-level-url", feature = "advanced-routing"))]
pub(super) async fn send_direct_route(
    inner: &ClientInner,
    method: &http::Method,
    uri: http::Uri,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    url: url::Url,
    transport_hints: &TransportHints,
    failure_context: Option<&crate::error::RequestFailureContext>,
    remaining_total: Option<Duration>,
) -> Result<Response> {
    let resolved_client;
    let direct_client = if let Some(target) = transport_hints.resolved_target.as_ref() {
        let http_origin = crate::http_origin::HttpOrigin::from_url(&url).ok_or_else(|| {
            Error::InvalidResolvedTarget(
                "resolved destinations require an HTTP or HTTPS URL".into(),
            )
        })?;
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
    let hyper_request = build_hyper_request(method, uri, version, headers, body)?;
    let send_future = crate::transport::direct::send_direct_request(
        direct_client,
        hyper_request,
        url,
        transport_hints.trace.as_deref(),
        failure_context,
    );
    super::send_with_total_timeout(send_future, remaining_total).await
}

/// SNI-direct route: cached override client separating DNS/TCP from TLS.
#[allow(clippy::too_many_arguments)]
#[cfg(all(feature = "high-level-url", feature = "advanced-routing"))]
pub(super) async fn send_sni_route(
    inner: &ClientInner,
    method: &http::Method,
    uri: http::Uri,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    url: url::Url,
    transport_hints: &TransportHints,
    failure_context: Option<&crate::error::RequestFailureContext>,
    remaining_total: Option<Duration>,
) -> Result<Response> {
    // SNI override separates DNS/TCP (to the original host) from TLS
    // (with the override hostname).
    let sni_hostname = transport_hints
        .sni_hostname
        .clone()
        .ok_or_else(|| Error::RequestBuild("SNI route selected without sni_hostname".into()))?;
    let sni_client = inner.sni_client(&sni_hostname).await?;
    let hyper_request = build_hyper_request(method, uri, version, headers, body)?;
    let send_future = crate::transport::direct::send_direct_request(
        &sni_client,
        hyper_request,
        url,
        transport_hints.trace.as_deref(),
        failure_context,
    );
    super::send_with_total_timeout(send_future, remaining_total).await
}

/// Send a request through the hyper/HTTP-1.1/2 transport.
///
/// Shared standard-path helper; UDS, specialized-direct, and SNI-direct
/// paths build their Hyper requests through [`build_hyper_request`] and call
/// their respective `transport::direct`/`transport::uds` converters, so this
/// helper exists only to keep the standard Hyper dispatch explicit.
#[allow(clippy::too_many_arguments)] // Keeps the direct-send helper explicit; timeout wrapping is the only added phase input.
#[cfg(feature = "high-level-url")]
pub(super) async fn send_hyper_request(
    inner: &ClientInner,
    method: &http::Method,
    url: url::Url,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    remaining_total: Option<Duration>,
    transport_hints: &TransportHints,
    failure_context: Option<&crate::error::RequestFailureContext>,
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
        failure_context,
    );

    super::send_with_total_timeout(send_future, remaining_total).await
}

#[cfg(test)]
mod tests {
    use super::*;

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
