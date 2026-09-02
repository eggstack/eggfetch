//! Content-encoding negotiation and decoder wrapping.

use bytes::Bytes;

use crate::body::ResponseBody;
use crate::compression::DecompressionLimit;
use crate::error::Result;
use crate::response::Response;

/// Apply decompression to a response if content-encoding is present.
///
/// Validates the encoding, decompresses streaming or buffered body,
/// strips `Content-Encoding` and `Content-Length` headers, and returns
/// the updated response.
pub(crate) fn apply_decompression(
    mut response: Response,
    content_encoding: Option<&str>,
    limit: DecompressionLimit,
) -> Result<Response> {
    // Validate limit before any decompression work so buffered and streaming
    // paths diverge no longer on invalid ratios.
    if content_encoding.is_some() {
        limit.validate()?;
    }
    if let Some(ce) = content_encoding {
        crate::compression::validate_content_encodings(ce)?;
    }

    let old_body = std::mem::replace(&mut response.body, ResponseBody::buffered(Bytes::new()));
    let mut decoder_applied = false;
    let new_body = match old_body {
        ResponseBody::Streaming { stream, lease } => {
            if let Some(ce) = content_encoding.filter(|value| !value.trim().is_empty()) {
                // Construct the encoded body once, attaching the pool
                // lease directly when present so ownership is never
                // routed through a destructure/rebuild round-trip.
                if let Some(lease) = lease {
                    decoder_applied = crate::compression::parse_content_encodings(ce).is_some();
                    ResponseBody::encoded_streaming_with_lease(stream, lease, ce.to_owned(), limit)
                } else {
                    decoder_applied = crate::compression::parse_content_encodings(ce).is_some();
                    ResponseBody::encoded_streaming(stream, ce.to_owned(), limit)
                }
            } else {
                ResponseBody::Streaming { stream, lease }
            }
        }
        ResponseBody::Buffered { bytes } => {
            if let Some(ce) = content_encoding {
                let decompressed = crate::compression::decompress_buffered(&bytes, ce, limit)?;
                decoder_applied = crate::compression::parse_content_encodings(ce).is_some();
                ResponseBody::buffered(decompressed)
            } else {
                ResponseBody::Buffered { bytes }
            }
        }
        ResponseBody::Consumed => ResponseBody::Consumed,
        body @ ResponseBody::EncodedStreaming { .. } => body,
    };
    response.set_body(new_body);

    if decoder_applied {
        response.headers_mut().remove("content-encoding");
        response.headers_mut().remove("content-length");
    }

    Ok(response)
}

#[cfg(all(test, feature = "compression-gzip"))]
mod tests {
    use super::*;
    use http::{HeaderMap, HeaderValue, StatusCode, Version};
    use url::Url;

    #[test]
    fn automatic_decompression_strips_visible_headers_but_keeps_wire_metadata() {
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", HeaderValue::from_static("gzip"));
        headers.insert("content-length", HeaderValue::from_static("42"));
        let response = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            headers,
            Url::parse("http://example.com").unwrap(),
            ResponseBody::streaming(Box::pin(futures_util::stream::empty())),
        );

        let response =
            apply_decompression(response, Some("gzip"), DecompressionLimit::default()).unwrap();

        assert!(response.headers().get("content-encoding").is_none());
        assert!(response.headers().get("content-length").is_none());
        assert_eq!(response.wire_content_encoding(), Some("gzip"));
        assert_eq!(response.wire_content_length(), Some("42"));
    }

    #[test]
    fn buffered_decompression_keeps_wire_metadata() {
        use std::io::Write;

        let mut gzip_encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gzip_encoder.write_all(b"body").unwrap();
        let encoded = gzip_encoder.finish().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", HeaderValue::from_static("gzip"));
        headers.insert(
            "content-length",
            HeaderValue::from_str(&encoded.len().to_string()).unwrap(),
        );
        let response = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            headers,
            Url::parse("http://example.com").unwrap(),
            ResponseBody::buffered(Bytes::from(encoded)),
        );

        let response =
            apply_decompression(response, Some("gzip"), DecompressionLimit::default()).unwrap();

        assert_eq!(response.wire_content_encoding(), Some("gzip"));
        assert!(response.wire_content_length().is_some());
    }

    #[test]
    fn identity_encoding_preserves_visible_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", HeaderValue::from_static("identity"));
        headers.insert("content-length", HeaderValue::from_static("4"));
        let response = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            headers,
            Url::parse("http://example.com").unwrap(),
            ResponseBody::buffered(Bytes::from_static(b"body")),
        );

        let response =
            apply_decompression(response, Some("identity"), DecompressionLimit::default()).unwrap();

        assert_eq!(
            response.headers().get("content-encoding"),
            Some(&HeaderValue::from_static("identity"))
        );
        assert_eq!(
            response.headers().get("content-length"),
            Some(&HeaderValue::from_static("4"))
        );
    }

    #[test]
    fn empty_encoded_buffered_body_uses_decompression_path() {
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", HeaderValue::from_static("gzip"));
        let response = Response::new(
            StatusCode::OK,
            Version::HTTP_11,
            headers,
            Url::parse("http://example.com").unwrap(),
            ResponseBody::buffered(Bytes::new()),
        );

        let response =
            apply_decompression(response, Some("gzip"), DecompressionLimit::default()).unwrap();
        assert!(response.body().is_empty());
        assert!(response.headers().get("content-encoding").is_none());
    }
}
