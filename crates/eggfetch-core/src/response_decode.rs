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
            let decoded_stream =
                crate::compression::decompress_stream(stream, content_encoding, true, limit)?;
            ResponseBody::Streaming {
                stream: decoded_stream,
                lease,
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
    };
    response.set_body(new_body);

    response.headers_mut().remove("content-encoding");
    response.headers_mut().remove("content-length");

    Ok(response)
}
