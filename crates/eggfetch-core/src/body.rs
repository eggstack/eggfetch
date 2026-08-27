//! Body model for requests and responses.
//!
//! # Body ownership rules
//!
//! Both request and response bodies follow single-consumption semantics:
//! a body can be consumed exactly once (by buffering or streaming). Once
//! consumed, further reads return an error or produce no data.
//!
//! # Streaming vs buffering
//!
//! Response bodies can be consumed in two ways:
//!
//! - **Buffered**: `bytes()` or `text()` collects the entire body into
//!   memory. Suitable for small-to-medium responses.
//! - **Streaming**: `bytes_stream()` yields body chunks incrementally.
//!   Suitable for large responses or when incremental processing is needed.
//!
//! Request bodies can be:
//!
//! - **Empty**: no body sent.
//! - **Bytes**: a fixed-size byte buffer with known length. Sent as a
//!   `Content-Length`-delimited body.
//! - **Stream**: an async stream of bytes. Piped through to the transport
//!   incrementally (no eager buffering). When `length` is `Some(n)`, the
//!   body is sent as a `Content-Length`-delimited body. When `length` is
//!   `None`, hyper's HTTP/1.1 machinery selects a safe transfer mode
//!   (e.g. chunked transfer encoding for HTTP/1.1).
//!
//! # Response lease (pool permit lifecycle)
//!
//! Streaming response bodies carry an internal `Arc<PoolGuard>` that
//! holds the pool permits acquired for the request. The permits are
//! released when the response body is dropped or fully consumed. This
//! guarantees that per-origin concurrency limits remain meaningful while
//! response bodies are in flight: a streaming response that is held
//! but not consumed continues to occupy its pool slot. Buffered and
//! already-consumed responses do not carry a lease.

use std::pin::Pin;
use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use futures_core::Stream;
use futures_util::StreamExt;
use http_body::Frame;
use pin_project_lite::pin_project;

use crate::compression::DecompressionLimit;
use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// Request body
// ---------------------------------------------------------------------------

/// A type-erased async stream of `Result<Bytes>` chunks.
pub type BoxBytesStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>;

/// Request body.
///
/// Supports empty bodies, fixed-size byte buffers, and async streams.
/// `Stream` bodies are **piped through** to the transport incrementally:
/// each chunk is sent as soon as it is produced, with no eager buffering.
/// The optional `length` hint drives `Content-Length` for known-size
/// streams; unknown-length streams use HTTP/1.1 chunked transfer encoding
/// (selected by hyper).
#[derive(Default)]
pub enum RequestBody {
    /// Empty body.
    #[default]
    Empty,
    /// Byte body with known length.
    Bytes(Bytes),
    /// Async stream body.
    ///
    /// Chunks are sent incrementally as the stream produces them. If
    /// `length` is `Some(n)`, a `Content-Length: n` header is set when
    /// the user has not supplied a conflicting value. If `length` is
    /// `None`, no `Content-Length` is set; the transport selects a
    /// safe transfer mode (chunked for HTTP/1.1).
    Stream {
        /// The stream of body chunks.
        stream: BoxBytesStream,
        /// Known length, if available.
        length: Option<usize>,
    },
}

/// Replayability classification for request bodies.
///
/// Used by redirect and retry code to determine whether a body can be
/// re-sent without re-consuming the original source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayClass {
    /// Empty or immutable byte body — always replayable.
    Immutable,
    /// Seekable body that can be replayed by resetting position.
    Seekable,
    /// One-shot stream — cannot be replayed.
    OneShot,
    /// Body has been consumed.
    Consumed,
}

impl std::fmt::Debug for RequestBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.debug_tuple("Empty").finish(),
            Self::Bytes(b) => f.debug_tuple("Bytes").field(b).finish(),
            Self::Stream { length, .. } => f
                .debug_struct("Stream")
                .field("length", length)
                .finish_non_exhaustive(),
        }
    }
}

