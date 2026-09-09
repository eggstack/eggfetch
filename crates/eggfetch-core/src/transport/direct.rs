//! Direct (non-proxy) hyper client send path.

#[cfg(feature = "http2")]
use std::error::Error as StdError;
use std::sync::Arc;

use bytes::Bytes;

use crate::body::{BoxBytesStream, ResponseBody, SharedTrailers};
use crate::error::{Error, Result};
use crate::network_stream::{ConnectionMetadata, NetworkStream, UpgradedStream};
use crate::response::Response;
use crate::trace::{OnEventAction, TraceEvent, TraceObserver, TracePhase};
use crate::transport::{HyperRequestBody, TimeoutHyperClient};

/// Emit the `send_request_headers/Started` trace event.
///
/// Shared by the standard Hyper, specialized-direct, SNI-direct, and UDS
/// paths so trace start/abort semantics cannot diverge.
///
/// # Errors
///
/// Returns [`Error::TraceCallbackAborted`] when the observer aborts dispatch.
pub(crate) fn emit_send_start(
    trace: Option<&dyn TraceObserver>,
    method: &str,
    target: &str,
) -> Result<()> {
    if let Some(observer) = trace {
        if observer.on_event(&TraceEvent::SendRequestHeaders {
            phase: TracePhase::Started,
            method: method.to_owned(),
            target: target.to_owned(),
        }) == OnEventAction::Abort
        {
            return Err(Error::TraceCallbackAborted);
        }
    }
    Ok(())
}

/// Emit the `receive_response_headers/Complete` trace event.
///
/// Shared by all Hyper response converters.
pub(crate) fn emit_receive_complete(trace: Option<&dyn TraceObserver>, status: u16) {
    if let Some(observer) = trace {
        observer.on_event(&TraceEvent::ReceiveResponseHeaders {
            phase: TracePhase::Complete,
            status,
        });
    }
}

/// Emit the `send_request_headers/Failed` trace event.
///
/// Shared by all Hyper dispatch error paths.
pub(crate) fn emit_send_failed(trace: Option<&dyn TraceObserver>) {
    if let Some(observer) = trace {
        let _ = observer.on_event(&TraceEvent::SendRequestHeaders {
            phase: TracePhase::Failed,
            method: String::new(),
            target: String::new(),
        });
    }
}

/// Convert a Hyper response into the core streaming `Response`.
///
/// Captures the upgrade future before consuming the body (for 101
/// responses `into_body()` would block forever because Hyper transfers the
/// connection IO to the upgrade handler), builds either an empty buffered
/// body (upgrading) or a streaming body, emits the receive-complete trace
/// event, awaits the upgrade future, and attaches the resulting
/// [`UpgradedStream`]. This is the single response-lifecycle implementation
/// shared by the standard and specialized-direct paths.
async fn finish_hyper_response(
    mut hyper_response: http::Response<hyper::body::Incoming>,
    url: url::Url,
    trace: Option<&dyn TraceObserver>,
) -> Response {
    let status = hyper_response.status().as_u16();
    let resp_version = hyper_response.version();
    let resp_headers = hyper_response.headers().clone();

    emit_receive_complete(trace, status);

    // Always try to capture the upgrade future before consuming the body.
    // For 101 responses, `into_body()` would block forever because hyper
    // transfers the connection IO to the upgrade handler — we must not
    // consume the Incoming body.
    let on_upgrade = hyper::upgrade::on(&mut hyper_response);
    let upgrading = is_upgrade_status(status);

    let mut response = if upgrading {
        // For upgrade responses, do NOT consume the body via into_body().
        // The Incoming body would block forever. Use an empty buffered body.
        let body = ResponseBody::buffered(Bytes::new());
        Response::new(
            http::StatusCode::from_u16(status).unwrap_or(http::StatusCode::OK),
            resp_version,
            resp_headers,
            url,
            body,
        )
    } else {
        let trailers = SharedTrailers::new();
        let stream: BoxBytesStream = wrap_incoming(hyper_response.into_body(), trailers.clone());
        let body = ResponseBody::streaming(stream);
        let mut response = Response::new(
            http::StatusCode::from_u16(status).unwrap_or(http::StatusCode::OK),
            resp_version,
            resp_headers,
            url,
            body,
        );
        response.set_trailers(trailers);
        response
    };

    // For upgrade-eligible responses, await the upgrade future and attach
    // the resulting UpgradedStream to the response.
    if upgrading {
        let upgraded = await_upgrade(on_upgrade).await;
        if let Some(stream) = upgraded {
            response.set_network_stream(NetworkStream::Upgraded(stream));
        }
    }

    response
}

