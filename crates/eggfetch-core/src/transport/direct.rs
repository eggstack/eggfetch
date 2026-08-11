//! Direct (non-proxy) hyper client send path.

use bytes::Bytes;

use crate::body::{BoxBytesStream, ResponseBody};
use crate::error::{Error, Result};
use crate::response::Response;
use crate::transport::{HyperRequestBody, TimeoutHyperClient};

/// Issue a hyper request and return a streaming `Response` bound to the
/// caller's URL.
pub(crate) async fn send_request(
    hyper_client: &TimeoutHyperClient,
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

/// Issue a request through the direct connector and return a streaming
/// `Response`. Used for requests with advanced socket options or local
/// address binding.
pub(crate) async fn send_direct_request(
    hyper_client: &crate::transport::TimeoutDirectClient,
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
///
/// # Trailers
///
/// HTTP trailers (both HTTP/1.1 chunked trailers and HTTP/2 trailing
/// HEADERS frames) are **not supported**. This adapter only yields data
/// frames. When a trailers frame arrives, the stream ends normally
/// (returns `Poll::Ready(None)`) without surfacing the trailer headers.
/// This is a known limitation; trailers may be supported in a future
/// milestone.
pub(crate) fn wrap_incoming(incoming: hyper::body::Incoming) -> BoxBytesStream {
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
///
/// When the `http2` feature is enabled, h2-specific error information
/// is extracted where possible. Hyper wraps h2 errors internally; we
/// inspect the error string for known h2 patterns and map them to
/// specific eggfetch error variants. When the specific h2 reason code
/// cannot be determined, the error falls through to the generic
/// `Error::Hyper` path.
pub(crate) fn map_send_error(err: hyper_util::client::legacy::Error) -> Error {
    let mut current: Option<&dyn std::error::Error> = Some(&err);
    while let Some(e) = current {
        if let Some(hyper_err) = e.downcast_ref::<hyper::Error>() {
            // Try to extract h2-specific error information.
            #[cfg(feature = "http2")]
            if let Some(h2_err) = classify_h2_hyper_error(hyper_err) {
                return h2_err;
            }
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

/// Attempt to classify a `hyper::Error` as a specific HTTP/2 error.
///
/// Hyper does not expose the inner `h2::Error` through a public API.
/// We inspect the error's `Display` output for known h2 error patterns
/// and map them to specific eggfetch error variants. When the pattern
/// cannot be determined, returns `None` to fall through to the generic
/// error path.
#[cfg(feature = "http2")]
fn classify_h2_hyper_error(err: &hyper::Error) -> Option<Error> {
    let msg = err.to_string();
    let lower = msg.to_lowercase();

    if lower.contains("goaway") || lower.contains("go away") {
        return Some(Error::Http2GoAway {
            last_stream_id: 0,
            debug_data: msg,
        });
    }
    if lower.contains("reset") || lower.contains("rst_stream") {
        return Some(Error::Http2StreamReset { reason: msg });
    }
    if lower.contains("flow control") {
        return Some(Error::Http2FlowControl(msg));
    }
    if lower.contains("http2") || lower.contains("h2") {
        return Some(Error::Http2Protocol(msg));
    }

    None
}
