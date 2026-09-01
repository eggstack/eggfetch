//! Direct (non-proxy) hyper client send path.

#[cfg(feature = "http2")]
use std::error::Error as StdError;
use std::sync::Arc;

use bytes::Bytes;

use crate::body::{BoxBytesStream, ResponseBody};
use crate::error::{Error, Result};
use crate::network_stream::{ConnectionMetadata, NetworkStream, UpgradedStream};
use crate::response::Response;
use crate::trace::{OnEventAction, TraceEvent, TraceObserver, TracePhase};
use crate::transport::{HyperRequestBody, TimeoutHyperClient};

/// Issue a hyper request and return a streaming `Response` bound to the
/// caller's URL.
///
/// When a trace observer is provided, emits `send_request_headers` and
/// `receive_response_headers` lifecycle events.
///
/// For 101 Switching Protocols and successful CONNECT responses,
/// captures the upgrade future and attaches an [`UpgradedStream`] to
/// the response. For ordinary responses, attaches read-only connection
/// metadata when available.
pub(crate) async fn send_request(
    hyper_client: &TimeoutHyperClient,
    request: http::Request<HyperRequestBody>,
    url: url::Url,
    trace: Option<&dyn TraceObserver>,
) -> Result<Response> {
    if let Some(observer) = trace {
        let method = request.method().as_str().to_owned();
        let target = request.uri().to_string();
        if observer.on_event(&TraceEvent::SendRequestHeaders {
            phase: TracePhase::Started,
            method,
            target,
        }) == OnEventAction::Abort
        {
            return Err(Error::TraceCallbackAborted);
        }
    }

    let result = hyper_client.request(request).await.map_err(map_send_error);

    match result {
        Ok(mut hyper_response) => {
            let status = hyper_response.status().as_u16();
            let resp_version = hyper_response.version();
            let resp_headers = hyper_response.headers().clone();

            if let Some(observer) = trace {
                observer.on_event(&TraceEvent::ReceiveResponseHeaders {
                    phase: TracePhase::Complete,
                    status,
                });
            }

            // Always try to capture the upgrade future before consuming
            // the body. For 101 responses, `into_body()` would block
            // forever because hyper transfers the connection IO to the
            // upgrade handler — we must not consume the Incoming body.
            let on_upgrade = hyper::upgrade::on(&mut hyper_response);
            let upgrading = is_upgrade_status(status);

            let mut response = if upgrading {
                // For upgrade responses, do NOT consume the body via
                // into_body(). The Incoming body would block forever.
                // Use an empty buffered body instead.
                let body = ResponseBody::buffered(Bytes::new());
                Response::new(
                    http::StatusCode::from_u16(status).unwrap_or(http::StatusCode::OK),
                    resp_version,
                    resp_headers,
                    url,
                    body,
                )
            } else {
                let stream: BoxBytesStream = wrap_incoming(hyper_response.into_body());
                let body = ResponseBody::streaming(stream);
                Response::new(
                    http::StatusCode::from_u16(status).unwrap_or(http::StatusCode::OK),
                    resp_version,
                    resp_headers,
                    url,
                    body,
                )
            };

            // For upgrade-eligible responses, await the upgrade future
            // and attach the resulting UpgradedStream to the response.
            if upgrading {
                let upgraded = await_upgrade(on_upgrade).await;
                if let Some(stream) = upgraded {
                    response.set_network_stream(NetworkStream::Upgraded(stream));
                }
            }

            Ok(response)
        }
        Err(e) => {
            if let Some(observer) = trace {
                let _ = observer.on_event(&TraceEvent::SendRequestHeaders {
                    phase: TracePhase::Failed,
                    method: String::new(),
                    target: String::new(),
                });
            }
            Err(e)
        }
    }
}