/// Issue a hyper request and return a streaming `Response` bound to the
/// caller's URL.
///
/// When a trace observer is provided, emits `send_request_headers` and
/// `receive_response_headers` lifecycle events.
///
/// For 101 Switching Protocols responses, captures the upgrade future and
/// attaches an [`UpgradedStream`] to the response. For ordinary responses,
/// attaches read-only connection metadata when available.
pub(crate) async fn send_request(
    hyper_client: &TimeoutHyperClient,
    request: http::Request<HyperRequestBody>,
    url: url::Url,
    trace: Option<&dyn TraceObserver>,
) -> Result<Response> {
    emit_send_start(trace, request.method().as_str(), &request.uri().to_string())?;

    let result = hyper_client.request(request).await.map_err(map_send_error);

    match result {
        Ok(hyper_response) => Ok(finish_hyper_response(hyper_response, url, trace).await),
        Err(e) => {
            emit_send_failed(trace);
            Err(e)
        }
    }
}

/// Issue a request through the direct connector and return a streaming
/// `Response`. Used for requests with advanced socket options, local
/// address binding, and the SNI-override path.
///
/// Shares the single [`finish_hyper_response`] lifecycle with the standard
/// Hyper path; only the concrete Hyper client type differs.
///
/// When a trace observer is provided, emits `send_request_headers` and
/// `receive_response_headers` lifecycle events.
pub(crate) async fn send_direct_request(
    hyper_client: &crate::transport::TimeoutDirectClient,
    request: http::Request<HyperRequestBody>,
    url: url::Url,
    trace: Option<&dyn TraceObserver>,
) -> Result<Response> {
    emit_send_start(trace, request.method().as_str(), &request.uri().to_string())?;

    let result = hyper_client.request(request).await.map_err(map_send_error);

    match result {
        Ok(hyper_response) => Ok(finish_hyper_response(hyper_response, url, trace).await),
        Err(e) => {
            emit_send_failed(trace);
            Err(e)
        }
    }
}

/// Returns `true` for HTTP status codes that indicate a protocol upgrade
/// on the direct (non-proxy) send path.
///
/// Only `101 Switching Protocols` triggers upgrade handling here.
/// Successful CONNECT (200) is handled in the proxy transport path.
pub(crate) fn is_upgrade_status(status: u16) -> bool {
    status == 101
}

/// Await the upgrade future and convert the result into an
/// [`UpgradedStream`].
///
/// Hyper's `Upgraded` preserves leading data in its internal `Rewind`
/// buffer. When the concrete IO type is known (custom direct connector
/// or UDS), the upgrade is downcast to recover real socket/TLS metadata
/// captured at the connector where it is observable; leading bytes are
/// carried as `leading_data`. For the standard opaque Hyper path the
/// socket addresses remain explicitly unavailable (`None`) and the
/// `Rewind` buffer is preserved inside the adapter.
///
/// No secret-bearing metadata is captured and no fake zero/default
/// addresses are reported as real observations.
pub(crate) async fn await_upgrade(on_upgrade: hyper::upgrade::OnUpgrade) -> Option<UpgradedStream> {
    match on_upgrade.await {
        Ok(upgraded) => Some(upgrade_with_connector_metadata(upgraded)),
        Err(_e) => {
            // Upgrade future failed — response headers are still valid;
            // the upgrade just couldn't be captured.
            None
        }
    }
}

