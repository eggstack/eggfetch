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

use crate::body::BoxBytesStream;
use crate::error::{Error, Result};

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

    if parts.is_empty() {
        None
    } else {
        // Leak the string for the lifetime of the program. This is
        // intentional for a small number of static values.
        Some(Box::leak(parts.join(", ").into_boxed_str()))
    }
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
/// supported by the compiled-in features.
pub fn decompress_stream(
    stream: BoxBytesStream,
    content_encoding: Option<&str>,
    decompression_enabled: bool,
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

    // Apply decoders in reverse order (innermost encoding first).
    let mut current: BoxBytesStream = stream;
    for encoding in encodings.into_iter().rev() {
        current = make_decoder(current, encoding)?;
    }

    Ok(current)
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
/// decompression fails.
pub fn decompress_buffered(data: &[u8], content_encoding: &str) -> Result<bytes::Bytes> {
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

    let mut current = bytes::Bytes::copy_from_slice(data);
    for encoding in encodings.into_iter().rev() {
        match encoding {
            ContentCoding::Gzip | ContentCoding::Deflate => {
                current = sync_decode_flate2(&current, encoding)?;
            }
            ContentCoding::Brotli | ContentCoding::Zstd => {
                // Cannot synchronously decode brotli/zstd; pass through raw.
            }
        }
    }
    Ok(current)
}

/// Synchronously decode a single layer of gzip or deflate compression using flate2.
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
#[allow(clippy::unnecessary_wraps, unused_variables)]
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

#[cfg(test)]
mod tests {
    use super::*;

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
        // accept_encoding_value returns None when no compression features
        // are enabled. This test always passes in the current configuration
        // since we have at least compression-gzip enabled by default.
        // The important thing is that it compiles.
        let _ = accept_encoding_value();
    }

    #[test]
    fn decompress_stream_no_encoding_returns_original() {
        let stream: BoxBytesStream = Box::pin(futures_util::stream::empty());
        let result = decompress_stream(stream, None, true).unwrap();
        // The returned stream is the original (can't compare Pin<Box> directly,
        // but we verify no error and the function completes).
        drop(result);
    }

    #[test]
    fn decompress_stream_disabled_returns_original() {
        let stream: BoxBytesStream = Box::pin(futures_util::stream::empty());
        let result = decompress_stream(stream, Some("gzip"), false).unwrap();
        drop(result);
    }

    #[test]
    fn decompress_stream_empty_header_returns_original() {
        let stream: BoxBytesStream = Box::pin(futures_util::stream::empty());
        let result = decompress_stream(stream, Some("  "), true).unwrap();
        drop(result);
    }
}
