//! Request preparation: header/body/version/limits normalization and pool
//! admission without transport I/O.
//!
//! Preparation owns effective decompression/body limits, proxy resolution
//! decisions, resolved-target compatibility checks, logical pool-key
//! construction/acquisition, write-timeout body wrapping, content length,
//! user-agent/accept-encoding, H2 forbidden-header policy, request
//! target/size validation, and remaining-total/deadline computation. It must
//! not perform transport I/O.

use std::sync::Arc;
use std::time::Duration;

use crate::body::RequestBody;
use crate::client::ClientInner;
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::pool::{OriginKey, PoolGuard};
#[cfg(feature = "proxy")]
use crate::proxy::{Proxy, ProxyConfig};
#[cfg(all(feature = "high-level-url", feature = "proxy"))]
use crate::request::ProxyOverride;
#[cfg(feature = "high-level-url")]
use crate::request::Request;
use crate::stream::write_timeout_stream;
use crate::timeout::{Timeout, TimeoutPhase};

/// Transport-ready state after request policy and body/header normalization.
///
/// Built once in the preparation phase. Transport dispatch then selects an
/// execution path without reimplementing content-length, version, or header
/// policy. The body is owned so one-shot streams are moved, never cloned,
/// to satisfy the abstraction.
///
/// High-level only: the native frame API bypasses URL policy and pool-key
/// construction goes through [`crate::http_origin::HttpOrigin`] directly.
#[cfg(feature = "high-level-url")]
pub(super) struct PreparedRequest {
    pub(super) method: http::Method,
    pub(super) url: url::Url,
    pub(super) uri: http::Uri,
    pub(super) headers: Headers,
    pub(super) body: RequestBody,
    pub(super) version: http::Version,
    pub(super) transport_hints: crate::transport_hints::TransportHints,
    pub(super) proxied_target: Option<crate::transport_hints::ResolvedTarget>,
    #[cfg(feature = "proxy")]
    pub(super) effective_proxy: Option<ProxyConfig>,
    pub(super) decompression_enabled: bool,
    pub(super) max_decoded_body_size: Option<usize>,
    pub(super) max_decompression_ratio: Option<f64>,
    pub(super) timeout: Timeout,
    pub(super) remaining_total: Option<Duration>,
    pub(super) deadline: Option<std::time::Instant>,
    pub(super) failure_context: Option<Arc<crate::error::RequestFailureContext>>,
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

/// Resolve the effective proxy configuration for a request.
///
/// Applies the tri-state override model:
/// - `Inherit`: use client-level proxy
/// - `Direct`: direct, no proxy
/// - `Override(config)`: use request-level proxy
#[cfg(feature = "proxy")]
pub(super) fn resolve_proxy(
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
                .find(|p| p.no_proxy_rules().is_none_or(|np| !np.should_bypass(url)))
                .map(Proxy::config)
        }
    }
}

