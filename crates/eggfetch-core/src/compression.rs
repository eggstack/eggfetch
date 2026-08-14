//! Response decompression.
//!
//! Provides streaming decompression of response bodies based on
//! `Content-Encoding` headers. Supports gzip, deflate, brotli, and
//! zstd via feature-gated `async-compression` decoders.
//!
//! # Policy
//!
//! When automatic decompression is enabled (the default when any
//! compression feature is compiled in), the client:
//!
//! 1. Advertises supported encodings in `Accept-Encoding`.
//! 2. Decodes compressed response bodies transparently.
//! 3. Strips `Content-Encoding` and `Content-Length` from decoded
//!    response headers.
//!
//! If the server responds with an unsupported encoding and automatic
//! decompression is enabled, an `Error::UnsupportedContentEncoding`
//! is returned. If decompression is disabled, the raw bytes pass
//! through unchanged.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;

use crate::body::BoxBytesStream;
use crate::error::{Error, Result};

/// Limits applied during response body decompression.
#[derive(Debug, Clone, Copy, Default)]
pub struct DecompressionLimit {
    /// Hard limit on total decoded bytes.
    pub max_decoded_body_size: Option<usize>,
    /// Ratio of decoded bytes to compressed bytes after which
    /// decompression is rejected.
    pub max_decompression_ratio: Option<f64>,
}

impl DecompressionLimit {
    /// Create a new limit with both fields set to `None` (unlimited).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_decoded_body_size: None,
            max_decompression_ratio: None,
        }
    }

    /// Returns `true` if neither limit is set.
    #[must_use]
    pub fn is_unlimited(&self) -> bool {
        self.max_decoded_body_size.is_none() && self.max_decompression_ratio.is_none()
    }
}

/// Maximum number of nested content encodings we will attempt to
/// decode. This prevents stack overflow or excessive resource use
/// from adversarial multi-layer compression.
const MAX_NESTING_DEPTH: usize = 4;

/// A content coding identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentCoding {
    /// gzip (RFC 1952)
    Gzip,
    /// deflate (RFC 1951, typically zlib-wrapped)
    Deflate,
    /// brotli (RFC 7932)
    Brotli,
    /// zstd (RFC 8478)
    Zstd,
}

impl ContentCoding {
    /// Return the wire identifier for this encoding.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gzip => "gzip",
            Self::Deflate => "deflate",
            Self::Brotli => "br",
            Self::Zstd => "zstd",
        }
    }

    /// Parse a wire identifier into a `ContentCoding`.
    fn from_wire(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "gzip" | "x-gzip" => Some(Self::Gzip),
            "deflate" => Some(Self::Deflate),
            "br" => Some(Self::Brotli),
            "zstd" => Some(Self::Zstd),
            _ => None,
        }
    }
}

/// Generate the `Accept-Encoding` header value for the compiled-in
/// features. Returns `None` if no compression features are enabled.
#[must_use]
pub fn accept_encoding_value() -> Option<&'static str> {
    static VALUE: OnceLock<Option<Box<str>>> = OnceLock::new();

    VALUE
        .get_or_init(|| {
            // Order matches mainstream client defaults (most common first).
            #[allow(unused_mut)]
            let mut parts: Vec<&str> = Vec::new();

            #[cfg(feature = "compression-gzip")]
            {
                parts.push("gzip");
                parts.push("deflate");
            }
            #[cfg(feature = "compression-deflate")]
            {
                // Only add deflate if gzip didn't already add it.
                #[cfg(not(feature = "compression-gzip"))]
                parts.push("deflate");
            }
            #[cfg(feature = "compression-brotli")]
            parts.push("br");
            #[cfg(feature = "compression-zstd")]
            parts.push("zstd");

            (!parts.is_empty()).then(|| parts.join(", ").into_boxed_str())
        })
        .as_deref()
}

