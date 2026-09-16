//! Physical connection admission and established-transport I/O guardrails.

use std::future::Future;
use std::io::{self, IoSlice};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use http::Uri;
use hyper_util::client::legacy::connect::{Connected, Connection};
use tokio::sync::TryAcquireError;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tower_service::Service;

use super::metrics::TransportMetrics;

/// Stable message used by [`crate::Error::is_physical_connection_admission_timeout`].
pub(crate) const PHYSICAL_ADMISSION_TIMEOUT: &str = "physical connection admission timeout";
/// Stable message used when a physical policy cannot admit any connection.
pub(crate) const INVALID_PHYSICAL_POLICY: &str =
    "invalid physical connection policy: max_live must be greater than zero, and admission_timeout requires max_live";

/// Direction of an established transport inactivity timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportIoDirection {
    /// No bytes arrived before the read deadline.
    Read,
    /// No bytes were accepted before the write deadline.
    Write,
}

/// Opt-in cap on live Hyper transport connections.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhysicalConnectionPolicy {
    /// Maximum number of live established Hyper connections for this client.
    /// Idle pooled connections retain their permit.
    pub max_live: Option<usize>,
    /// Maximum time to wait for a physical connection permit.
    pub admission_timeout: Option<Duration>,
}

/// Opt-in inactivity limits for established transport I/O.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TransportIoTimeout {
    /// Maximum time without read progress.
    pub read: Option<Duration>,
    /// Maximum time without write progress.
    pub write: Option<Duration>,
}

/// Shared per-client lifecycle state.
#[derive(Clone, Default)]
pub(crate) struct LifecycleConfig {
    pub(crate) admission: Option<Arc<Semaphore>>,
    pub(crate) admission_timeout: Option<Duration>,
    pub(crate) io_timeout: TransportIoTimeout,
    pub(crate) invalid: bool,
    pub(crate) metrics: Arc<TransportMetrics>,
}

impl LifecycleConfig {
    pub(crate) fn from_policy(
        physical: PhysicalConnectionPolicy,
        io_timeout: TransportIoTimeout,
        metrics: Arc<TransportMetrics>,
    ) -> Self {
        let invalid = physical.max_live == Some(0)
            || (physical.max_live.is_none() && physical.admission_timeout.is_some());
        Self {
            admission: physical
                .max_live
                .filter(|max| *max > 0)
                .map(|max| Arc::new(Semaphore::new(max))),
            admission_timeout: physical.admission_timeout,
            io_timeout,
            invalid,
            metrics,
        }
    }
}

/// Connector wrapper that acquires one permit per newly established stream.
#[derive(Clone)]
pub(crate) struct LifecycleConnector<C> {
    inner: C,
    config: Arc<LifecycleConfig>,
}

impl<C> LifecycleConnector<C> {
    pub(crate) fn new(inner: C, config: Arc<LifecycleConfig>) -> Self {
        Self { inner, config }
    }
}

