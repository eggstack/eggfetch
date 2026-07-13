//! Async client entry point.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http::Method;
use hyper_util::rt::TokioExecutor;

use crate::body::{BoxBytesStream, RequestBody, ResponseBody};
use crate::error::{Error, Result};
use crate::headers::Headers;
use crate::pool::{OriginKey, Pool, PoolConfig, PoolGuard, PoolMetrics};
#[cfg(feature = "proxy")]
use crate::proxy::{Proxy, ProxyAuth, ProxyConfig};
use crate::redirect::{self, RedirectPolicy};
#[cfg(feature = "proxy")]
use crate::request::ProxyOverride;
use crate::request::{Request, RequestBuilder};
use crate::response::{HistoryEntry, Response};
use crate::stream::{read_timeout_stream, write_timeout_stream};
use crate::timeout::{Timeout, TimeoutPhase};

#[cfg(feature = "cookies")]
use crate::cookie::CookieJar;

type Connector = hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>;
type HyperRequestBody =
    http_body_util::combinators::UnsyncBoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>>;
type HyperClient = hyper_util::client::legacy::Client<Connector, HyperRequestBody>;

/// Shared client configuration.
#[derive(Debug, Clone)]
struct ClientConfig {
    default_headers: Headers,
    user_agent: Option<String>,
    timeout: Option<Timeout>,
    redirect: RedirectPolicy,
    auth: Option<crate::auth::AuthScheme>,
    #[cfg(feature = "cookies")]
    cookie_jar: CookieJar,
    automatic_decompression: bool,
    #[cfg(feature = "proxy")]
    proxy: Option<Proxy>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            default_headers: Headers::new(),
            user_agent: None,
            timeout: None,
            redirect: RedirectPolicy::default(),
            auth: None,
            #[cfg(feature = "cookies")]
            cookie_jar: CookieJar::new(),
            automatic_decompression: true,
            #[cfg(feature = "proxy")]
            proxy: None,
        }
    }
}

/// Async HTTP client.
///
/// The client manages connection pooling and shared configuration. Create one
/// with [`Client::new`] or [`Client::builder`].
#[derive(Clone)]
pub struct Client {
    inner: Arc<ClientInner>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").finish_non_exhaustive()
    }
}

struct ClientInner {
    hyper_client: HyperClient,
    config: ClientConfig,
    pool: Pool,
}

