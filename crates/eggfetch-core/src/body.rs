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
//! - **Bytes**: a fixed-size byte buffer with known length.
//! - **Stream**: an async stream of bytes with potentially unknown length.
//!   Uses chunked transfer encoding for HTTP/1.1 when length is unknown.

use std::pin::Pin;

use bytes::{Bytes, BytesMut};
use futures_core::Stream;
use futures_util::StreamExt;

use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// Request body
// ---------------------------------------------------------------------------

/// A type-erased async stream of `Result<Bytes>` chunks.
pub type BoxBytesStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>;

/// Request body.
///
/// Supports empty bodies, fixed-size byte buffers, and async streams.
/// Streams may have unknown length, in which case the engine uses chunked
/// transfer encoding for HTTP/1.1.
#[derive(Default)]
pub enum RequestBody {
    /// Empty body.
    #[default]
    Empty,
    /// Byte body with known length.
    Bytes(Bytes),
    /// Async stream body.
    ///
    /// The `Option<usize>` is the known length, if available. When `None`,
    /// the engine uses chunked transfer encoding.
    Stream {
        /// The stream of body chunks.
        stream: BoxBytesStream,
        /// Known length, if available.
        length: Option<usize>,
    },
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

    /// Consume the body and return all bytes.
    ///
    /// For `Empty` and `Bytes` variants this is immediate. For `Stream`
    /// variants, all chunks are collected.
    pub(crate) async fn into_bytes(self) -> Result<Bytes> {
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

    /// Convert into a hyper-compatible body (always `Full<Bytes>`).
    ///
    /// For stream bodies, this collects all chunks into a single buffer.
    pub(crate) async fn into_hyper_body(self) -> Result<http_body_util::Full<Bytes>> {
        let bytes = self.into_bytes().await?;
        Ok(http_body_util::Full::new(bytes))
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

/// Response body handle.
///
/// Supports two consumption modes:
///
/// - **Buffered**: the entire body is held in memory. Created by
///   `ResponseBody::buffered()`. Calling `bytes()` or `text()` is O(1).
/// - **Streaming**: body chunks are yielded incrementally. Created by
///   `ResponseBody::streaming()`. The stream must be fully consumed or
///   dropped before the connection can be reused.
///
/// # Single-consumption semantics
///
/// A response body can be consumed once via `bytes()`, `text()`, or
/// `bytes_stream()`. After consumption, further reads return an error.
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
            Self::Consumed => f.debug_struct("Consumed").finish(),
        }
    }
}

impl ResponseBody {
    /// Create a buffered response body.
    pub fn buffered(bytes: Bytes) -> Self {
        Self::Buffered { bytes }
    }

    /// Create a streaming response body from a type-erased stream.
    pub fn streaming(stream: BoxBytesStream) -> Self {
        Self::Streaming { stream }
    }

    /// Returns `true` if the body is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Buffered { bytes } => bytes.is_empty(),
            Self::Streaming { .. } => false, // unknown until consumed
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
            Self::Streaming { .. } => None,
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
    /// # Errors
    ///
    /// Returns an error if the body has already been consumed or if a
    /// stream chunk fails.
    pub async fn bytes(&mut self) -> Result<Bytes> {
        let old = std::mem::replace(
            self,
            Self::Buffered {
                bytes: Bytes::new(),
            },
        );
        match old {
            Self::Buffered { bytes } => Ok(bytes),
            Self::Streaming { mut stream } => {
                let mut buf = BytesMut::new();
                while let Some(chunk) = stream.next().await {
                    buf.extend_from_slice(&chunk?);
                }
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
        match self {
            Self::Buffered { bytes } => {
                let bytes = std::mem::take(bytes);
                Ok(Box::pin(futures_util::stream::once(
                    async move { Ok(bytes) },
                )))
            }
            Self::Streaming { .. } => {
                let old = std::mem::replace(self, Self::Consumed);
                if let Self::Streaming { stream } = old {
                    Ok(stream)
                } else {
                    unreachable!()
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
        // After consumption, body is replaced with empty buffered.
        let bytes = body.bytes().await.unwrap();
        assert!(bytes.is_empty());
    }

    #[tokio::test]
    async fn response_body_double_stream_errors() {
        let stream = Box::pin(futures_util::stream::iter(vec![Ok(Bytes::from("x"))]));
        let mut body = ResponseBody::streaming(stream);
        let _ = body.bytes_stream().unwrap();
        let err = body.bytes_stream();
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn request_body_into_bytes() {
        let body = RequestBody::from(Bytes::from("hello"));
        let bytes = body.into_bytes().await.unwrap();
        assert_eq!(bytes, "hello");
    }

    #[tokio::test]
    async fn request_body_stream_into_bytes() {
        let chunks = vec![Ok(Bytes::from("a")), Ok(Bytes::from("bb"))];
        let stream = Box::pin(futures_util::stream::iter(chunks));
        let body = RequestBody::from_stream(stream, Some(3));
        let bytes = body.into_bytes().await.unwrap();
        assert_eq!(bytes, "abb");
    }

    #[tokio::test]
    async fn request_body_empty_into_bytes() {
        let body = RequestBody::Empty;
        let bytes = body.into_bytes().await.unwrap();
        assert!(bytes.is_empty());
    }
}