/// Parse a `Content-Encoding` header value into an ordered list of
/// content codings. The list is in wire order (outermost first).
///
/// Returns `None` if the header is empty or contains only whitespace.
fn parse_content_encodings(header: &str) -> Option<Vec<ContentCoding>> {
    let encodings: Vec<ContentCoding> = header
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(ContentCoding::from_wire)
        .collect();

    if encodings.is_empty() {
        None
    } else {
        Some(encodings)
    }
}

/// Check whether a `Content-Encoding` header value contains only
/// supported encodings.
///
/// Returns `Ok(())` if the header is absent, empty, or all encodings
/// are supported.
///
/// # Errors
///
/// Returns [`Error::UnsupportedContentEncoding`] if any encoding is
/// unsupported.
pub fn validate_content_encodings(header_value: &str) -> Result<()> {
    for token in header_value.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if ContentCoding::from_wire(token).is_none() {
            return Err(Error::UnsupportedContentEncoding(token.to_string()));
        }
    }
    Ok(())
}

/// Wrap a byte stream in decompression decoders based on the
/// `Content-Encoding` header.
///
/// Decoders are applied in reverse order (innermost encoding first).
/// For example, `Content-Encoding: gzip, br` means decode brotli
/// first, then gzip.
///
/// Returns the original stream unchanged if there are no encodings
/// to decode or if decompression is disabled.
///
/// # Errors
///
/// Returns [`Error::Decompression`] if the nesting depth exceeds the
/// maximum or if a decoder fails. Returns
/// [`Error::UnsupportedContentEncoding`] if any encoding is not
/// supported by the compiled-in features. Returns
/// [`Error::DecodedBodyTooLarge`] or
/// [`Error::DecompressionRatioExceeded`] if the decoded body exceeds
/// the configured limits.
pub fn decompress_stream(
    stream: BoxBytesStream,
    content_encoding: Option<&str>,
    decompression_enabled: bool,
    limit: DecompressionLimit,
) -> Result<BoxBytesStream> {
    if !decompression_enabled {
        return Ok(stream);
    }

    let header = match content_encoding {
        Some(h) if !h.trim().is_empty() => h,
        _ => return Ok(stream),
    };

    let Some(encodings) = parse_content_encodings(header) else {
        return Ok(stream);
    };

    if encodings.len() > MAX_NESTING_DEPTH {
        return Err(Error::Decompression(format!(
            "content encoding nesting depth {} exceeds maximum {}",
            encodings.len(),
            MAX_NESTING_DEPTH
        )));
    }

    // Validate all encodings are supported before building the chain.
    validate_content_encodings(header)?;

    // Wrap the original compressed stream with a counter so we can
    // track compressed bytes consumed through the decoder chain.
    let counting = CountingStream::new(stream);
    let counter = counting.counter();

    // Apply decoders in reverse order (innermost encoding first).
    let mut current: BoxBytesStream = Box::pin(counting);
    for encoding in encodings.into_iter().rev() {
        current = make_decoder(current, encoding)?;
    }

    if limit.is_unlimited() {
        return Ok(current);
    }

    Ok(Box::pin(LimitingStream::new(current, limit, counter)))
}

