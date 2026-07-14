//! Direct (non-proxy) hyper client send path.

use bytes::Bytes;

use crate::body::{BoxBytesStream, ResponseBody};
use crate::error::{Error, Result};
use crate::response::Response;
use crate::transport::{HyperClient, HyperRequestBody};

/// Issue a hyper request and return a streaming `Response` bound to the
/// caller's URL.
pub(crate) async fn send_request(
    hyper_client: &HyperClient,
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
fn wrap_incoming(incoming: hyper::body::Incoming) -> BoxBytesStream {
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
fn map_send_error(err: hyper_util::client::legacy::Error) -> Error {
    let mut current: Option<&dyn std::error::Error> = Some(&err);
    while let Some(e) = current {
        if let Some(hyper_err) = e.downcast_ref::<hyper::Error>() {
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