impl<C> Service<Uri> for LifecycleConnector<C>
where
    C: Service<Uri> + Send + 'static,
    C::Response: hyper::rt::Read + hyper::rt::Write + Connection + Unpin + Send + 'static,
    C::Error: Into<Box<dyn std::error::Error + Send + Sync>> + 'static,
    C::Future: Send + 'static,
{
    type Response = LifecycleConnection<C::Response>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let config = self.config.clone();
        if config.invalid {
            return Box::pin(async {
                Err(
                    Box::new(crate::error::Error::Pool(INVALID_PHYSICAL_POLICY.into()))
                        as Box<dyn std::error::Error + Send + Sync>,
                )
            });
        }
        let inner_future = self.inner.call(dst);
        Box::pin(async move {
            let permit = if let Some(semaphore) = config.admission.as_ref() {
                let result = match semaphore.clone().try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(TryAcquireError::Closed) => {
                        return Err(Box::new(crate::error::Error::Pool(
                            "physical connection admission closed".into(),
                        ))
                            as Box<dyn std::error::Error + Send + Sync>);
                    }
                    Err(TryAcquireError::NoPermits) => {
                        config.metrics.record_physical_admission_wait();
                        match config.admission_timeout {
                            Some(timeout) => {
                                tokio::time::timeout(timeout, semaphore.clone().acquire_owned())
                                    .await
                                    .map_err(|_| {
                                        config.metrics.record_physical_admission_timeout();
                                        Box::new(crate::error::Error::Pool(
                                            PHYSICAL_ADMISSION_TIMEOUT.into(),
                                        ))
                                            as Box<dyn std::error::Error + Send + Sync>
                                    })?
                                    .map_err(|_| {
                                        Box::new(crate::error::Error::Pool(
                                            "physical connection admission closed".into(),
                                        ))
                                            as Box<dyn std::error::Error + Send + Sync>
                                    })?
                            }
                            None => semaphore.clone().acquire_owned().await.map_err(|_| {
                                Box::new(crate::error::Error::Pool(
                                    "physical connection admission closed".into(),
                                ))
                                    as Box<dyn std::error::Error + Send + Sync>
                            })?,
                        }
                    }
                };
                Some(AdmissionPermit::new(result, config.metrics.clone()))
            } else {
                None
            };
            let inner = inner_future.await.map_err(Into::into)?;
            Ok(LifecycleConnection::new(
                inner,
                permit,
                config.io_timeout,
                config.metrics.clone(),
            ))
        })
    }
}

/// A semaphore permit whose lifetime is also reflected in transport metrics.
pub(crate) struct AdmissionPermit {
    _permit: OwnedSemaphorePermit,
    metrics: Arc<TransportMetrics>,
}

impl AdmissionPermit {
    fn new(permit: OwnedSemaphorePermit, metrics: Arc<TransportMetrics>) -> Self {
        metrics.record_physical_connection_admitted();
        Self {
            _permit: permit,
            metrics,
        }
    }
}

impl Drop for AdmissionPermit {
    fn drop(&mut self) {
        self.metrics.record_physical_connection_released();
    }
}

/// Established connection wrapper retaining its physical permit.
pub(crate) struct LifecycleConnection<C> {
    inner: C,
    connection_permit: Option<AdmissionPermit>,
    io_timeout: TransportIoTimeout,
    metrics: Arc<TransportMetrics>,
    read_timer: Option<Pin<Box<tokio::time::Sleep>>>,
    write_timer: Option<Pin<Box<tokio::time::Sleep>>>,
}

impl<C> LifecycleConnection<C> {
    fn new(
        inner: C,
        permit: Option<AdmissionPermit>,
        io_timeout: TransportIoTimeout,
        metrics: Arc<TransportMetrics>,
    ) -> Self {
        Self {
            inner,
            connection_permit: permit,
            io_timeout,
            metrics,
            read_timer: None,
            write_timer: None,
        }
    }

    pub(crate) fn into_inner_and_permit(self) -> (C, Option<AdmissionPermit>) {
        (self.inner, self.connection_permit)
    }

    fn poll_timer(
        timer: &mut Option<Pin<Box<tokio::time::Sleep>>>,
        timeout: Option<Duration>,
        cx: &mut Context<'_>,
        direction: TransportIoDirection,
        metrics: &TransportMetrics,
    ) -> Poll<io::Result<()>> {
        let Some(timeout) = timeout else {
            return Poll::Pending;
        };
        let sleep = timer.get_or_insert_with(|| Box::pin(tokio::time::sleep(timeout)));
        if sleep.as_mut().poll(cx).is_ready() {
            *timer = None;
            metrics.record_transport_io_timeout(direction);
            Poll::Ready(Err(io::Error::new(
                io::ErrorKind::TimedOut,
                crate::error::Error::TransportIoTimeout {
                    direction,
                    elapsed: timeout,
                },
            )))
        } else {
            Poll::Pending
        }
    }
}