impl Client {
    /// Create a new client with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Create a [`ClientBuilder`] for configuring a client.
    #[must_use]
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Create a GET request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn get(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::GET, url)
    }

    /// Create a POST request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn post(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::POST, url)
    }

    /// Create a PUT request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn put(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::PUT, url)
    }

    /// Create a PATCH request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn patch(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::PATCH, url)
    }

    /// Create a DELETE request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn delete(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::DELETE, url)
    }

    /// Create a HEAD request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn head(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::HEAD, url)
    }

    /// Create an OPTIONS request builder for the given URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn options(&self, url: &str) -> Result<RequestBuilder> {
        self.request(Method::OPTIONS, url)
    }

    /// Create a request builder for the given method and URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] if `url` cannot be parsed.
    pub fn request(&self, method: Method, url: &str) -> Result<RequestBuilder> {
        let parsed = parse_url(url)?;
        Ok(RequestBuilder::new(self.clone(), method, parsed))
    }

    /// Returns a reference to the connection pool metrics.
    #[must_use]
    pub fn pool_metrics(&self) -> &PoolMetrics {
        self.inner.pool.metrics()
    }

    /// Returns a reference to the client's cookie jar.
    ///
    /// Only available when the `cookies` feature is enabled.
    #[cfg(feature = "cookies")]
    #[must_use]
    pub fn cookies(&self) -> &CookieJar {
        &self.inner.config.cookie_jar
    }

    /// Send a request and return the response, following redirects if
    /// the client's redirect policy allows.
    ///
    /// The redirect loop enforces `max_redirects`, performs method
    /// rewrites per HTTP semantics, strips sensitive headers on
    /// cross-origin hops, and records redirect history.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails at any stage (connect, TLS,
    /// protocol, body) or if a timeout elapses.
    #[allow(clippy::too_many_lines)]
    pub(crate) async fn send(&self, request: Request) -> Result<Response> {
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

        // Merge client defaults before any cookie, auth, or redirect policy is
        // evaluated. Sensitive defaults must be present when a redirect hop
        // strips them; merging at the transport boundary would reintroduce
        // credentials after that stripping decision.
        let mut merged_headers = self.inner.config.default_headers.clone().into_inner();
        for name in request_headers.keys() {
            merged_headers.remove(name);
        }
        merged_headers.extend(request_headers.into_inner());
        let headers = Headers::from(merged_headers);

        // Merge client-level and request-level timeouts.
        let timeout = match self.inner.config.timeout {
            Some(client_timeout) => client_timeout.merge(request_timeout),
            None => request_timeout.unwrap_or_default(),
        };

        // Use request-level redirect override if present, otherwise client-level.
        let effective_redirect = request_redirect
            .as_ref()
            .unwrap_or(&self.inner.config.redirect);

        // Fast path: redirects disabled — send directly without buffering.
        // This preserves streaming body semantics for ordinary requests.
        if !effective_redirect.follow {
            let mut request = Request::new(method, url);

            // Cookie injection for fast path (before headers are moved).
            #[cfg(feature = "cookies")]
            let has_cookie_header = headers.contains("cookie");

            *request.headers_mut() = headers;
            request.set_body(body);
            request.set_version(version);
            request.set_timeout(Some(timeout));

            // Carry request-level auth fields.
            {
                request.set_auth(req_auth);
                request.set_auth_disabled(req_auth_disabled);
            }

            #[cfg(feature = "cookies")]
            if !has_cookie_header {
                if let Some(cookie_header) =
                    self.inner.config.cookie_jar.cookies_for_url(request.url())
                {
                    request.headers_mut().insert("cookie", &cookie_header)?;
                }
            }

            // Apply auth if configured and not disabled at request level.
            {
                let effective_auth = crate::auth::resolve_request_auth(
                    request.auth(),
                    request.is_auth_disabled(),
                    self.inner.config.auth.as_ref(),
                    request.headers(),
                )?;
                if let Some(auth) = effective_auth {
                    auth.apply(request.headers_mut())?;
                }
            }

            let response = self.send_single_request(request, &timeout).await?;

            // Process Set-Cookie headers from the response.
            #[cfg(feature = "cookies")]
            {
                let set_cookie_headers: Vec<String> = response
                    .headers()
                    .get_all("set-cookie")
                    .iter()
                    .filter_map(|v| v.to_str().ok().map(str::to_owned))
                    .collect();
                if !set_cookie_headers.is_empty() {
                    self.inner
                        .config
                        .cookie_jar
                        .update_from_response(response.url(), &set_cookie_headers);
                }
            }

            return Ok(response);
        }

        let mut history = Vec::new();
        let mut redirect_count = 0usize;
        let start_time = std::time::Instant::now();

        // Keep replayable bodies as bytes, but send a live stream on the
        // first hop. A streamed upload only needs to fail when a redirect
        // actually requires replay; eagerly collecting it here defeats
        // backpressure and can turn an unbounded upload into an OOM risk.
        let (mut replay_body, mut cur_body) = match body {
            RequestBody::Empty => (Some(Bytes::new()), RequestBody::Empty),
            RequestBody::Bytes(bytes) => (Some(bytes.clone()), RequestBody::Bytes(bytes)),
            stream @ RequestBody::Stream { .. } => (None, stream),
        };

        // State for the current hop.
        let mut cur_method = method;
        let mut cur_url = url;
        let mut cur_headers = headers;
        let mut cur_version = version;
        #[cfg(feature = "cookies")]
        let mut cookie_header_allowed = cur_headers.contains("cookie");

        // Track the previous request URL to detect cross-origin redirects.
        // On the first iteration (before any redirect), this is None and
        // client auth is applied normally. On subsequent iterations, if the
        // previous URL differs in origin from the current URL, client auth
        // is suppressed to prevent credential leakage.
        let mut prev_url: Option<url::Url> = None;
        let mut credentials_allowed = true;

        loop {
            // Compute remaining total timeout for this hop.
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

            // Build and send the request for this hop.
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

            // Request-level auth follows same-origin redirects, but must be
            // suppressed after the chain crosses an origin boundary.
            hop_request.set_auth(if credentials_allowed {
                req_auth.clone()
            } else {
                None
            });
            hop_request.set_auth_disabled(req_auth_disabled);

            // Cookie injection for redirect hop.
            #[cfg(feature = "cookies")]
            {
                // A user-supplied Cookie header may survive same-origin
                // redirects, but never gets reintroduced after it was
                // stripped on a cross-origin hop. Jar cookies are recomputed
                // for each destination instead of carrying a serialized
                // header from the previous hop.
                if !cookie_header_allowed {
                    hop_request.headers_mut().remove("cookie");
                }
                if !cookie_header_allowed && !hop_request.headers().contains("cookie") {
                    if let Some(cookie_header) = self
                        .inner
                        .config
                        .cookie_jar
                        .cookies_for_url(hop_request.url())
                    {
                        hop_request.headers_mut().insert("cookie", &cookie_header)?;
                    }
                }
            }

            // Apply auth for this redirect hop.
            // Cross-origin hops already had Authorization stripped by
            // strip_headers_for_redirect in build_redirect_request.
            // Client auth is NOT automatically reapplied to cross-origin hops
            // to prevent credential leakage to third-party origins.
            {
                let effective_auth = crate::auth::resolve_request_auth(
                    hop_request.auth(),
                    hop_request.is_auth_disabled(),
                    if credentials_allowed {
                        self.inner.config.auth.as_ref()
                    } else {
                        None
                    },
                    hop_request.headers(),
                )?;
                if let Some(auth) = effective_auth {
                    auth.apply(hop_request.headers_mut())?;
                }
            }

            let mut response = self.send_single_request(hop_request, &hop_timeout).await?;

            // Process Set-Cookie headers from the response (important for
            // redirect chains).
            #[cfg(feature = "cookies")]
            {
                let set_cookie_headers: Vec<String> = response
                    .headers()
                    .get_all("set-cookie")
                    .iter()
                    .filter_map(|v| v.to_str().ok().map(str::to_owned))
                    .collect();
                if !set_cookie_headers.is_empty() {
                    self.inner
                        .config
                        .cookie_jar
                        .update_from_response(response.url(), &set_cookie_headers);
                }
            }

            if !redirect::is_redirect_status(response.status()) || !effective_redirect.follow {
                // Not a redirect, or redirects disabled.
                response.set_history(history);
                return Ok(response);
            }

            // --- Handle redirect ---

            // Get the Location header.
            let location = if let Some(v) = response.headers().get("location") {
                v.to_str()
                    .map_err(|e| Error::InvalidRedirectLocation(e.to_string()))?
                    .to_owned()
            } else {
                response.set_history(history);
                return Ok(response);
            };

            // Drain the redirect response body to release the pool permit.
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

            // Check max redirects.
            redirect_count += 1;
            if redirect_count > effective_redirect.max_redirects {
                return Err(Error::TooManyRedirects {
                    followed: redirect_count - 1,
                    max: effective_redirect.max_redirects,
                });
            }

            // Build a temporary request to pass to build_redirect_request.
            let redirect_status = response.status();
            let drop_body = redirect::drops_body_on_redirect(redirect_status, &cur_method);
            if drop_body && replay_body.is_none() {
                // A method-rewriting redirect discards the live upload; all
                // subsequent hops now carry a replayable empty body.
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

            // Save a metadata-only snapshot in history (no body, no pool permit).
            history.push(HistoryEntry::from_response(&response));

            // Determine the new method and body for the next hop.
            let new_method = redirect::redirect_method(redirect_status, &cur_method);

            // Extract the new state from the redirect request.
            let (_, new_url, new_headers, new_body, new_version, _, _, _, _, _, _) =
                redirect_req.into_parts();

            // Record this hop's URL before updating to the redirect target
            // so the next hop can detect cross-origin redirects.
            prev_url = Some(cur_url.clone());

            cur_method = new_method;
            cur_url = new_url;
            cur_headers = new_headers;
            cur_body = new_body;
            cur_version = new_version;
        }
    }

    /// Send a single HTTP request and return the streaming response.
    ///
    /// This handles pool acquisition, timeout application, and body
    /// processing for one request/response cycle. It does NOT handle
    /// redirects—that is the responsibility of [`Client::send`].
    #[allow(clippy::too_many_lines)]
    async fn send_single_request(&self, request: Request, timeout: &Timeout) -> Result<Response> {
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

        // Determine effective decompression setting.
        let decompression_enabled =
            request_decompress.unwrap_or(self.inner.config.automatic_decompression);

        // Inject Accept-Encoding if automatic decompression is enabled
        // and the user has not supplied their own.
        let mut headers = headers;
        if decompression_enabled && !headers.contains("accept-encoding") {
            if let Some(value) = crate::compression::accept_encoding_value() {
                headers.insert("accept-encoding", value)?;
            }
        }

        // Determine effective proxy configuration.
        #[cfg(feature = "proxy")]
        let effective_proxy = self.resolve_proxy(&url, &proxy_override);
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
                match tokio::time::timeout(duration, self.inner.pool.acquire(origin.as_ref())).await
                {
                    Ok(guard) => guard,
                    Err(_) => {
                        return Err(Error::Timeout {
                            phase,
                            elapsed: duration,
                        })
                    }
                }
            }
            None => self.inner.pool.acquire(origin.as_ref()).await,
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

        // Add user-agent if not already set.
        let mut headers = headers;
        if let Some(ref ua) = self.inner.config.user_agent {
            if !headers.contains("user-agent") {
                headers.insert(http::header::USER_AGENT.as_str(), ua.as_str())?;
            }
        }

        // Send through proxy or directly.
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
                    remaining_total,
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

                let send_future =
                    send_request(&self.inner.hyper_client, hyper_request, url.clone());

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

        // Apply decompression if enabled.
        if decompression_enabled {
            let content_encoding = response
                .headers()
                .get("content-encoding")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);

            if let Some(ce) = &content_encoding {
                crate::compression::validate_content_encodings(ce)?;
            }

            let old_body =
                std::mem::replace(&mut response.body, ResponseBody::buffered(Bytes::new()));
            let new_body = match old_body {
                ResponseBody::Streaming { stream, lease } => {
                    let decoded_stream = crate::compression::decompress_stream(
                        stream,
                        content_encoding.as_deref(),
                        true,
                    )?;
                    ResponseBody::Streaming {
                        stream: decoded_stream,
                        lease,
                    }
                }
                ResponseBody::Buffered { bytes } => {
                    if content_encoding.is_some() && !bytes.is_empty() {
                        let decompressed = crate::compression::decompress_buffered(
                            &bytes,
                            content_encoding.as_deref().unwrap(),
                        )?;
                        ResponseBody::buffered(decompressed)
                    } else {
                        ResponseBody::Buffered { bytes }
                    }
                }
                ResponseBody::Consumed => ResponseBody::Consumed,
            };
            response.set_body(new_body);

            // Strip Content-Encoding and Content-Length after decompression.
            response.headers_mut().remove("content-encoding");
            response.headers_mut().remove("content-length");
        }

        apply_read_timeout_and_lease(&mut response, guard, timeout.read);

        Ok(response)
    }

    /// Resolve the effective proxy configuration for a request.
    ///
    /// Applies the tri-state override model:
    /// - `Inherit`: use client-level proxy
    /// - `Direct`: direct, no proxy
    /// - `Override(config)`: use request-level proxy
    #[cfg(feature = "proxy")]
    fn resolve_proxy(&self, url: &url::Url, proxy_override: &ProxyOverride) -> Option<ProxyConfig> {
        match proxy_override {
            ProxyOverride::Override(config) => Some(config.clone()),
            ProxyOverride::Direct => None,
            ProxyOverride::Inherit => self
                .inner
                .config
                .proxy
                .as_ref()
                .filter(|p| p.should_use_for_scheme(url.scheme()))
                .filter(|p| p.no_proxy_rules().map_or(true, |np| !np.should_bypass(url)))
                .map(Proxy::config),
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

/// Issue a hyper request and return a streaming `Response` bound to the
/// caller's URL.
async fn send_request(
    hyper_client: &HyperClient,
    request: http::Request<HyperRequestBody>,
    url: url::Url,
) -> Result<Response> {
    let hyper_response = hyper_client
        .request(request)
        .await
        .map_err(map_send_error)?;

    let status = hyper_response.status();
    let resp_version = hyper_response.version();
    let resp_headers = hyper_response.headers().clone();
    let stream: BoxBytesStream = wrap_incoming(hyper_response.into_body());
    let body = ResponseBody::streaming(stream);

    Ok(Response::new(status, resp_version, resp_headers, url, body))
}

/// Wrap a hyper `Incoming` body into a `BoxBytesStream`.
fn wrap_incoming(incoming: hyper::body::Incoming) -> BoxBytesStream {
    use futures_core::Stream;
    use http_body::Body;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    struct IncomingStream {
        inner: hyper::body::Incoming,
    }

    impl Stream for IncomingStream {
        type Item = Result<Bytes>;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            match Pin::new(&mut self.inner).poll_frame(cx) {
                Poll::Ready(Some(Ok(frame))) => {
                    if let Ok(data) = frame.into_data() {
                        Poll::Ready(Some(Ok(data)))
                    } else {
                        // Non-data frame (e.g., trailers); skip and poll again.
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                }
                Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(Error::Body(e.to_string())))),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            }
        }
    }

    Box::pin(IncomingStream { inner: incoming })
}

