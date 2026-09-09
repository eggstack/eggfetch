//! Incoming response type.
//!
//! A `Response` holds the HTTP status, version, headers, final URL, and a
//! body handle. The body can be consumed in two ways:
//!
//! - **Buffered**: `bytes()` or `text()` collects the entire body.
//! - **Streaming**: `bytes_stream()` yields decoded body chunks incrementally;
//!   `raw_bytes_stream()` selects encoded chunks for compressed responses.
//!
//! # Body ownership
//!
//! The body can be consumed exactly once. After calling `bytes()`,
//! `text()`, `bytes_stream()`, or `raw_bytes_stream()`, further reads return
//! an error.

use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Bytes, BytesMut};
use futures_core::Stream;
use http::{HeaderMap, StatusCode, Version};
use url::Url;

use crate::body::{BoxBytesStream, ResponseBody, SharedTrailers};
use crate::error::Result;
use crate::network_stream::NetworkStream;

/// A metadata-only snapshot of a redirect response.
///
/// History entries intentionally omit the response body. This makes it
/// structurally impossible for redirect history to retain active body
/// streams or pool permits.
#[derive(Clone)]
pub struct HistoryEntry {
    status: StatusCode,
    version: Version,
    headers: HeaderMap,
    url: Url,
    reason_phrase: String,
}

impl std::fmt::Debug for HistoryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryEntry")
            .field("status", &self.status)
            .field("version", &self.version)
            .field("headers", &redacted_headers(&self.headers))
            .field("url", &redacted_url(&self.url))
            .field("reason_phrase", &self.reason_phrase)
            .finish()
    }
}

impl HistoryEntry {
    /// Create a history entry from a response that has already been
    /// drained (body consumed).
    pub(crate) fn from_response(response: &Response) -> Self {
        let reason = response
            .wire_reason_phrase
            .clone()
            .or_else(|| response.status.canonical_reason().map(str::to_owned))
            .unwrap_or_default();
        Self {
            status: response.status,
            version: response.version,
            headers: response.headers.clone(),
            url: response.url.clone(),
            reason_phrase: reason,
        }
    }

    /// Returns the HTTP status code.
    #[must_use]
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Returns the HTTP version.
    #[must_use]
    pub fn version(&self) -> Version {
        self.version
    }

    /// Returns the response headers.
    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns the URL.
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Returns the reason phrase.
    #[must_use]
    pub fn reason_phrase(&self) -> &str {
        &self.reason_phrase
    }
}