impl<C: Connection> Connection for LifecycleConnection<C> {
    fn connected(&self) -> Connected {
        self.inner.connected()
    }
}

impl<C> hyper::rt::Read for LifecycleConnection<C>
where
    C: hyper::rt::Read + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        let Some(timeout) = self.io_timeout.read else {
            return Pin::new(&mut self.inner).poll_read(cx, buf);
        };
        let metrics = self.metrics.clone();
        let capacity = buf.remaining();
        if capacity == 0 {
            return Pin::new(&mut self.inner).poll_read(cx, buf);
        }

        // Hyper's ReadBufCursor is intentionally one-way: once passed to the
        // inner reader it cannot report how far it advanced. Read into a
        // temporary initialized buffer when the opt-in guard is enabled so
        // the wrapper can distinguish byte progress from EOF/zero progress
        // without unsafe pointer inspection.
        let mut scratch = vec![0_u8; capacity];
        let mut read_buf = hyper::rt::ReadBuf::new(&mut scratch);
        let result = Pin::new(&mut self.inner).poll_read(cx, read_buf.unfilled());
        let bytes_read = read_buf.filled().len();
        match result {
            Poll::Ready(Ok(())) if bytes_read > 0 => {
                buf.put_slice(&read_buf.filled()[..bytes_read]);
                self.read_timer = None;
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Ok(())) => {
                // Preserve an already-running deadline across zero-progress
                // reads. A first zero-progress read is treated as EOF by
                // Hyper and therefore does not need to manufacture a timer.
                if self.read_timer.is_some() {
                    match Self::poll_timer(
                        &mut self.read_timer,
                        Some(timeout),
                        cx,
                        TransportIoDirection::Read,
                        &metrics,
                    ) {
                        Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
                        Poll::Ready(Ok(())) | Poll::Pending => Poll::Ready(Ok(())),
                    }
                } else {
                    Poll::Ready(Ok(()))
                }
            }
            Poll::Ready(Err(error)) => {
                self.read_timer = None;
                Poll::Ready(Err(error))
            }
            Poll::Pending => Self::poll_timer(
                &mut self.read_timer,
                Some(timeout),
                cx,
                TransportIoDirection::Read,
                &metrics,
            ),
        }
    }
}

// Hyper's upgrade downcast requires Tokio's IO traits in addition to Hyper's
// runtime traits. This forwarding implementation is used only when an
// established connection is handed to an application-owned 101 stream.
impl<C> tokio::io::AsyncRead for LifecycleConnection<C>
where
    C: tokio::io::AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<C> hyper::rt::Write for LifecycleConnection<C>
