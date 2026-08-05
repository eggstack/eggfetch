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
    if let Some(ce) = content_encoding {
        crate::compression::validate_content_encodings(ce)?;
    }

    let old_body = std::mem::replace(&mut response.body, ResponseBody::buffered(Bytes::new()));
    let new_body = match old_body {
        ResponseBody::Streaming { stream, lease } => {
            if let Some(ce) = content_encoding.filter(|value| !value.trim().is_empty()) {
                let mut body = ResponseBody::encoded_streaming(stream, ce.to_owned(), limit);
                if let Some(lease) = lease {
                    body = match body {
                        ResponseBody::EncodedStreaming {
                            stream,
                            content_encoding,
                            limit,
                            ..
                        } => ResponseBody::encoded_streaming_with_lease(
                            stream,
                            lease,
                            content_encoding,
                            limit,
                        ),
                        _ => unreachable!(),
                    };
                }
                body
            } else {
                ResponseBody::Streaming { stream, lease }
            }
        }
        ResponseBody::Buffered { bytes } => {
            if let Some(ce) = content_encoding {
                if bytes.is_empty() {
                    ResponseBody::Buffered { bytes }
                } else {
                    let decompressed = crate::compression::decompress_buffered(&bytes, ce, limit)?;
                    ResponseBody::buffered(decompressed)
                }
            } else {
                ResponseBody::Buffered { bytes }
            }
        }
        ResponseBody::Consumed => ResponseBody::Consumed,
        body @ ResponseBody::EncodedStreaming { .. } => body,
    };
    response.set_body(new_body);

    response.headers_mut().remove("content-encoding");
    response.headers_mut().remove("content-length");

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
}