/// An HTTP response.
///
/// # Trailers
///
/// HTTP trailers (HTTP/1.1 chunked trailers, HTTP/2 trailing HEADERS, and
/// HTTP/3 trailing headers where the h3 crate exposes them) are captured
/// without buffering the body. The body stream yields data frames exactly
/// as before; when the trailing-headers frame arrives it is stored and the
/// stream ends normally. Use [`Self::trailers`] after fully consuming the
/// body (`bytes()`, `bytes_stream()` to EOF) to retrieve them.
///
/// Lifecycle: `trailers()` returns `None` until the body has advanced far
/// enough for trailers to arrive, when the response carried no trailers,
/// or when a body error occurred before trailers (errors remain body
/// errors, never fabricated trailer state). Partial consumption or early
/// drop never reports trailers and releases the pool permit without
/// waiting for them. Read timeouts apply while waiting for trailers at
/// the body boundary; the explicit native `total` deadline bounds
/// transport setup and is not restarted per trailer wait.
pub struct Response {
    status: StatusCode,
    version: Version,
    headers: HeaderMap,
    url: Url,
    /// Original wire value retained for compatibility adapters that expose
    /// response metadata after automatic decompression.
    wire_content_encoding: Option<String>,
    /// Original wire value retained for compatibility adapters that expose
    /// response metadata after automatic decompression.
    wire_content_length: Option<String>,
    /// Original wire reason phrase from the HTTP status line.
    ///
    /// HTTP/1.x responses include a reason phrase after the status code.
    /// This field preserves the original wire bytes so compatibility
    /// adapters can expose the exact phrase rather than deriving one from
    /// the status code alone. HTTP/2 has no wire reason phrase; this
    /// field will be `None` for H2 responses.
    wire_reason_phrase: Option<String>,
    pub(crate) body: ResponseBody,
    history: Vec<HistoryEntry>,
    /// Shared trailer store populated as the body stream advances.
    ///
    /// Cloned into the body stream at construction so `trailers()` stays
    /// available after `bytes()` collection. `None` until trailers arrive.
    trailers: SharedTrailers,
    /// Optional network stream handle for connection metadata and
    /// upgraded-connection IO. Set for responses where the underlying
    /// transport metadata is available:
    ///
    /// - **101 Switching Protocols**: set to `NetworkStream::Upgraded`
    ///   with full IO access.
    /// - **Successful CONNECT (200)**: set to `NetworkStream::Upgraded`
    ///   when the tunnel is established (handled in the proxy path).
    /// - **Ordinary HTTP/1.1 and HTTP/2 responses**: always `None`.
    ///   Hyper's connection pool retains ownership of the socket and
    ///   does not expose per-response socket metadata. Raw IO would
    ///   corrupt pool state. This is a documented bounded difference
    ///   from httpcore, which owns its connections directly.
    network_stream: Option<NetworkStream>,
}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("version", &self.version)
            .field("headers", &redacted_headers(&self.headers))
            .field("url", &redacted_url(&self.url))
            .field("wire_content_encoding", &self.wire_content_encoding)
            .field("wire_content_length", &self.wire_content_length)
            .field("wire_reason_phrase", &self.wire_reason_phrase)
            .field("body", &self.body)
            .field("history", &self.history)
            .field(
                "trailers",
                &self.trailers.get().map(|h| redacted_headers(&h)),
            )
            .field(
                "network_stream",
                &self.network_stream.as_ref().map(|_| "..."),
            )
            .finish()
    }
}

fn redacted_headers(headers: &HeaderMap) -> HeaderMap {
    crate::redact::redact_headers(headers)
}

fn redacted_url(url: &Url) -> String {
    crate::redact::redact_url(url)
}

impl Response {
    /// Create a new response (crate-internal).
    pub(crate) fn new(
        status: StatusCode,
        version: Version,
        headers: HeaderMap,
        url: Url,
        body: ResponseBody,
    ) -> Self {
        let wire_content_encoding = headers
            .get("content-encoding")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let wire_content_length = headers
            .get("content-length")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        Self {
            status,
            version,
            headers,
            url,
            wire_content_encoding,
            wire_content_length,
            wire_reason_phrase: None,
            body,
            history: Vec::new(),
            trailers: SharedTrailers::new(),
            network_stream: None,
        }
    }

    /// Replace the body of this response. Crate-internal: used to attach
    /// a leased body after pool acquisition.
    pub(crate) fn set_body(&mut self, body: ResponseBody) {
        self.body = body;
    }

    /// Attach the shared trailer store linked to the body stream.
    ///
    /// Transports create one [`SharedTrailers`], hand a clone to the
    /// streaming adapter, and install the same instance here so
    /// [`Self::trailers`] observes trailers populated during streaming.
    pub(crate) fn set_trailers(&mut self, trailers: SharedTrailers) {
        self.trailers = trailers;
    }

    /// Returns received HTTP trailers, if any.
    ///
    /// Returns `None` until the body has been fully consumed and trailers
    /// have arrived, when the response carried no trailers, or when a body
    /// error occurred before trailers. Duplicate/multi-value trailing
    /// headers are preserved in the returned [`HeaderMap`].
    ///
    /// This never buffers the body: drive the body to EOF via `bytes()` or
    /// `bytes_stream()` first, then call this method.
    #[must_use]
    pub fn trailers(&self) -> Option<HeaderMap> {
        self.trailers.get()
    }

    /// Consume the response and return its body.
    #[allow(dead_code)]
    pub(crate) fn into_body(self) -> ResponseBody {
        self.body
    }

