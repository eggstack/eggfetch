//! Node.js bindings for eggfetch via N-API.

#![deny(clippy::all)]

mod client;
mod response;

pub use client::*;
pub use response::*;