/// Decompress a fully buffered byte slice using synchronous decoding.
///
/// Used for response bodies that have already been collected into memory.
/// Currently supports gzip and deflate via `flate2`. For brotli and zstd,
/// the raw bytes are returned unchanged (use the streaming path for full
/// decompression).
///
/// # Errors
///
/// Returns an error if the content encoding is unsupported or
/// decompression fails. Returns [`Error::DecodedBodyTooLarge`] or
/// [`Error::DecompressionRatioExceeded`] if the decoded body exceeds
/// the configured limits.
pub fn decompress_buffered(
    data: &[u8],
    content_encoding: &str,
    limit: DecompressionLimit,
) -> Result<bytes::Bytes> {
    let encodings = parse_content_encodings(content_encoding)
        .ok_or_else(|| Error::Decompression("empty content encoding".into()))?;

    if encodings.len() > MAX_NESTING_DEPTH {
        return Err(Error::Decompression(format!(
            "content encoding nesting depth {} exceeds maximum {}",
            encodings.len(),
            MAX_NESTING_DEPTH
        )));
    }

    validate_content_encodings(content_encoding)?;

    // Only decode gzip and deflate synchronously. Brotli and zstd are
    // left as-is; use the streaming path for full decompression.
    let has_sync_encodings = encodings
        .iter()
        .any(|e| matches!(e, ContentCoding::Gzip | ContentCoding::Deflate));

    if !has_sync_encodings {
        return Ok(bytes::Bytes::copy_from_slice(data));
    }

    let compressed_len = data.len();
    #[allow(unused_mut)]
    let mut current = bytes::Bytes::copy_from_slice(data);
    for encoding in encodings.into_iter().rev() {
        match encoding {
            #[cfg(any(feature = "compression-gzip", feature = "compression-deflate"))]
            ContentCoding::Gzip | ContentCoding::Deflate => {
                current = sync_decode_flate2(&current, encoding)?;
            }
            #[cfg(not(any(feature = "compression-gzip", feature = "compression-deflate")))]
            ContentCoding::Gzip | ContentCoding::Deflate => {
                // Cannot synchronously decode without flate2; pass through raw.
            }
            ContentCoding::Brotli | ContentCoding::Zstd => {
                // Cannot synchronously decode brotli/zstd; pass through raw.
            }
        }
    }

    if let Some(max) = limit.max_decoded_body_size {
        if current.len() > max {
            return Err(Error::DecodedBodyTooLarge);
        }
    }
    if let Some(max_ratio) = limit.max_decompression_ratio {
        if compressed_len > 0 {
            #[allow(clippy::cast_precision_loss)]
            let ratio = current.len() as f64 / compressed_len as f64;
            if ratio > max_ratio {
                return Err(Error::DecompressionRatioExceeded);
            }
        }
    }

    Ok(current)
}

/// Synchronously decode a single layer of gzip or deflate compression using flate2.
#[cfg(any(feature = "compression-gzip", feature = "compression-deflate"))]
fn sync_decode_flate2(data: &bytes::Bytes, encoding: ContentCoding) -> Result<bytes::Bytes> {
    use std::io::Read;

    match encoding {
        ContentCoding::Gzip => {
            let mut decoder = flate2::read::GzDecoder::new(&data[..]);
            let mut output = Vec::new();
            decoder
                .read_to_end(&mut output)
                .map_err(|e| Error::Decompression(e.to_string()))?;
            Ok(bytes::Bytes::from(output))
        }
        ContentCoding::Deflate => {
            let mut decoder = flate2::read::DeflateDecoder::new(&data[..]);
            let mut output = Vec::new();
            decoder
                .read_to_end(&mut output)
                .map_err(|e| Error::Decompression(e.to_string()))?;
            Ok(bytes::Bytes::from(output))
        }
        _ => unreachable!("sync_decode_flate2 called with non-flate2 encoding"),
    }
}