    /// Returns the HTTP status code.
    #[must_use]
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Returns the HTTP version.
    #[must_use]
    pub fn version(&self) -> Version {
        self.version
    }

    /// Returns the response headers.
    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns a mutable reference to the response headers.
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        &mut self.headers
    }

    /// Returns the original wire `Content-Encoding` value, if present.
    ///
    /// Automatic decompression may remove this value from [`Self::headers`];
    /// compatibility adapters can use this read-only snapshot to preserve
    /// wire metadata without changing the core decoded-header policy.
    #[must_use]
    pub fn wire_content_encoding(&self) -> Option<&str> {
        self.wire_content_encoding.as_deref()
    }

    /// Returns the original wire `Content-Length` value, if present.
    ///
    /// The value describes encoded wire bytes and is never derived from the
    /// decompressed response body.
    #[must_use]
    pub fn wire_content_length(&self) -> Option<&str> {
        self.wire_content_length.as_deref()
    }

    /// Returns the original wire reason phrase, if present.
    ///
    /// HTTP/1.x responses include a reason phrase after the status code.
    /// This preserves the original wire bytes rather than deriving from
    /// the status code alone. HTTP/2 has no wire reason phrase.
    #[must_use]
    pub fn wire_reason_phrase(&self) -> Option<&str> {
        self.wire_reason_phrase.as_deref()
    }

    /// Set the wire reason phrase for this response.
    ///
    /// Used by transports that parse the HTTP/1.x status line directly
    /// and can extract the original reason phrase.
    #[allow(dead_code)]
    pub(crate) fn set_wire_reason_phrase(&mut self, reason: Option<String>) {
        self.wire_reason_phrase = reason;
    }

    /// Returns the final URL of the response (after any redirects).
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Returns the redirect history (prior responses in order).
    #[must_use]
    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }

    /// Returns a mutable reference to the redirect history.
    pub fn history_mut(&mut self) -> &mut Vec<HistoryEntry> {
        &mut self.history
    }

    /// Set the redirect history.
    pub(crate) fn set_history(&mut self, history: Vec<HistoryEntry>) {
        self.history = history;
    }

    /// Returns `true` if the status code indicates success (2xx).
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    /// Returns a reference to the response body.
    #[must_use]
    pub fn body(&self) -> &ResponseBody {
        &self.body
    }

    /// Returns a mutable reference to the response body.
    pub fn body_mut(&mut self) -> &mut ResponseBody {
        &mut self.body
    }

    /// Consume the response and return the body bytes.
    ///
    /// For buffered bodies, this is O(1). For streaming bodies, this
    /// collects all chunks.
    ///
    /// # Errors
    ///
    /// Returns an error if the body has already been consumed or if a
    /// stream chunk fails.
    pub async fn bytes(&mut self) -> Result<Bytes> {
        self.body.bytes().await
    }

    /// Consume the response and return the body as a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns an error if the body is not valid UTF-8 or has already
    /// been consumed.
    pub async fn text(&mut self) -> Result<String> {
        self.body.text().await
    }

    /// Consume the response body and return a stream of byte chunks.
    ///
    /// For buffered bodies, yields the entire body as a single chunk.
    /// For streaming bodies, yields chunks incrementally.
    ///
    /// # Errors
    ///
    /// Returns an error if the body has already been consumed.
    pub fn bytes_stream(&mut self) -> Result<BoxBytesStream> {
        self.body.bytes_stream()
    }

    /// Consume the response body and return encoded byte chunks.
    ///
    /// Compressed streaming responses select the original encoded transport
    /// body. Raw and decoded body selection are mutually exclusive and
    /// single-consumption operations.
    ///
    /// # Errors
    ///
    /// Returns an error if the response body has already been consumed.
    pub fn raw_bytes_stream(&mut self) -> Result<BoxBytesStream> {
        self.body.raw_bytes_stream()
    }

    /// Consume the response body and return a stream of text lines.
    ///
    /// Each line is decoded as UTF-8 without the trailing newline.
    /// Incomplete lines at chunk boundaries are buffered until the
    /// next chunk completes them.
    ///
    /// # Errors
    ///
    /// Returns an error if the body has already been consumed or if a
    /// chunk contains invalid UTF-8.
    pub fn text_lines(&mut self) -> Result<impl Stream<Item = Result<String>>> {
        let byte_stream = self.bytes_stream()?;
        Ok(LineStream::new(byte_stream))
    }

    /// Returns a reference to the network stream handle, if available.
    ///
    /// For upgraded connections (101/CONNECT), this provides full
    /// IO access. For ordinary pooled connections, this provides
    /// read-only metadata.
    #[must_use]
    pub fn network_stream(&self) -> Option<&NetworkStream> {
        self.network_stream.as_ref()
    }

    /// Returns a mutable reference to the network stream handle.
    pub fn network_stream_mut(&mut self) -> Option<&mut NetworkStream> {
        self.network_stream.as_mut()
    }

    /// Take the network stream out of the response, leaving `None` in
    /// its place. This is the canonical way to obtain ownership of an
    /// upgraded stream while still keeping the rest of the response.
    #[must_use]
    pub fn take_network_stream(&mut self) -> Option<NetworkStream> {
        self.network_stream.take()
    }

    /// Set the network stream handle for this response.
    pub(crate) fn set_network_stream(&mut self, stream: NetworkStream) {
        self.network_stream = Some(stream);
    }

    /// Consume the response and return the network stream, if any.
    ///
    /// This is the primary way to obtain ownership of an upgraded
    /// stream (101 Switching Protocols or successful CONNECT).
    #[must_use]
    pub fn into_network_stream(self) -> Option<NetworkStream> {
        self.network_stream
    }
}

