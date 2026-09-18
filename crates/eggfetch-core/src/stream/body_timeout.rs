//! Unified response-body timeout: per-chunk read inactivity plus the
//! absolute native total deadline.
//!
//! The total deadline is an absolute `std::time::Instant` computed once at
//! request preparation and never reset. Timers are created lazily on first
//! body poll in the consuming runtime so a response handed to another
//! runtime (e.g. the synchronous Python adapter) never carries a
//! runtime-bound `Sleep` across the boundary. An already-expired total
//! deadline wins before polling the inner body, and when both deadlines are
//! observably expired at the same poll boundary `Total` is preferred as the
//! outer lifecycle cap. A ready chunk that arrives at or after the absolute
//! deadline never extends the request.

use std::future::Future as _;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use futures_core::Stream;
use pin_project_lite::pin_project;
use tokio::time::{Instant, Sleep};

use crate::body::BoxBytesStream;
use crate::error::{Error, Result};
use crate::timeout::{ResponseDeadline, TimeoutPhase};

pin_project! {
    /// Stream wrapper enforcing read inactivity and absolute total deadlines.
    ///
    /// After emitting a timeout error the stream fuses: the next poll
    /// returns `None` so repeated polling does not repeatedly emit errors.
    /// The outer lease wrapper releases the pool permit on that terminal
    /// error/EOF; dropping remains ordinary cancellation.
    #[must_use = "streams do nothing unless polled"]
    pub struct BodyTimeoutStream<S> {
        #[pin]
        inner: S,
        read: Option<Duration>,
        read_deadline: Option<Pin<Box<Sleep>>>,
        total: Option<ResponseDeadline>,
        total_sleep: Option<Pin<Box<Sleep>>>,
        started: bool,
        timed_out: bool,
    }
}

impl<S> BodyTimeoutStream<S> {
    /// Wrap `stream` with an optional per-chunk read timeout and an
    /// optional absolute total deadline.
    pub(crate) fn new(stream: S, read: Option<Duration>, total: Option<ResponseDeadline>) -> Self {
        Self {
            inner: stream,
            read,
            read_deadline: None,
            total,
            total_sleep: None,
            started: false,
            timed_out: false,
        }
    }

    fn total_timeout_error(total: &ResponseDeadline) -> Error {
        Error::Timeout {
            phase: TimeoutPhase::Total,
            elapsed: total.elapsed(),
        }
    }
}

impl<S> Stream for BodyTimeoutStream<S>
where
    S: Stream<Item = Result<Bytes>>,
{
    type Item = Result<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut me = self.project();

        if *me.timed_out {
            return Poll::Ready(None);
        }

        // First poll in the consuming runtime: materialize timers here so
        // no Tokio sleep crosses a runtime boundary. An already-expired
        // total deadline wins without polling the inner transport.
        if !*me.started {
            *me.started = true;
            if let Some(total) = me.total.as_ref() {
                if total.is_expired() {
                    *me.timed_out = true;
                    let err = Self::total_timeout_error(total);
                    return Poll::Ready(Some(Err(err)));
                }
                let tokio_deadline = Instant::from_std(total.deadline());
                *me.total_sleep = Some(Box::pin(tokio::time::sleep_until(tokio_deadline)));
            }
            if let Some(read) = *me.read {
                *me.read_deadline = Some(Box::pin(tokio::time::sleep(read)));
            }
        }

        // Absolute deadline is authoritative: a ready chunk at or after the
        // deadline must not extend the request. Check wall-clock before
        // polling the inner source.
        if let Some(total) = me.total.as_ref() {
            if total.is_expired() {
                *me.read_deadline = None;
                *me.total_sleep = None;
                *me.timed_out = true;
                let err = Self::total_timeout_error(total);
                return Poll::Ready(Some(Err(err)));
            }
        }

        // Poll the inner stream. A ready chunk wins over a pending
        // deadline, but never over an already-expired absolute deadline
        // (checked above and re-checked below for the same-poll race).
        match me.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                // Same-poll race: the chunk and the absolute deadline
                // became observable together; the outer lifecycle cap wins.
                if let Some(total) = me.total.as_ref() {
                    if total.is_expired() {
                        *me.read_deadline = None;
                        *me.total_sleep = None;
                        *me.timed_out = true;
                        let err = Self::total_timeout_error(total);
                        return Poll::Ready(Some(Err(err)));
                    }
                }
                if let (Some(read_deadline), Some(read)) =
                    (me.read_deadline.as_mut(), me.read.as_ref())
                {
                    read_deadline.as_mut().reset(Instant::now() + *read);
                }
                return Poll::Ready(Some(Ok(bytes)));
            }
            Poll::Ready(Some(Err(e))) => {
                *me.read_deadline = None;
                *me.total_sleep = None;
                return Poll::Ready(Some(Err(e)));
            }
            Poll::Ready(None) => {
                *me.read_deadline = None;
                *me.total_sleep = None;
                return Poll::Ready(None);
            }
            Poll::Pending => {}
        }

        // Inner is pending: whichever deadline fires first surfaces. When
        // both are observably expired, prefer Total as the outer cap.
        let total_fired = match me.total_sleep.as_mut() {
            Some(sleep) => sleep.as_mut().poll(cx).is_ready(),
            None => false,
        };
        if total_fired {
            if let Some(total) = me.total.as_ref() {
                let err = Self::total_timeout_error(total);
                *me.read_deadline = None;
                *me.total_sleep = None;
                *me.timed_out = true;
                return Poll::Ready(Some(Err(err)));
            }
        }
        match me.read_deadline.as_mut() {
            Some(read_deadline) => match read_deadline.as_mut().poll(cx) {
                Poll::Ready(()) => {
                    *me.read_deadline = None;
                    *me.total_sleep = None;
                    *me.timed_out = true;
                    Poll::Ready(Some(Err(Error::Timeout {
                        phase: TimeoutPhase::Read,
                        elapsed: me.read.unwrap_or(Duration::ZERO),
                    })))
                }
                Poll::Pending => Poll::Pending,
            },
            None => Poll::Pending,
        }
    }
}

