//! Physical connection admission and established-transport I/O guardrails.

use std::future::Future;
use std::io::{self, IoSlice};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use http::Uri;
use hyper_util::client::legacy::connect::{Connected, Connection};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tower_service::Service;

/// Stable message used by [`crate::Error::is_physical_connection_admission_timeout`].
pub(crate) const PHYSICAL_ADMISSION_TIMEOUT: &str = "physical connection admission timeout";

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
}

impl LifecycleConfig {
    pub(crate) fn from_policy(
        physical: PhysicalConnectionPolicy,
        io_timeout: TransportIoTimeout,
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
        let inner_future = self.inner.call(dst);
        Box::pin(async move {
            let permit = if let Some(semaphore) = config.admission.as_ref() {
                let result = match config.admission_timeout {
                    Some(timeout) => {
                        tokio::time::timeout(timeout, semaphore.clone().acquire_owned())
                            .await
                            .map_err(|_| {
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
                        )) as Box<dyn std::error::Error + Send + Sync>
                    })?,
                };
                Some(result)
            } else {
                None
            };
            let inner = inner_future.await.map_err(Into::into)?;
            Ok(LifecycleConnection::new(inner, permit, config.io_timeout))
        })
    }
}

/// Established connection wrapper retaining its physical permit.
pub(crate) struct LifecycleConnection<C> {
    inner: C,
    connection_permit: Option<OwnedSemaphorePermit>,
    io_timeout: TransportIoTimeout,
    read_timer: Option<Pin<Box<tokio::time::Sleep>>>,
    write_timer: Option<Pin<Box<tokio::time::Sleep>>>,
}

impl<C> LifecycleConnection<C> {
    fn new(inner: C, permit: Option<OwnedSemaphorePermit>, io_timeout: TransportIoTimeout) -> Self {
        Self {
            inner,
            connection_permit: permit,
            io_timeout,
            read_timer: None,
            write_timer: None,
        }
    }

    pub(crate) fn into_inner_and_permit(self) -> (C, Option<OwnedSemaphorePermit>) {
        (self.inner, self.connection_permit)
    }

    fn poll_timer(
        timer: &mut Option<Pin<Box<tokio::time::Sleep>>>,
        timeout: Option<Duration>,
        cx: &mut Context<'_>,
        direction: TransportIoDirection,
    ) -> Poll<io::Result<()>> {
        let Some(timeout) = timeout else {
            return Poll::Pending;
        };
        let timer = timer.get_or_insert_with(|| Box::pin(tokio::time::sleep(timeout)));
        if timer.as_mut().poll(cx).is_ready() {
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
        buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        match Pin::new(&mut self.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                self.read_timer = None;
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(error)) => {
                self.read_timer = None;
                Poll::Ready(Err(error))
            }
            Poll::Pending => {
                let timeout = self.io_timeout.read;
                match Self::poll_timer(
                    &mut self.read_timer,
                    timeout,
                    cx,
                    TransportIoDirection::Read,
                ) {
                    Poll::Ready(error) => Poll::Ready(error),
                    Poll::Pending => Poll::Pending,
                }
            }
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
                )
            }
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
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
                )
            }
        }
    }
}

impl<C> tokio::io::AsyncWrite for LifecycleConnection<C>
where
    C: tokio::io::AsyncWrite + Unpin,
{
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
    use std::convert::Infallible;

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

    #[tokio::test]
    async fn physical_permit_is_retained_until_connection_drop() {
        let config = Arc::new(LifecycleConfig::from_policy(
            PhysicalConnectionPolicy {
                max_live: Some(1),
                admission_timeout: Some(Duration::from_millis(20)),
            },
            TransportIoTimeout::default(),
        ));
        let mut connector = LifecycleConnector::new(MockConnector, config);
        let uri: Uri = "http://example.invalid".parse().unwrap();
        let first = connector.call(uri.clone()).await.unwrap();
        let second = tokio::time::timeout(Duration::from_millis(100), connector.call(uri.clone()))
            .await
            .unwrap();
        let Err(second) = second else {
            panic!("second connection unexpectedly admitted");
        };
        assert!(second.to_string().contains(PHYSICAL_ADMISSION_TIMEOUT));
        drop(first);
        assert!(connector.call(uri).await.is_ok());
    }

    #[test]
    fn invalid_policy_does_not_construct_a_zero_semaphore() {
        let config = LifecycleConfig::from_policy(
            PhysicalConnectionPolicy {
                max_live: Some(0),
                admission_timeout: None,
            },
            TransportIoTimeout::default(),
        );
        assert!(config.invalid);
        assert!(config.admission.is_none());
    }
}