/// Map a hyper-util legacy client error to an eggfetch [`Error`].
///
/// When the underlying body reports a streaming error (such as our
/// write-timeout adapter's [`Error::Timeout`]), the error is wrapped
/// through hyper as `hyper::Error::User(Body, _)` inside the legacy
/// client's `SendRequest` variant. Unwrap that path so callers see
/// the original error directly.
fn map_send_error(err: hyper_util::client::legacy::Error) -> Error {
    let mut current: Option<&dyn std::error::Error> = Some(&err);
    while let Some(e) = current {
        if let Some(hyper_err) = e.downcast_ref::<hyper::Error>() {
            let mut src: Option<&dyn std::error::Error> = Some(hyper_err);
            while let Some(s) = src {
                if let Some(body_err) = s.downcast_ref::<Error>() {
                    return body_err.clone();
                }
                src = s.source();
            }
        }
        current = e.source();
    }
    Error::HyperClient(std::sync::Arc::new(err))
}

/// Apply `Content-Length` header to known-size request bodies when the
/// user has not provided one. For known-size bodies with a user-supplied
/// `Content-Length`, reject mismatches.
///
/// Returns an error if the user-supplied `Content-Length` conflicts with
/// a known-size body.
fn apply_content_length(headers: Headers, body: &crate::body::RequestBody) -> Result<Headers> {
    let known = match body {
        crate::body::RequestBody::Empty => Some(0u64),
        crate::body::RequestBody::Bytes(b) => Some(b.len() as u64),
        crate::body::RequestBody::Stream {
            length: Some(n), ..
        } => Some(*n as u64),
        crate::body::RequestBody::Stream { length: None, .. } => None,
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
            // Inject Content-Length.
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
/// dropped. The body is consumed and a new body is placed back into the
/// response.
///
/// This function performs the lease transfer by:
/// 1. Taking the body out of the response.
/// 2. Replacing the streaming body with a leased variant that owns an
///    `Arc<PoolGuard>`.
/// 3. Wrapping the inner stream with a read-timeout adapter if a read
///    timeout is configured.
/// 4. Putting the new body back into the response.
fn apply_read_timeout_and_lease(
    response: &mut Response,
    guard: PoolGuard,
    read_timeout: Option<Duration>,
) {
    let body = std::mem::replace(&mut response.body, ResponseBody::buffered(Bytes::new()));

    let new_body = match body {
        ResponseBody::Streaming { mut stream, .. } => {
            // Apply read-timeout wrapper if configured.
            if let Some(dur) = read_timeout {
                let inner = std::mem::replace(
                    &mut stream,
                    Box::pin(futures_util::stream::empty::<Result<Bytes>>()),
                );
                stream = read_timeout_stream(inner, dur);
            }
            ResponseBody::streaming_with_lease(stream, Arc::new(guard))
        }
        // Buffered and Consumed bodies do not need the lease: the
        // guard is held only for the duration of this function and
        // dropped here.
        other => {
            drop(guard);
            other
        }
    };

    response.set_body(new_body);
}

/// Builder for configuring a [`Client`].
pub struct ClientBuilder {
    default_headers: Headers,
    user_agent: Option<String>,
    pool_config: PoolConfig,
    timeout: Option<Timeout>,
    redirect: RedirectPolicy,
    auth: Option<crate::auth::AuthScheme>,
    #[cfg(feature = "cookies")]
    cookie_jar: Option<CookieJar>,
    automatic_decompression: Option<bool>,
    #[cfg(feature = "proxy")]
    proxy: Option<Proxy>,
}

impl ClientBuilder {
    /// Create a new client builder with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            default_headers: Headers::new(),
            user_agent: None,
            pool_config: PoolConfig::default(),
            timeout: None,
            redirect: RedirectPolicy::default(),
            auth: None,
            #[cfg(feature = "cookies")]
            cookie_jar: None,
            automatic_decompression: None,
            #[cfg(feature = "proxy")]
            proxy: None,
        }
    }

    /// Add a default header to all requests made by this client.
    ///
    /// # Errors
    ///
    /// Returns an error if `name` or `value` is not a valid header field.
    pub fn default_header(mut self, name: &str, value: &str) -> Result<Self> {
        self.default_headers.insert(name, value)?;
        Ok(self)
    }

    /// Set the default user-agent header.
    #[must_use]
    pub fn user_agent(mut self, agent: &str) -> Self {
        self.user_agent = Some(agent.to_owned());
        self
    }

    /// Set the maximum number of idle (unused) connections in the pool.
    #[must_use]
    pub fn max_idle_connections(mut self, max: usize) -> Self {
        self.pool_config.max_idle_connections = Some(max);
        self
    }

    /// Set the maximum number of idle connections per individual host.
    #[must_use]
    pub fn max_idle_connections_per_host(mut self, max: usize) -> Self {
        self.pool_config.max_idle_connections_per_host = Some(max);
        self
    }

    /// Set the maximum total number of concurrent connections.
    #[must_use]
    pub fn max_connections(mut self, max: usize) -> Self {
        self.pool_config.max_connections = Some(max);
        self
    }

    /// Set the maximum number of concurrent connections per individual host.
    #[must_use]
    pub fn max_connections_per_host(mut self, max: usize) -> Self {
        self.pool_config.max_connections_per_host = Some(max);
        self
    }

    /// Set the duration after which idle connections are closed.
    #[must_use]
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.pool_config.idle_timeout = Some(timeout);
        self
    }

    /// Set the default timeout for all requests made by this client.
    #[must_use]
    pub fn timeout(mut self, timeout: Timeout) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the redirect policy for this client.
    #[must_use]
    pub fn follow_redirects(mut self, follow: bool) -> Self {
        self.redirect.follow = follow;
        self
    }

    /// Set the maximum number of redirects to follow.
    #[must_use]
    pub fn max_redirects(mut self, max: usize) -> Self {
        self.redirect.max_redirects = max;
        self
    }

    /// Set the full redirect policy.
    #[must_use]
    pub fn redirect_policy(mut self, policy: RedirectPolicy) -> Self {
        self.redirect = policy;
        self
    }

    /// Set a shared cookie jar for this client.
    ///
    /// When set, the client will automatically inject matching cookies
    /// into requests and update the jar from `Set-Cookie` response headers.
    ///
    /// Only available when the `cookies` feature is enabled.
    #[cfg(feature = "cookies")]
    #[must_use]
    pub fn cookie_jar(mut self, jar: CookieJar) -> Self {
        self.cookie_jar = Some(jar);
        self
    }

    /// Set default authentication for all requests made by this client.
    ///
    /// The configured auth is applied to every request unless overridden
    /// or disabled at the request level. Auth is recomputed per redirect
    /// hop; cross-origin redirects never carry client auth.
    #[must_use]
    pub fn auth(mut self, auth: impl Into<crate::auth::AuthScheme>) -> Self {
        self.auth = Some(auth.into());
        self
    }

    /// Set the default proxy for all requests made by this client.
    ///
    /// When set, all matching requests are routed through the specified
    /// proxy. Can be overridden or disabled per-request.
    #[cfg(feature = "proxy")]
    #[must_use]
    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.proxy = Some(proxy);
        self
    }

    /// Set `NO_PROXY` bypass rules for the default proxy.
    ///
    /// When set, URLs matching any bypass rule are sent directly
    /// without going through the proxy.
    #[cfg(feature = "proxy")]
    #[must_use]
    pub fn no_proxy(mut self, no_proxy: crate::proxy::NoProxy) -> Self {
        if let Some(proxy) = self.proxy.take() {
            self.proxy = Some(proxy.no_proxy(no_proxy));
        }
        self
    }

    /// Enable or disable automatic response decompression.
    ///
    /// When enabled (the default), the client sends an
    /// `Accept-Encoding` header and transparently decompresses
    /// response bodies. Decoded `Content-Encoding` and
    /// `Content-Length` headers are removed from the response.
    ///
    /// Can be overridden per-request via
    /// [`RequestBuilder::decompress`].
    #[must_use]
    pub fn automatic_decompression(mut self, enabled: bool) -> Self {
        self.automatic_decompression = Some(enabled);
        self
    }

    /// Build the client.
    ///
    /// Native system roots are preferred. If the platform root store is
    /// unavailable, the client falls back to the packaged Mozilla root set
    /// while retaining certificate and hostname verification.
    #[must_use]
    pub fn build(self) -> Client {
        let https = match hyper_rustls::HttpsConnectorBuilder::new().with_native_roots() {
            Ok(builder) => builder.https_or_http().enable_http1().build(),
            Err(_) => build_webpki_connector(),
        };

        let mut builder = hyper_util::client::legacy::Client::builder(TokioExecutor::new());
        if let Some(timeout) = self.pool_config.idle_timeout {
            builder.pool_idle_timeout(timeout);
        }
        let hyper_client: HyperClient = builder.build(https);

        #[cfg(feature = "cookies")]
        let cookie_jar = self.cookie_jar.unwrap_or_default();

        let automatic_decompression = self.automatic_decompression.unwrap_or(true);

        let config = ClientConfig {
            default_headers: self.default_headers,
            user_agent: self.user_agent,
            timeout: self.timeout,
            redirect: self.redirect,
            auth: self.auth,
            #[cfg(feature = "cookies")]
            cookie_jar,
            automatic_decompression,
            #[cfg(feature = "proxy")]
            proxy: self.proxy,
        };

        let pool = Pool::new(self.pool_config);

        Client {
            inner: Arc::new(ClientInner {
                hyper_client,
                config,
                pool,
            }),
        }
    }
}