where
    C: hyper::rt::Write + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let metrics = self.metrics.clone();
        if buf.is_empty() {
            return Pin::new(&mut self.inner).poll_write(cx, buf);
        }
        match Pin::new(&mut self.inner).poll_write(cx, buf) {
            Poll::Ready(Ok(written)) => {
                if written > 0 {
                    self.write_timer = None;
                }
                Poll::Ready(Ok(written))
            }
            Poll::Ready(Err(error)) => {
                self.write_timer = None;
                Poll::Ready(Err(error))
            }
            Poll::Pending => {
                let timeout = self.io_timeout.write;
                match Self::poll_timer(
                    &mut self.write_timer,
                    timeout,
                    cx,
                    TransportIoDirection::Write,
                    &metrics,
                ) {
                    Poll::Ready(error) => Poll::Ready(error.map(|()| 0)),
                    Poll::Pending => Poll::Pending,
                }
            }
        }
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let metrics = self.metrics.clone();
        if bufs.iter().all(|buf| buf.is_empty()) {
            return Pin::new(&mut self.inner).poll_write_vectored(cx, bufs);
        }
        match Pin::new(&mut self.inner).poll_write_vectored(cx, bufs) {
            Poll::Ready(Ok(written)) => {
                if written > 0 {
                    self.write_timer = None;
                }
                Poll::Ready(Ok(written))
            }
            Poll::Ready(Err(error)) => {
                self.write_timer = None;
                Poll::Ready(Err(error))
            }
            Poll::Pending => {
                let timeout = self.io_timeout.write;
                match Self::poll_timer(
                    &mut self.write_timer,
                    timeout,
                    cx,
                    TransportIoDirection::Write,
                    &metrics,
                ) {
                    Poll::Ready(error) => Poll::Ready(error.map(|()| 0)),
                    Poll::Pending => Poll::Pending,
                }
            }
        }
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let metrics = self.metrics.clone();
        match Pin::new(&mut self.inner).poll_flush(cx) {
            Poll::Ready(result) => {
                self.write_timer = None;
                Poll::Ready(result)
            }
            Poll::Pending => {
                let timeout = self.io_timeout.write;
                Self::poll_timer(
                    &mut self.write_timer,
                    timeout,
                    cx,
                    TransportIoDirection::Write,
                    &metrics,
                )
            }
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let metrics = self.metrics.clone();
        match Pin::new(&mut self.inner).poll_shutdown(cx) {
            Poll::Ready(result) => {
                self.write_timer = None;
                Poll::Ready(result)
            }
            Poll::Pending => {
                let timeout = self.io_timeout.write;
                Self::poll_timer(
                    &mut self.write_timer,
                    timeout,
                    cx,
                    TransportIoDirection::Write,
                    &metrics,
                )
            }
        }
    }
}