impl RequestBody {
    /// Returns `true` if the body is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Returns the known length of the body, if available.
    ///
    /// Stream bodies may not have a known length until fully consumed.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Bytes(b) => b.len(),
            Self::Stream { length, .. } => length.unwrap_or(0),
        }
    }

    /// Returns `true` if the body length is known.
    #[must_use]
    pub fn has_known_length(&self) -> bool {
        match self {
            Self::Empty | Self::Bytes(_) => true,
            Self::Stream { length, .. } => length.is_some(),
        }
    }

    /// Returns `true` if this body is replayable (can be sent multiple times).
    ///
    /// Only byte bodies are replayable. Streams are consumed on use.
    #[must_use]
    pub fn is_replayable(&self) -> bool {
        matches!(self, Self::Empty | Self::Bytes(_))
    }

    /// Returns the replayability classification for this body.
    #[must_use]
    pub fn replay_class(&self) -> ReplayClass {
        match self {
            Self::Empty | Self::Bytes(_) => ReplayClass::Immutable,
            Self::Stream { .. } => ReplayClass::OneShot,
        }
    }

    /// Attempt to clone this body for use in a redirect request.
    ///
    /// Returns a cloned body if the body is replayable (empty or bytes),
    /// or an error if the body is a live stream that cannot be resent.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BodyNotReplayableForRedirect`] if the body is a
    /// live stream that cannot be replayed for a redirect.
    pub fn try_clone_for_redirect(&self) -> Result<Self> {
        match self {
            Self::Empty => Ok(Self::Empty),
            Self::Bytes(b) => Ok(Self::Bytes(b.clone())),
            Self::Stream { .. } => Err(Error::BodyNotReplayableForRedirect),
        }
    }

    /// Consume the body and return all bytes.
    ///
    /// For byte bodies, returns the bytes directly. For stream bodies,
    /// collects all chunks. For empty bodies, returns empty bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if a stream chunk fails.
    pub async fn into_bytes(self) -> Result<Bytes> {
        use futures_util::StreamExt;
        match self {
            Self::Empty => Ok(Bytes::new()),
            Self::Bytes(b) => Ok(b),
            Self::Stream { mut stream, .. } => {
                let mut buf = BytesMut::new();
                while let Some(chunk) = stream.next().await {
                    buf.extend_from_slice(&chunk?);
                }
                Ok(buf.freeze())
            }
        }
    }

    /// Create a stream body with known length.
    pub fn from_stream<S>(stream: S, length: Option<usize>) -> Self
    where
        S: Stream<Item = Result<Bytes>> + Send + 'static,
    {
        Self::Stream {
            stream: Box::pin(stream),
            length,
        }
    }

    /// Convert into a hyper-compatible boxed body.
    ///
    /// Stream bodies are wrapped as a `StreamBody` so that chunks are
    /// piped through to the transport incrementally. Bytes and empty
    /// bodies are wrapped as `Full` and `Empty` respectively. No eager
    /// buffering is performed.
    #[allow(clippy::type_complexity)]
    pub(crate) fn into_http_body(
        self,
    ) -> http_body_util::combinators::UnsyncBoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>>
    {
        use http_body_util::combinators::UnsyncBoxBody;
        use http_body_util::{BodyExt, Empty, Full, StreamBody};

        match self {
            Self::Empty => Empty::<Bytes>::new()
                .map_err(|err| -> Box<dyn std::error::Error + Send + Sync> { match err {} })
                .boxed_unsync(),
            Self::Bytes(b) => Full::new(b)
                .map_err(|err| -> Box<dyn std::error::Error + Send + Sync> { match err {} })
                .boxed_unsync(),
            Self::Stream { stream, .. } => {
                let framed = stream.map(|chunk_result| {
                    chunk_result
                        .map(Frame::data)
                        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })
                });
                let body: UnsyncBoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>> =
                    BodyExt::boxed_unsync(StreamBody::new(framed));
                body
            }
        }
    }
}

impl From<Bytes> for RequestBody {
    fn from(b: Bytes) -> Self {
        Self::Bytes(b)
    }
}

impl From<Vec<u8>> for RequestBody {
    fn from(v: Vec<u8>) -> Self {
        Self::Bytes(Bytes::from(v))
    }
}

impl From<&[u8]> for RequestBody {
    fn from(s: &[u8]) -> Self {
        Self::Bytes(Bytes::copy_from_slice(s))
    }
}

impl From<String> for RequestBody {
    fn from(s: String) -> Self {
        Self::Bytes(Bytes::from(s))
    }
}

impl From<&str> for RequestBody {
    fn from(s: &str) -> Self {
        Self::Bytes(Bytes::from(s.to_owned()))
    }
}