/// Build a connector with the packaged Mozilla root set.
///
/// Kept as a separate function so the fallback construction path can be
/// exercised without depending on the host's native trust store.
fn build_webpki_connector() -> Connector {
    hyper_rustls::HttpsConnectorBuilder::new()
        .with_webpki_roots()
        .https_or_http()
        .enable_http1()
        .build()
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_url(url_str: &str) -> Result<url::Url> {
    let url = url::Url::parse(url_str).map_err(|e| Error::InvalidUrl(e.to_string()))?;
    if !url.username().is_empty() || url.password().is_some() {
        // URL userinfo is both easy to leak through diagnostics and
        // surprising for an HTTP client with explicit auth APIs.
        return Err(Error::InvalidUrl(
            "URL userinfo is not supported; configure authentication explicitly".into(),
        ));
    }
    match url.scheme() {
        "http" | "https" => Ok(url),
        other => Err(Error::Unsupported(format!(
            "URL scheme '{other}' is not supported; use http or https"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Proxy request handling
// ---------------------------------------------------------------------------

/// Send a request through a proxy.
///
/// Routes to HTTP forwarding or CONNECT tunneling based on the
/// destination scheme.
#[cfg(feature = "proxy")]
async fn send_proxy_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    proxy_config: &ProxyConfig,
    remaining_total: Option<std::time::Duration>,
) -> Result<Response> {
    match dest_url.scheme() {
        "http" => {
            send_http_proxy_request(
                dest_url,
                method,
                headers,
                body,
                version,
                proxy_config,
                remaining_total,
            )
            .await
        }
        "https" => {
            send_https_connect_request(
                dest_url,
                method,
                headers,
                body,
                version,
                proxy_config,
                remaining_total,
            )
            .await
        }
        other => Err(Error::Unsupported(format!(
            "unsupported destination scheme '{other}' through proxy"
        ))),
    }
}

/// Connect to the proxy, returning a buffered TCP stream.
#[cfg(feature = "proxy")]
async fn connect_to_proxy(
    proxy_config: &ProxyConfig,
    remaining_total: Option<std::time::Duration>,
) -> Result<tokio::io::BufReader<tokio::net::TcpStream>> {
    let proxy_host = proxy_config.host().unwrap_or("127.0.0.1");
    let proxy_port = proxy_config.port();

    let connect_future = async {
        let stream = tokio::net::TcpStream::connect((proxy_host, proxy_port))
            .await
            .map_err(|e| Error::ProxyConnect(format!("failed to connect to proxy: {e}")))?;
        stream
            .set_nodelay(true)
            .map_err(|e| Error::ProxyConnect(format!("failed to set nodelay: {e}")))?;
        Ok::<_, Error>(stream)
    };

    let stream = match remaining_total {
        Some(dur) => match tokio::time::timeout(dur, connect_future).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err(Error::Timeout {
                    phase: TimeoutPhase::ProxyConnect,
                    elapsed: dur,
                });
            }
        },
        None => connect_future.await?,
    };

    Ok(tokio::io::BufReader::new(stream))
}

/// Send an HTTP request through an HTTP forward proxy.
#[cfg(feature = "proxy")]
async fn send_http_proxy_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    proxy_config: &ProxyConfig,
    remaining_total: Option<std::time::Duration>,
) -> Result<Response> {
    let mut stream = connect_to_proxy(proxy_config, remaining_total).await?;

    // Write the proxy request with absolute-form URI.
    let absolute_uri = dest_url.as_str();
    write_proxy_request(
        &mut stream,
        method,
        absolute_uri,
        version,
        headers,
        proxy_config.auth(),
        body,
    )
    .await?;

    // Read the response from the proxy.
    let (status, resp_headers, initial_buf) = read_proxy_response(&mut stream).await?;

    let url = dest_url.clone();
    let status = http::StatusCode::from_u16(status)
        .map_err(|e| Error::MalformedProxyResponse(format!("invalid status code: {e}")))?;

    let mut resp_headers_map = http::HeaderMap::new();
    for (name, value) in &resp_headers {
        let name = http::HeaderName::from_bytes(name.as_bytes())
            .map_err(|e| Error::MalformedProxyResponse(format!("invalid header name: {e}")))?;
        let value = http::HeaderValue::from_str(value)
            .map_err(|e| Error::MalformedProxyResponse(format!("invalid header value: {e}")))?;
        resp_headers_map.append(name, value);
    }

    // Return the body as a streaming response.
    let stream_reader = stream.into_inner();
    let (read_half, _write_half) = stream_reader.into_split();
    let body_stream = ProxyResponseStream::new(initial_buf, read_half);
    let body_stream = Box::pin(body_stream) as BoxBytesStream;
    let body = ResponseBody::streaming(body_stream);

    Ok(Response::new(status, version, resp_headers_map, url, body))
}

/// Send an HTTPS request through an HTTP proxy using CONNECT tunneling.
#[cfg(feature = "proxy")]
async fn send_https_connect_request(
    dest_url: &url::Url,
    method: &http::Method,
    headers: &Headers,
    body: RequestBody,
    version: http::Version,
    proxy_config: &ProxyConfig,
    remaining_total: Option<std::time::Duration>,
) -> Result<Response> {
    use tokio::io::AsyncWriteExt;

    let mut stream = connect_to_proxy(proxy_config, remaining_total).await?;

    // Send CONNECT request.
    let dest_host = dest_url
        .host_str()
        .ok_or_else(|| Error::InvalidUrl("destination URL has no host".into()))?;
    let dest_port = dest_url.port_or_known_default().unwrap_or(443);
    let connect_target = format!("{dest_host}:{dest_port}");

    let mut connect_req =
        format!("CONNECT {connect_target} HTTP/1.1\r\nHost: {connect_target}\r\n");
    if let Some(auth) = proxy_config.auth() {
        use std::fmt::Write;
        let _ = write!(
            connect_req,
            "Proxy-Authorization: {}\r\n",
            auth.header_value()
        );
    }
    connect_req.push_str("\r\n");

    stream
        .write_all(connect_req.as_bytes())
        .await
        .map_err(|e| Error::ProxyConnect(format!("failed to send CONNECT: {e}")))?;

    // Read the CONNECT response.
    let (status, _resp_headers, initial_buf) = read_proxy_response(&mut stream).await?;

    if status != 200 {
        let body_str = initial_buf.iter().take(256).copied().collect::<Vec<u8>>();
        let body_str = String::from_utf8_lossy(&body_str).into_owned();
        return Err(Error::ProxyConnectRejected {
            status,
            body: body_str,
        });
    }

    // The tunnel is established. Get the raw TCP stream.
    let tcp_stream = stream.into_inner();

    // Wrap with initial buffer for TLS.
    let tunnel = ProxyTunnel::new(initial_buf, tcp_stream);

    // Perform TLS handshake with the destination through the tunnel.
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let tls_connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(tls_config));

    let domain = rustls::pki_types::ServerName::try_from(dest_host.to_owned())
        .map_err(|e| Error::Tls(format!("invalid TLS server name: {e}")))?;

    let tls_handshake = tls_connector.connect(domain, tunnel);
    let tls_stream = match remaining_total {
        Some(dur) => match tokio::time::timeout(dur, tls_handshake).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                return Err(Error::Tls(format!(
                    "TLS handshake through tunnel failed: {e}"
                )))
            }
            Err(_) => {
                return Err(Error::Timeout {
                    phase: TimeoutPhase::ProxyTls,
                    elapsed: dur,
                });
            }
        },
        None => tls_handshake
            .await
            .map_err(|e| Error::Tls(format!("TLS handshake through tunnel failed: {e}")))?,
    };

    // Send the actual HTTP request over the TLS connection.
    let absolute_uri = dest_url.as_str();
    let mut tls_buf = tokio::io::BufReader::new(tls_stream);

    write_proxy_request(
        &mut tls_buf,
        method,
        absolute_uri,
        version,
        headers,
        None, // No proxy auth for the destination request.
        body,
    )
    .await?;

    // Read the response from the destination.
    let (status, resp_headers, initial_buf) = read_proxy_response(&mut tls_buf).await?;

    let url = dest_url.clone();
    let status = http::StatusCode::from_u16(status)
        .map_err(|e| Error::MalformedProxyResponse(format!("invalid status code: {e}")))?;

    let mut resp_headers_map = http::HeaderMap::new();
    for (name, value) in &resp_headers {
        let name = http::HeaderName::from_bytes(name.as_bytes())
            .map_err(|e| Error::MalformedProxyResponse(format!("invalid header name: {e}")))?;
        let value = http::HeaderValue::from_str(value)
            .map_err(|e| Error::MalformedProxyResponse(format!("invalid header value: {e}")))?;
        resp_headers_map.append(name, value);
    }

    let stream_reader = tls_buf.into_inner();
    // For TLS streams, we can't easily extract the inner stream.
    // Use the initial_buf approach with the TLS stream wrapped.
    let body_stream = TlsProxyResponseStream::new(initial_buf, stream_reader);
    let body_stream = Box::pin(body_stream) as BoxBytesStream;
    let body = ResponseBody::streaming(body_stream);

    Ok(Response::new(status, version, resp_headers_map, url, body))
}

