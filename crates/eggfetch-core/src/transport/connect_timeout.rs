//! Connect-phase timeout wrapper for the hyper connector.
//!
//! Wraps an inner connector (typically `hyper_rustls::HttpsConnector`) and
//! applies a deadline to the connection establishment phase (DNS resolution,
//! TCP connect, TLS handshake).  The timeout does **not** cover request
//! sending or response reading – those are handled by the `write` and `read`
//! fields of [`Timeout`](crate::Timeout).

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use http::Uri;
use tower_service::Service;

/// A wrapper around a connector that enforces a connect-phase timeout.
///
/// The timeout bounds the entire connection-establishment sequence:
/// DNS resolution → TCP connect → TLS handshake.  This is the behaviour
/// expected by HTTPX's `timeout.connect` field.
///
/// When no timeout is configured (`None`), the inner connector is called
/// directly without any wrapping overhead.
#[derive(Clone)]
pub(crate) struct ConnectTimeout<C> {
    inner: C,
    timeout: Option<Duration>,
}

impl<C> ConnectTimeout<C> {
    /// Wrap `inner` with an optional connect-phase timeout.
    pub(crate) fn new(inner: C, timeout: Option<Duration>) -> Self {
        Self { inner, timeout }
    }
}

impl<C> Service<Uri> for ConnectTimeout<C>
where
    C: Service<Uri> + Send + 'static,
    C::Response: Send + 'static,
    C::Error: Into<Box<dyn std::error::Error + Send + Sync>> + 'static,
    C::Future: Send + 'static,
{
    type Response = C::Response;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let inner_future = self.inner.call(dst);

        match self.timeout {
            Some(duration) => Box::pin(async move {
                tokio::time::timeout(duration, inner_future)
                    .await
                    .map_err(|_| {
                        Box::new(crate::error::Error::Timeout {
                            phase: crate::timeout::TimeoutPhase::Connect,
                            elapsed: duration,
                        }) as Box<dyn std::error::Error + Send + Sync>
                    })?
                    .map_err(Into::into)
            }),
            None => Box::pin(async move { inner_future.await.map_err(Into::into) }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;

    /// A trivial connector that always succeeds after a configurable delay.
    #[derive(Clone)]
    struct MockConnector {
        delay: Duration,
    }

    impl Service<Uri> for MockConnector {
        type Response = tokio::io::DuplexStream;
        type Error = Infallible;
        type Future =
            std::pin::Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _dst: Uri) -> Self::Future {
            let delay = self.delay;
            Box::pin(async move {
                tokio::time::sleep(delay).await;
                Ok(tokio::io::duplex(1024).0)
            })
        }
    }

    #[tokio::test]
    async fn connect_succeeds_within_deadline() {
        let connector = MockConnector {
            delay: Duration::from_millis(10),
        };
        let mut wrapped = ConnectTimeout::new(connector, Some(Duration::from_secs(1)));

        let uri: Uri = "http://example.com".parse().unwrap();
        let result = wrapped.call(uri).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn connect_timeout_fires() {
        let connector = MockConnector {
            delay: Duration::from_secs(60),
        };
        let mut wrapped = ConnectTimeout::new(connector, Some(Duration::from_millis(50)));

        let uri: Uri = "http://example.com".parse().unwrap();
        let result = wrapped.call(uri).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("timeout"),
            "expected 'timeout' in error message, got: {msg}"
        );
    }

    #[tokio::test]
    async fn no_timeout_delegates_directly() {
        let connector = MockConnector {
            delay: Duration::from_millis(10),
        };
        let mut wrapped = ConnectTimeout::new(connector, None);

        let uri: Uri = "http://example.com".parse().unwrap();
        let result = wrapped.call(uri).await;
        assert!(result.is_ok());
    }
}