/// Create a decoder for a single content coding.
#[allow(
    clippy::unnecessary_wraps,
    clippy::needless_pass_by_value,
    unused_variables
)]
fn make_decoder(stream: BoxBytesStream, encoding: ContentCoding) -> Result<BoxBytesStream> {
    match encoding {
        ContentCoding::Gzip => {
            #[cfg(feature = "compression-gzip")]
            {
                use async_compression::tokio::bufread::GzipDecoder;
                use futures_util::StreamExt;
                use tokio::io::BufReader;
                use tokio_util::io::ReaderStream;

                let reader = StreamReader::new(stream);
                let decoder = GzipDecoder::new(BufReader::new(reader));
                let stream = ReaderStream::new(decoder);
                Ok(Box::pin(stream.map(|r| {
                    r.map_err(|e| Error::Decompression(e.to_string()))
                })))
            }
            #[cfg(not(feature = "compression-gzip"))]
            {
                Err(Error::UnsupportedContentEncoding("gzip".to_string()))
            }
        }
        ContentCoding::Deflate => {
            #[cfg(feature = "compression-deflate")]
            {
                use async_compression::tokio::bufread::DeflateDecoder;
                use futures_util::StreamExt;
                use tokio::io::BufReader;
                use tokio_util::io::ReaderStream;

                let reader = StreamReader::new(stream);
                let decoder = DeflateDecoder::new(BufReader::new(reader));
                let stream = ReaderStream::new(decoder);
                Ok(Box::pin(stream.map(|r| {
                    r.map_err(|e| Error::Decompression(e.to_string()))
                })))
            }
            #[cfg(not(feature = "compression-deflate"))]
            {
                Err(Error::UnsupportedContentEncoding("deflate".to_string()))
            }
        }
        ContentCoding::Brotli => {
            #[cfg(feature = "compression-brotli")]
            {
                use async_compression::tokio::bufread::BrotliDecoder;
                use futures_util::StreamExt;
                use tokio::io::BufReader;
                use tokio_util::io::ReaderStream;

                let reader = StreamReader::new(stream);
                let decoder = BrotliDecoder::new(BufReader::new(reader));
                let stream = ReaderStream::new(decoder);
                Ok(Box::pin(stream.map(|r| {
                    r.map_err(|e| Error::Decompression(e.to_string()))
                })))
            }
            #[cfg(not(feature = "compression-brotli"))]
            {
                Err(Error::UnsupportedContentEncoding("br".to_string()))
            }
        }
        ContentCoding::Zstd => {
            #[cfg(feature = "compression-zstd")]
            {
                use async_compression::tokio::bufread::ZstdDecoder;
                use futures_util::StreamExt;
                use tokio::io::BufReader;
                use tokio_util::io::ReaderStream;

                let reader = StreamReader::new(stream);
                let decoder = ZstdDecoder::new(BufReader::new(reader));
                let stream = ReaderStream::new(decoder);
                Ok(Box::pin(stream.map(|r| {
                    r.map_err(|e| Error::Decompression(e.to_string()))
                })))
            }
            #[cfg(not(feature = "compression-zstd"))]
            {
                Err(Error::UnsupportedContentEncoding("zstd".to_string()))
            }
        }
    }
}

/// A helper type that adapts a `BoxBytesStream` (yielding `Result<Bytes>`)
/// into an `AsyncRead` for use with `async-compression` decoders.
#[allow(dead_code)]
struct StreamReader {
    stream: BoxBytesStream,
    buffer: bytes::BytesMut,
    offset: usize,
}

#[allow(dead_code)]
impl StreamReader {
    fn new(stream: BoxBytesStream) -> Self {
        Self {
            stream,
            buffer: bytes::BytesMut::new(),
            offset: 0,
        }
    }
}

impl tokio::io::AsyncRead for StreamReader {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        use futures_core::Stream;
        use std::pin::Pin;

        // If we have buffered data, return it first.
        if self.offset < self.buffer.len() {
            let remaining = &self.buffer[self.offset..];
            let to_copy = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..to_copy]);
            self.offset += to_copy;
            if self.offset >= self.buffer.len() {
                self.buffer.clear();
                self.offset = 0;
            }
            return std::task::Poll::Ready(Ok(()));
        }

        // Try to get the next chunk from the stream.
        match Pin::new(&mut self.stream).poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(chunk))) => {
                self.buffer.extend_from_slice(&chunk);
                self.offset = 0;
                let to_copy = self.buffer.len().min(buf.remaining());
                buf.put_slice(&self.buffer[..to_copy]);
                self.offset = to_copy;
                std::task::Poll::Ready(Ok(()))
            }
            std::task::Poll::Ready(Some(Err(e))) => {
                std::task::Poll::Ready(Err(std::io::Error::other(e.to_string())))
            }
            std::task::Poll::Ready(None) => std::task::Poll::Ready(Ok(())),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