/// Write an HTTP request to a stream.
#[cfg(feature = "proxy")]
async fn write_proxy_request<S: tokio::io::AsyncWrite + Unpin>(
    stream: &mut S,
    method: &http::Method,
    uri: &str,
    version: http::Version,
    headers: &Headers,
    proxy_auth: Option<&ProxyAuth>,
    body: RequestBody,
) -> Result<()> {
    use std::fmt::Write;
    use tokio::io::AsyncWriteExt;

    let version_str = match version {
        http::Version::HTTP_10 => "HTTP/1.0",
        _ => "HTTP/1.1",
    };

    let mut request = format!("{method} {uri} {version_str}\r\n");

    // Add Host header if not present.
    if !headers.contains("host") {
        if let Ok(parsed) = url::Url::parse(uri) {
            let host = if let Some(port) = parsed.port() {
                format!("{}:{port}", parsed.host_str().unwrap_or(""))
            } else {
                parsed.host_str().unwrap_or("").to_string()
            };
            let _ = write!(request, "Host: {host}\r\n");
        }
    }

    // Write regular headers.
    for (name, value) in headers.iter() {
        // Skip proxy-authorization from destination headers.
        if name.as_str().eq_ignore_ascii_case("proxy-authorization") {
            continue;
        }
        if let Ok(value_str) = value.to_str() {
            let _ = write!(request, "{}: {value_str}\r\n", name.as_str());
        }
    }

    // Add Proxy-Authorization if configured.
    if let Some(auth) = proxy_auth {
        let _ = write!(request, "Proxy-Authorization: {}\r\n", auth.header_value());
    }

    request.push_str("\r\n");

    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| Error::ProxyConnect(format!("failed to write request: {e}")))?;

    // Write the body.
    match body {
        RequestBody::Empty => {}
        RequestBody::Bytes(bytes) => {
            stream
                .write_all(&bytes)
                .await
                .map_err(|e| Error::ProxyConnect(format!("failed to write body: {e}")))?;
        }
        RequestBody::Stream {
            stream: mut body_stream,
            ..
        } => {
            use bytes::BytesMut;
            use futures_util::StreamExt;
            let mut buf = BytesMut::with_capacity(8192);
            while let Some(chunk) = body_stream.next().await {
                let chunk = chunk.map_err(|e| Error::Body(e.to_string()))?;
                buf.extend_from_slice(&chunk);
                if buf.len() >= 8192 {
                    stream
                        .write_all(&buf)
                        .await
                        .map_err(|e| Error::ProxyConnect(format!("failed to write body: {e}")))?;
                    buf.clear();
                }
            }
            if !buf.is_empty() {
                stream
                    .write_all(&buf)
                    .await
                    .map_err(|e| Error::ProxyConnect(format!("failed to write body: {e}")))?;
            }
        }
    }

    stream
        .flush()
        .await
        .map_err(|e| Error::ProxyConnect(format!("failed to flush: {e}")))?;

    Ok(())
}

