//! C ABI bindings for eggfetch-core.
//!
//! This crate exposes eggfetch-core as a C-compatible library for consumption
//! by Node.js (N-API), Ruby (FFI), Zig, or any language that can call C ABI
//! functions. All operations are blocking — the internal tokio runtime handles
//! async I/O transparently.
//!
//! # Thread Safety
//!
//! - [`ClientHandle`] is `Send + Sync` and may be shared across threads.
//! - [`RequestHandle`], [`ResponseHandle`], and [`ErrorHandle`] are single-thread,
//!   single-use. Create, use, and free them on one thread.
//! - All callback functions must be `extern "C"`.

#![allow(unsafe_code)]

mod client;
mod error;
mod ffi_response;
mod handle;
mod request;
mod response;
mod runtime;

pub use client::*;
pub use error::*;
pub use ffi_response::*;
pub use handle::*;
pub use request::*;
pub use response::*;