/// A stream wrapper that counts the compressed bytes yielded by the
/// underlying stream. The count is exposed via a shared
/// `Arc<AtomicUsize>` so that downstream limiters can read the
/// compressed byte count without needing to see the raw stream.
struct CountingStream {
    inner: BoxBytesStream,
    count: Arc<AtomicUsize>,
}

impl CountingStream {
    fn new(inner: BoxBytesStream) -> Self {
        Self {
            inner,
            count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn counter(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.count)
    }
}

impl futures_core::Stream for CountingStream {
    type Item = <BoxBytesStream as futures_core::Stream>::Item;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::pin::Pin;

        match Pin::new(&mut self.inner).poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(chunk))) => {
                self.count.fetch_add(chunk.len(), Ordering::Relaxed);
                std::task::Poll::Ready(Some(Ok(chunk)))
            }
            other => other,
        }
    }
}

struct LimitingStream {
    inner: BoxBytesStream,
    limit: DecompressionLimit,
    decoded_bytes: usize,
    compressed_counter: Arc<AtomicUsize>,
}

impl LimitingStream {
    fn new(
        inner: BoxBytesStream,
        limit: DecompressionLimit,
        compressed_counter: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            inner,
            limit,
            decoded_bytes: 0,
            compressed_counter,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn check_limit(&self) -> Result<()> {
        if let Some(max) = self.limit.max_decoded_body_size {
            if self.decoded_bytes > max {
                return Err(Error::DecodedBodyTooLarge);
            }
        }
        if let Some(max_ratio) = self.limit.max_decompression_ratio {
            let compressed = self.compressed_counter.load(Ordering::Relaxed);
            if compressed > 0 {
                let ratio = self.decoded_bytes as f64 / compressed as f64;
                if ratio > max_ratio {
                    return Err(Error::DecompressionRatioExceeded);
                }
            }
        }
        Ok(())
    }
}

impl futures_core::Stream for LimitingStream {
    type Item = Result<bytes::Bytes>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::pin::Pin;

