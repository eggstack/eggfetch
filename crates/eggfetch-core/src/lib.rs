//! Async-first HTTP client engine for eggfetch.
//!
//! This crate is the single owner of HTTP behavior. The CLI and Python
//! bindings are thin adapters that delegate to it.
//!
//! Status: skeleton. The actual networking engine lands in later milestones.

#![deny(missing_docs)]

pub mod body;
pub mod client;
pub mod config;
pub mod error;
pub mod headers;
pub mod request;
pub mod response;
pub mod timeout;

pub use client::Client;
pub use error::{Error, Result};
pub use request::{Method, Request, RequestBuilder};
pub use response::Response;