impl<C> tokio::io::AsyncWrite for LifecycleConnection<C>
where
    C: tokio::io::AsyncWrite + Unpin,
{
    // Intentionally untimed: this impl serves app-owned upgraded (101)
    // streams (WS/SSE over Tokio I/O), where the application — not the
    // Hyper dispatch path — owns write pacing. The `hyper::rt::Write`
    // impl above enforces `io_timeout.write` for protocol writes; routing
    // upgraded-stream writes through the same inactivity timer here would
    // time out legitimate application idle periods.
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::rt::Write as _;
    use std::convert::Infallible;
    use std::future::poll_fn;
    use tokio::io::AsyncWriteExt;

    struct MockConnection {
        inner: hyper_util::rt::TokioIo<tokio::io::DuplexStream>,
    }

    impl hyper::rt::Read for MockConnection {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: hyper::rt::ReadBufCursor<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.inner).poll_read(cx, buf)
        }
    }

    impl hyper::rt::Write for MockConnection {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.inner).poll_write(cx, buf)
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.inner).poll_flush(cx)
        }

        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.inner).poll_shutdown(cx)
        }
    }

    impl Connection for MockConnection {
        fn connected(&self) -> Connected {
            Connected::new()
        }
    }

    struct ZeroReadConnection {
        zero_reads: usize,
    }

    impl hyper::rt::Read for ZeroReadConnection {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: hyper::rt::ReadBufCursor<'_>,
        ) -> Poll<io::Result<()>> {
            if self.zero_reads == 0 {
                self.zero_reads = 1;
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }
    }

    impl hyper::rt::Write for ZeroReadConnection {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    impl Connection for ZeroReadConnection {
        fn connected(&self) -> Connected {
            Connected::new()
        }
    }

    struct PendingWriteConnection;

    impl hyper::rt::Read for PendingWriteConnection {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: hyper::rt::ReadBufCursor<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Pending
        }
    }

    impl hyper::rt::Write for PendingWriteConnection {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Pending
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Pending
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Pending
        }

        fn is_write_vectored(&self) -> bool {
            true
        }

        fn poll_write_vectored(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _bufs: &[IoSlice<'_>],
        ) -> Poll<io::Result<usize>> {
            Poll::Pending
        }
    }

    impl Connection for PendingWriteConnection {
        fn connected(&self) -> Connected {
            Connected::new()
        }
    }

    async fn read_once<C: hyper::rt::Read + Unpin>(
        connection: &mut C,
        storage: &mut [u8],
    ) -> io::Result<usize> {
        let mut read_buf = hyper::rt::ReadBuf::new(storage);
        poll_fn(|cx| Pin::new(&mut *connection).poll_read(cx, read_buf.unfilled()))
            .await
            .map(|()| read_buf.filled().len())
    }

    #[derive(Clone)]
    struct MockConnector;

    impl Service<Uri> for MockConnector {
        type Response = MockConnection;
        type Error = Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _dst: Uri) -> Self::Future {
            Box::pin(async {
                let (stream, _peer) = tokio::io::duplex(64);
                Ok(MockConnection {
                    inner: hyper_util::rt::TokioIo::new(stream),
                })
            })
        }
    }

    #[derive(Clone)]
    struct BlockingConnector;

    impl Service<Uri> for BlockingConnector {
        type Response = MockConnection;
        type Error = Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _dst: Uri) -> Self::Future {
            Box::pin(std::future::pending())
        }
    }

    #[derive(Clone)]
    struct FailingConnector;

    impl Service<Uri> for FailingConnector {
        type Response = MockConnection;
        type Error = io::Error;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _dst: Uri) -> Self::Future {
            Box::pin(async { Err(io::Error::other("connect failed")) })
        }
    }

    #[tokio::test]
    async fn physical_permit_is_retained_until_connection_drop() {
        let config = Arc::new(LifecycleConfig::from_policy(
            PhysicalConnectionPolicy {
                max_live: Some(1),
                admission_timeout: Some(Duration::from_millis(20)),
            },
            TransportIoTimeout::default(),
            Arc::new(TransportMetrics::new()),
        ));
        let mut connector = LifecycleConnector::new(MockConnector, config.clone());
        let uri: Uri = "http://example.invalid".parse().unwrap();
        let first = connector.call(uri.clone()).await.unwrap();
        let second = tokio::time::timeout(Duration::from_millis(100), connector.call(uri.clone()))
            .await
            .unwrap();
        let Err(second) = second else {
            panic!("second connection unexpectedly admitted");
        };
        assert!(second.to_string().contains(PHYSICAL_ADMISSION_TIMEOUT));
        let snapshot = config.metrics.snapshot();
        assert_eq!(snapshot.physical_admission_waits, 1);
        assert_eq!(snapshot.physical_admission_timeouts, 1);
        assert_eq!(snapshot.physical_connections_live, 1);
        drop(first);
        assert_eq!(config.metrics.snapshot().physical_connections_live, 0);
        assert!(connector.call(uri).await.is_ok());
    }

    #[tokio::test]
    async fn failed_and_cancelled_connections_release_admission_permits() {
        let metrics = Arc::new(TransportMetrics::new());
        let config = Arc::new(LifecycleConfig::from_policy(
            PhysicalConnectionPolicy {
                max_live: Some(1),
                admission_timeout: None,
            },
            TransportIoTimeout::default(),
            metrics.clone(),
        ));
        let uri: Uri = "http://example.invalid".parse().unwrap();

        let mut failing = LifecycleConnector::new(FailingConnector, config.clone());
        assert!(failing.call(uri.clone()).await.is_err());
        assert!(config
            .admission
            .as_ref()
            .expect("admission semaphore")
            .try_acquire()
            .is_ok());

        let mut connector = LifecycleConnector::new(BlockingConnector, config.clone());
        let pending = connector.call(uri);
        let task = tokio::spawn(pending);
        tokio::task::yield_now().await;
        task.abort();
        let _ = task.await;
        assert!(config
            .admission
            .as_ref()
            .expect("admission semaphore")
            .try_acquire()
            .is_ok());
        assert_eq!(metrics.snapshot().physical_connections_live, 0);
    }

    #[tokio::test]
    async fn zero_progress_reads_do_not_reset_an_active_deadline() {
        let metrics = Arc::new(TransportMetrics::new());
        let mut connection = LifecycleConnection::new(
            ZeroReadConnection { zero_reads: 0 },
            None,
            TransportIoTimeout {
                read: Some(Duration::from_millis(15)),
                write: None,
            },
            metrics.clone(),
        );
        let mut storage = [0_u8; 8];
        assert_eq!(read_once(&mut connection, &mut storage).await.unwrap(), 0);
        let result = tokio::time::timeout(
            Duration::from_millis(100),
            read_once(&mut connection, &mut storage),
        )
        .await
        .expect("read inactivity timer should fire");
        let error = result.expect_err("stalled read should time out");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(error
            .to_string()
            .contains("transport Read inactivity timeout"));
        assert_eq!(metrics.snapshot().transport_read_inactivity_timeouts, 1);
    }

    #[tokio::test]
    async fn read_progress_resets_the_inactivity_deadline() {
        let (stream, mut peer) = tokio::io::duplex(64);
        let metrics = Arc::new(TransportMetrics::new());
        let mut connection = LifecycleConnection::new(
            MockConnection {
                inner: hyper_util::rt::TokioIo::new(stream),
            },
            None,
            TransportIoTimeout {
                read: Some(Duration::from_millis(20)),
                write: None,
            },
            metrics.clone(),
        );
        let writer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            peer.write_all(&[1]).await.unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
            peer.write_all(&[2]).await.unwrap();
        });
        let mut storage = [0_u8; 1];
        assert_eq!(
            tokio::time::timeout(
                Duration::from_millis(100),
                read_once(&mut connection, &mut storage)
            )
            .await
            .unwrap()
            .unwrap(),
            1
        );
        assert_eq!(
            tokio::time::timeout(
                Duration::from_millis(100),
                read_once(&mut connection, &mut storage)
            )
            .await
            .unwrap()
            .unwrap(),
            1
        );
        writer.await.unwrap();
        assert_eq!(metrics.snapshot().transport_read_inactivity_timeouts, 0);
    }

    #[tokio::test]
    async fn vectored_write_stall_uses_the_write_inactivity_guard() {
        let metrics = Arc::new(TransportMetrics::new());
        let mut connection = LifecycleConnection::new(
            PendingWriteConnection,
            None,
            TransportIoTimeout {
                read: None,
                write: Some(Duration::from_millis(15)),
            },
            metrics.clone(),
        );
        let bufs = [IoSlice::new(b"one"), IoSlice::new(b"two")];
        let result = tokio::time::timeout(
            Duration::from_millis(100),
            poll_fn(|cx| Pin::new(&mut connection).poll_write_vectored(cx, &bufs)),
        )
        .await
        .expect("write inactivity timer should fire");
        let error = result.expect_err("stalled vectored write should time out");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert_eq!(metrics.snapshot().transport_write_inactivity_timeouts, 1);
    }

    #[test]
    fn invalid_policy_does_not_construct_a_zero_semaphore() {
        let config = LifecycleConfig::from_policy(
            PhysicalConnectionPolicy {
                max_live: Some(0),
                admission_timeout: None,
            },
            TransportIoTimeout::default(),
            Arc::new(TransportMetrics::new()),
        );
        assert!(config.invalid);
        assert!(config.admission.is_none());
    }

    #[tokio::test]
    async fn invalid_policy_fails_before_the_inner_connector_is_called() {
        let config = Arc::new(LifecycleConfig::from_policy(
            PhysicalConnectionPolicy {
                max_live: Some(0),
                admission_timeout: None,
            },
            TransportIoTimeout::default(),
            Arc::new(TransportMetrics::new()),
        ));
        let mut connector = LifecycleConnector::new(MockConnector, config);
        let uri: Uri = "http://example.invalid".parse().unwrap();
        let Err(error) = connector.call(uri).await else {
            panic!("invalid policy unexpectedly succeeded");
        };
        assert!(error
            .to_string()
            .contains("max_live must be greater than zero"));
    }
}