// ---------------------------------------------------------------------------
// Response body
// ---------------------------------------------------------------------------

/// A handle to a pool permit. Cloning the handle is cheap; dropping the
/// last handle releases the permit back to the pool.
pub(crate) type PoolGuardArc = Arc<crate::pool::PoolGuard>;

pin_project! {
    /// A response stream that keeps the pool lease alive while it is in use.
    struct LeasedResponseStream {
        #[pin]
        inner: BoxBytesStream,
        _lease: Option<PoolGuardArc>,
    }
}

impl Stream for LeasedResponseStream {
    type Item = Result<Bytes>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let mut this = self.project();
        let item = this.inner.as_mut().poll_next(cx);
        if matches!(item, std::task::Poll::Ready(Some(Err(_)) | None)) {
            // Release the permit as soon as the stream reaches a terminal
            // state, rather than waiting for the caller to drop the iterator.
            this._lease.take();
        }
        item
    }
}

/// Response body handle.
///
/// Supports two consumption modes:
///
/// - **Buffered**: the entire body is held in memory. Created by
///   `ResponseBody::buffered()`. Calling `bytes()` or `text()` is O(1).
/// - **Streaming**: body chunks are yielded incrementally. Created by
///   `ResponseBody::streaming()`. The stream must be fully consumed or
///   dropped before the attached pool lease is released.
///
/// # Single-consumption semantics
///
/// A response body can be consumed once via `bytes()`, `text()`,
/// `bytes_stream()`, or `raw_bytes_stream()`. For compressed streaming bodies,
/// raw and decoded selection are mutually exclusive. After consumption,
/// further reads return an error.
pub enum ResponseBody {
    /// Fully buffered body.
    Buffered {
        /// The buffered body bytes.
        bytes: Bytes,
    },
    /// Streaming body.
    Streaming {
        /// The body stream.
        stream: BoxBytesStream,
        /// Optional pool permit holder. Released on drop.
        lease: Option<PoolGuardArc>,
    },
    /// A compressed streaming body whose encoded source is retained until
    /// the caller selects raw or decoded consumption.
    EncodedStreaming {
        /// The encoded body stream.
        stream: BoxBytesStream,
        /// Optional pool permit holder. Released on drop.
        lease: Option<PoolGuardArc>,
        /// The original `Content-Encoding` header value.
        content_encoding: String,
        /// Limits applied when decoded mode is selected.
        limit: DecompressionLimit,
    },
    /// The streaming body has already been consumed.
    Consumed,
}

impl std::fmt::Debug for ResponseBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Buffered { bytes } => f
                .debug_struct("Buffered")
                .field("len", &bytes.len())
                .finish(),
            Self::Streaming { .. } => f.debug_struct("Streaming").finish_non_exhaustive(),
            Self::EncodedStreaming { .. } => {
                f.debug_struct("EncodedStreaming").finish_non_exhaustive()
            }
            Self::Consumed => f.debug_struct("Consumed").finish(),
        }
    }
}

impl ResponseBody {
    /// Create a buffered response body.
    #[must_use]
    pub fn buffered(bytes: Bytes) -> Self {
        Self::Buffered { bytes }
    }

    /// Create a streaming response body from a type-erased stream.
    #[must_use]
    pub fn streaming(stream: BoxBytesStream) -> Self {
        Self::Streaming {
            stream,
            lease: None,
        }
    }

    /// Create a streaming response body with an attached pool permit.
    /// The permit is released when the body is dropped.
    pub(crate) fn streaming_with_lease(stream: BoxBytesStream, lease: PoolGuardArc) -> Self {
        Self::Streaming {
            stream,
            lease: Some(lease),
        }
    }

    /// Create a deferred-decode streaming response body.
    pub(crate) fn encoded_streaming(
        stream: BoxBytesStream,
        content_encoding: String,
        limit: DecompressionLimit,
    ) -> Self {
        Self::EncodedStreaming {
            stream,
            lease: None,
            content_encoding,
            limit,
        }
    }

    /// Create a deferred-decode streaming response body with an attached
    /// pool permit.
    pub(crate) fn encoded_streaming_with_lease(
        stream: BoxBytesStream,
        lease: PoolGuardArc,
        content_encoding: String,
        limit: DecompressionLimit,
    ) -> Self {
        Self::EncodedStreaming {
            stream,
            lease: Some(lease),
            content_encoding,
            limit,
        }
    }