/// Wrap a type-erased response stream with read and/or total deadlines.
///
/// Returns the original stream ordering unchanged when neither deadline is
/// configured.
pub(crate) fn body_timeout_stream(
    stream: BoxBytesStream,
    read: Option<Duration>,
    total: Option<ResponseDeadline>,
) -> BoxBytesStream {
    if read.is_none() && total.is_none() {
        return stream;
    }
    Box::pin(BodyTimeoutStream::new(stream, read, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{stream, StreamExt};

    #[tokio::test]
    async fn total_only_stall_reports_total() {
        let inner = stream::pending::<Result<Bytes>>();
        let deadline = ResponseDeadline::new(
            std::time::Instant::now() + Duration::from_millis(40),
            Duration::from_millis(40),
        );
        let mut s = Box::pin(BodyTimeoutStream::new(inner, None, Some(deadline)));
        let err = s.next().await.unwrap().unwrap_err();
        assert!(matches!(
            err,
            Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            }
        ));
        assert!(s.next().await.is_none());
    }

    #[tokio::test]
    async fn already_expired_total_wins_without_polling_inner() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let polls = Arc::new(AtomicUsize::new(0));
        let polls_inner = polls.clone();
        let inner = stream::unfold((), move |()| {
            let polls_inner = polls_inner.clone();
            async move {
                polls_inner.fetch_add(1, Ordering::SeqCst);
                Some((Ok(Bytes::from_static(b"late")), ()))
            }
        });
        let deadline = ResponseDeadline::new(
            std::time::Instant::now()
                .checked_sub(Duration::from_millis(1))
                .unwrap_or_else(std::time::Instant::now),
            Duration::from_millis(10),
        );
        let mut s = Box::pin(BodyTimeoutStream::new(inner, None, Some(deadline)));
        // Delay first poll until after the deadline; the ready inner chunk
        // must not extend the request.
        tokio::time::sleep(Duration::from_millis(5)).await;
        let err = s.next().await.unwrap().unwrap_err();
        assert!(matches!(
            err,
            Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            }
        ));
        assert_eq!(polls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn read_wins_when_shorter() {
        let inner = stream::pending::<Result<Bytes>>();
        let deadline = ResponseDeadline::new(
            std::time::Instant::now() + Duration::from_secs(5),
            Duration::from_secs(5),
        );
        let mut s = Box::pin(BodyTimeoutStream::new(
            inner,
            Some(Duration::from_millis(30)),
            Some(deadline),
        ));
        let err = s.next().await.unwrap().unwrap_err();
        assert!(matches!(
            err,
            Error::Timeout {
                phase: TimeoutPhase::Read,
                ..
            }
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn total_wins_when_shorter_despite_trickle() {
        // Chunks arrive within the read budget but aggregate past total.
        let inner = stream::unfold(0, |i| async move {
            if i < 10 {
                tokio::time::sleep(Duration::from_millis(10)).await;
                Some((Ok(Bytes::from_static(b"x")), i + 1))
            } else {
                None
            }
        });
        let deadline = ResponseDeadline::new(
            std::time::Instant::now() + Duration::from_millis(45),
            Duration::from_millis(45),
        );
        let mut s = Box::pin(BodyTimeoutStream::new(
            inner,
            Some(Duration::from_secs(5)),
            Some(deadline),
        ));
        let mut saw_total = false;
        while let Some(item) = s.next().await {
            if let Err(Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            }) = item
            {
                saw_total = true;
                break;
            }
        }
        assert!(saw_total, "trickle must not extend total");
    }

    #[tokio::test]
    async fn read_timer_resets_per_chunk() {
        let inner = stream::iter(vec![
            Ok(Bytes::from_static(b"a")),
            Ok(Bytes::from_static(b"b")),
        ]);
        let mut s = Box::pin(BodyTimeoutStream::new(
            inner,
            Some(Duration::from_secs(1)),
            None,
        ));
        assert_eq!(s.next().await.unwrap().unwrap(), "a");
        assert_eq!(s.next().await.unwrap().unwrap(), "b");
        assert!(s.next().await.is_none());
    }
}
