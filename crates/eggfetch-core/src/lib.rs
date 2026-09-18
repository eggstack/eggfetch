//! Async-first HTTP client engine for eggfetch.
//!
//! This crate is the single owner of HTTP behavior. The CLI and Python
//! bindings are thin adapters that delegate to it.
//!
//! # Example
//!
//! ```no_run
//! # async fn example() -> eggfetch_core::Result<()> {
//! use eggfetch_core::Client;
//!
//! let client = Client::new();
//! let mut response = client
//!     .get("https://example.com")?
//!     .header("user-agent", "eggfetch")
//!     .query("q", "test")
//!     .send()
//!     .await?;
//!
//! assert!(response.status().is_success());
//! let bytes = response.bytes().await?;
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]

pub mod auth;
pub mod body;
pub mod client;
pub mod compression;
#[cfg(feature = "cookies")]
pub mod cookie;
pub mod error;
mod h2_headers;
pub mod headers;
pub(crate) mod http_origin;
pub mod http_version;
pub mod limits;
#[cfg(feature = "multipart")]
pub mod multipart;
pub mod network_stream;
pub(crate) mod pipeline;
pub mod pool;
#[cfg(feature = "proxy")]
pub mod proxy;
pub mod redact;
#[cfg(feature = "redirects")]
pub mod redirect;
#[cfg(feature = "high-level-url")]
pub mod request;
#[cfg(feature = "high-level-url")]
pub mod response;
#[cfg(feature = "high-level-url")]
pub(crate) mod response_decode;
#[cfg(feature = "logical-retry")]
pub mod retry;
pub mod service;
pub(crate) mod stream;
pub mod timeout;
#[cfg(feature = "tls-rustls")]
pub mod tls;
pub mod trace;
pub mod transport;
pub mod transport_hints;

#[cfg(feature = "basic-auth")]
pub use auth::BasicAuth;
pub use auth::{AuthScheme, BearerAuth};
pub use body::{BoxBytesStream, NativeResponseBody, RequestBody, ResponseBody, SharedTrailers};
pub use client::{Client, ClientBuilder};
pub use compression::{accept_encoding_value, ContentCoding};
pub use error::{Error, NetworkFailureKind, RequestFailure, Result};
pub use headers::Headers;
pub use http::Method;
pub use http_version::HttpVersionPolicy;
pub use limits::Limits;
#[cfg(feature = "multipart")]
pub use multipart::{Boundary, Multipart, MultipartEncoder, Part, PartBody};
pub use network_stream::{
    ConnectionMetadata, ExtraInfo, NetworkStream, TlsInfo, TransportKind, UpgradedStream,
    UpgradedStreamVariant,
};
pub use pool::{Pool, PoolConfig};
#[cfg(feature = "proxy")]
pub use proxy::{
    NoProxy, NoProxyRule, Proxy, ProxyAuth, ProxyConfig, ProxyDecision, ProxyEnvironment, ProxyRule,
};
pub use redact::{is_sensitive_header, redact_headers, SENSITIVE_HEADERS};
#[cfg(feature = "high-level-url")]
pub use redact::{redact_url, redact_url_string};
#[cfg(all(feature = "high-level-url", feature = "redirects"))]
pub use redirect::{
    build_redirect_request_with_redirect_policy, check_https_downgrade, is_https_downgrade,
};
#[cfg(feature = "redirects")]
pub use redirect::{RedirectDowngradePolicy, RedirectPolicy};
#[cfg(feature = "high-level-url")]
pub use request::{ProxyOverride, Request, RequestBuilder};
#[cfg(feature = "high-level-url")]
pub use response::{HistoryEntry, Response};
#[cfg(feature = "logical-retry")]
pub use retry::{
    BackoffPolicy, MethodPolicy, ReplayCheck, RetryCause, RetryContext, RetryPolicy,
    RetryPolicyBuilder, StatusPolicy,
};
pub use service::NativeHttpService;
pub use timeout::{Timeout, TimeoutBuilder, TimeoutPhase};
#[cfg(feature = "tls-rustls")]
pub use tls::{ClientIdentity, TlsConfig, TlsConfigBuilder, TlsVersion, TrustStore};
pub use transport::dialer::{DialError, DialErrorKind, DialFuture, DialStream, DialTarget, Dialer};
/// Socket option for direct TCP connections.
pub use transport::direct_connector::{SocketOption, SocketOptionKind};
pub use transport::lifecycle::{
    PhysicalConnectionPolicy, TransportIoDirection, TransportIoTimeout,
};
#[cfg(feature = "http3")]
pub use transport::metrics::{H3CloseKind, H3CloseSummary, H3ConnectionDiagnostic, H3RouteKind};
pub use transport::metrics::{TransportMetrics, TransportSnapshot};
pub use transport_hints::{NativeRequestOptions, ResolvedTarget, TransportHints};