    /// Returns `true` if the body is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Buffered { bytes } => bytes.is_empty(),
            Self::Streaming { .. } | Self::EncodedStreaming { .. } => false, // unknown until consumed
            Self::Consumed => true,
        }
    }

    /// Returns the buffered body length, if available.
    ///
    /// For streaming bodies, returns `None` until the body is fully
    /// collected.
    #[must_use]
    pub fn len(&self) -> Option<usize> {
        match self {
            Self::Buffered { bytes } => Some(bytes.len()),
            Self::Streaming { .. } | Self::EncodedStreaming { .. } => None,
            Self::Consumed => Some(0),
        }
    }

    /// Returns `true` if this is a buffered body.
    #[must_use]
    pub fn is_buffered(&self) -> bool {
        matches!(self, Self::Buffered { .. })
    }

    /// Consume the body and return all bytes.
    ///
    /// For buffered bodies, this is O(1). For streaming bodies, this
    /// collects all chunks into a single `Bytes` buffer.
    ///
    /// # Limits
    ///
    /// For `EncodedStreaming` bodies the configured
    /// [`DecompressionLimit`] is enforced via the decompression pipeline.
    /// `Streaming` and `Buffered` bodies (e.g. a non-encoded `identity`
    /// response) do not carry a `DecompressionLimit`, so this method
    /// will buffer the entire body in memory.  Callers expecting large
    /// unencoded bodies should use [`Self::bytes_stream`] and apply
    /// their own size cap instead.
    ///
    /// [`DecompressionLimit`]: crate::compression::DecompressionLimit
    ///
    /// # Errors
    ///
    /// Returns an error if the body has already been consumed or if a
    /// stream chunk fails.
    pub async fn bytes(&mut self) -> Result<Bytes> {
        let old = std::mem::replace(self, Self::Consumed);
        match old {
            Self::Buffered { bytes } => Ok(bytes),
            Self::Streaming {
                mut stream, lease, ..
            } => {
                let mut buf = BytesMut::new();
                while let Some(chunk) = stream.next().await {
                    buf.extend_from_slice(&chunk?);
                }
                drop(lease);
                Ok(buf.freeze())
            }
            Self::EncodedStreaming {
                stream,
                lease,
                content_encoding,
                limit,
            } => {
                let mut stream = crate::compression::decompress_stream(
                    stream,
                    Some(&content_encoding),
                    true,
                    limit,
                )?;
                let mut buf = BytesMut::new();
                while let Some(chunk) = stream.next().await {
                    buf.extend_from_slice(&chunk?);
                }
                drop(lease);
                Ok(buf.freeze())
            }
            Self::Consumed => Err(Error::Body("body already consumed".into())),
        }
    }

    /// Consume the body and return it as a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns an error if the body is not valid UTF-8 or has already
    /// been consumed.
    pub async fn text(&mut self) -> Result<String> {
        let bytes = self.bytes().await?;
        String::from_utf8(bytes.to_vec()).map_err(|e| Error::Body(e.to_string()))
    }

    /// Consume the body and return a stream of byte chunks.
    ///
    /// For buffered bodies, yields the entire body as a single chunk.
    /// For streaming bodies, yields chunks incrementally.
    ///
    /// # Errors
    ///
    /// Returns an error if the body has already been consumed.
    pub fn bytes_stream(&mut self) -> Result<BoxBytesStream> {
        self.take_stream(false)
    }

    /// Consume the body and return the encoded response byte stream.
    ///
    /// For compressed streaming responses, this selects the original encoded
    /// transport body and bypasses the existing decoder chain. The selection
    /// is one-shot; a body cannot subsequently be consumed in decoded mode.
    /// Buffered bodies follow the ordinary streaming path because their
    /// encoded representation is no longer available.
    ///
    /// # Errors
    ///
    /// Returns an error if the body has already been consumed or if the
    /// decoder configuration is invalid while selecting a stream.
    pub fn raw_bytes_stream(&mut self) -> Result<BoxBytesStream> {
        self.take_stream(true)
    }

    fn take_stream(&mut self, raw: bool) -> Result<BoxBytesStream> {
        match self {
            Self::Buffered { bytes } => {
                let bytes = std::mem::take(bytes);
                *self = Self::Consumed;
                Ok(Box::pin(futures_util::stream::once(
                    async move { Ok(bytes) },
                )))
            }
            Self::Streaming { .. } => {
                let old = std::mem::replace(self, Self::Consumed);
                if let Self::Streaming { stream, lease } = old {
                    Ok(Box::pin(LeasedResponseStream {
                        inner: stream,
                        _lease: lease,
                    }))
                } else {
                    // Defensive: the outer match guarantees this arm, but
                    // a future variant addition must surface as an error,
                    // not a panic.
                    Err(Error::Body("response body state mismatch".into()))
                }
            }
            Self::EncodedStreaming { .. } => {
                let old = std::mem::replace(self, Self::Consumed);
                if let Self::EncodedStreaming {
                    stream,
                    lease,
                    content_encoding,
                    limit,
                } = old
                {
                    let stream = if raw {
                        Ok(stream)
                    } else {
                        crate::compression::decompress_stream(
                            stream,
                            Some(&content_encoding),
                            true,
                            limit,
                        )
                    }?;
                    Ok(Box::pin(LeasedResponseStream {
                        inner: stream,
                        _lease: lease,
                    }))
                } else {
                    // Defensive: the outer match guarantees this arm, but
                    // a future variant addition must surface as an error,
                    // not a panic.
                    Err(Error::Body("response body state mismatch".into()))
                }
            }
            Self::Consumed => Err(Error::Body("streaming body already consumed".into())),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_empty() {
        let body = RequestBody::Empty;
        assert!(body.is_empty());
        assert_eq!(body.len(), 0);
        assert!(body.has_known_length());
        assert!(body.is_replayable());
    }

    #[test]
    fn request_body_bytes() {
        let body = RequestBody::from(Bytes::from("hello"));
        assert!(!body.is_empty());
        assert_eq!(body.len(), 5);
        assert!(body.has_known_length());
        assert!(body.is_replayable());
    }

    #[test]
    fn request_body_stream_unknown_length() {
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = RequestBody::from_stream(stream, None);
        assert!(!body.is_empty());
        assert_eq!(body.len(), 0);
        assert!(!body.has_known_length());
        assert!(!body.is_replayable());
    }

    #[test]
    fn request_body_stream_known_length() {
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = RequestBody::from_stream(stream, Some(1024));
        assert!(!body.is_empty());
        assert_eq!(body.len(), 1024);
        assert!(body.has_known_length());
        assert!(!body.is_replayable());
    }

    #[test]
    fn try_clone_for_redirect_empty() {
        let body = RequestBody::Empty;
        let cloned = body.try_clone_for_redirect().unwrap();
        assert!(cloned.is_empty());
    }

    #[test]
    fn try_clone_for_redirect_bytes() {
        let body = RequestBody::from(Bytes::from("hello"));
        let cloned = body.try_clone_for_redirect().unwrap();
        match cloned {
            RequestBody::Bytes(b) => assert_eq!(b, "hello"),
            _ => panic!("expected bytes"),
        }
    }

    #[test]
    fn try_clone_for_redirect_stream_errors() {
        let stream = futures_util::stream::empty::<Result<Bytes>>();
        let body = RequestBody::from_stream(stream, None);
        assert!(body.try_clone_for_redirect().is_err());
    }

    #[test]
    fn response_body_buffered() {
        let body = ResponseBody::buffered(Bytes::from("hello"));
        assert!(!body.is_empty());
        assert_eq!(body.len(), Some(5));
        assert!(body.is_buffered());
    }

    #[test]
    fn response_body_streaming() {
        let stream = Box::pin(futures_util::stream::empty());
        let body = ResponseBody::streaming(stream);
        assert!(!body.is_buffered());
    }

    #[tokio::test]
    async fn response_body_buffered_bytes() {
        let mut body = ResponseBody::buffered(Bytes::from("hello world"));
        let bytes = body.bytes().await.unwrap();
        assert_eq!(bytes, "hello world");
    }

    #[tokio::test]
    async fn response_body_buffered_text() {
        let mut body = ResponseBody::buffered(Bytes::from("hello"));
        let text = body.text().await.unwrap();
        assert_eq!(text, "hello");
    }

    #[tokio::test]
    async fn response_body_buffered_stream_yields_single_chunk() {
        let mut body = ResponseBody::buffered(Bytes::from("data"));
        let mut stream = body.bytes_stream().unwrap();
        let chunk = stream.next().await.unwrap().unwrap();
        assert_eq!(chunk, "data");
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn response_body_streaming_yields_chunks() {
        let chunks = vec![
            Ok(Bytes::from("a")),
            Ok(Bytes::from("bb")),
            Ok(Bytes::from("ccc")),
        ];
        let stream = Box::pin(futures_util::stream::iter(chunks));
        let mut body = ResponseBody::streaming(stream);
        let mut stream = body.bytes_stream().unwrap();
        assert_eq!(stream.next().await.unwrap().unwrap(), "a");
        assert_eq!(stream.next().await.unwrap().unwrap(), "bb");
        assert_eq!(stream.next().await.unwrap().unwrap(), "ccc");
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn response_body_streaming_collect() {
        let chunks = vec![Ok(Bytes::from("hello ")), Ok(Bytes::from("world"))];
        let stream = Box::pin(futures_util::stream::iter(chunks));
        let mut body = ResponseBody::streaming(stream);
        let bytes = body.bytes().await.unwrap();
        assert_eq!(bytes, "hello world");
    }

    #[tokio::test]
    async fn response_body_double_consume_replaces() {
        let mut body = ResponseBody::buffered(Bytes::from("data"));
        let _ = body.bytes().await.unwrap();
        assert!(body.bytes().await.is_err());
    }

    #[tokio::test]
    async fn response_body_double_stream_errors() {
        let stream = Box::pin(futures_util::stream::iter(vec![Ok(Bytes::from("x"))]));
        let mut body = ResponseBody::streaming(stream);
        let _ = body.bytes_stream().unwrap();
        let err = body.bytes_stream();
        assert!(err.is_err());
    }

    #[cfg(feature = "compression-gzip")]
    fn gzip_bytes(input: &[u8]) -> Bytes {
        use std::io::Write;

        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(input).unwrap();
        Bytes::from(encoder.finish().unwrap())
    }

    #[cfg(feature = "compression-gzip")]
    #[tokio::test]
    async fn encoded_streaming_selects_raw_once() {
        let encoded = gzip_bytes(b"compressed body");
        let expected = encoded.clone();
        let stream = Box::pin(futures_util::stream::iter(vec![Ok(encoded)]));
        let mut body = ResponseBody::encoded_streaming(
            stream,
            "gzip".to_owned(),
            DecompressionLimit::default(),
        );

        let mut raw = body.raw_bytes_stream().unwrap();
        assert_eq!(raw.next().await.unwrap().unwrap(), expected);
        assert!(raw.next().await.is_none());
        assert!(body.bytes_stream().is_err());
    }

    #[cfg(feature = "compression-gzip")]
    #[tokio::test]
    async fn encoded_streaming_selects_decoded_once() {
        let encoded = gzip_bytes(b"compressed body");
        let stream = Box::pin(futures_util::stream::iter(vec![Ok(encoded)]));
        let mut body = ResponseBody::encoded_streaming(
            stream,
            "gzip".to_owned(),
            DecompressionLimit::default(),
        );

        let mut decoded = body.bytes_stream().unwrap();
        assert_eq!(decoded.next().await.unwrap().unwrap(), "compressed body");
        assert!(decoded.next().await.is_none());
        assert!(body.raw_bytes_stream().is_err());
    }

    #[tokio::test]
    async fn uncompressed_raw_selection_is_passthrough() {
        let stream = Box::pin(futures_util::stream::iter(vec![Ok(Bytes::from("body"))]));
        let mut body = ResponseBody::streaming(stream);
        let mut raw = body.raw_bytes_stream().unwrap();
        assert_eq!(raw.next().await.unwrap().unwrap(), "body");
        assert!(raw.next().await.is_none());
    }

    #[tokio::test]
    async fn response_body_buffered_double_bytes_errors() {
        let mut body = ResponseBody::buffered(Bytes::from("data"));
        let first = body.bytes().await.unwrap();
        assert_eq!(first, "data");
        assert!(body.bytes().await.is_err());
    }

    #[tokio::test]
    async fn response_body_buffered_text_invalid_utf8() {
        let mut body = ResponseBody::buffered(Bytes::from(vec![0xFF, 0xFE]));
        let err = body.text().await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn response_body_streaming_bytes_then_stream_returns_empty() {
        let stream = Box::pin(futures_util::stream::iter(vec![Ok(Bytes::from("x"))]));
        let mut body = ResponseBody::streaming(stream);
        let _ = body.bytes().await.unwrap();
        assert!(body.bytes_stream().is_err());
    }

    #[tokio::test]
    async fn response_body_consumed_is_empty() {
        let body = ResponseBody::Consumed;
        assert!(body.is_empty());
        assert_eq!(body.len(), Some(0));
    }

    #[tokio::test]
    async fn response_body_buffered_bytes_stream_then_bytes_returns_empty() {
        let mut body = ResponseBody::buffered(Bytes::from("data"));
        let mut stream = body.bytes_stream().unwrap();
        let chunk = stream.next().await.unwrap().unwrap();
        assert_eq!(chunk, "data");
        assert!(stream.next().await.is_none());
        assert!(body.bytes().await.is_err());
    }

    #[tokio::test]
    async fn into_http_body_empty_produces_empty_body() {
        use http_body::Body;
        use http_body_util::BodyExt;

        let body = RequestBody::Empty;
        let mut hyper_body = body.into_http_body();
        let frame = futures_util::future::poll_fn(|cx| {
            use std::pin::Pin;
            Pin::new(&mut hyper_body).poll_frame(cx)
        })
        .await;
        assert!(frame.is_none());
        let _ = hyper_body.collect().await;
    }

    #[tokio::test]
    async fn into_http_body_bytes_produces_one_data_frame() {
        use http_body::Body;
        use std::pin::Pin;

        let body = RequestBody::from(Bytes::from("hello"));
        let mut hyper_body = body.into_http_body();

        let frame = futures_util::future::poll_fn(|cx| Pin::new(&mut hyper_body).poll_frame(cx))
            .await
            .expect("frame present")
            .expect("frame ok");
        assert!(frame.is_data());
        let data = frame.into_data().unwrap();
        assert_eq!(data, "hello");

        let end =
            futures_util::future::poll_fn(|cx| Pin::new(&mut hyper_body).poll_frame(cx)).await;
        assert!(end.is_none());
    }

    #[tokio::test]
    async fn into_http_body_stream_pipes_through_without_eager_buffering() {
        use http_body::Body;
        use std::pin::Pin;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let polled = Arc::new(AtomicUsize::new(0));
        let polled_inner = polled.clone();

        let chunks = vec![
            Ok(Bytes::from("a")),
            Ok(Bytes::from("b")),
            Ok(Bytes::from("c")),
        ];
        let chunk_stream = futures_util::stream::iter(chunks).inspect(move |_| {
            polled_inner.fetch_add(1, Ordering::SeqCst);
        });
        let body = RequestBody::from_stream(chunk_stream.map(|r| -> Result<Bytes> { r }), Some(3));
        let mut hyper_body = body.into_http_body();

        let frame = futures_util::future::poll_fn(|cx| Pin::new(&mut hyper_body).poll_frame(cx))
            .await
            .expect("frame present")
            .expect("frame ok");
        let data = frame.into_data().unwrap();
        assert_eq!(data, "a");
        assert_eq!(
            polled.load(Ordering::SeqCst),
            1,
            "stream should not be eagerly polled"
        );
    }

    #[tokio::test]
    async fn leased_stream_holds_pool_slot_until_drop() {
        let pool = crate::pool::Pool::new(crate::pool::PoolConfig {
            max_connections_per_host: Some(1),
            ..Default::default()
        });
        let origin = crate::pool::OriginKey::from_parts("http", "example.com", 80);
        let guard = pool.acquire(Some(&origin)).await;
        let mut body = ResponseBody::streaming_with_lease(
            Box::pin(futures_util::stream::pending()),
            Arc::new(guard),
        );
        let stream = body.bytes_stream().unwrap();

        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(10),
            pool.acquire(Some(&origin))
        )
        .await
        .is_err());

        drop(stream);
        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(100),
            pool.acquire(Some(&origin))
        )
        .await
        .is_ok());
    }
}
