//! Per-chunk read timeout wrapper for response body streams.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use futures_core::Stream;
use pin_project_lite::pin_project;
use tokio::time::{Instant, Sleep};

use crate::body::BoxBytesStream;
use crate::error::{Error, Result};
use crate::timeout::TimeoutPhase;

pin_project! {
    /// A stream wrapper that enforces a per-chunk read timeout.
    ///
    /// If no chunk arrives within `duration`, the stream yields
    /// `Err(Error::Timeout { phase: Read, .. })` and then terminates.
    /// Inner stream errors are propagated without modification.
    #[must_use = "streams do nothing unless polled"]
    pub struct ReadTimeoutStream<S> {
        #[pin]
        inner: S,
        deadline: Option<Pin<Box<Sleep>>>,
        duration: Duration,
        started: bool,
        timed_out: bool,
    }
}

impl<S> ReadTimeoutStream<S> {
    /// Wrap `stream` so that each chunk must arrive within `duration`.
    pub(crate) fn new(stream: S, duration: Duration) -> Self {
        Self {
            inner: stream,
            deadline: None,
            duration,
            started: false,
            timed_out: false,
        }
    }
}

impl<S> Stream for ReadTimeoutStream<S>
where
    S: Stream<Item = Result<Bytes>>,
{
    type Item = Result<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.project();

        if *me.timed_out {
            return Poll::Ready(None);
        }

        // Start the per-chunk timer when the body is first consumed. The
        // response may be handed to a different buffering runtime by the
        // synchronous Python adapter after headers have arrived.
        if !*me.started {
            *me.started = true;
            *me.deadline = Some(Box::pin(tokio::time::sleep(*me.duration)));
        }

        // Poll the inner stream first. A ready chunk always wins over a
        // firing deadline.
        match me.inner.poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                // Chunk arrived; reset the deadline.
                if let Some(deadline) = me.deadline.as_mut() {
                    deadline.as_mut().reset(Instant::now() + *me.duration);
                }
                return Poll::Ready(Some(Ok(bytes)));
            }
            Poll::Ready(Some(Err(e))) => {
                // Inner error: propagate immediately, do not mask with
                // a timeout.
                *me.deadline = None;
                return Poll::Ready(Some(Err(e)));
            }
            Poll::Ready(None) => {
                *me.deadline = None;
                return Poll::Ready(None);
            }
            Poll::Pending => {}
        }

        // Inner stream is pending. Check the deadline.
        match me.deadline.as_mut() {
            Some(deadline) => match deadline.as_mut().poll(cx) {
                Poll::Ready(()) => {
                    *me.deadline = None;
                    *me.timed_out = true;
                    Poll::Ready(Some(Err(Error::Timeout {
                        phase: TimeoutPhase::Read,
                        elapsed: *me.duration,
                    })))
                }
                Poll::Pending => Poll::Pending,
            },
            None => Poll::Pending,
        }
    }
}

/// Convenience constructor: wrap a `BoxBytesStream` with a read timeout.
pub(crate) fn read_timeout_stream(stream: BoxBytesStream, duration: Duration) -> BoxBytesStream {
    Box::pin(ReadTimeoutStream::new(stream, duration))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{stream, StreamExt};

    #[tokio::test]
    async fn yields_chunks_within_timeout() {
        let chunks = vec![Ok(Bytes::from("a")), Ok(Bytes::from("b"))];
        let inner = stream::iter(chunks);
        let mut s = Box::pin(ReadTimeoutStream::new(inner, Duration::from_secs(1)));
        assert_eq!(s.next().await.unwrap().unwrap(), "a");
        assert_eq!(s.next().await.unwrap().unwrap(), "b");
        assert!(s.next().await.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn yields_timeout_error_on_stall() {
        let inner = stream::pending::<Result<Bytes>>();
        let mut s = Box::pin(ReadTimeoutStream::new(inner, Duration::from_millis(50)));
        let result = s.next().await;
        let err = result.unwrap().unwrap_err();
        match err {
            Error::Timeout {
                phase: TimeoutPhase::Read,
                ..
            } => {}
            other => panic!("expected read timeout, got {other:?}"),
        }
        assert!(s.as_ref().get_ref().deadline.is_none());
        assert!(s.next().await.is_none());
    }

    #[tokio::test]
    async fn propagates_inner_error() {
        let inner = stream::iter(vec![Err(Error::Body("oops".into()))]);
        let mut s = Box::pin(ReadTimeoutStream::new(inner, Duration::from_secs(1)));
        let err = s.next().await.unwrap().unwrap_err();
        assert!(matches!(err, Error::Body(_)));
    }

    #[tokio::test]
    async fn inner_end_of_stream_terminates() {
        let inner = stream::empty::<Result<Bytes>>();
        let mut s = Box::pin(ReadTimeoutStream::new(inner, Duration::from_secs(1)));
        assert!(s.next().await.is_none());
        assert!(s.as_ref().get_ref().deadline.is_none());
    }
}