/// Read an HTTP response from a proxy or destination.
///
/// Returns `(status_code, headers, remaining_initial_bytes)`.
#[cfg(feature = "proxy")]
async fn read_proxy_response<S: tokio::io::AsyncRead + Unpin>(
    stream: &mut tokio::io::BufReader<S>,
) -> Result<(u16, Vec<(String, String)>, Vec<u8>)> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt};

    // Read status line.
    let mut status_line = String::new();
    stream
        .read_line(&mut status_line)
        .await
        .map_err(|e| Error::ProxyConnect(format!("failed to read proxy response status: {e}")))?;

    if status_line.is_empty() {
        return Err(Error::MalformedProxyResponse(
            "proxy closed connection before response".into(),
        ));
    }

    // Parse "HTTP/1.1 200 OK"
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| {
            Error::MalformedProxyResponse(format!("invalid status line: {status_line}"))
        })?;

    // Read headers.
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        stream.read_line(&mut line).await.map_err(|e| {
            Error::ProxyConnect(format!("failed to read proxy response header: {e}"))
        })?;

        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break; // End of headers.
        }

        if let Some((name, value)) = trimmed.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }

    // Extract any remaining buffered bytes from the BufReader.
    let mut initial_buf = Vec::new();
    let buf_ref = stream.buffer();
    if !buf_ref.is_empty() {
        initial_buf.extend_from_slice(buf_ref);
        let consumed = buf_ref.len();
        // Advance the BufReader past the buffered data.
        let mut discard = vec![0u8; consumed];
        let _ = stream.read(&mut discard).await;
    }

    Ok((status_code, headers, initial_buf))
}

