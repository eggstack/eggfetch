//! Experimental Node.js bindings for eggfetch via N-API.
//!
//! # Support status: experimental prototype
//!
//! This crate is a deliberately narrow prototype, not a supported binding.
//! Requests execute through the synchronous C ABI (`eggfetch-ffi`) inside
//! `spawn_blocking`; request bodies are UTF-8 strings only; responses are
//! buffered eagerly; errors are single unstructured strings; and
//! `index.d.ts` carries no generated declarations. See
//! `docs/architecture/ffi-and-node.md` for the explicit support contract
//! and the list of intentionally unsupported capabilities.

#![deny(clippy::all)]

mod client;
mod response;

pub use client::*;
pub use response::*;
