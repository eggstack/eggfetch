//! Caller-supplied byte-stream dialing for native Rust consumers.

use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use http::Uri;
use hyper_util::client::legacy::connect::{Connected, Connection};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite};
use tower_service::Service;

/// The logical destination supplied to a [`Dialer`].
///
/// This contains only the URL's logical host and effective port. It never
/// contains credentials, headers, paths, query strings, or request bodies.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct DialTarget {
    host: String,
    port: u16,
}

impl DialTarget {
    /// Create a logical dialing target.
    #[must_use]
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }

    /// Return the logical host or IP literal.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Return the effective HTTP or HTTPS port.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }
}

impl fmt::Debug for DialTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DialTarget")
            .field("host", &self.host)
            .field("port", &self.port)
            .finish()
    }
}

/// Broad category for an application-owned dialing failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialErrorKind {
    /// The route could not reach the logical destination.
    Connection,
    /// Establishment exceeded the caller's route deadline or was cancelled.
    Timeout,
    /// The caller-owned transport rejected its credentials or authorization.
    Authentication,
    /// The route or destination was explicitly rejected.
    Rejected,
    /// A failure that does not fit another category.
    Other,
}

/// Error returned by a caller-supplied [`Dialer`].
///
/// The source is retained so an embedding application can recover its own
/// transport error. Callers should ensure their error's `Display` and `Debug`
/// implementations do not expose secrets.
pub struct DialError {
    kind: DialErrorKind,
    message: String,
    source: Option<Arc<dyn StdError + Send + Sync>>,
}

impl DialError {
    /// Construct an error without a nested source.
    #[must_use]
    pub fn new(kind: DialErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    /// Construct an error while retaining the caller's source error.
    #[must_use]
    pub fn with_source<E>(kind: DialErrorKind, message: impl Into<String>, source: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self {
            kind,
            message: message.into(),
            source: Some(Arc::new(source)),
        }
    }

    /// Return the broad failure category.
    #[must_use]
    pub const fn kind(&self) -> DialErrorKind {
        self.kind
    }

    /// Return the caller-provided safe description.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Clone for DialError {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            message: self.message.clone(),
            source: self.source.clone(),
        }
    }
}

impl fmt::Debug for DialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DialError")
            .field("kind", &self.kind)
            .field("message", &self.message)
            // The source remains available through `Error::source`, but its
            // Debug implementation belongs to the embedding application and
            // may contain secrets or unbounded transport state.
            .field("source", &self.source.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl fmt::Display for DialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl StdError for DialError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn StdError + 'static))
    }
}

impl fmt::Display for DialErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Connection => "connection",
            Self::Timeout => "timeout",
            Self::Authentication => "authentication",
            Self::Rejected => "rejected",
            Self::Other => "transport",
        })
    }
}

/// Object-safe stream returned by a [`Dialer`].
pub trait DialStreamTrait: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T> DialStreamTrait for T where T: AsyncRead + AsyncWrite + Send + Unpin {}

/// Erased Tokio-compatible stream supplied by a caller-owned route.
pub type DialStream = Box<dyn DialStreamTrait>;

/// Boxed future returned by [`Dialer::dial`].
pub type DialFuture<'a> = Pin<Box<dyn Future<Output = Result<DialStream, DialError>> + Send + 'a>>;

/// Supplies the raw byte stream used to reach a logical HTTP(S) origin.
///
/// Eggfetch remains responsible for HTTP framing, destination TLS, SNI and
/// certificate verification. A dialer controls only the physical route to
/// the supplied logical host and port.
pub trait Dialer: Send + Sync + 'static {
    /// Establish a raw stream for the logical destination.
    fn dial(&self, target: DialTarget) -> DialFuture<'_>;
}

impl<T> Dialer for Arc<T>
where
    T: Dialer + ?Sized,
{
    fn dial(&self, target: DialTarget) -> DialFuture<'_> {
        self.as_ref().dial(target)
    }
}

/// Crate-internal Hyper adapter for the public [`Dialer`] contract.
#[derive(Clone)]
pub(crate) struct DialerConnector {
    dialer: Arc<dyn Dialer>,
}

/// Hyper-runtime adapter around the caller's Tokio stream.
pub(crate) struct DialerConnection {
    inner: TokioIo<DialStream>,
}

impl DialerConnection {
    fn new(stream: DialStream) -> Self {
        Self {
            inner: TokioIo::new(stream),
        }
    }
}

impl hyper::rt::Read for DialerConnection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: hyper::rt::ReadBufCursor<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl hyper::rt::Write for DialerConnection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> std::task::Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }
}

impl Connection for DialerConnection {
    fn connected(&self) -> Connected {
        Connected::new()
    }
}

impl DialerConnector {
    pub(crate) fn new(dialer: Arc<dyn Dialer>) -> Self {
        Self { dialer }
    }
}

impl Service<Uri> for DialerConnector {
    type Response = DialerConnection;
    type Error = Box<dyn StdError + Send + Sync>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let host = dst.host().map(str::to_owned);
        let port = dst.port_u16().or_else(|| match dst.scheme_str() {
            Some("https") => Some(443),
            Some("http") => Some(80),
            _ => None,
        });
        let dialer = self.dialer.clone();
        Box::pin(async move {
            let host = host.ok_or_else(|| {
                Box::new(crate::error::Error::Connect(
                    "custom dialer target has no host".into(),
                )) as Box<dyn StdError + Send + Sync>
            })?;
            let port = port.ok_or_else(|| {
                Box::new(crate::error::Error::Connect(
                    "custom dialer target has no effective port".into(),
                )) as Box<dyn StdError + Send + Sync>
            })?;
            dialer
                .dial(DialTarget::new(host, port))
                .await
                .map(DialerConnection::new)
                .map_err(|error| {
                    Box::new(crate::error::Error::CustomTransport(Arc::new(error)))
                        as Box<dyn StdError + Send + Sync>
                })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct SecretSource;

    impl fmt::Display for SecretSource {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("secret-source")
        }
    }

    impl StdError for SecretSource {}

    #[test]
    fn debug_does_not_delegate_to_caller_source() {
        let error = DialError::with_source(
            DialErrorKind::Connection,
            "safe route description",
            SecretSource,
        );
        let debug = format!("{error:?}");
        assert!(debug.contains("safe route description"));
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("SecretSource"));
        assert!(StdError::source(&error).is_some());
    }
}
