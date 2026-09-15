//! Resolver provenance for the standard Hyper connector.

use std::error::Error as StdError;
use std::fmt;
use std::net::SocketAddr;
use std::task::{Context, Poll};

use futures_util::future::{MapErr, TryFutureExt};
use hyper_util::client::legacy::connect::dns::{GaiResolver, Name};
use tower_service::Service;

/// The resolver used by the production standard Hyper connector.
pub(crate) type DefaultResolver = ClassifyingResolver<GaiResolver>;

/// Preserve the fact that a resolver failed before Hyper's connector collapses
/// the failure into its opaque client error.
#[derive(Clone)]
pub(crate) struct ClassifyingResolver<R> {
    inner: R,
}

impl<R> ClassifyingResolver<R> {
    pub(crate) fn new(inner: R) -> Self {
        Self { inner }
    }
}

/// Private marker for a resolver failure on the standard route.
///
/// The original error remains the source so the legacy Hyper error keeps its
/// existing diagnostics while detailed native requests can identify the
/// resolver boundary without matching display text.
#[derive(Debug)]
pub(crate) struct ResolverFailure {
    source: Box<dyn StdError + Send + Sync>,
}

impl fmt::Display for ResolverFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DNS resolution failed")
    }
}

impl StdError for ResolverFailure {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&*self.source)
    }
}

fn mark_failure<E>(error: E) -> ResolverFailure
where
    E: Into<Box<dyn StdError + Send + Sync>>,
{
    ResolverFailure {
        source: error.into(),
    }
}

impl<R> Service<Name> for ClassifyingResolver<R>
where
    R: Service<Name>,
    R::Response: Iterator<Item = SocketAddr>,
    R::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Response = R::Response;
    type Error = ResolverFailure;
    type Future = MapErr<R::Future, fn(R::Error) -> ResolverFailure>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(mark_failure::<R::Error>)
    }

    fn call(&mut self, name: Name) -> Self::Future {
        self.inner.call(name).map_err(mark_failure::<R::Error>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::{ready, Ready};
    use std::io;
    use std::net::{IpAddr, Ipv4Addr};

    #[derive(Clone)]
    struct StaticResolver(Vec<SocketAddr>);

    impl Service<Name> for StaticResolver {
        type Response = std::vec::IntoIter<SocketAddr>;
        type Error = io::Error;
        type Future = Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _: Name) -> Self::Future {
            ready(Ok(self.0.clone().into_iter()))
        }
    }

    #[derive(Clone, Copy)]
    struct FailingResolver;

    impl Service<Name> for FailingResolver {
        type Response = std::vec::IntoIter<SocketAddr>;
        type Error = io::Error;
        type Future = Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _: Name) -> Self::Future {
            ready(Err(io::Error::new(
                io::ErrorKind::NotFound,
                "synthetic resolver failure",
            )))
        }
    }

    #[derive(Clone, Copy)]
    struct ReadinessFailingResolver;

    impl Service<Name> for ReadinessFailingResolver {
        type Response = std::vec::IntoIter<SocketAddr>;
        type Error = io::Error;
        type Future = Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Err(io::Error::other(
                "synthetic resolver readiness failure",
            )))
        }

        fn call(&mut self, _: Name) -> Self::Future {
            ready(Ok(Vec::new().into_iter()))
        }
    }

    fn test_name() -> Name {
        "synthetic.example".parse().expect("test name is valid")
    }

    #[tokio::test]
    async fn success_preserves_resolver_address_order() {
        let addresses = vec![
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081),
        ];
        let mut resolver = ClassifyingResolver::new(StaticResolver(addresses.clone()));
        let result = resolver
            .call(test_name())
            .await
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(result, addresses);
    }

    #[tokio::test]
    async fn call_failure_marks_and_preserves_original_source() {
        let mut resolver = ClassifyingResolver::new(FailingResolver);
        let error = resolver
            .call(test_name())
            .await
            .expect_err("resolver should fail");
        assert_eq!(error.to_string(), "DNS resolution failed");
        let source = error
            .source()
            .expect("original resolver error is preserved");
        assert_eq!(
            source.downcast_ref::<io::Error>().unwrap().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn readiness_failure_uses_the_same_marker() {
        let mut resolver = ClassifyingResolver::new(ReadinessFailingResolver);
        let mut context = Context::from_waker(futures_util::task::noop_waker_ref());
        let Poll::Ready(result) = resolver.poll_ready(&mut context) else {
            panic!("poll should be ready");
        };
        let error = result.expect_err("resolver should fail readiness");
        assert_eq!(error.to_string(), "DNS resolution failed");
        assert_eq!(
            error
                .source()
                .expect("original readiness error is preserved")
                .downcast_ref::<io::Error>()
                .unwrap()
                .kind(),
            io::ErrorKind::Other
        );
    }
}
