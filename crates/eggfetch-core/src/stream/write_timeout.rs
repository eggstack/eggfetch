//! Per-chunk write timeout wrapper for request body streams.

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
    /// A stream wrapper that enforces a per-chunk write timeout.
    ///
    /// If the caller does not produce the next chunk within `duration`,
    /// the stream yields `Err(Error::Timeout { phase: Write, .. })` and
    /// then terminates. This guards against stalled upload producers;
    /// it does not directly observe OS-level socket write stalls.
    #[must_use = "streams do nothing unless polled"]
    pub struct WriteTimeoutStream<S> {
        #[pin]
        inner: S,
        deadline: Option<Pin<Box<Sleep>>>,
        duration: Duration,
        started: bool,
        timed_out: bool,
    }
}

impl<S> WriteTimeoutStream<S> {
    /// Wrap `stream` so that each chunk must be produced within `duration`.
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

impl<S> Stream for WriteTimeoutStream<S>
where
    S: Stream<Item = Result<Bytes>>,
{
    type Item = Result<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.project();

        if *me.timed_out {
            return Poll::Ready(None);
        }

        // Start the per-chunk timer when the body is first polled. The
        // wrapper is installed before connection establishment, so an
        // eager timer would charge connect/TLS/proxy setup time against
        // the first chunk's write budget.
        if !*me.started {
            *me.started = true;
            *me.deadline = Some(Box::pin(tokio::time::sleep(*me.duration)));
        }

        match me.inner.poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                if let Some(deadline) = me.deadline.as_mut() {
                    deadline.as_mut().reset(Instant::now() + *me.duration);
                }
                return Poll::Ready(Some(Ok(bytes)));
            }
            Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Pending => {}
        }

        // Inner stream is pending. Check the deadline.
        match me.deadline.as_mut() {
            Some(deadline) => match deadline.as_mut().poll(cx) {
                Poll::Ready(()) => {
                    *me.deadline = None;
                    *me.timed_out = true;
                    Poll::Ready(Some(Err(Error::Timeout {
                        phase: TimeoutPhase::Write,
                        elapsed: *me.duration,
                    })))
                }
                Poll::Pending => Poll::Pending,
            },
            None => Poll::Pending,
        }
    }
}

/// Convenience constructor: wrap a `BoxBytesStream` with a write timeout.
pub(crate) fn write_timeout_stream(stream: BoxBytesStream, duration: Duration) -> BoxBytesStream {
    Box::pin(WriteTimeoutStream::new(stream, duration))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{stream, StreamExt};

    #[tokio::test]
    async fn yields_chunks_within_timeout() {
        let chunks = vec![Ok(Bytes::from("a")), Ok(Bytes::from("b"))];
        let inner = stream::iter(chunks);
        let mut s = Box::pin(WriteTimeoutStream::new(inner, Duration::from_secs(1)));
        assert_eq!(s.next().await.unwrap().unwrap(), "a");
        assert_eq!(s.next().await.unwrap().unwrap(), "b");
        assert!(s.next().await.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn yields_timeout_error_on_stall() {
        let inner = stream::pending::<Result<Bytes>>();
        let mut s = Box::pin(WriteTimeoutStream::new(inner, Duration::from_millis(50)));
        let err = s.next().await.unwrap().unwrap_err();
        match err {
            Error::Timeout {
                phase: TimeoutPhase::Write,
                ..
            } => {}
            other => panic!("expected write timeout, got {other:?}"),
        }
        assert!(s.next().await.is_none());
    }

    #[tokio::test]
    async fn propagates_inner_error() {
        let inner = stream::iter(vec![Err(Error::Body("nope".into()))]);
        let mut s = Box::pin(WriteTimeoutStream::new(inner, Duration::from_secs(1)));
        let err = s.next().await.unwrap().unwrap_err();
        assert!(matches!(err, Error::Body(_)));
    }

    #[tokio::test]
    async fn timer_starts_on_first_poll_not_construction() {
        let inner = stream::pending::<Result<Bytes>>();
        let mut s = Box::pin(WriteTimeoutStream::new(inner, Duration::from_millis(80)));
        // Simulate connect/TLS/proxy setup outlasting the write budget
        // before the body is ever polled.
        tokio::time::sleep(Duration::from_millis(160)).await;
        let first_poll = std::time::Instant::now();
        let err = s.next().await.unwrap().unwrap_err();
        match err {
            Error::Timeout {
                phase: TimeoutPhase::Write,
                ..
            } => {}
            other => panic!("expected write timeout, got {other:?}"),
        }
        // The full budget must be available from the first poll; an
        // eagerly-started timer would already have expired above.
        assert!(
            first_poll.elapsed() >= Duration::from_millis(60),
            "write timer must start on first poll, not construction"
        );
    }
}
