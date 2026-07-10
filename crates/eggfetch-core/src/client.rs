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
use crate::redirect::{self, RedirectPolicy};
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
    #[cfg(feature = "cookies")]
    cookie_jar: CookieJar,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            default_headers: Headers::new(),
            user_agent: None,
            timeout: None,
            redirect: RedirectPolicy::default(),
            #[cfg(feature = "cookies")]
            cookie_jar: CookieJar::new(),
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
        let (method, url, headers, body, version, request_timeout, request_redirect) =
            request.into_parts();

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

            #[cfg(feature = "cookies")]
            if !has_cookie_header {
                if let Some(cookie_header) =
                    self.inner.config.cookie_jar.cookies_for_url(request.url())
                {
                    request.headers_mut().insert("cookie", &cookie_header)?;
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

        // Redirect path: buffer the body into bytes for replayability across hops.
        let body_bytes = body.into_bytes().await?;

        let mut history = Vec::new();
        let mut redirect_count = 0usize;
        let start_time = std::time::Instant::now();

        // State for the current hop.
        let mut cur_method = method;
        let mut cur_url = url;
        let mut cur_headers = headers;
        let mut cur_body = body_bytes;
        let mut cur_version = version;

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
            hop_request.set_body(RequestBody::Bytes(cur_body.clone()));
            hop_request.set_version(cur_version);
            hop_request.set_timeout(Some(hop_timeout));

            // Cookie injection for redirect hop.
            #[cfg(feature = "cookies")]
            {
                if !hop_request.headers().contains("cookie") {
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
                v.to_str().unwrap_or("").to_string()
            } else {
                response.set_history(history);
                return Ok(response);
            };

            // Drain the redirect response body to release the pool permit.
            {
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
            let mut temp_request = Request::new(cur_method.clone(), cur_url.clone());
            *temp_request.headers_mut() = cur_headers.clone();
            temp_request.set_body(RequestBody::Bytes(cur_body.clone()));
            temp_request.set_version(cur_version);

            let (redirect_req, _) =
                redirect::build_redirect_request(&temp_request, response.status(), &location)?;

            // Save the redirect status before pushing response to history.
            let redirect_status = response.status();

            // Save a metadata-only snapshot in history (no body, no pool permit).
            history.push(HistoryEntry::from_response(&response));

            // Determine the new method and body for the next hop.
            let new_method = redirect::redirect_method(redirect_status, &cur_method);
            let drop_body = redirect::drops_body_on_redirect(redirect_status, &cur_method);

            // Extract the new state from the redirect request.
            let (_, new_url, new_headers, _, new_version, _, _) = redirect_req.into_parts();

            cur_method = new_method;
            cur_url = new_url;
            cur_headers = new_headers;
            cur_body = if drop_body { Bytes::new() } else { cur_body };
            cur_version = new_version;
        }
    }

    /// Send a single HTTP request and return the streaming response.
    ///
    /// This handles pool acquisition, timeout application, and body
    /// processing for one request/response cycle. It does NOT handle
    /// redirects—that is the responsibility of [`Client::send`].
    async fn send_single_request(&self, request: Request, timeout: &Timeout) -> Result<Response> {
        let (method, url, headers, body, version, _request_timeout, _request_redirect) =
            request.into_parts();

        let uri: http::Uri = url
            .as_str()
            .parse()
            .map_err(|e| Error::InvalidUrl(format!("failed to convert url to URI: {e}")))?;

        // Build the origin key for pool slot acquisition.
        let origin = OriginKey::from_url(url.scheme(), &url);

        // Acquire a pool slot, respecting pool timeout.
        let guard = match timeout.pool {
            Some(dur) => {
                match tokio::time::timeout(dur, self.inner.pool.acquire(origin.as_ref())).await {
                    Ok(guard) => guard,
                    Err(_) => {
                        return Err(Error::Timeout {
                            phase: TimeoutPhase::Pool,
                            elapsed: dur,
                        });
                    }
                }
            }
            None => self.inner.pool.acquire(origin.as_ref()).await,
        };

        // Apply write timeout to streamed request bodies.
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

        // Determine whether the user has supplied Content-Length.
        let user_content_length = headers
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        // Apply Content-Length for known-size bodies when not user-supplied.
        let headers = apply_content_length(headers, &body, user_content_length)?;

        let mut http_request = http::Request::builder()
            .method(method)
            .uri(uri)
            .version(version);

        // Apply default headers first, then request headers (overrides).
        for (name, value) in self.inner.config.default_headers.iter() {
            http_request = http_request.header(name, value);
        }
        for (name, value) in headers.iter() {
            http_request = http_request.header(name, value);
        }

        // Apply user-agent if set and not already present.
        if let Some(ref ua) = self.inner.config.user_agent {
            if !headers.contains("user-agent") {
                http_request = http_request.header(
                    http::header::USER_AGENT,
                    http::HeaderValue::from_str(ua)
                        .map_err(|e| Error::InvalidHeaderValue(e.to_string()))?,
                );
            }
        }

        let hyper_request = http_request
            .body(body.into_http_body())
            .map_err(|e| Error::RequestBuild(e.to_string()))?;

        // Send request and collect response headers. The pool permit
        // is moved into the response body lifetime so that streaming
        // bodies hold the permit until they are consumed or dropped.
        let send_future = send_request(&self.inner.hyper_client, hyper_request, url.clone());

        let mut response = match timeout.total {
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
        };

        // Apply read timeout to the response body stream and attach
        // the pool permit to keep per-origin limits meaningful while
        // the body is in flight.
        apply_read_timeout_and_lease(&mut response, guard, timeout.read);

        Ok(response)
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
fn apply_content_length(
    headers: Headers,
    body: &crate::body::RequestBody,
    user_content_length: Option<u64>,
) -> Result<Headers> {
    let known = match body {
        crate::body::RequestBody::Empty => Some(0u64),
        crate::body::RequestBody::Bytes(b) => Some(b.len() as u64),
        crate::body::RequestBody::Stream {
            length: Some(n), ..
        } => Some(*n as u64),
        crate::body::RequestBody::Stream { length: None, .. } => None,
    };

    if let Some(known_len) = known {
        if let Some(user_len) = user_content_length {
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
    #[cfg(feature = "cookies")]
    cookie_jar: Option<CookieJar>,
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
            #[cfg(feature = "cookies")]
            cookie_jar: None,
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

    /// Build the client.
    ///
    /// # Panics
    ///
    /// Panics if the system TLS root certificates cannot be loaded. This
    /// should not happen on any standard operating system.
    #[must_use]
    pub fn build(self) -> Client {
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .expect("failed to load native roots")
            .https_or_http()
            .enable_http1()
            .build();

        let mut builder = hyper_util::client::legacy::Client::builder(TokioExecutor::new());
        if let Some(timeout) = self.pool_config.idle_timeout {
            builder.pool_idle_timeout(timeout);
        }
        let hyper_client: HyperClient = builder.build(https);

        #[cfg(feature = "cookies")]
        let cookie_jar = self.cookie_jar.unwrap_or_default();

        let config = ClientConfig {
            default_headers: self.default_headers,
            user_agent: self.user_agent,
            timeout: self.timeout,
            redirect: self.redirect,
            #[cfg(feature = "cookies")]
            cookie_jar,
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

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_url(url_str: &str) -> Result<url::Url> {
    let url = url::Url::parse(url_str).map_err(|e| Error::InvalidUrl(e.to_string()))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        other => Err(Error::Unsupported(format!(
            "URL scheme '{other}' is not supported; use http or https"
        ))),
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
        let out = apply_content_length(headers, &body, None).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "0");
    }

    #[test]
    fn apply_content_length_bytes_body() {
        let headers = Headers::new();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let out = apply_content_length(headers, &body, None).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "5");
    }

    #[test]
    fn apply_content_length_stream_known() {
        let headers = Headers::new();
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = crate::body::RequestBody::from_stream(stream, Some(7));
        let out = apply_content_length(headers, &body, None).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "7");
    }

    #[test]
    fn apply_content_length_stream_unknown() {
        let headers = Headers::new();
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = crate::body::RequestBody::from_stream(stream, None);
        let out = apply_content_length(headers, &body, None).unwrap();
        assert!(out.get("content-length").is_none());
    }

    #[test]
    fn apply_content_length_user_matches() {
        let mut headers = Headers::new();
        headers.insert("content-length", "5").unwrap();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let out = apply_content_length(headers, &body, Some(5)).unwrap();
        assert_eq!(out.get("content-length").unwrap().to_str().unwrap(), "5");
    }

    #[test]
    fn apply_content_length_user_mismatch_errors() {
        let mut headers = Headers::new();
        headers.insert("content-length", "10").unwrap();
        let body = crate::body::RequestBody::from(Bytes::from("hello"));
        let err = apply_content_length(headers, &body, Some(10)).unwrap_err();
        assert_eq!(err.kind(), "request_build");
    }
}
