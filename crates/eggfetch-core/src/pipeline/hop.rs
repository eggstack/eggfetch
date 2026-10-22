//! Shared hop preparation for the lean and redirect request paths.
//!
//! This module owns the single preparation path used by both entry points:
//! the exhaustive `Request::into_parts()` destructuring, the default/request
//! header merge, the client/request timeout merge, hop request construction,
//! cookie-jar injection, auth resolution, and the `Set-Cookie` jar update.
//!
//! The module is deliberately ungated with respect to `redirects`: the lean
//! single-hop path and the redirect loop differ only in what they do *after*
//! preparation, never in how a hop is prepared. Keeping the builder here
//! (rather than inside either entry path) is what makes that structural
//! instead of a convention.
//!
//! `retry_request` and `advance_redirect_hop` remain the only places a
//! *subsequent* hop request is constructed; this module owns the first hop.

use std::sync::Arc;

use crate::body::RequestBody;
use crate::client::Client;
use crate::error::Result;
use crate::headers::Headers;
use crate::request::Request;
use crate::response::Response;
use crate::timeout::Timeout;

/// Parameters for building one hop request.
///
/// This is the single policy path shared by the lean single-hop dispatch and
/// every iteration of the redirect-enabled loop. Cookie injection, auth
/// resolution, transport-hint handling, and credential scoping live here so
/// the two entry paths cannot diverge.
#[allow(
    clippy::struct_excessive_bools,
    reason = "hop policy is three orthogonal booleans (first-hop, credentials, cookies); an enum would obscure call sites"
)]
pub(super) struct HopBuildParams {
    pub(super) method: http::Method,
    pub(super) url: url::Url,
    pub(super) headers: Headers,
    pub(super) body: RequestBody,
    pub(super) version: http::Version,
    pub(super) timeout: Timeout,
    pub(super) decompress: Option<bool>,
    pub(super) max_decoded_body_size: Option<usize>,
    pub(super) max_decompression_ratio: Option<f64>,
    #[cfg(feature = "proxy")]
    pub(super) proxy_override: crate::request::ProxyOverride,
    pub(super) transport_hints: crate::transport_hints::TransportHints,
    pub(super) proxied_target: Option<crate::transport_hints::ResolvedTarget>,
    pub(super) auth: Option<crate::auth::AuthScheme>,
    pub(super) auth_disabled: bool,
    /// True only for the first hop of the logical request.
    pub(super) is_first_hop: bool,
    /// Preserve the request-scoped resolved destination on a same-origin hop.
    pub(super) preserve_resolved_target: bool,
    /// False after a cross-origin redirect; gates client-auth reapplication.
    pub(super) credentials_allowed: bool,
    /// False after a cross-origin redirect; gates cookie injection.
    pub(super) cookie_allowed: bool,
    pub(super) failure_context: Option<Arc<crate::error::RequestFailureContext>>,
}

