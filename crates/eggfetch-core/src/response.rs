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

use crate::body::{BoxBytesStream, ResponseBody};
use crate::error::Result;

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
        let reason = response.status.canonical_reason().unwrap_or("").to_string();
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
pub struct Response {
    status: StatusCode,
    version: Version,
    headers: HeaderMap,
    url: Url,
    pub(crate) body: ResponseBody,
    history: Vec<HistoryEntry>,
}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("version", &self.version)
            .field("headers", &redacted_headers(&self.headers))
            .field("url", &redacted_url(&self.url))
            .field("body", &self.body)
            .field("history", &self.history)
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
        Self {
            status,
            version,
            headers,
            url,
            body,
            history: Vec::new(),
        }
    }

    /// Replace the body of this response. Crate-internal: used to attach
    /// a leased body after pool acquisition.
    pub(crate) fn set_body(&mut self, body: ResponseBody) {
        self.body = body;
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
}

/// Stream adapter that splits a byte stream into text lines.
struct LineStream {
    stream: BoxBytesStream,
    buffer: BytesMut,
}

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
        // Try to find a newline in the buffer first.
        if let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
            let line_bytes = self.buffer.split_to(pos + 1);
            let mut line = &line_bytes[..line_bytes.len() - 1]; // strip \n
            if line.ends_with(b"\r") {
                line = &line[..line.len() - 1];
            }
            let line = String::from_utf8(line.to_vec())
                .map_err(|e| crate::error::Error::Body(e.to_string()))?;
            return Poll::Ready(Some(Ok(line)));
        }

        // No newline in buffer; try to pull more data.
        match Pin::new(&mut self.stream).poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                self.buffer.extend_from_slice(&chunk);
                // Re-schedule to check for newlines in the updated buffer.
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => {
                // Stream ended; flush remaining buffer as a final line.
                if self.buffer.is_empty() {
                    Poll::Ready(None)
                } else {
                    let remaining = std::mem::take(&mut self.buffer);
                    let mut line = &remaining[..remaining.len()];
                    if line.ends_with(b"\r") {
                        line = &line[..line.len() - 1];
                    }
                    let line = String::from_utf8(line.to_vec())
                        .map_err(|e| crate::error::Error::Body(e.to_string()))?;
                    Poll::Ready(Some(Ok(line)))
                }
            }
            Poll::Pending => Poll::Pending,
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
}
