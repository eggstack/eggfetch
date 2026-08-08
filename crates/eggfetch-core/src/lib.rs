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
pub mod http_version;
pub mod limits;
#[cfg(feature = "multipart")]
pub mod multipart;
pub(crate) mod pipeline;
pub mod pool;
#[cfg(feature = "proxy")]
pub mod proxy;
pub mod redact;
pub mod redirect;
pub mod request;
pub mod response;
pub(crate) mod response_decode;
pub mod retry;
pub(crate) mod stream;
pub mod timeout;
pub mod tls;
pub mod transport;

pub use auth::{AuthScheme, BasicAuth, BearerAuth};
pub use body::{BoxBytesStream, RequestBody, ResponseBody};
pub use client::{Client, ClientBuilder};
pub use compression::{accept_encoding_value, ContentCoding};
pub use error::{Error, Result};
pub use headers::Headers;
pub use http::Method;
pub use http_version::HttpVersionPolicy;
pub use limits::Limits;
#[cfg(feature = "multipart")]
pub use multipart::{Boundary, Multipart, MultipartEncoder, Part, PartBody};
pub use pool::{Pool, PoolConfig};
#[cfg(feature = "proxy")]
pub use proxy::{NoProxy, NoProxyRule, Proxy, ProxyAuth, ProxyConfig, ProxyDecision, ProxyRule};
pub use redact::{
    is_sensitive_header, redact_headers, redact_url, redact_url_string, SENSITIVE_HEADERS,
};
pub use redirect::RedirectPolicy;
pub use request::{ProxyOverride, Request, RequestBuilder};
pub use response::{HistoryEntry, Response};
pub use retry::{
    BackoffPolicy, MethodPolicy, ReplayCheck, RetryCause, RetryContext, RetryPolicy,
    RetryPolicyBuilder, StatusPolicy,
};
pub use timeout::{Timeout, TimeoutBuilder, TimeoutPhase};
pub use tls::{ClientIdentity, TlsConfig, TlsConfigBuilder, TlsVersion, TrustStore};
/// Socket option for direct TCP connections.
pub use transport::direct_connector::SocketOption;