/// Prepare the first hop of a logical request.
///
/// Performs the exhaustive [`Request::into_parts`] destructuring — so a new
/// request field fails to compile here rather than being silently dropped —
/// then the default/request header merge and the client/request timeout
/// merge, and returns the hop parameters for [`build_hop_request`].
///
/// The returned parameters carry first-hop policy: transport hints apply, and
/// credentials and cookies are allowed. The redirect loop overrides the
/// per-hop policy for later hops; the lean path uses them as returned.
///
/// The redirect policy override is intentionally *not* handled here: this
/// module must not depend on `redirects`-gated items, so the redirect entry
/// path resolves the effective policy from the request before handing it to
/// this builder.
pub(super) fn prepare_first_hop(client: &Client, request: Request) -> HopBuildParams {
    // Destructure exhaustively so new `RequestParts` fields fail to compile
    // here rather than being silently dropped before dispatch.
    let crate::request::RequestParts {
        method,
        url,
        headers: request_headers,
        body,
        version,
        timeout: request_timeout,
        #[cfg(feature = "redirects")]
            redirect: _,
        auth: req_auth,
        auth_disabled: req_auth_disabled,
        decompress: request_decompress,
        max_decoded_body_size: request_max_decoded_body_size,
        max_decompression_ratio: request_max_decompression_ratio,
        proxy_override: request_proxy,
        #[cfg(feature = "logical-retry")]
            retry: _,
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
        failure_context,
    }
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
/// - The effective auth is resolved and applied here so both the lean path
///   and the redirect loop observe identical header state on the wire.
///
/// # Errors
///
/// Returns an error if cookie insertion or auth application fails.
pub(super) fn build_hop_request(client: &Client, params: HopBuildParams) -> Result<Request> {
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
    // retained only when the redirect loop explicitly permits it. The SNI
    // override is retained together with the pin: keeping the IP while
    // dropping SNI would silently change the TLS identity mid-redirect.
    if is_first_hop {
        hop.set_transport_hints(transport_hints);
        hop.set_proxied_target(proxied_target);
    } else if preserve_resolved_target {
        let resolved_hints = crate::transport_hints::TransportHints {
            resolved_target: transport_hints.resolved_target,
            sni_hostname: transport_hints.sni_hostname,
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

    #[cfg(test)]
    record_prepared_hop(&hop);

    Ok(hop)
}

/// Update the client's cookie jar from a response's `Set-Cookie` headers.
///
/// Runs after the response is received and before it is returned to the
/// caller, so a caller observing the jar immediately after a response sees
/// the same state on every request path.
pub(super) fn update_cookie_jar(client: &Client, response: &Response) {
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
    #[cfg(not(feature = "cookies"))]
    let _ = (client, response);
}

/// Record the hop request this builder produced.
///
/// The capture happens after the hop's headers are set and after cookie
/// injection and auth application, so it observes the request exactly as it
/// is handed to the transport.
#[cfg(test)]
pub(super) fn record_prepared_hop(request: &Request) {
    test_support::record(request);
}

/// Test-only support for the lean/redirect preparation-equivalence fixture.
///
/// The lean path and the redirect path are mutually exclusive at compile
/// time, so a single build cannot compare them directly. The fixture
/// therefore captures the outgoing request each path prepares and compares
/// both against the single hand-written expectation in
/// [`test_support::expected_snapshot`]: a divergence in either path breaks
/// its own build's assertion, and the shared expectation is what ties the
/// two builds together.
#[cfg(test)]
pub(crate) mod test_support {
    use std::cell::RefCell;
    use std::time::Duration;

    use bytes::Bytes;

    use crate::auth::AuthScheme;
    use crate::body::RequestBody;
    use crate::client::Client;
    use crate::headers::Headers;
    use crate::request::Request;
    use crate::timeout::Timeout;
    use crate::transport_hints::TransportHints;

    /// The timeout phases of a prepared hop.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct TimeoutSnapshot {
        pub(crate) pool: Option<Duration>,
        pub(crate) connect: Option<Duration>,
        pub(crate) write: Option<Duration>,
        pub(crate) read: Option<Duration>,
        pub(crate) total: Option<Duration>,
    }

    impl TimeoutSnapshot {
        fn of(timeout: Option<&Timeout>) -> Self {
            match timeout {
                Some(timeout) => Self {
                    pool: timeout.pool,
                    connect: timeout.connect,
                    write: timeout.write,
                    read: timeout.read,
                    total: timeout.total,
                },
                None => Self {
                    pool: None,
                    connect: None,
                    write: None,
                    read: None,
                    total: None,
                },
            }
        }
    }

    /// The observable state of one prepared hop request.
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[allow(
        clippy::struct_excessive_bools,
        reason = "the snapshot records independent observable facts about one hop; an enum per fact would obscure the comparison"
    )]
    pub(crate) struct HopSnapshot {
        pub(crate) method: String,
        pub(crate) url: String,
        /// Sorted header name/value pairs, so the comparison does not depend
        /// on `HeaderMap` iteration order.
        pub(crate) headers: Vec<(String, String)>,
        pub(crate) body: Vec<u8>,
        pub(crate) body_is_stream: bool,
        pub(crate) version: String,
        pub(crate) timeout: TimeoutSnapshot,
        /// Whether the hop carries a request-scoped auth scheme.
        pub(crate) has_request_auth: bool,
        pub(crate) auth_disabled: bool,
        pub(crate) decompress: Option<bool>,
        pub(crate) max_decoded_body_size: Option<usize>,
        pub(crate) max_decompression_ratio: Option<String>,
        pub(crate) transport_target: Option<String>,
        pub(crate) transport_sni: Option<String>,
        pub(crate) transport_trace: bool,
        pub(crate) has_proxied_target: bool,
    }

    thread_local! {
        static RECORDED: RefCell<Vec<HopSnapshot>> = const { RefCell::new(Vec::new()) };
    }

    /// Record a prepared hop request.
    pub(crate) fn record(request: &Request) {
        let snapshot = snapshot(request);
        RECORDED.with(|recorded| recorded.borrow_mut().push(snapshot));
    }

    /// Take the most recently recorded hop request.
    pub(crate) fn take_recorded() -> Option<HopSnapshot> {
        RECORDED.with(|recorded| recorded.borrow_mut().pop())
    }

    /// Drop every recorded hop request.
    pub(crate) fn clear_recorded() {
        RECORDED.with(|recorded| recorded.borrow_mut().clear());
    }

    fn snapshot(request: &Request) -> HopSnapshot {
        let mut headers: Vec<(String, String)> = request
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    value
                        .to_str()
                        .map_or_else(|_| "<binary>".to_owned(), str::to_owned),
                )
            })
            .collect();
        headers.sort();

        let (body, body_is_stream) = match request.body() {
            RequestBody::Empty => (Vec::new(), false),
            RequestBody::Bytes(bytes) => (bytes.to_vec(), false),
            RequestBody::Stream { .. } => (Vec::new(), true),
        };

        HopSnapshot {
            method: request.method().to_string(),
            url: request.url().to_string(),
            headers,
            body,
            body_is_stream,
            version: format!("{:?}", request.version()),
            timeout: TimeoutSnapshot::of(request.timeout()),
            has_request_auth: request.auth().is_some(),
            auth_disabled: request.is_auth_disabled(),
            decompress: request.decompress(),
            max_decoded_body_size: request.max_decoded_body_size(),
            max_decompression_ratio: request
                .max_decompression_ratio()
                .map(|ratio| ratio.to_string()),
            transport_target: request
                .transport_hints()
                .target
                .as_ref()
                .map(|target| String::from_utf8_lossy(target).into_owned()),
            transport_sni: request.transport_hints().sni_hostname.clone(),
            transport_trace: request.transport_hints().trace.is_some(),
            has_proxied_target: request.proxied_target().is_some(),
        }
    }

    /// The client used by the equivalence scenario.
    ///
    /// It carries a default header, a client-level auth scheme, and a
    /// client-level timeout so the fixture exercises the default/request
    /// header merge, the auth precedence rule, and the timeout merge.
    pub(crate) fn scenario_client() -> Client {
        let mut default_headers = Headers::new();
        default_headers
            .insert("x-default", "d")
            .expect("valid default header");
        Client::builder()
            .default_headers(default_headers)
            .auth(AuthScheme::bearer("client-token").expect("valid auth"))
            .timeout(Timeout {
                connect: Some(Duration::from_secs(1)),
                read: Some(Duration::from_secs(2)),
                total: Some(Duration::from_secs(3)),
                ..Timeout::default()
            })
            .build()
    }

    /// The request used by the equivalence scenario.
    ///
    /// It carries a request header, a body, a request-level timeout, a
    /// request-level auth scheme, and a wire-target transport hint.
    pub(crate) fn scenario_request(client: &Client, with_request_auth: bool) -> Request {
        let mut builder = client
            .post("http://127.0.0.1:9/path")
            .expect("valid URL")
            .header("x-custom", "keep")
            .bytes("hello")
            .timeout(Timeout {
                write: Some(Duration::from_secs(5)),
                ..Timeout::default()
            })
            .transport_hints(TransportHints {
                target: Some(Bytes::from_static(b"/wire?q=1")),
                sni_hostname: None,
                resolved_target: None,
                trace: None,
            });
        if with_request_auth {
            builder = builder.auth(AuthScheme::bearer("request-token").expect("valid auth"));
        }
        builder.build().expect("valid request")
    }

    /// The single expected prepared hop, written out by hand.
    ///
    /// This is the contract both request paths are compared against. The
    /// values follow the preparation policy: request headers replace
    /// same-named default headers, the request timeout overrides the client
    /// timeout per phase, the request auth scheme wins over the client one,
    /// and the wire-target hint survives on the first hop.
    pub(crate) fn expected_snapshot(with_request_auth: bool) -> HopSnapshot {
        let mut headers = vec![
            ("x-custom".to_owned(), "keep".to_owned()),
            ("x-default".to_owned(), "d".to_owned()),
        ];
        if with_request_auth {
            headers.push((
                "authorization".to_owned(),
                "Bearer request-token".to_owned(),
            ));
        } else {
            headers.push(("authorization".to_owned(), "Bearer client-token".to_owned()));
        }
        headers.sort();

        HopSnapshot {
            method: "POST".to_owned(),
            url: "http://127.0.0.1:9/path".to_owned(),
            headers,
            body: b"hello".to_vec(),
            body_is_stream: false,
            version: "HTTP/1.1".to_owned(),
            timeout: TimeoutSnapshot {
                pool: None,
                connect: Some(Duration::from_secs(1)),
                write: Some(Duration::from_secs(5)),
                read: Some(Duration::from_secs(2)),
                total: Some(Duration::from_secs(3)),
            },
            has_request_auth: with_request_auth,
            auth_disabled: false,
            decompress: None,
            max_decoded_body_size: None,
            max_decompression_ratio: None,
            transport_target: Some("/wire?q=1".to_owned()),
            transport_sni: None,
            transport_trace: false,
            has_proxied_target: false,
        }
    }

    /// The cookie the equivalence server sets, and the jar state it must
    /// produce.
    #[cfg(feature = "cookies")]
    pub(crate) const SET_COOKIE: &str = "session=abc";

    /// The expected `Cookie` header for the jar state after the response.
    #[cfg(feature = "cookies")]
    pub(crate) const EXPECTED_COOKIE_HEADER: &str = "session=abc";

    /// Start a one-shot HTTP server that answers with a `Set-Cookie` header.
    ///
    /// Returns the URL to request and the accept task.
    #[cfg(feature = "cookies")]
    pub(crate) async fn spawn_set_cookie_server() -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback listener");
        let address = listener.local_addr().expect("local address");
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept connection");
            let mut buffer = vec![0u8; 4096];
            let _ = socket.read(&mut buffer).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nSet-Cookie: {SET_COOKIE}; Path=/\r\n\r\n"
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
        });
        (format!("http://{address}/"), handle)
    }
}