/// Stream adapter that splits a byte stream into text lines.
struct LineStream {
    stream: BoxBytesStream,
    buffer: BytesMut,
}

const MAX_LINE_LENGTH: usize = 1024 * 1024;

impl LineStream {
    fn new(stream: BoxBytesStream) -> Self {
        Self {
            stream,
            buffer: BytesMut::new(),
        }
    }
}

impl Stream for LineStream {
    type Item = Result<String>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Fast path: emit a line already sitting in the buffer. The
            // length cap applies here too — a single chunk may carry an
            // over-long line piggybacked behind a valid one.
            if let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
                if pos > MAX_LINE_LENGTH {
                    return Poll::Ready(Some(Err(crate::error::Error::Body(format!(
                        "response line exceeded maximum length of {MAX_LINE_LENGTH} bytes"
                    )))));
                }
                let line_bytes = self.buffer.split_to(pos + 1);
                let mut line = &line_bytes[..line_bytes.len() - 1]; // strip \n
                if line.ends_with(b"\r") {
                    line = &line[..line.len() - 1];
                }
                let line = std::str::from_utf8(line)
                    .map(str::to_owned)
                    .map_err(|e| crate::error::Error::Body(e.to_string()))?;
                return Poll::Ready(Some(Ok(line)));
            }

            // No newline yet; a partially assembled line must stay bounded.
            if self.buffer.len() > MAX_LINE_LENGTH {
                return Poll::Ready(Some(Err(crate::error::Error::Body(format!(
                    "response line exceeded maximum length of {MAX_LINE_LENGTH} bytes"
                )))));
            }

            // Pull more data. Ready chunks are drained in this loop rather
            // than rescheduling through the executor per chunk.
            match Pin::new(&mut self.stream).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    if chunk.is_empty() {
                        // Empty chunks carry no data. Ignore them and let the
                        // upstream stream determine whether another wake is
                        // needed.
                        continue;
                    }
                    // Bound the buffer before extending: a single chunk of
                    // size `MAX+1` without a newline would otherwise push the
                    // buffer to `MAX + chunk` before the next loop's limit
                    // check. If the combined buffer would exceed `MAX` and
                    // there is no newline within the first `MAX` bytes, reject
                    // immediately without allocating the oversize tail.
                    if self.buffer.len().saturating_add(chunk.len()) > MAX_LINE_LENGTH {
                        let remaining = MAX_LINE_LENGTH.saturating_sub(self.buffer.len());
                        // `buffer` is known to contain no newline (otherwise
                        // we would have emitted at the top of the loop), so
                        // only need to inspect the prefix of `chunk` that fits
                        // within the limit.
                        let prefix = &chunk[..remaining.min(chunk.len())];
                        if !prefix.contains(&b'\n') {
                            // Also check if chunk itself contains an overlong
                            // line that straddles the boundary: if the first
                            // newline in the full chunk is beyond `MAX`, the
                            // first line is already overlong.
                            if let Some(pos) = chunk.iter().position(|&b| b == b'\n') {
                                if self.buffer.len() + pos > MAX_LINE_LENGTH {
                                    return Poll::Ready(Some(Err(
                                        crate::error::Error::Body(format!(
                                            "response line exceeded maximum length of {MAX_LINE_LENGTH} bytes"
                                        )),
                                    )));
                                }
                            } else {
                                return Poll::Ready(Some(Err(
                                    crate::error::Error::Body(format!(
                                        "response line exceeded maximum length of {MAX_LINE_LENGTH} bytes"
                                    )),
                                )));
                            }
                        }
                    }
                    self.buffer.extend_from_slice(&chunk);
                }
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
                Poll::Ready(None) => {
                    // Stream ended; flush remaining buffer as a final line.
                    if self.buffer.is_empty() {
                        return Poll::Ready(None);
                    }
                    let remaining = std::mem::take(&mut self.buffer);
                    if remaining.len() > MAX_LINE_LENGTH {
                        return Poll::Ready(Some(Err(crate::error::Error::Body(format!(
                            "response line exceeded maximum length of {MAX_LINE_LENGTH} bytes"
                        )))));
                    }
                    let mut line = &remaining[..remaining.len()];
                    if line.ends_with(b"\r") {
                        line = &line[..line.len() - 1];
                    }
                    let line = std::str::from_utf8(line)
                        .map(str::to_owned)
                        .map_err(|e| crate::error::Error::Body(e.to_string()))?;
                    return Poll::Ready(Some(Ok(line)));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;

    #[test]
    fn response_debug_redacts_sensitive_headers_and_url_parts() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Bearer secret"));
        headers.insert("set-cookie", HeaderValue::from_static("session=secret"));
        let response = Response::new(
            StatusCode::FOUND,
            Version::HTTP_11,
            headers,
            Url::parse("https://user:pass@example.com/path?token=secret#fragment").unwrap(),
            ResponseBody::buffered(Bytes::new()),
        );
        let rendered = format!("{response:?}");
        assert!(!rendered.contains("secret"));
        assert!(!rendered.contains("user:pass"));
        assert!(rendered.contains("https://example.com/path"));
    }

    #[test]
    fn response_retains_wire_content_metadata_without_changing_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", HeaderValue::from_static("gzip"));
        headers.insert("content-length", HeaderValue::from_static("42"));
        let response = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            headers,
            Url::parse("http://example.com").unwrap(),
            ResponseBody::buffered(Bytes::new()),
        );

        assert_eq!(response.wire_content_encoding(), Some("gzip"));
        assert_eq!(response.wire_content_length(), Some("42"));
        assert_eq!(
            response.headers().get("content-encoding"),
            Some(&HeaderValue::from_static("gzip"))
        );
        assert_eq!(
            response.headers().get("content-length"),
            Some(&HeaderValue::from_static("42"))
        );
    }
    use futures_util::StreamExt;

    #[tokio::test]
    async fn response_bytes_and_text() {
        let mut resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::buffered(Bytes::from("hello")),
        );
        assert!(resp.is_success());
        let text = resp.text().await.unwrap();
        assert_eq!(text, "hello");
    }

    #[tokio::test]
    async fn response_streaming_bytes_collect() {
        let chunks = vec![Ok(Bytes::from("hello ")), Ok(Bytes::from("world"))];
        let stream = Box::pin(futures_util::stream::iter(chunks));
        let mut resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::streaming(stream),
        );
        let bytes = resp.bytes().await.unwrap();
        assert_eq!(bytes, "hello world");
    }

    #[tokio::test]
    async fn response_text_lines_rejects_unbounded_line() {
        let stream = Box::pin(futures_util::stream::iter(vec![Ok(Bytes::from(vec![
            b'a'; MAX_LINE_LENGTH + 1
        ]))]));
        let mut response = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::streaming(stream),
        );
        let mut lines = response.text_lines().unwrap();
        let error = lines.next().await.unwrap().unwrap_err();
        assert!(matches!(error, crate::error::Error::Body(_)));
    }

    #[tokio::test]
    async fn response_text_lines_rejects_overlong_line_after_valid_line() {
        // A single chunk carrying a valid line followed by an over-long
        // line must not smuggle the second line past MAX_LINE_LENGTH via
        // the buffered fast path.
        let mut payload = b"ok\n".to_vec();
        payload.extend_from_slice(&vec![b'a'; MAX_LINE_LENGTH + 1]);
        payload.push(b'\n');
        let stream = Box::pin(futures_util::stream::iter(vec![Ok(Bytes::from(payload))]));
        let mut resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::streaming(stream),
        );
        let mut lines = resp.text_lines().unwrap();
        assert_eq!(lines.next().await.unwrap().unwrap(), "ok");
        let error = lines.next().await.unwrap().unwrap_err();
        assert!(matches!(error, crate::error::Error::Body(_)));
    }

    #[tokio::test]
    async fn response_text_lines_rejects_unbounded_terminal_line() {
        // The terminal flush path (`Poll::Ready(None)`) must enforce
        // `MAX_LINE_LENGTH` even when the over-long buffer never sees a
        // trailing newline.
        let stream = Box::pin(futures_util::stream::iter(vec![Ok(Bytes::from(vec![
            b'a'; MAX_LINE_LENGTH + 1
        ]))]));
        let mut resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::streaming(stream),
        );
        let mut lines = resp.text_lines().unwrap();
        let error = lines.next().await.unwrap().unwrap_err();
        assert!(matches!(error, crate::error::Error::Body(_)));
    }

    #[tokio::test]
    async fn response_bytes_stream_yields_chunks() {
        let chunks = vec![Ok(Bytes::from("a")), Ok(Bytes::from("b"))];
        let stream = Box::pin(futures_util::stream::iter(chunks));
        let mut resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::streaming(stream),
        );
        let mut stream = resp.bytes_stream().unwrap();
        assert_eq!(stream.next().await.unwrap().unwrap(), "a");
        assert_eq!(stream.next().await.unwrap().unwrap(), "b");
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn response_text_lines_basic() {
        let chunks = vec![Ok(Bytes::from("line1\nline2\nline3\n"))];
        let stream = Box::pin(futures_util::stream::iter(chunks));
        let mut resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::streaming(stream),
        );
        let mut lines = resp.text_lines().unwrap();
        assert_eq!(lines.next().await.unwrap().unwrap(), "line1");
        assert_eq!(lines.next().await.unwrap().unwrap(), "line2");
        assert_eq!(lines.next().await.unwrap().unwrap(), "line3");
        assert!(lines.next().await.is_none());
    }

    #[tokio::test]
    async fn response_text_lines_crlf() {
        let chunks = vec![Ok(Bytes::from("line1\r\nline2\r\n"))];
        let stream = Box::pin(futures_util::stream::iter(chunks));
        let mut resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::streaming(stream),
        );
        let mut lines = resp.text_lines().unwrap();
        assert_eq!(lines.next().await.unwrap().unwrap(), "line1");
        assert_eq!(lines.next().await.unwrap().unwrap(), "line2");
        assert!(lines.next().await.is_none());
    }

    #[tokio::test]
    async fn response_text_lines_across_chunks() {
        let chunks = vec![
            Ok(Bytes::from("hel")),
            Ok(Bytes::from("lo\nwor")),
            Ok(Bytes::from("ld\n")),
        ];
        let stream = Box::pin(futures_util::stream::iter(chunks));
        let mut resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::streaming(stream),
        );
        let mut lines = resp.text_lines().unwrap();
        assert_eq!(lines.next().await.unwrap().unwrap(), "hello");
        assert_eq!(lines.next().await.unwrap().unwrap(), "world");
        assert!(lines.next().await.is_none());
    }

    #[tokio::test]
    async fn response_text_lines_no_trailing_newline() {
        let chunks = vec![Ok(Bytes::from("hello"))];
        let stream = Box::pin(futures_util::stream::iter(chunks));
        let mut resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::streaming(stream),
        );
        let mut lines = resp.text_lines().unwrap();
        assert_eq!(lines.next().await.unwrap().unwrap(), "hello");
        assert!(lines.next().await.is_none());
    }

    #[test]
    fn trailers_absent_initially() {
        let resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::buffered(Bytes::new()),
        );
        assert!(resp.trailers().is_none());
    }

    #[test]
    fn trailers_preserve_duplicate_multi_value() {
        use crate::body::SharedTrailers;
        let shared = SharedTrailers::new();
        let mut map = HeaderMap::new();
        map.append("x-trailer", HeaderValue::from_static("a"));
        map.append("x-trailer", HeaderValue::from_static("b"));
        map.insert("x-single", HeaderValue::from_static("v"));
        shared.store(map);
        let mut resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::buffered(Bytes::new()),
        );
        resp.set_trailers(shared);
        let trailers = resp.trailers().expect("trailers present");
        let values: Vec<_> = trailers
            .get_all("x-trailer")
            .iter()
            .map(|v| v.to_str().unwrap().to_owned())
            .collect();
        assert_eq!(values, vec!["a", "b"]);
        assert_eq!(trailers.get("x-single").unwrap(), "v");
    }

    #[tokio::test]
    async fn trailers_available_after_bytes_collection() {
        use crate::body::SharedTrailers;
        // Simulate a transport that populates trailers when the body is
        // driven to EOF (same contract as `wrap_incoming`).
        let shared = SharedTrailers::new();
        let shared_for_stream = shared.clone();
        let stream = Box::pin(futures_util::stream::unfold(0, move |step| {
            let shared_for_stream = shared_for_stream.clone();
            async move {
                if step == 0 {
                    Some((Ok(Bytes::from("hello")), 1))
                } else if step == 1 {
                    let mut map = HeaderMap::new();
                    map.insert("x-done", HeaderValue::from_static("yes"));
                    shared_for_stream.store(map);
                    None
                } else {
                    None
                }
            }
        }));
        let mut resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::streaming(stream),
        );
        resp.set_trailers(shared);
        assert!(
            resp.trailers().is_none(),
            "partial consumption reports None"
        );
        let bytes = resp.bytes().await.unwrap();
        assert_eq!(bytes, "hello");
        assert_eq!(
            resp.trailers().unwrap().get("x-done").unwrap(),
            "yes",
            "trailers available after body completion"
        );
    }

    #[tokio::test]
    async fn partial_consumption_does_not_falsely_report_trailers() {
        let stream = Box::pin(futures_util::stream::iter(vec![Ok(Bytes::from("a"))]));
        let mut resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::streaming(stream),
        );
        // No trailers wired; even after partial read there must be none.
        let mut s = resp.bytes_stream().unwrap();
        assert_eq!(s.next().await.unwrap().unwrap(), "a");
        assert!(resp.trailers().is_none());
    }

    #[tokio::test]
    async fn body_error_before_trailers_leaves_no_trailers() {
        use crate::body::SharedTrailers;
        let shared = SharedTrailers::new();
        let stream = Box::pin(futures_util::stream::iter(vec![
            Ok(Bytes::from("a")),
            Err(crate::error::Error::Body("boom".into())),
        ]));
        let mut resp = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            HeaderMap::new(),
            Url::parse("http://example.com").unwrap(),
            ResponseBody::streaming(stream),
        );
        resp.set_trailers(shared);
        let err = resp.bytes().await.unwrap_err();
        assert!(matches!(err, crate::error::Error::Body(_)));
        assert!(
            resp.trailers().is_none(),
            "body errors must not fabricate trailer state"
        );
    }
}