/// Streaming response body from an HTTP proxy.
///
/// Yields data from an initial buffer first, then reads from the
/// underlying TCP stream.
#[cfg(feature = "proxy")]
struct ProxyResponseStream {
    initial_buf: std::io::Cursor<Vec<u8>>,
    inner: tokio::net::tcp::OwnedReadHalf,
}

#[cfg(feature = "proxy")]
impl ProxyResponseStream {
    fn new(initial_buf: Vec<u8>, inner: tokio::net::tcp::OwnedReadHalf) -> Self {
        Self {
            initial_buf: std::io::Cursor::new(initial_buf),
            inner,
        }
    }
}

#[cfg(feature = "proxy")]
impl futures_core::Stream for ProxyResponseStream {
    type Item = Result<Bytes>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::io::Read as _;

        // Drain the initial buffer first.
        if self.initial_buf.position() < self.initial_buf.get_ref().len() as u64 {
            let mut chunk = vec![0u8; 8192];
            let n = match self.initial_buf.read(&mut chunk) {
                Ok(n) => n,
                Err(e) => {
                    return std::task::Poll::Ready(Some(Err(Error::Body(format!(
                        "failed to read initial buffer: {e}"
                    )))));
                }
            };
            if n > 0 {
                chunk.truncate(n);
                return std::task::Poll::Ready(Some(Ok(Bytes::from(chunk))));
            }
        }

        // Read from the inner TCP stream using poll_read.
        let mut chunk = vec![0u8; 8192];
        let mut read_buf = tokio::io::ReadBuf::new(&mut chunk);
        match tokio::io::AsyncRead::poll_read(
            std::pin::Pin::new(&mut self.inner),
            cx,
            &mut read_buf,
        ) {
            std::task::Poll::Ready(Ok(())) => {
                let n = read_buf.filled().len();
                if n > 0 {
                    chunk.truncate(n);
                    std::task::Poll::Ready(Some(Ok(Bytes::from(chunk))))
                } else {
                    std::task::Poll::Ready(None)
                }
            }
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Some(Err(Error::Body(
                format!("proxy stream read error: {e}"),
            )))),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

/// Streaming response body from a TLS connection through a proxy tunnel.
///
/// Yields data from an initial buffer first, then reads from the
/// underlying TLS stream.
#[cfg(feature = "proxy")]
struct TlsProxyResponseStream<S> {
    initial_buf: std::io::Cursor<Vec<u8>>,
    inner: S,
}

#[cfg(feature = "proxy")]
impl<S> TlsProxyResponseStream<S> {
    fn new(initial_buf: Vec<u8>, inner: S) -> Self {
        Self {
            initial_buf: std::io::Cursor::new(initial_buf),
            inner,
        }
    }
}

#[cfg(feature = "proxy")]
impl<S: tokio::io::AsyncRead + Unpin> futures_core::Stream for TlsProxyResponseStream<S> {
    type Item = Result<Bytes>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::io::Read as _;

        // Drain the initial buffer first.
        if self.initial_buf.position() < self.initial_buf.get_ref().len() as u64 {
            let mut chunk = vec![0u8; 8192];
            let n = match self.initial_buf.read(&mut chunk) {
                Ok(n) => n,
                Err(e) => {
                    return std::task::Poll::Ready(Some(Err(Error::Body(format!(
                        "failed to read initial buffer: {e}"
                    )))));
                }
            };
            if n > 0 {
                chunk.truncate(n);
                return std::task::Poll::Ready(Some(Ok(Bytes::from(chunk))));
            }
        }

