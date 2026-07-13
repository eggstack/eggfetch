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
pub mod headers;
#[cfg(feature = "multipart")]
pub mod multipart;
pub mod pool;
#[cfg(feature = "proxy")]
pub mod proxy;
pub mod redirect;
pub mod request;
pub mod response;
pub(crate) mod stream;
pub mod timeout;

pub use auth::{AuthScheme, BasicAuth, BearerAuth};
pub use body::{BoxBytesStream, RequestBody, ResponseBody};
pub use client::{Client, ClientBuilder};
pub use compression::{accept_encoding_value, ContentCoding};
pub use error::{Error, Result};
pub use headers::Headers;
pub use http::Method;
#[cfg(feature = "multipart")]
pub use multipart::{Boundary, Multipart, MultipartEncoder, Part, PartBody};
pub use pool::{Pool, PoolConfig};
#[cfg(feature = "proxy")]
pub use proxy::{NoProxy, NoProxyRule, Proxy, ProxyAuth, ProxyConfig, ProxyDecision, ProxyRule};
pub use redirect::RedirectPolicy;
pub use request::{ProxyOverride, Request, RequestBuilder};
pub use response::{HistoryEntry, Response};
pub use timeout::{Timeout, TimeoutBuilder, TimeoutPhase};