        match Pin::new(&mut self.inner).poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(chunk))) => {
                self.decoded_bytes += chunk.len();
                if let Err(e) = self.check_limit() {
                    return std::task::Poll::Ready(Some(Err(e)));
                }
                std::task::Poll::Ready(Some(Ok(chunk)))
            }
            std::task::Poll::Ready(Some(Err(e))) => std::task::Poll::Ready(Some(Err(e))),
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::FutureExt;

    #[test]
    fn content_coding_from_wire() {
        assert_eq!(ContentCoding::from_wire("gzip"), Some(ContentCoding::Gzip));
        assert_eq!(
            ContentCoding::from_wire("x-gzip"),
            Some(ContentCoding::Gzip)
        );
        assert_eq!(
            ContentCoding::from_wire("deflate"),
            Some(ContentCoding::Deflate)
        );
        assert_eq!(ContentCoding::from_wire("br"), Some(ContentCoding::Brotli));
        assert_eq!(ContentCoding::from_wire("zstd"), Some(ContentCoding::Zstd));
        assert_eq!(ContentCoding::from_wire("identity"), None);
        assert_eq!(ContentCoding::from_wire("unknown"), None);
        assert_eq!(ContentCoding::from_wire("GZIP"), Some(ContentCoding::Gzip));
        assert_eq!(
            ContentCoding::from_wire(" Br "),
            Some(ContentCoding::Brotli)
        );
    }

    #[test]
    fn content_coding_as_str() {
        assert_eq!(ContentCoding::Gzip.as_str(), "gzip");
        assert_eq!(ContentCoding::Deflate.as_str(), "deflate");
        assert_eq!(ContentCoding::Brotli.as_str(), "br");
        assert_eq!(ContentCoding::Zstd.as_str(), "zstd");
    }

    #[test]
    fn parse_content_encodings_single() {
        let encs = parse_content_encodings("gzip").unwrap();
        assert_eq!(encs, vec![ContentCoding::Gzip]);
    }

    #[test]
    fn parse_content_encodings_multiple() {
        let encs = parse_content_encodings("gzip, br").unwrap();
        assert_eq!(encs, vec![ContentCoding::Gzip, ContentCoding::Brotli]);
    }

    #[test]
    fn parse_content_encodings_with_whitespace() {
        let encs = parse_content_encodings("  gzip , br  ").unwrap();
        assert_eq!(encs, vec![ContentCoding::Gzip, ContentCoding::Brotli]);
    }

    #[test]
    fn parse_content_encodings_empty() {
        assert!(parse_content_encodings("").is_none());
        assert!(parse_content_encodings("  ").is_none());
    }

    #[test]
    fn parse_content_encodings_unknown_filtered() {
        let encs = parse_content_encodings("gzip, identity, br").unwrap();
        assert_eq!(encs, vec![ContentCoding::Gzip, ContentCoding::Brotli]);
    }

    #[test]
    fn validate_content_encodings_supported() {
        assert!(validate_content_encodings("gzip").is_ok());
        assert!(validate_content_encodings("gzip, br").is_ok());
        assert!(validate_content_encodings("").is_ok());
    }

    #[test]
    fn validate_content_encodings_unsupported() {
        let err = validate_content_encodings("gzip, weird").unwrap_err();
        assert_eq!(err.kind(), "unsupported_content_encoding");
        assert!(err.to_string().contains("weird"));
    }

    #[test]
    fn accept_encoding_value_requires_compiled_features() {
        let _ = accept_encoding_value();
    }

    #[cfg(not(any(
        feature = "compression-gzip",
        feature = "compression-deflate",
        feature = "compression-brotli",
        feature = "compression-zstd"
    )))]
    #[test]
    fn accept_encoding_value_none_without_compression_features() {
        assert!(
            accept_encoding_value().is_none(),
            "accept_encoding_value() should return None when no compression features are compiled"
        );
    }

    #[cfg(all(
        feature = "compression-gzip",
        not(feature = "compression-brotli"),
        not(feature = "compression-zstd")
    ))]
    #[test]
    fn accept_encoding_value_gzip_only() {
        let val = accept_encoding_value().unwrap();
        assert!(val.contains("gzip"), "should contain gzip, got: {val}");
        assert!(
            !val.contains("br"),
            "should not contain br without brotli feature, got: {val}"
        );
        assert!(
            !val.contains("zstd"),
            "should not contain zstd without zstd feature, got: {val}"
        );
    }

    #[test]
    fn accept_encoding_value_deterministic_order() {
        let val1 = accept_encoding_value();
        let val2 = accept_encoding_value();
        assert_eq!(
            val1, val2,
            "accept_encoding_value() should return the same value across calls"
        );
    }

    #[test]
    fn decompress_stream_no_encoding_returns_original() {
        let stream: BoxBytesStream = Box::pin(futures_util::stream::empty());
        let result = decompress_stream(stream, None, true, DecompressionLimit::new()).unwrap();
        drop(result);
    }

    #[test]
    fn decompress_stream_disabled_returns_original() {
        let stream: BoxBytesStream = Box::pin(futures_util::stream::empty());
        let result =
            decompress_stream(stream, Some("gzip"), false, DecompressionLimit::new()).unwrap();
        drop(result);
    }

    #[test]
    fn decompress_stream_empty_header_returns_original() {
        let stream: BoxBytesStream = Box::pin(futures_util::stream::empty());
        let result =
            decompress_stream(stream, Some("  "), true, DecompressionLimit::new()).unwrap();
        drop(result);
    }

    #[test]
    fn decompression_limit_new_is_unlimited() {
        let limit = DecompressionLimit::new();
        assert!(limit.is_unlimited());
        assert!(limit.max_decoded_body_size.is_none());
        assert!(limit.max_decompression_ratio.is_none());
    }

    #[cfg(any(feature = "compression-gzip", feature = "compression-deflate"))]
    #[test]
    fn decompression_limit_max_size_rejects() {
        let data = gzip_compress(b"hello world");
        let limit = DecompressionLimit {
            max_decoded_body_size: Some(5),
            max_decompression_ratio: None,
        };
        let err = decompress_buffered(&data, "gzip", limit).unwrap_err();
        assert_eq!(err.kind(), "decoded_body_too_large");
    }

    #[cfg(any(feature = "compression-gzip", feature = "compression-deflate"))]
    #[test]
    fn decompression_limit_max_size_allows_within() {
        let data = gzip_compress(b"hi");
        let limit = DecompressionLimit {
            max_decoded_body_size: Some(1024),
            max_decompression_ratio: None,
        };
        let result = decompress_buffered(&data, "gzip", limit).unwrap();
        assert_eq!(&result[..], b"hi");
    }

    #[cfg(any(feature = "compression-gzip", feature = "compression-deflate"))]
    #[test]
    fn decompression_limit_ratio_rejects() {
        let data = gzip_compress(b"hello world, this is a test of compression");
        let limit = DecompressionLimit {
            max_decoded_body_size: None,
            max_decompression_ratio: Some(0.5),
        };
        let err = decompress_buffered(&data, "gzip", limit).unwrap_err();
        assert_eq!(err.kind(), "decompression_ratio_exceeded");
    }

    #[cfg(any(feature = "compression-gzip", feature = "compression-deflate"))]
    #[test]
    fn decompression_limit_ratio_allows_within() {
        let data = gzip_compress(b"hello world, this is a test of compression");
        let limit = DecompressionLimit {
            max_decoded_body_size: None,
            max_decompression_ratio: Some(100.0),
        };
        let result = decompress_buffered(&data, "gzip", limit).unwrap();
        assert_eq!(&result[..], b"hello world, this is a test of compression");
    }

    #[cfg(any(feature = "compression-gzip", feature = "compression-deflate"))]
    #[test]
    fn decompression_limit_unlimited_allows() {
        let data = gzip_compress(b"hello world");
        let limit = DecompressionLimit::new();
        let result = decompress_buffered(&data, "gzip", limit).unwrap();
        assert_eq!(&result[..], b"hello world");
    }

    #[test]
    fn limiting_stream_enforces_max_decoded_body_size() {
        let limit = DecompressionLimit {
            max_decoded_body_size: Some(3),
            max_decompression_ratio: None,
        };
        let stream: BoxBytesStream = Box::pin(futures_util::stream::iter(vec![
            Ok(bytes::Bytes::from("ab")),
            Ok(bytes::Bytes::from("cd")),
        ]));
        let counter = Arc::new(AtomicUsize::new(0));
        let mut limited = LimitingStream::new(stream, limit, counter);

        let chunk1 = futures_util::StreamExt::next(&mut limited)
            .now_or_never()
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(chunk1, "ab");

        let err = futures_util::StreamExt::next(&mut limited)
            .now_or_never()
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert_eq!(err.kind(), "decoded_body_too_large");
    }

    #[test]
    fn limiting_stream_allows_within_limit() {
        let limit = DecompressionLimit {
            max_decoded_body_size: Some(1024),
            max_decompression_ratio: None,
        };
        let stream: BoxBytesStream = Box::pin(futures_util::stream::iter(vec![
            Ok(bytes::Bytes::from("a")),
            Ok(bytes::Bytes::from("b")),
        ]));
        let counter = Arc::new(AtomicUsize::new(0));
        let mut limited = LimitingStream::new(stream, limit, counter);

        let chunk1 = futures_util::StreamExt::next(&mut limited)
            .now_or_never()
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(chunk1, "a");
        let chunk2 = futures_util::StreamExt::next(&mut limited)
            .now_or_never()
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(chunk2, "b");
    }

    fn gzip_compress(data: &[u8]) -> Vec<u8> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }
}
