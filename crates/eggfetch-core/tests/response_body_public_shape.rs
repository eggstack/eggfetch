#![allow(missing_docs, dead_code, clippy::unwrap_used, clippy::expect_used)]
//! Public `ResponseBody` shape compatibility regression.
//!
//! `ResponseBody` is public and not `#[non_exhaustive]`, so downstream Rust
//! code may exhaustively destructure its variants. This test compiles as an
//! external crate and matches the exact 0.1.6 / pre-corrective field shapes
//! without `..`. Adding a public field (as `dd52f8c4` did with
//! `read_timeout` / `total_deadline`) fails here with E0027 instead of
//! shipping a patch-level source break.

use eggfetch_core::ResponseBody;

#[allow(dead_code)]
fn consume_shape(body: ResponseBody) {
    match body {
        ResponseBody::Buffered { bytes } => {
            let _ = bytes;
        }
        ResponseBody::Streaming { stream, lease } => {
            let _ = (stream, lease);
        }
        ResponseBody::EncodedStreaming {
            stream,
            lease,
            content_encoding,
            limit,
        } => {
            let _ = (stream, lease, content_encoding, limit);
        }
        ResponseBody::Consumed => {}
    }
}

#[test]
fn response_body_public_shape_matches_published_api() {
    // Compile-time proof only: `consume_shape` must exhaustively match the
    // published variant field shapes above without `..`.
}