/// Whether the built-in proxy routes would apply to a native origin.
///
/// Mirrors [`resolve_proxy`] with `Inherit` but uses native
/// scheme/host/explicit-port components so the native frame API never
/// reparses its URI through `url::Url`. Shares the exact
/// `should_use_for_scheme` + `should_bypass_components` evaluation with the
/// high-level path.
#[cfg(feature = "proxy")]
pub(super) fn native_would_use_proxy(
    inner: &ClientInner,
    origin: &crate::http_origin::HttpOrigin,
    explicit_port: Option<u16>,
) -> bool {
    let scheme = origin.scheme_str();
    let host = origin.host();
    inner
        .config
        .proxy
        .iter()
        .chain(inner.config.environment_proxies.iter())
        .filter(|p| p.should_use_for_scheme(scheme))
        .any(|p| {
            p.no_proxy_rules()
                .is_none_or(|np| !np.should_bypass_components(scheme, host, explicit_port))
        })
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
///
/// High-level only; the native path uses [`resolve_native_request_uri`].
#[cfg(feature = "high-level-url")]
pub(super) fn resolve_request_uri(
    url: &url::Url,
    transport_hints: &crate::transport_hints::TransportHints,
) -> Result<http::Uri> {
    let logical_uri: http::Uri = url
        .as_str()
        .parse()
        .map_err(|e| Error::InvalidUrl(format!("failed to convert url to URI: {e}")))?;
    if let Some(ref target) = transport_hints.target {
        validate_target(target)?;
        let target = std::str::from_utf8(target)
            .map_err(|_| Error::InvalidUrl("target extension is not valid UTF-8".into()))?;
        // Hyper's legacy client requires an absolute URI to select a
        // connector, then converts that URI to origin-form on the wire. Keep
        // the logical scheme/authority for connector routing and TLS while
        // replacing only the path-and-query portion with the caller's wire
        // target. This also preserves the `*` request target.
        let target_uri = target.parse::<http::Uri>().ok();
        let path_and_query = target_uri
            .as_ref()
            .and_then(http::Uri::path_and_query)
            .cloned()
            .or_else(|| target.parse().ok())
            .ok_or_else(|| Error::InvalidUrl("failed to convert target to URI".into()))?;
        let mut parts = logical_uri.into_parts();
        parts.path_and_query = Some(path_and_query);
        http::Uri::from_parts(parts)
            .map_err(|e| Error::InvalidUrl(format!("failed to convert target to URI: {e}")))
    } else {
        Ok(logical_uri)
    }
}

/// Build the native wire URI from an already-validated `http::Uri`.
///
/// Returns the original absolute URI when no `TransportHints::target`
/// override is present. When `target` is present, validates it with the
/// shared request-smuggling guard and replaces only `path_and_query` while
/// preserving the original scheme/authority used for connector routing and
/// TLS. Host-header policy is unchanged. Shares [`validate_target`] with
/// the high-level helper so security checks cannot diverge.
pub(super) fn resolve_native_request_uri(
    uri: http::Uri,
    transport_hints: &crate::transport_hints::TransportHints,
) -> Result<http::Uri> {
    if let Some(ref target) = transport_hints.target {
        validate_target(target)?;
        let target_str = std::str::from_utf8(target)
            .map_err(|_| Error::InvalidUrl("target extension is not valid UTF-8".into()))?;
        // Same wire-target logic as the high-level helper: Hyper's legacy
        // client requires an absolute URI for connector selection, then
        // converts to origin-form on the wire. Preserve `*` support.
        let target_uri = target_str.parse::<http::Uri>().ok();
        let path_and_query = target_uri
            .as_ref()
            .and_then(http::Uri::path_and_query)
            .cloned()
            .or_else(|| target_str.parse().ok())
            .ok_or_else(|| Error::InvalidUrl("failed to convert target to URI".into()))?;
        let mut parts = uri.into_parts();
        parts.path_and_query = Some(path_and_query);
        http::Uri::from_parts(parts)
            .map_err(|e| Error::InvalidUrl(format!("failed to convert target to URI: {e}")))
    } else {
        Ok(uri)
    }
}

/// Preparation phase for single-request dispatch: normalize headers, body,
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
///
/// High-level only; requires the `high-level-url` feature.
#[cfg(feature = "high-level-url")]
#[allow(
    clippy::too_many_lines,
    reason = "preparation centralizes header/body/version/proxy/pool policy in one place so transports do not reimplement it"
)]
pub(super) async fn prepare_single_request(
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
        #[cfg(feature = "redirects")]
            redirect: _request_redirect,
        auth: _,
        auth_disabled: _,
        decompress: request_decompress,
        proxy_override,
        #[cfg(feature = "logical-retry")]
            retry: _,
        transport_hints,
        proxied_target,
        failure_context,
        max_decoded_body_size: request_max_decoded_body_size,
        max_decompression_ratio: request_max_decompression_ratio,
    } = request.into_parts();

    #[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
    if inner.lifecycle.invalid {
        return Err(Error::RequestBuild(
            "physical connection policy requires max_live > 0 and admission_timeout only with max_live"
                .into(),
        ));
    }

    if let Some(target) = &transport_hints.resolved_target {
        let expected_port = url.port_or_known_default().ok_or_else(|| {
            Error::InvalidResolvedTarget(
                "resolved destinations require an HTTP or HTTPS URL".into(),
            )
        })?;
        if target
            .addresses()
            .iter()
            .any(|address| address.port() != expected_port)
        {
            return Err(Error::InvalidResolvedTarget(format!(
                "all resolved destinations must use the URL's effective port {expected_port}"
            )));
        }
    }

    if let Some(target) = &proxied_target {
        let expected_port = url.port_or_known_default().ok_or_else(|| {
            Error::InvalidResolvedTarget("proxied destinations require an HTTP or HTTPS URL".into())
        })?;
        if target
            .addresses()
            .iter()
            .any(|address| address.port() != expected_port)
        {
            return Err(Error::InvalidResolvedTarget(format!(
                "all proxied destinations must use the URL's effective port {expected_port}"
            )));
        }
    }

    #[cfg(feature = "tls-rustls")]
    if url.scheme() == "https" {
        if let Some(error) = &inner.tls_config_error {
            return Err(Error::Tls(error.clone()));
        }
    }

    let decompression_enabled = request_decompress.unwrap_or(inner.config.automatic_decompression);
    let max_decoded_body_size =
        request_max_decoded_body_size.or(inner.config.max_decoded_body_size);
    let max_decompression_ratio =
        request_max_decompression_ratio.or(inner.config.max_decompression_ratio);

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

    if proxied_target.is_some() {
        #[cfg(feature = "proxy")]
        {
            let proxy = effective_proxy.as_ref().ok_or_else(|| {
                Error::Unsupported(
                    "caller-supplied proxied destinations require proxy routing".into(),
                )
            })?;
            if proxy.socks_remote_dns() {
                return Err(Error::Unsupported(
                    "proxied destinations are incompatible with SOCKS5H remote DNS".into(),
                ));
            }
            if url.scheme() == "http" && !proxy.is_socks() {
                return Err(Error::Unsupported(
                    "proxied destinations are unsupported for plaintext HTTP forward proxies"
                        .into(),
                ));
            }
        }
        #[cfg(not(feature = "proxy"))]
        {
            return Err(Error::Unsupported(
                "caller-supplied proxied destinations require proxy support".into(),
            ));
        }
    }

    if transport_hints.resolved_target.is_some() {
        #[cfg(not(feature = "advanced-routing"))]
        {
            return Err(Error::Unsupported(
                "caller-supplied resolved destinations require the advanced-routing feature".into(),
            ));
        }
        #[cfg(feature = "proxy")]
        if effective_proxy.is_some() {
            return Err(Error::Unsupported(
                "caller-supplied resolved destinations require direct routing; disable the proxy explicitly".into(),
            ));
        }
        #[cfg(all(unix, feature = "advanced-routing"))]
        if inner.uds_client.is_some() {
            return Err(Error::Unsupported(
                "caller-supplied resolved destinations are incompatible with Unix-domain routing"
                    .into(),
            ));
        }
    }

    #[cfg(not(feature = "advanced-routing"))]
    if transport_hints.sni_hostname.is_some() {
        return Err(Error::Unsupported(
            "SNI override requires the advanced-routing feature".into(),
        ));
    }

    #[cfg(feature = "advanced-routing")]
    if inner.config.dialer.is_some() {
        if transport_hints.resolved_target.is_some() {
            return Err(Error::Unsupported(
                "custom dialing is incompatible with caller-supplied resolved destinations".into(),
            ));
        }
        if inner.direct_connector_config.is_some() {
            return Err(Error::Unsupported(
                "custom dialing is incompatible with local-address or socket-option routing".into(),
            ));
        }
        if inner.config.uds_configured {
            return Err(Error::Unsupported(
                "custom dialing is incompatible with Unix-domain routing".into(),
            ));
        }
        #[cfg(feature = "http3")]
        if matches!(
            inner.config.http_version_policy,
            crate::HttpVersionPolicy::Http3Only
        ) {
            return Err(Error::Unsupported(
                "custom dialing is incompatible with HTTP/3".into(),
            ));
        }
    }

    let origin = if inner.pool.needs_origin_key() {
        #[cfg(feature = "proxy")]
        {
            match effective_proxy {
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
            }
        }
        #[cfg(not(feature = "proxy"))]
        {
            OriginKey::from_url(url.scheme(), &url)
        }
    } else {
        None
    };

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
    headers.validate_request_size_uri(&method, &request_uri)?;

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
        proxied_target,
        #[cfg(feature = "proxy")]
        effective_proxy,
        decompression_enabled,
        max_decoded_body_size,
        max_decompression_ratio,
        timeout: *timeout,
        remaining_total,
        deadline,
        failure_context,
    };
    Ok((prepared, guard))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "high-level-url")]
    #[test]
    fn target_override_keeps_logical_authority_for_routing() {
        // The wire URI carries the caller's path while the logical URL
        // remains authoritative for routing/TLS/Host decisions upstream.
        let url = url::Url::parse("https://example.com/original").expect("valid URL");
        let hints = crate::transport_hints::TransportHints {
            target: Some(bytes::Bytes::from("/wire-path?q=1")),
            ..Default::default()
        };
        let uri = resolve_request_uri(&url, &hints).expect("target resolves");
        assert_eq!(
            uri.path_and_query().map(http::uri::PathAndQuery::as_str),
            Some("/wire-path?q=1")
        );
        assert_eq!(uri.host(), Some("example.com"));
    }

    #[test]
    fn native_target_override_keeps_logical_authority() {
        let uri: http::Uri = "https://example.com/original".parse().expect("valid URI");
        let hints = crate::transport_hints::TransportHints {
            target: Some(bytes::Bytes::from("/wire-path?q=1")),
            ..Default::default()
        };
        let resolved = resolve_native_request_uri(uri.clone(), &hints).expect("target resolves");
        assert_eq!(
            resolved
                .path_and_query()
                .map(http::uri::PathAndQuery::as_str),
            Some("/wire-path?q=1")
        );
        assert_eq!(resolved.host(), Some("example.com"));
        // No override returns the original URI unchanged.
        let plain = crate::transport_hints::TransportHints::default();
        assert_eq!(
            resolve_native_request_uri(uri.clone(), &plain).unwrap(),
            uri
        );
    }

    #[test]
    fn native_target_override_preserves_star_and_error_classification() {
        let uri: http::Uri = "https://example.com/original".parse().expect("valid URI");
        let star = crate::transport_hints::TransportHints {
            target: Some(bytes::Bytes::from_static(b"*")),
            ..Default::default()
        };
        let resolved = resolve_native_request_uri(uri.clone(), &star).expect("star resolves");
        assert_eq!(
            resolved
                .path_and_query()
                .map(http::uri::PathAndQuery::as_str),
            Some("*")
        );

        let invalid_utf8 = crate::transport_hints::TransportHints {
            target: Some(bytes::Bytes::from_static(b"/\xff")),
            ..Default::default()
        };
        assert!(matches!(
            resolve_native_request_uri(uri, &invalid_utf8),
            Err(Error::InvalidUrl(message)) if message.contains("not valid UTF-8")
        ));
    }

    #[test]
    fn validate_target_rejects_smuggling_vectors() {
        assert!(validate_target(b"/ok-path").is_ok());
        assert!(validate_target(b"").is_err());
        assert!(validate_target(b"/bad\r\n").is_err());
        assert!(validate_target(b"   ").is_err());
        assert!(validate_target(b" /leading").is_err());
    }
}