        // Read from the inner stream.
        let mut chunk = vec![0u8; 8192];
        let mut read_buf = tokio::io::ReadBuf::new(&mut chunk);
        match tokio::io::AsyncRead::poll_read(
            std::pin::Pin::new(&mut self.inner),
            cx,
            &mut read_buf,
        ) {
            std::task::Poll::Ready(Ok(())) => {
                let n = read_buf.filled().len();
                if n > 0 {
                    chunk.truncate(n);
                    std::task::Poll::Ready(Some(Ok(Bytes::from(chunk))))
                } else {
                    std::task::Poll::Ready(None)
                }
            }
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Some(Err(Error::Body(
                format!("proxy stream read error: {e}"),
            )))),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

/// IO wrapper for CONNECT tunnels that holds initial buffered bytes.
///
/// After the CONNECT handshake, the proxy may have sent some bytes
/// that are part of the TLS stream. This wrapper preserves them.
#[cfg(feature = "proxy")]
struct ProxyTunnel {
    initial_buf: std::io::Cursor<Vec<u8>>,
    inner: tokio::net::TcpStream,
}

#[cfg(feature = "proxy")]
impl ProxyTunnel {
    fn new(initial_buf: Vec<u8>, inner: tokio::net::TcpStream) -> Self {
        Self {
            initial_buf: std::io::Cursor::new(initial_buf),
            inner,
        }
    }
}

#[cfg(feature = "proxy")]
impl tokio::io::AsyncRead for ProxyTunnel {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // Drain initial buffer first.
        let pos = self.initial_buf.position();
        let total = self.initial_buf.get_ref().len() as u64;
        if pos < total {
            let unfilled = buf.initialize_unfilled();
            let pos_usize = usize::try_from(pos).unwrap_or(usize::MAX);
            let remaining = &self.initial_buf.get_ref()[pos_usize..];
            let n = std::cmp::min(remaining.len(), unfilled.len());
            unfilled[..n].copy_from_slice(&remaining[..n]);
            self.initial_buf.set_position(pos + n as u64);
            buf.advance(n);
            return std::task::Poll::Ready(Ok(()));
        }

        // Delegate to inner stream.
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

#[cfg(feature = "proxy")]
impl tokio::io::AsyncWrite for ProxyTunnel {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_constructs() {
        let _client = Client::new();
    }

    #[test]
    fn client_default() {
        let _client = Client::default();
    }

    #[test]
    fn client_builder() {
        let _client = Client::builder().user_agent("test-agent").build();
    }

    #[test]
    fn tls_root_store_paths_construct() {
        // Native roots are environment-dependent, but when available the
        // production path must build the same verified connector shape.
        if let Ok(builder) = hyper_rustls::HttpsConnectorBuilder::new().with_native_roots() {
            let _ = builder.https_or_http().enable_http1().build();
        }
        let _ = build_webpki_connector();
    }

    #[test]
    fn parse_valid_urls() {
        assert!(parse_url("https://example.com").is_ok());
        assert!(parse_url("http://localhost:8080").is_ok());
    }

    #[test]
    fn parse_invalid_schemes() {
        assert!(parse_url("ftp://example.com").is_err());
        assert!(parse_url("file:///tmp/test").is_err());
    }

    #[test]
    fn parse_invalid_urls() {
        assert!(parse_url("not a url").is_err());
    }

    #[test]
    fn get_request_builder() {
        let client = Client::new();
        let builder = client.get("https://example.com").unwrap();
        let req = builder.build().unwrap();
        assert_eq!(*req.method(), Method::GET);
    }

    #[test]
    fn apply_content_length_empty_body() {
        let headers = Headers::new();
        let body = crate::body::RequestBody::Empty;
        let out = apply_content_length(headers, &body).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "0");
    }

    #[test]
    fn apply_content_length_bytes_body() {
        let headers = Headers::new();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let out = apply_content_length(headers, &body).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "5");
    }

    #[test]
    fn apply_content_length_stream_known() {
        let headers = Headers::new();
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = crate::body::RequestBody::from_stream(stream, Some(7));
        let out = apply_content_length(headers, &body).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "7");
    }

    #[test]
    fn apply_content_length_stream_unknown() {
        let headers = Headers::new();
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = crate::body::RequestBody::from_stream(stream, None);
        let out = apply_content_length(headers, &body).unwrap();
        assert!(out.get("content-length").is_none());
    }

    #[test]
    fn apply_content_length_user_matches() {
        let mut headers = Headers::new();
        headers.insert("content-length", "5").unwrap();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let out = apply_content_length(headers, &body).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "5");
    }

    #[test]
    fn apply_content_length_user_mismatch_errors() {
        let mut headers = Headers::new();
        headers.insert("content-length", "10").unwrap();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let err = apply_content_length(headers, &body).unwrap_err();
        assert_eq!(err.kind(), "request_build");
    }

    #[test]
    fn apply_content_length_rejects_invalid_value() {
        let mut headers = Headers::new();
        headers.insert("content-length", "not-a-number").unwrap();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let err = apply_content_length(headers, &body).unwrap_err();
        assert_eq!(err.kind(), "invalid_header_value");
    }

    #[test]
    fn apply_content_length_rejects_unknown_stream_override() {
        let mut headers = Headers::new();
        headers.insert("content-length", "5").unwrap();
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = crate::body::RequestBody::from_stream(stream, None);
        let err = apply_content_length(headers, &body).unwrap_err();
        assert_eq!(err.kind(), "request_build");
    }

    #[test]
    fn parse_url_rejects_userinfo_without_echoing_credentials() {
        let err = parse_url("https://user:secret@example.com").unwrap_err();
        assert_eq!(err.kind(), "invalid_url");
        assert!(!err.to_string().contains("secret"));
    }

    #[cfg(feature = "proxy")]
    #[tokio::test]
    async fn proxy_auth_conflict_with_header() {
        let proxy = Proxy::all("http://proxy.example:8080")
            .unwrap()
            .auth(ProxyAuth::basic("user", "pass").unwrap());
        let client = Client::builder().proxy(proxy).build();
        let request = client
            .get("http://destination.example")
            .unwrap()
            .header("proxy-authorization", "Basic dXNlcjpwYXNz")
            .build()
            .unwrap();
        let err = client.send(request).await.unwrap_err();
        assert_eq!(err.kind(), "conflicting_auth");
        assert!(err.to_string().contains("Proxy-Authorization"));
    }

    #[cfg(feature = "proxy")]
    #[tokio::test]
    async fn proxy_auth_no_conflict_without_header() {
        let proxy = Proxy::all("http://proxy.example:8080")
            .unwrap()
            .auth(ProxyAuth::basic("user", "pass").unwrap());
        let client = Client::builder().proxy(proxy).build();
        let request = client
            .get("http://destination.example")
            .unwrap()
            .build()
            .unwrap();
        let err = client.send(request).await.unwrap_err();
        assert_ne!(err.kind(), "conflicting_auth");
    }

    #[cfg(feature = "proxy")]
    #[tokio::test]
    async fn proxy_auth_no_conflict_with_header_only() {
        let proxy = Proxy::all("http://proxy.example:8080").unwrap();
        let client = Client::builder().proxy(proxy).build();
        let request = client
            .get("http://destination.example")
            .unwrap()
            .header("proxy-authorization", "Basic dXNlcjpwYXNz")
            .build()
            .unwrap();
        let err = client.send(request).await.unwrap_err();
        assert_ne!(err.kind(), "conflicting_auth");
    }
}
