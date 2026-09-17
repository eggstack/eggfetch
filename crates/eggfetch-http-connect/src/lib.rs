//! Reusable HTTP/1 CONNECT wire mechanics over a caller-owned async stream.
//!
//! This crate serializes `CONNECT` request bytes and parses CONNECT
//! response heads. It does not dial sockets, perform TLS, retry, decide
//! whether a status is acceptable, or own request deadlines.
//!
//! - The caller supplies an already-connected async byte stream.
//! - [`ConnectTarget`] owns authority formatting (including IPv6 rules).
//! - [`encode_connect_request`] serializes the request head.
//! - [`read_connect_response_head`] parses the bounded response head while
//!   preserving read-ahead bytes for the tunnel.
//! - Timeouts stay with the caller: wrap the encode/write future with a
//!   write deadline and the read future with a read deadline.
//!
//! ```rust
//! use eggfetch_http_connect::{
//!     ConnectRequest, ConnectResponseLimits, ConnectTarget,
//!     encode_connect_request, read_connect_response_head,
//! };
//! use tokio::io::BufReader;
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let target = ConnectTarget::new("example.com", 443)?;
//! let request = ConnectRequest {
//!     target: &target,
//!     proxy_authorization: None,
//!     extra_headers: &[],
//!     max_head_bytes: 64 * 1024,
//! };
//! let bytes = encode_connect_request(&request)?;
//! assert!(bytes.starts_with(b"CONNECT example.com:443 HTTP/1.1\r\n"));
//!
//! // In-memory stream: response head plus tunneled payload in one write.
//! let payload = b"HTTP/1.1 200 Connection Established\r\n\r\ntunneled-bytes";
//! let stream = tokio::io::duplex(1024);
//! let (mut client, mut server) = stream;
//! {
//!     use tokio::io::AsyncWriteExt;
//!     server.write_all(payload).await?;
//!     drop(server);
//! }
//! let mut reader = BufReader::new(client);
//! let head =
//!     read_connect_response_head(&mut reader, &ConnectResponseLimits::default()).await?;
//! assert_eq!(head.status, 200);
//! // Read-ahead remains readable through the same buffered stream.
//! {
//!     use tokio::io::AsyncReadExt;
//!     let mut rest = Vec::new();
//!     reader.read_to_end(&mut rest).await?;
//!     assert_eq!(rest, b"tunneled-bytes");
//! }
//! # Ok(())
//! # }
//! ```

mod error;
mod request;
mod response;
mod target;

pub use error::ConnectError;
pub use request::{basic_auth_value, encode_connect_request, ConnectRequest};
pub use response::{read_connect_response_head, ConnectResponseHead, ConnectResponseLimits};
pub use target::ConnectTarget;
