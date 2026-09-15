//! Service adapters for native transport-oriented consumers.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;

use crate::{Client, Error, NativeRequestOptions, NativeResponseBody};

/// A Tower service adapter for eggfetch's native frame-preserving HTTP path.
///
/// This service delegates every request to [`Client::execute_http_body`]. It
/// is always ready to accept a request; origin-aware logical pool admission,
/// physical connection admission, and transport backpressure happen in the
/// future returned by [`tower_service::Service::call`]. Consequently,
/// Tower's readiness-based load-shedding layers do not observe eggfetch pool
/// saturation. Wrap this service in caller-owned Tower layers when a separate
/// global concurrency or load-shed policy is needed.
///
/// The native path does not apply eggfetch's high-level redirects, logical
/// retries, cookies, authentication, decompression, or decoded-body limits.
/// Request extensions are not guaranteed to be forwarded to Hyper; middleware
/// may use them before the request reaches this service, but they are not an
/// eggfetch transport contract.
#[derive(Clone, Debug)]
pub struct NativeHttpService {
    client: Client,
    options: NativeRequestOptions,
}

impl NativeHttpService {
    /// Create a service using default native transport options.
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            options: NativeRequestOptions::default(),
        }
    }

    /// Configure the native transport options used by every service request.
    #[must_use]
    pub fn with_options(mut self, options: NativeRequestOptions) -> Self {
        self.options = options;
        self
    }
}

impl Client {
    /// Create a Tower service for the native frame-preserving HTTP path.
    ///
    /// The returned service is configured with [`NativeRequestOptions::default`].
    /// Use [`NativeHttpService::with_options`] to configure service-instance
    /// defaults, or call [`Client::execute_http_body`] directly for options
    /// that vary by request.
    #[must_use]
    pub fn native_service(&self) -> NativeHttpService {
        NativeHttpService::new(self.clone())
    }
}

impl<B> tower_service::Service<http::Request<B>> for NativeHttpService
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: std::error::Error + Send + Sync + 'static,
{
    type Response = http::Response<NativeResponseBody>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    /// Eggfetch admission is origin-aware and requires the request, so it is
    /// performed by the future returned from [`tower_service::Service::call`].
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    /// Execute one native request through the existing client transport path.
    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        let client = self.client.clone();
        let options = self.options.clone();
        Box::pin(async move { client.execute_http_body(request, options).await })
    }
}