/// Issue a request through the direct connector and return a streaming
/// `Response`. Used for requests with advanced socket options or local
/// address binding.
///
/// When a trace observer is provided, emits `send_request_headers` and
/// `receive_response_headers` lifecycle events.
pub(crate) async fn send_direct_request(
    hyper_client: &crate::transport::TimeoutDirectClient,
    request: http::Request<HyperRequestBody>,
    url: url::Url,
    trace: Option<&dyn TraceObserver>,
) -> Result<Response> {
    if let Some(observer) = trace {
        let method = request.method().as_str().to_owned();
        let target = request.uri().to_string();
        if observer.on_event(&TraceEvent::SendRequestHeaders {
            phase: TracePhase::Started,
            method,
            target,
        }) == OnEventAction::Abort
        {
            return Err(Error::TraceCallbackAborted);
        }
    }

    let result = hyper_client.request(request).await.map_err(map_send_error);

    match result {
        Ok(mut hyper_response) => {
            let status = hyper_response.status().as_u16();
            let resp_version = hyper_response.version();
            let resp_headers = hyper_response.headers().clone();

            if let Some(observer) = trace {
                observer.on_event(&TraceEvent::ReceiveResponseHeaders {
                    phase: TracePhase::Complete,
                    status,
                });
            }

            let on_upgrade = hyper::upgrade::on(&mut hyper_response);
            let upgrading = is_upgrade_status(status);

            let mut response = if upgrading {
                let body = ResponseBody::buffered(Bytes::new());
                Response::new(
                    http::StatusCode::from_u16(status).unwrap_or(http::StatusCode::OK),
                    resp_version,
                    resp_headers,
                    url,
                    body,
                )
            } else {
                let stream: BoxBytesStream = wrap_incoming(hyper_response.into_body());
                let body = ResponseBody::streaming(stream);
                Response::new(
                    http::StatusCode::from_u16(status).unwrap_or(http::StatusCode::OK),
                    resp_version,
                    resp_headers,
                    url,
                    body,
                )
            };

            if upgrading {
                let upgraded = await_upgrade(on_upgrade).await;
                if let Some(stream) = upgraded {
                    response.set_network_stream(NetworkStream::Upgraded(stream));
                }
            }

            Ok(response)
        }
        Err(e) => {
            if let Some(observer) = trace {
                let _ = observer.on_event(&TraceEvent::SendRequestHeaders {
                    phase: TracePhase::Failed,
                    method: String::new(),
                    target: String::new(),
                });
            }
            Err(e)
        }
    }
}

/// Returns `true` for HTTP status codes that indicate a protocol upgrade
/// on the direct (non-proxy) send path.
///
/// Only `101 Switching Protocols` triggers upgrade handling here.
/// Successful CONNECT (200) is handled in the proxy transport path.
fn is_upgrade_status(status: u16) -> bool {
    status == 101
}