/// Convert an [`hyper::upgrade::Upgraded`] into an [`UpgradedStream`],
/// recovering connector-observable metadata where possible.
///
/// Attempt order: direct-connector `DirectStream` (real local/remote
/// addrs + TLS version/cipher/ALPN), UDS `UdsStream` (UDS transport
/// without inventing IPs), then opaque fallback (explicitly unavailable
/// metadata, `Rewind` preserved). Leading data handling differs per
/// branch: downcast branches carry `read_buf` explicitly; the opaque
/// branch preserves Hyper's internal `Rewind` and yields it on first
/// reads (see `upgraded_stream_leading_data_through_hyper`).
pub(crate) fn upgrade_with_connector_metadata(
    upgraded: hyper::upgrade::Upgraded,
) -> UpgradedStream {
    // Direct connector: real socket addresses + TLS info observable.
    match upgraded
        .downcast::<hyper_util::rt::TokioIo<crate::transport::direct_connector::DirectStream>>()
    {
        Ok(parts) => {
            let direct = parts.io.into_inner();
            let leading = parts.read_buf;
            match direct {
                crate::transport::direct_connector::DirectStream::Tcp(tcp) => {
                    UpgradedStream::from_tcp(tcp, leading)
                }
                crate::transport::direct_connector::DirectStream::Tls(tls) => {
                    let (_, conn) = tls.get_ref();
                    let tls_info = crate::network_stream::tls_info_from_rustls(conn, None);
                    UpgradedStream::from_tls(*tls, leading, tls_info)
                }
            }
        }
        Err(upgraded) => {
            // UDS connector: report UDS transport without IP addresses.
            #[cfg(unix)]
            match upgraded.downcast::<hyper_util::rt::TokioIo<crate::transport::uds::UdsStream>>() {
                Ok(parts) => {
                    let uds = parts.io.into_inner();
                    let leading = parts.read_buf;
                    let (kind, tls_info) = match &uds {
                        crate::transport::uds::UdsStream::Plain(_) => {
                            (crate::network_stream::TransportKind::Unix, None)
                        }
                        crate::transport::uds::UdsStream::Tls(tls) => {
                            let (_, conn) = tls.get_ref();
                            (
                                crate::network_stream::TransportKind::TlsUnix,
                                Some(crate::network_stream::tls_info_from_rustls(conn, None)),
                            )
                        }
                    };
                    let metadata = Arc::new(ConnectionMetadata {
                        local_addr: None,
                        peer_addr: None,
                        transport_kind: kind,
                        tls_info,
                    });
                    UpgradedStream::from_adapter(uds, leading, metadata)
                }
                Err(upgraded) => opaque_upgraded(upgraded),
            }
            #[cfg(not(unix))]
            {
                opaque_upgraded(upgraded)
            }
        }
    }
}

/// Opaque fallback: socket metadata explicitly unavailable.
///
/// Preserves Hyper's internal `Rewind` buffer (leading bytes yielded on
/// first reads) by wrapping the full `Upgraded` value.
fn opaque_upgraded(upgraded: hyper::upgrade::Upgraded) -> UpgradedStream {
    let leading = Bytes::new();
    let adapter = hyper_util::rt::TokioIo::new(upgraded);
    let metadata = Arc::new(ConnectionMetadata::default());
    UpgradedStream::from_adapter(adapter, leading, metadata)
}

/// Wrap a hyper `Incoming` body into a `BoxBytesStream`.
///
/// Data frames are yielded exactly as before. When a trailers frame
/// arrives (HTTP/1.1 chunked trailers or HTTP/2 trailing HEADERS where
/// Hyper exposes them), the headers are stored in `trailers` and the
/// stream ends normally. Body errors before trailers remain body errors;
/// no trailer state is fabricated. Partial consumption leaves `trailers`
/// empty; callers must drive the stream to EOF before reading trailers.
/// Cancellation and pool-lease lifetime are preserved: dropping the stream
/// early simply never populates trailers.
pub(crate) fn wrap_incoming(
    incoming: hyper::body::Incoming,
    trailers: SharedTrailers,
) -> BoxBytesStream {
    use futures_core::Stream;
    use http_body::Body;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    struct IncomingStream {
        inner: hyper::body::Incoming,
        trailers: SharedTrailers,
    }

    impl Stream for IncomingStream {
        type Item = Result<Bytes>;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            match Pin::new(&mut self.inner).poll_frame(cx) {
                Poll::Ready(Some(Ok(frame))) => {
                    if frame.is_data() {
                        match frame.into_data() {
                            Ok(data) => Poll::Ready(Some(Ok(data))),
                            Err(_) => {
                                Poll::Ready(Some(Err(Error::Body("invalid data frame".into()))))
                            }
                        }
                    } else if frame.is_trailers() {
                        if let Ok(headers) = frame.into_trailers() {
                            self.trailers.store(headers);
                            #[cfg(feature = "tracing")]
                            tracing::debug!("eggfetch: captured HTTP trailers");
                        }
                        // Non-trailers non-data frames end cleanly without
                        // fabrication.
                        Poll::Ready(None)
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

    Box::pin(IncomingStream {
        inner: incoming,
        trailers,
    })
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
                // h2 0.4 does not expose the GOAWAY last_stream_id through
                // its public API (Kind::GoAway holds debug bytes/reason only),
                // so report 0 as a placeholder with full detail in debug_data.
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
        // Same placeholder as above: the message fallback carries no
        // last_stream_id, so report 0 with the raw message preserved.
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
