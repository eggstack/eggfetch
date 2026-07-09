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

pub mod body;
pub mod client;
pub mod error;
pub mod headers;
pub mod pool;
pub mod request;
pub mod response;
pub(crate) mod stream;
pub mod timeout;

pub use body::{BoxBytesStream, RequestBody, ResponseBody};
pub use client::{Client, ClientBuilder};
pub use error::{Error, Result};
pub use headers::Headers;
pub use http::Method;
pub use pool::{Pool, PoolConfig};
pub use request::{Request, RequestBuilder};
pub use response::Response;
pub use timeout::{Timeout, TimeoutBuilder, TimeoutPhase};