/// Await the upgrade future and convert the result into an
/// [`UpgradedStream`].
///
/// Hyper's `Upgraded` preserves leading data in its internal `Rewind`
/// buffer. These bytes (sent by the server in the same write as the
/// 101/CONNECT response headers) are yielded on the first reads from
/// the `Upgraded` stream. We wrap it with `hyper_util::rt::TokioIo`
/// which bridges Hyper's IO traits to Tokio's `AsyncRead + AsyncWrite`.
///
/// Socket address metadata is not available from Hyper's `Upgraded`
/// directly. The metadata is set to defaults; real metadata can be
/// captured at the connector level in a future enhancement.
async fn await_upgrade(on_upgrade: hyper::upgrade::OnUpgrade) -> Option<UpgradedStream> {
    match on_upgrade.await {
        Ok(upgraded) => {
            // Hyper's Upgraded wraps a Rewind buffer that preserves
            // leading data (bytes read past the response headers before
            // the upgrade completed). These bytes are returned first
            // when reading from the Upgraded stream.
            //
            // We cannot extract the leading data separately without
            // downcasting to the concrete IO type (which we don't know).
            // The leading data is preserved inside Hyper's internal
            // rewind buffer and will be yielded on the first reads.
            //
            // End-to-end test: upgraded_stream_leading_data_through_hyper
            // verifies that a server-sent leading payload is returned
            // by the first `read()` on the upgraded stream.
            let leading = Bytes::new();
            // Use hyper-util's TokioIo adapter to bridge Hyper's IO
            // traits to Tokio's AsyncRead + AsyncWrite.
            let adapter = hyper_util::rt::TokioIo::new(upgraded);
            // We don't have socket addresses from Hyper's Upgraded
            // directly. The metadata will be set to defaults; real
            // metadata can be captured at the connector level in a
            // future enhancement.
            let metadata = Arc::new(ConnectionMetadata::default());
            Some(UpgradedStream::from_adapter(adapter, leading, metadata))
        }
        Err(_e) => {
            // Upgrade future failed — response headers are still valid;
            // the upgrade just couldn't be captured.
            None
        }
    }
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
                        Poll::Ready(None)
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
/// Prefer the typed `h2::Error` exposed through Hyper's source chain. The
/// message fallback is retained for Hyper errors that do not expose that
/// cause, but it only recognizes the existing protocol markers.
#[cfg(feature = "http2")]
fn classify_h2_hyper_error(err: &hyper::Error) -> Option<Error> {
    let msg = err.to_string();
    let mut source = StdError::source(err);
    while let Some(cause) = source {
        if let Some(h2_err) = cause.downcast_ref::<h2::Error>() {
            if h2_err.is_io() {
                return None;
            }
            if h2_err.is_go_away() {
                return Some(Error::Http2GoAway {
                    last_stream_id: 0,
                    debug_data: h2_err.to_string(),
                });
            }
            if h2_err.is_reset() {
                let reason = h2_err
                    .reason()
                    .map_or_else(|| h2_err.to_string(), |reason| reason.to_string());
                return Some(Error::Http2StreamReset { reason });
            }
            if h2_err.reason() == Some(h2::Reason::FLOW_CONTROL_ERROR) {
                return Some(Error::Http2FlowControl(h2_err.to_string()));
            }
            return Some(Error::Http2Protocol(h2_err.to_string()));
        }
        source = StdError::source(cause);
    }

    classify_h2_message(&msg)
}

#[cfg(feature = "http2")]
fn classify_h2_message(msg: &str) -> Option<Error> {
    let lower = msg.to_ascii_lowercase();

    if lower.contains("goaway") || lower.contains("go away") {
        return Some(Error::Http2GoAway {
            last_stream_id: 0,
            debug_data: msg.to_string(),
        });
    }
    if lower.contains("rst_stream") || lower.contains("refused_stream") {
        return Some(Error::Http2StreamReset {
            reason: msg.to_string(),
        });
    }
    if lower.contains("flow control") {
        return Some(Error::Http2FlowControl(msg.to_string()));
    }
    if lower.contains("http2") || lower.contains("h2") {
        return Some(Error::Http2Protocol(msg.to_string()));
    }

    None
}

#[cfg(all(test, feature = "http2"))]
mod tests {
    use super::classify_h2_message;

    #[test]
    fn fallback_classifies_current_hyper_h2_messages() {
        assert_eq!(
            classify_h2_message("http2 error: GOAWAY received: NO_ERROR")
                .expect("GOAWAY marker should classify")
                .kind(),
            "http2_go_away"
        );
        assert_eq!(
            classify_h2_message("http2 error: stream error received: REFUSED_STREAM")
                .expect("stream reset marker should classify")
                .kind(),
            "http2_stream_reset"
        );
        assert_eq!(
            classify_h2_message("http2 error: flow control window exhausted")
                .expect("flow-control marker should classify")
                .kind(),
            "http2_flow_control"
        );
    }

    #[test]
    fn fallback_ignores_unrelated_messages() {
        assert!(classify_h2_message("connection reset by peer").is_none());
    }
}
